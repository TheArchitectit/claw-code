//! The QueryEngine for orchestrating LLM interactions.

use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::messages::{
    AssistantMessageContent, ContentBlock, Message, NormalizedMessage, PermissionDenial,
    QueryResult as QueryResultMessage, UserMessage, UserMessageContent,
};
use crate::messages::{AssistantMessage, NormalizedAssistantMessage, NormalizedUserMessage};
use crate::types::{MessageId, QueryEngineError, QueryResult, SessionId, Usage as RuntimeUsage};

use super::config::{QueryEngineConfig, CLAUDE_3_5_SONNET_PRICE, CLAUDE_3_5_SONNET_OUTPUT_PRICE, CLAUDE_3_HAIKU_PRICE, CLAUDE_3_HAIKU_OUTPUT_PRICE, CLAUDE_3_OPUS_PRICE, CLAUDE_3_OPUS_OUTPUT_PRICE};
use super::execution::QueryExecution;
use super::types::{ConversationResult, SubmitMessageOptions};

/// The QueryEngine owns the query lifecycle and session state for a conversation.
///
/// One QueryEngine per conversation. Each submit_message() call starts a new
/// turn within the same conversation. State persists across turns.
pub struct QueryEngine {
    pub(super) config: QueryEngineConfig,
    pub(super) messages: Arc<RwLock<Vec<Message>>>,
    pub(super) permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    pub(super) total_usage: Arc<Mutex<RuntimeUsage>>,
    pub(super) total_cost: Arc<Mutex<f64>>,
    pub(super) turn_count: Arc<Mutex<u32>>,
    pub(super) session_id: SessionId,
}

