//! QueryExecution implementation for managing active query lifecycles.

use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::context::ToolUseContext;
use crate::llm_client::StopReason;
use crate::messages::{
    AssistantMessageContent, ContentBlock, Message, NormalizedMessage,
    NormalizedAssistantMessage, QueryResultMessage, ToolResultContent, UserMessage,
};
use crate::permissions::PermissionResult;
use crate::tool::ToolOutput;
use crate::types::{MessageId, QueryEngineError, QueryResult, SessionId, ToolUseId, Usage as RuntimeUsage};

use super::config::QueryEngineConfig;
use super::streaming::QueryExecutionOps;
use super::types::{MessageStream, QueryExecutionState};

/// An active query execution that yields messages.
pub struct QueryExecution {
    config: QueryEngineConfig,
    messages: Arc<RwLock<Vec<Message>>>,
    permission_denials: Arc<Mutex<Vec<crate::messages::PermissionDenial>>>,
    total_usage: Arc<Mutex<RuntimeUsage>>,
    total_cost: Arc<Mutex<f64>>,
    start_time: std::time::Instant,
    session_id: SessionId,
    user_message: UserMessage,
    state: QueryExecutionState,
}

impl QueryExecution {
    /// Create a new query execution.
    pub(super) fn new(
        config: QueryEngineConfig,
        messages: Arc<RwLock<Vec<Message>>>,
        permission_denials: Arc<Mutex<Vec<crate::messages::PermissionDenial>>>,
        total_usage: Arc<Mutex<RuntimeUsage>>,
        total_cost: Arc<Mutex<f64>>,
        start_time: std::time::Instant,
        session_id: SessionId,
        user_message: UserMessage,
    ) -> Self {
        Self {
            config,
            messages,
            permission_denials,
            total_usage,
            total_cost,
            start_time,
            session_id,
            user_message,
            state: QueryExecutionState::Initial,
        }
    }

    /// Execute the query and return all messages.
    ///
    /// This is a simplified stub implementation that demonstrates the structure.
    pub async fn execute(&mut self) -> QueryResult<Vec<NormalizedMessage>> {
        self.state = QueryExecutionState::Processing;

        let mut output_messages: Vec<NormalizedMessage> = Vec::new();

        // Add the user message
        output_messages.push(NormalizedMessage::User(crate::messages::NormalizedUserMessage {
            r#type: "user".to_string(),
            message: self.user_message.content.clone(),
            session_id: self.session_id,
            parent_tool_use_id: None,
            uuid: self.user_message.id,
            timestamp: self.user_message.timestamp,
            is_replay: Some(false),
            is_synthetic: Some(false),
        }));

        // Run the LLM conversation loop
        let result = self.run_conversation_loop(&mut output_messages).await?;

        output_messages.push(NormalizedMessage::Result(result.clone()));
        self.state = QueryExecutionState::Completed(result);

        Ok(output_messages)
    }