impl QueryEngine {
    /// Create a new QueryEngine with the given configuration.
    #[must_use]
    pub fn new(config: QueryEngineConfig) -> Self {
        let session_id = SessionId::new();
        let messages = Arc::new(RwLock::new(config.initial_messages.clone()));

        Self {
            config,
            messages,
            permission_denials: Arc::new(Mutex::new(Vec::new())),
            total_usage: Arc::new(Mutex::new(RuntimeUsage::default())),
            total_cost: Arc::new(Mutex::new(0.0)),
            turn_count: Arc::new(Mutex::new(0)),
            session_id,
        }
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> SessionId {
        self.session_id
    }

    /// Get the current messages.
    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    /// Submit a message to the conversation.
    ///
    /// This initiates a new turn in the conversation, processes the user input,
    /// and yields messages as they are generated.
    ///
    /// # Arguments
    /// * `prompt` - The user prompt (text or content blocks)
    /// * `options` - Optional message options
    pub async fn submit_message(
        &self,
        prompt: impl Into<String>,
        _options: Option<SubmitMessageOptions>,
    ) -> QueryResult<QueryExecution> {
        let prompt = prompt.into();
        let start_time = std::time::Instant::now();

        // Create user message
        let user_msg = self.create_user_message(&prompt).await?;

        // Add to messages
        {
            let mut guard = self.messages.write().await;
            guard.push(Message::User(user_msg.clone()));
        }

        // Increment turn count
        {
            let mut guard = self.turn_count.lock().await;
            *guard += 1;
        }

        // Check max turns
        if let Some(max_turns) = self.config.max_turns {
            let current_turn = *self.turn_count.lock().await;
            if current_turn > max_turns {
                return Err(QueryEngineError::MaxTurnsExceeded { max_turns });
            }
        }

        // Create the execution context
        let execution = QueryExecution::new(
            self.config.clone(),
            self.messages.clone(),
            self.permission_denials.clone(),
            self.total_usage.clone(),
            self.total_cost.clone(),
            start_time,
            self.session_id,
            user_msg,
        );

        Ok(execution)
    }

    /// Create a user message from a prompt.
    async fn create_user_message(&self, prompt: &str) -> QueryResult<UserMessage> {
        let content = UserMessageContent {
            role: "user".to_string(),
            content: vec![ContentBlock::Text {
                text: prompt.to_string(),
                citation: None,
            }],
        };

        Ok(UserMessage {
            id: MessageId::new(),
            session_id: self.session_id,
            timestamp: chrono::Utc::now(),
            content,
            is_meta: None,
            is_visible_in_transcript_only: None,
            tool_use_result: None,
            parent_tool_use_id: None,
            is_replay: None,
            is_synthetic: None,
        })
    }

    /// Reset the conversation state.
    pub async fn reset(&self) {
        let mut guard = self.messages.write().await;
        guard.clear();
        guard.extend(self.config.initial_messages.clone());

        let mut denials = self.permission_denials.lock().await;
        denials.clear();

        let mut usage = self.total_usage.lock().await;
        *usage = RuntimeUsage::default();

        let mut cost = self.total_cost.lock().await;
        *cost = 0.0;

        let mut turns = self.turn_count.lock().await;
        *turns = 0;
    }

    /// Get the total usage for this session.
    pub async fn total_usage(&self) -> RuntimeUsage {
        *self.total_usage.lock().await
    }

    /// Get the total cost for this session.
    pub async fn total_cost(&self) -> f64 {
        *self.total_cost.lock().await
    }

    /// Add cost from usage for a specific model.
    ///
    /// # Arguments
    /// * `usage` - The token usage (input and output tokens)
    /// * `model` - The model name used for the request
    pub async fn add_usage_cost(&self, usage: RuntimeUsage, model: &str) {
        let cost = self.calculate_cost(usage, model);
        let mut total = self.total_cost.lock().await;
        *total += cost;
    }

    /// Check if the current cost exceeds the max budget.
    ///
    /// # Returns
    /// `true` if the budget is exceeded or would be exceeded, `false` otherwise
    pub async fn check_budget(&self) -> bool {
        let current_cost = *self.total_cost.lock().await;
        self.config.would_exceed_budget(current_cost)
    }

    /// Calculate the cost for a given usage and model.
    ///
    /// # Arguments
    /// * `usage` - The token usage
    /// * `model` - The model name
    ///
    /// # Returns
    /// The cost in USD
    fn calculate_cost(&self, usage: RuntimeUsage, model: &str) -> f64 {
        self.config.calculate_cost(model, usage.input_tokens, usage.output_tokens)
    }

    /// Get model pricing information.
    ///
    /// # Returns
    /// Tuple of (input_price_per_million, output_price_per_million) for the given model
    pub fn get_model_pricing(&self, model: &str) -> (f64, f64) {
        (
            self.config.get_input_price(model),
            self.config.get_output_price(model),
        )
    }

    /// Get all model pricing constants.
    pub fn get_pricing_constants() -> serde_json::Value {
        serde_json::json!({
            "claude_3_5_sonnet": {
                "input_per_million": CLAUDE_3_5_SONNET_PRICE,
                "output_per_million": CLAUDE_3_5_SONNET_OUTPUT_PRICE,
            },
            "claude_3_haiku": {
                "input_per_million": CLAUDE_3_HAIKU_PRICE,
                "output_per_million": CLAUDE_3_HAIKU_OUTPUT_PRICE,
            },
            "claude_3_opus": {
                "input_per_million": CLAUDE_3_OPUS_PRICE,
                "output_per_million": CLAUDE_3_OPUS_OUTPUT_PRICE,
            },
        })
    }

    /// Run a complete conversation turn with simulated tool-call loop.
    ///
    /// This is a high-level method that:
    /// 1. Takes a user message
    /// 2. Simulates the assistant thinking
    /// 3. Optionally "calls" tools (simulated)
    /// 4. Returns the final response
    pub async fn run(
        &self,
        message: impl Into<String>,
        options: Option<SubmitMessageOptions>,
    ) -> QueryResult<ConversationResult> {
        let message = message.into();
        let start_time = std::time::Instant::now();

        // Submit the message to start a new turn
        let mut execution = self.submit_message(&message, options).await?;

        // Execute the conversation
        let normalized_messages = execution.execute().await?;

        // Extract the assistant response
        let assistant_response = normalized_messages
            .iter()
            .filter_map(|m| match m {
                NormalizedMessage::Assistant(a) => a.content.first().and_then(|c| match c {
                    ContentBlock::Text { text, .. } => Some(text.clone()),
                    _ => Some("[non-text content]".to_string()),
                }),
                _ => None,
            })
            .last()
            .unwrap_or_else(|| "No response".to_string());

        let duration = start_time.elapsed();
        let usage = self.total_usage().await;
        let cost = self.total_cost().await;

        Ok(ConversationResult {
            response: assistant_response,
            messages: normalized_messages,
            duration_ms: duration.as_millis() as u64,
            usage,
            cost_usd: cost,
            session_id: self.session_id,
        })
    }

    /// Run a conversation with simulated tool calls.
    ///
    /// This demonstrates how the QueryEngine would handle a tool-call loop:
    /// 1. User sends message
    /// 2. "LLM" decides to use a tool
    /// 3. Tool is executed
    /// 4. Result sent back to "LLM"
    /// 5. Final response generated
    pub async fn run_with_tools(
        &self,
        message: impl Into<String>,
        options: Option<SubmitMessageOptions>,
    ) -> QueryResult<ConversationResult> {
        let message = message.into();
        let start_time = std::time::Instant::now();

        // Submit the user message
        let execution = self.submit_message(&message, options).await?;

        // Build conversation history
        let mut conversation_messages: Vec<NormalizedMessage> = Vec::new();

        // Add user message
        conversation_messages.push(NormalizedMessage::User(NormalizedUserMessage {
            r#type: "user".to_string(),
            message: UserMessageContent {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: message.clone(),
                    citation: None,
                }],
            },
            session_id: self.session_id,
            parent_tool_use_id: None,
            uuid: MessageId::new(),
            timestamp: chrono::Utc::now(),
            is_replay: Some(false),
            is_synthetic: Some(false),
        }));