    /// Run the main conversation loop with LLM and tool execution.
    async fn run_conversation_loop(
        &self,
        output_messages: &mut Vec<NormalizedMessage>,
    ) -> QueryResult<QueryResultMessage> {
        let mut turn_count: u32 = 0;
        let max_turns = self.config.max_turns.unwrap_or(100);

        loop {
            // Check max turns
            turn_count += 1;
            if turn_count > max_turns {
                return Err(QueryEngineError::MaxTurnsExceeded { max_turns });
            }

            // Check budget
            if let Some(max_budget) = self.config.max_budget_usd {
                let current_cost = *self.total_cost.lock().await;
                if current_cost > max_budget {
                    return Err(QueryEngineError::MaxBudgetExceeded { max_budget });
                }
            }

            // Create ops for streaming
            let ops = QueryExecutionOps::new(
                self.config.clone(),
                self.messages.clone(),
                self.permission_denials.clone(),
                self.total_usage.clone(),
                self.total_cost.clone(),
                self.start_time,
                self.session_id,
                QueryExecutionState::Processing,
            );

            // Get LLM response
            let (assistant_msg, stop_reason, usage) = ops.get_llm_response().await?;

            // Update usage and cost
            {
                let mut total = self.total_usage.lock().await;
                total.input_tokens += usage.input_tokens;
                total.output_tokens += usage.output_tokens;
            }
            // Update cost using the same model that was used
            let model = assistant_msg.content.model.clone();
            ops.update_cost(usage, &model).await;

            // Add assistant message to conversation
            {
                let mut guard = self.messages.write().await;
                guard.push(Message::Assistant(assistant_msg.clone()));
            }

            // Add to output messages
            output_messages.push(NormalizedMessage::Assistant(NormalizedAssistantMessage {
                r#type: "assistant".to_string(),
                content: assistant_msg.content.content.clone(),
                session_id: self.session_id,
                parent_tool_use_id: assistant_msg.parent_tool_use_id.clone(),
                uuid: assistant_msg.id,
                timestamp: assistant_msg.timestamp,
                stop_reason: Some(stop_reason.to_string()),
                usage: assistant_msg.content.usage.clone(),
            }));

            // Handle stop reason
            match stop_reason {
                StopReason::EndTurn | StopReason::MaxTokens | StopReason::StopSequence => {
                    // Conversation complete
                    let duration = self.start_time.elapsed();
                    let total_usage = *self.total_usage.lock().await;
                    let total_cost = *self.total_cost.lock().await;

                    return Ok(QueryResultMessage::Success {
                        duration_ms: duration.as_millis() as u64,
                        duration_api_ms: duration.as_millis() as u64,
                        is_error: false,
                        num_turns: turn_count,
                        result: self.extract_response_text(&assistant_msg.content.content),
                        stop_reason: Some(stop_reason.to_string()),
                        session_id: self.session_id,
                        total_cost_usd: total_cost,
                        usage: total_usage,
                        model_usage: None,
                        permission_denials: self.permission_denials.lock().await.clone(),
                        fast_mode_state: None,
                        uuid: MessageId::new(),
                    });
                }
                StopReason::ToolUse => {
                    // Process tool calls and continue the loop
                    let tool_calls = self.extract_tool_calls(&assistant_msg.content.content);
                    if tool_calls.is_empty() {
                        // No actual tool calls found, end the conversation
                        let duration = self.start_time.elapsed();
                        let total_usage = *self.total_usage.lock().await;
                        let total_cost = *self.total_cost.lock().await;

                        return Ok(QueryResultMessage::Success {
                            duration_ms: duration.as_millis() as u64,
                            duration_api_ms: duration.as_millis() as u64,
                            is_error: false,
                            num_turns: turn_count,
                            result: self.extract_response_text(&assistant_msg.content.content),
                            stop_reason: Some("end_turn".to_string()),
                            session_id: self.session_id,
                            total_cost_usd: total_cost,
                            usage: total_usage,
                            model_usage: None,
                            permission_denials: self.permission_denials.lock().await.clone(),
                            fast_mode_state: None,
                            uuid: MessageId::new(),
                        });
                    }

                    // Execute tools - concurrency-safe tools in parallel, others sequentially
                    self.execute_tools_parallel(tool_calls, output_messages).await?;
                }
                StopReason::Error | StopReason::ContentFiltered => {
                    // Handle error cases - return an error result
                    let duration = self.start_time.elapsed();
                    let total_usage = *self.total_usage.lock().await;
                    let total_cost = *self.total_cost.lock().await;
                    return Ok(QueryResultMessage::Error {
                        duration_ms: duration.as_millis() as u64,
                        duration_api_ms: duration.as_millis() as u64,
                        is_error: true,
                        num_turns: turn_count,
                        stop_reason: Some(stop_reason.to_string()),
                        session_id: self.session_id,
                        total_cost_usd: total_cost,
                        usage: total_usage,
                        model_usage: None,
                        permission_denials: self.permission_denials.lock().await.clone(),
                        fast_mode_state: None,
                        uuid: MessageId::new(),
                        errors: vec![format!("LLM stopped with error: {:?}", stop_reason)],
                    });
                }
            }
        }
    }

    /// Extract tool calls from assistant content.
    fn extract_tool_calls(
        &self,
        content: &[ContentBlock],
    ) -> Vec<(ToolUseId, String, serde_json::Value)> {
        content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::ToolUse { id, name, input } => {
                    Some((id.clone(), name.clone(), input.clone()))
                }
                _ => None,
            })
            .collect()
    }

    /// Extract text response from content blocks.
    fn extract_response_text(&self, content: &[ContentBlock]) -> String {
        content
            .iter()
            .filter_map(|block| match block {
                ContentBlock::Text { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Execute multiple tools with parallel execution for concurrency-safe tools.
    ///
    /// This method separates tools into two groups:
    /// 1. Concurrency-safe tools: executed in parallel using join_all
    /// 2. Non-concurrency-safe tools: executed sequentially to avoid conflicts
    ///
    /// Results are collected and added to the conversation.
    async fn execute_tools_parallel(
        &self,
        tool_calls: Vec<(ToolUseId, String, serde_json::Value)>,
        output_messages: &mut Vec<NormalizedMessage>,
    ) -> QueryResult<()> {
        // Separate tools into concurrency-safe and non-concurrency-safe groups
        let mut safe_calls: Vec<(ToolUseId, String, serde_json::Value)> = Vec::new();
        let mut unsafe_calls: Vec<(ToolUseId, String, serde_json::Value)> = Vec::new();

        for (tool_use_id, tool_name, tool_input) in tool_calls {
            if let Some(tool) = self.config.tool_registry.get(&tool_name) {
                if tool.is_concurrency_safe(&tool_input) {
                    safe_calls.push((tool_use_id, tool_name, tool_input));
                } else {
                    unsafe_calls.push((tool_use_id, tool_name, tool_input));
                }
            } else {
                // Tool not found - treat as unsafe (will error when executed)
                unsafe_calls.push((tool_use_id, tool_name, tool_input));
            }
        }

        // Execute concurrency-safe tools in parallel
        let safe_futures: Vec<_> = safe_calls
            .into_iter()
            .map(|(tool_use_id, tool_name, tool_input)| {
                let tool_name_clone = tool_name.clone();
                async move {
                    let result = self
                        .execute_tool(&tool_name, tool_input, tool_use_id.clone())
                        .await;
                    (tool_use_id, tool_name_clone, result)
                }
            })
            .collect();

        let safe_results = futures::future::join_all(safe_futures).await;

        // Process parallel results
        for (tool_use_id, _tool_name, tool_result) in safe_results {
            self.add_tool_result_to_conversation(tool_use_id, tool_result, output_messages)
                .await;
        }

        // Execute non-concurrency-safe tools sequentially
        for (tool_use_id, tool_name, tool_input) in unsafe_calls {
            let tool_result = self
                .execute_tool(&tool_name, tool_input, tool_use_id.clone())
                .await;
            self.add_tool_result_to_conversation(tool_use_id, tool_result, output_messages)
                .await;
        }

        Ok(())
    }

    /// Add a tool result to the conversation history and output messages.
    async fn add_tool_result_to_conversation(
        &self,
        tool_use_id: ToolUseId,
        tool_result: QueryResult<ToolOutput>,
        output_messages: &mut Vec<NormalizedMessage>,
    ) {
        let is_error = tool_result.is_err();

        // Extract output string and new messages before consuming tool_result
        let (output_str, new_messages) = match &tool_result {
            Ok(output) => {
                let s = match &output.data {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (s, output.new_messages.clone())
            }
            Err(_) => (String::new(), Vec::new()),
        };

        let result_content = match tool_result {
            Ok(_) => {
                vec![ToolResultContent::Text { text: output_str }]
            }
            Err(e) => {
                vec![ToolResultContent::Error {
                    message: format!("Tool execution failed: {}", e),
                }]
            }
        };

        // Create tool result block
        let tool_result_block = ContentBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: result_content,
            is_error: Some(is_error),
        };

        // Add tool result to conversation history as an Assistant message
        // (This represents the tool result being provided back to the LLM)
        {
            let mut guard = self.messages.write().await;
            guard.push(Message::Assistant(crate::messages::AssistantMessage {
                id: MessageId::new(),
                session_id: self.session_id,
                timestamp: chrono::Utc::now(),
                content: AssistantMessageContent {
                    role: "assistant".to_string(),
                    content: vec![tool_result_block.clone()],
                    model: self
                        .config
                        .user_specified_model
                        .clone()
                        .unwrap_or_else(|| "claude-3-5-sonnet".to_string()),
                    stop_reason: None,
                    usage: None,
                },
                parent_tool_use_id: Some(tool_use_id.clone()),
            }));
        }

        // Add any new messages from the tool output to the conversation
        for msg in new_messages {
            let mut guard = self.messages.write().await;
            guard.push(msg);
        }

        // Add to normalized output messages
        output_messages.push(NormalizedMessage::Assistant(NormalizedAssistantMessage {
            r#type: "assistant".to_string(),
            content: vec![tool_result_block],
            session_id: self.session_id,
            parent_tool_use_id: Some(tool_use_id),
            uuid: MessageId::new(),
            timestamp: chrono::Utc::now(),
            stop_reason: None,
            usage: None,
        }));
    }

    /// Execute a tool call.
    ///
    /// This is the core method for executing tools during the conversation loop.
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        tool_use_id: ToolUseId,
    ) -> QueryResult<ToolOutput> {
        // Find the tool
        let tool = self
            .config
            .tool_registry
            .get(tool_name)
            .ok_or_else(|| QueryEngineError::Tool(crate::types::ToolError::not_found(tool_name)))?;

        // Create tool use context
        let context = self.create_tool_use_context().await?;

        // Check permissions
        let permission_result =
            (self.config.can_use_tool)(tool.as_ref(), &input, &context, tool_use_id.clone()).await;

        match permission_result {
            PermissionResult::Decision(decision) => {
                if !decision.is_allowed() {
                    // Record denial
                    let denial = crate::messages::PermissionDenial {
                        tool_name: tool_name.to_string(),
                        tool_use_id: tool_use_id.clone(),
                        tool_input: input.clone(),
                        reason: format!("Permission denied: {:?}", decision.behavior()),
                        timestamp: chrono::Utc::now(),
                    };
                    self.permission_denials.lock().await.push(denial);

                    return Err(QueryEngineError::Tool(crate::types::ToolError::permission_denied(
                        format!("Permission denied: {:?}", decision.behavior()),
                    )));
                }
            }
            PermissionResult::Passthrough { .. } => {
                // Handle passthrough - for now just continue
            }
        }

        // Execute the tool
        let output = tool
            .execute(input, &context, tool_use_id, None)
            .await
            .map_err(QueryEngineError::Tool)?;

        // Add any new messages from the tool
        for msg in &output.new_messages {
            let mut guard = self.messages.write().await;
            guard.push(msg.clone());
        }

        Ok(output)
    }

    /// Create a tool use context for tool execution.
    async fn create_tool_use_context(&self) -> QueryResult<ToolUseContext> {
        let messages = self.messages.read().await.clone();

        let context = ToolUseContext::new(self.session_id, &self.config.cwd)
            .with_messages(messages)
            .with_main_loop_model(
                self.config
                    .user_specified_model
                    .clone()
                    .unwrap_or_else(|| "claude-3-5-sonnet".to_string()),
            )
            .with_non_interactive(true);

        Ok(context)
    }

    /// Check if the query is complete.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(
            self.state,
            QueryExecutionState::Completed(_) | QueryExecutionState::Failed(_)
        )
    }

    /// Get the final result if complete.
    #[must_use]
    pub fn get_result(&self) -> Option<&QueryResultMessage> {
        match &self.state {
            QueryExecutionState::Completed(result) => Some(result),
            _ => None,
        }
    }

    /// Convert this execution into a message stream.
    ///
    /// This spawns the execution in a background task and returns
    /// a stream that yields messages as they are produced.
    pub fn into_stream(mut self) -> MessageStream {
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            match self.execute().await {
                Ok(messages) => {
                    for msg in messages {
                        if tx.send(msg).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                }
                Err(_e) => {
                    // Stream ends on error - the error will be in the result
                }
            }
        });

        MessageStream { receiver: rx }
    }
}