        // Simulate tool-call loop based on message content
        let tool_calls = self.detect_simulated_tool_calls(&message).await;

        if !tool_calls.is_empty() {
            // Simulate tool use
            for (tool_name, tool_input) in &tool_calls {
                let tool_use_id = crate::types::ToolUseId::generate();

                // Add tool_use block to conversation
                conversation_messages.push(NormalizedMessage::Assistant(NormalizedAssistantMessage {
                    r#type: "assistant".to_string(),
                    content: vec![ContentBlock::ToolUse {
                        id: tool_use_id.clone(),
                        name: tool_name.clone(),
                        input: tool_input.clone(),
                    }],
                    session_id: self.session_id,
                    parent_tool_use_id: None,
                    uuid: MessageId::new(),
                    timestamp: chrono::Utc::now(),
                    stop_reason: None,
                    usage: None,
                }));

                // Execute the tool (using the real registry)
                let tool_result = match execution
                    .execute_tool(tool_name, tool_input.clone(), tool_use_id.clone())
                    .await
                {
                    Ok(output) => ContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: vec![crate::messages::ToolResultContent::Text {
                            text: output.data.to_string(),
                        }],
                        is_error: Some(false),
                    },
                    Err(e) => ContentBlock::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: vec![crate::messages::ToolResultContent::Text {
                            text: format!("Error: {}", e),
                        }],
                        is_error: Some(true),
                    },
                };

                // Add tool_result block
                conversation_messages.push(NormalizedMessage::Assistant(NormalizedAssistantMessage {
                    r#type: "assistant".to_string(),
                    content: vec![tool_result],
                    session_id: self.session_id,
                    parent_tool_use_id: Some(tool_use_id.clone()),
                    uuid: MessageId::new(),
                    timestamp: chrono::Utc::now(),
                    stop_reason: None,
                    usage: None,
                }));
            }
        }

        // Generate final assistant response
        let final_response = if tool_calls.is_empty() {
            format!("I received your message: '{}'", message)
        } else {
            format!(
                "I used {} tool(s) to help with your request: '{}'",
                tool_calls.len(),
                message
            )
        };

        conversation_messages.push(NormalizedMessage::Assistant(NormalizedAssistantMessage {
            r#type: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: final_response.clone(),
                citation: None,
            }],
            session_id: self.session_id,
            parent_tool_use_id: None,
            uuid: MessageId::new(),
            timestamp: chrono::Utc::now(),
            stop_reason: None,
            usage: None,
        }));

        let duration = start_time.elapsed();
        let usage = self.total_usage().await;
        let cost = self.total_cost().await;

        // Add final result message
        conversation_messages.push(NormalizedMessage::Result(QueryResultMessage::Success {
            duration_ms: duration.as_millis() as u64,
            duration_api_ms: duration.as_millis() as u64 / 2,
            is_error: false,
            num_turns: 1 + tool_calls.len() as u32,
            result: final_response.clone(),
            stop_reason: Some("end_turn".to_string()),
            session_id: self.session_id,
            total_cost_usd: cost,
            usage,
            model_usage: None,
            permission_denials: self.permission_denials.lock().await.clone(),
            fast_mode_state: None,
            uuid: MessageId::new(),
        }));

        Ok(ConversationResult {
            response: final_response,
            messages: conversation_messages,
            duration_ms: duration.as_millis() as u64,
            usage,
            cost_usd: cost,
            session_id: self.session_id,
        })
    }

    /// Detect simulated tool calls based on message content.
    async fn detect_simulated_tool_calls(&self, message: &str) -> Vec<(String, serde_json::Value)> {
        let mut calls = Vec::new();
        let lower = message.to_lowercase();

        // Simulate detecting tool calls based on keywords
        if lower.contains("list") || lower.contains("show") {
            calls.push(("TaskListTool".to_string(), serde_json::json!({})));
        }

        if lower.contains("create") || lower.contains("add") {
            calls.push((
                "TaskCreateTool".to_string(),
                serde_json::json!({
                    "subject": "Simulated task from query"
                }),
            ));
        }

        calls
    }
}
