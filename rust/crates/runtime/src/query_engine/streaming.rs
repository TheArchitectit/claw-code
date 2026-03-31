//! Streaming and helper methods for query execution.

use std::time::Duration;

use tokio::time::sleep;

use crate::context::ToolUseContext;
use crate::llm_client::{LlmClient, LlmRequest, LlmStreamChunk, MockLlmClient, StopReason};
use crate::messages::{
    AssistantMessage, AssistantMessageContent, ContentBlock, Message, NormalizedMessage,
    NormalizedAssistantMessage, QueryResultMessage, ToolResultContent,
};
use crate::permissions::PermissionResult;
use crate::tool::ToolOutput;
use crate::types::{LlmApiError, MessageId, QueryEngineError, QueryResult, SessionId, ToolUseId, Usage as RuntimeUsage};

use super::config::{QueryEngineConfig, FallbackStatistics, FALLBACK_DELAY_MS, MAX_FALLBACK_ATTEMPTS};
use super::types::{MessageStream, QueryExecutionState};

/// Methods for LLM streaming and helper operations.
pub struct QueryExecutionOps {
    pub config: QueryEngineConfig,
    pub messages: std::sync::Arc<tokio::sync::RwLock<Vec<Message>>>,
    pub permission_denials: std::sync::Arc<tokio::sync::Mutex<Vec<crate::messages::PermissionDenial>>>,
    pub total_usage: std::sync::Arc<tokio::sync::Mutex<RuntimeUsage>>,
    pub total_cost: std::sync::Arc<tokio::sync::Mutex<f64>>,
    pub start_time: std::time::Instant,
    pub session_id: SessionId,
    pub state: std::sync::Arc<std::sync::Mutex<QueryExecutionState>>,
    /// Fallback statistics tracking.
    pub fallback_stats: std::sync::Arc<tokio::sync::Mutex<FallbackStatistics>>,
}

impl QueryExecutionOps {
    /// Update the total cost based on usage and model.
    pub async fn update_cost(&self, usage: RuntimeUsage, model: &str) {
        let cost = self.config.calculate_cost(model, usage.input_tokens, usage.output_tokens);
        let mut total = self.total_cost.lock().await;
        *total += cost;
    }

    /// Create a new ops struct from execution components.
    pub fn new(
        config: QueryEngineConfig,
        messages: std::sync::Arc<tokio::sync::RwLock<Vec<Message>>>,
        permission_denials: std::sync::Arc<tokio::sync::Mutex<Vec<crate::messages::PermissionDenial>>>,
        total_usage: std::sync::Arc<tokio::sync::Mutex<RuntimeUsage>>,
        total_cost: std::sync::Arc<tokio::sync::Mutex<f64>>,
        start_time: std::time::Instant,
        session_id: SessionId,
        state: QueryExecutionState,
    ) -> Self {
        Self {
            config,
            messages,
            permission_denials,
            total_usage,
            total_cost,
            start_time,
            session_id,
            state: std::sync::Arc::new(std::sync::Mutex::new(state)),
            fallback_stats: std::sync::Arc::new(tokio::sync::Mutex::new(FallbackStatistics::default())),
        }
    }

    /// Get response from LLM using streaming with fallback support.
    ///
    /// This method attempts to get a response from the primary model, and if that fails
    /// with a fallback-triggering error, it will retry with fallback models in the chain.
    pub async fn get_llm_response(&self) -> QueryResult<(AssistantMessage, StopReason, RuntimeUsage)> {
        let primary_model = self
            .config
            .user_specified_model
            .clone()
            .unwrap_or_else(|| "claude-3-5-sonnet".to_string());

        // Track models we've attempted
        let mut attempted_models: Vec<String> = vec![primary_model.clone()];
        let mut fallback_attempts: u32 = 0;
        let max_fallback_attempts = self.config.max_fallback_attempts;

        // Try primary model first
        let mut current_model = primary_model.clone();

        loop {
            // Attempt to get response with current model
            match self.get_llm_response_for_model(&current_model).await {
                Ok(result) => {
                    // Success - update fallback stats if we used a fallback model
                    if fallback_attempts > 0 {
                        let mut stats = self.fallback_stats.lock().await;
                        let cost = self.config.calculate_cost(
                            &current_model,
                            result.2.input_tokens,
                            result.2.output_tokens,
                        );
                        stats.record_success(&current_model, cost);
                    }
                    return Ok(result);
                }
                Err(err) => {
                    // Check if error should trigger fallback
                    if !self.config.fallback_enabled {
                        return Err(err);
                    }

                    // Check if this is a fallback-triggering error
                    let should_fallback = match &err {
                        QueryEngineError::LlmApi(api_err) => api_err.is_fallback_trigger(),
                        QueryEngineError::Api { message } => {
                            // Check for fallback-triggering patterns in generic API errors
                            let msg_lower = message.to_lowercase();
                            msg_lower.contains("rate limit")
                                || msg_lower.contains("429")
                                || msg_lower.contains("timeout")
                                || msg_lower.contains("unavailable")
                                || msg_lower.contains("context length")
                        }
                        _ => false,
                    };

                    if !should_fallback {
                        return Err(err);
                    }

                    // Extract error for model selection
                    let api_error = match &err {
                        QueryEngineError::LlmApi(api_err) => api_err.clone(),
                        _ => LlmApiError::Other {
                            message: err.to_string(),
                        },
                    };

                    // Check if we've exhausted fallback attempts
                    fallback_attempts += 1;
                    if fallback_attempts > max_fallback_attempts {
                        let mut stats = self.fallback_stats.lock().await;
                        stats.record_failure();
                        return Err(QueryEngineError::MaxRetriesExceeded {
                            max_retries: max_fallback_attempts,
                            last_error: err.to_string(),
                        });
                    }

                    // Get next fallback model based on error type
                    if let Some(next_model) = self
                        .config
                        .select_fallback_model(&api_error, &attempted_models)
                    {
                        // Record the fallback attempt
                        {
                            let mut stats = self.fallback_stats.lock().await;
                            stats.record_attempt(&current_model, &next_model);
                        }

                        // Wait before retry (with exponential backoff)
                        let delay_ms = FALLBACK_DELAY_MS * fallback_attempts as u64;
                        sleep(Duration::from_millis(delay_ms)).await;

                        attempted_models.push(next_model.clone());
                        current_model = next_model;
                    } else {
                        // No more fallback models available
                        let mut stats = self.fallback_stats.lock().await;
                        stats.record_failure();
                        return Err(err);
                    }
                }
            }
        }
    }

    /// Get response from a specific model without fallback handling.
    ///
    /// This is the core implementation that streams from a single model.
    async fn get_llm_response_for_model(
        &self,
        model: &str,
    ) -> QueryResult<(AssistantMessage, StopReason, RuntimeUsage)> {
        // Get current messages
        let messages = self.messages.read().await.clone();

        // Get tools from registry
        let tools = self.get_available_tools().await;

        let request = LlmRequest::new(model.to_string())
            .with_system(self.config.custom_system_prompt.clone().unwrap_or_default())
            .with_messages(messages)
            .with_tools(tools)
            .with_max_tokens(4096);

        // Get the LLM stream
        let mut stream = if let Some(ref client) = self.config.llm_client {
            client
                .stream(request)
                .await
                .map_err(|e| QueryEngineError::Api {
                    message: e.to_string(),
                })?
        } else {
            // Default mock response for testing
            let mock_client: MockLlmClient = MockLlmClient::simple_text_response(
                "I understand your message. I'm responding without an LLM client configured.",
            );
            mock_client
                .stream(request)
                .await
                .map_err(|e| QueryEngineError::Api {
                    message: e.to_string(),
                })?
        };

        // Accumulators for streaming content
        let mut content_blocks: Vec<ContentBlock> = Vec::new();
        let mut current_text: Option<String> = None;
        let mut current_thinking: Option<(String, Option<String>)> = None; // (thinking, signature)
        let mut pending_tool_use: Option<(ToolUseId, String, String)> = None; // (id, name, accumulated_json)
        let mut usage = RuntimeUsage::default();
        let mut stop_reason = StopReason::EndTurn;
        let mut cancelled = false;

        // Process stream chunks
        let mut chunk_count = 0;
        while let Some(chunk) = stream.next().await {
            chunk_count += 1;

            // Check for cancellation every 10 chunks
            if chunk_count % 10 == 0 {
                // Check if abort controller has been notified
                // We use try_recv to avoid blocking - if there's no notification, continue
                if let Ok(_) = tokio::time::timeout(
                    std::time::Duration::from_millis(0),
                    self.config.abort_controller.notified(),
                )
                .await
                {
                    cancelled = true;
                    break;
                }
            }

            match chunk {
                LlmStreamChunk::Text { text } => {
                    // Accumulate text content
                    if let Some(ref mut current) = current_text {
                        current.push_str(&text);
                    } else {
                        // If there was previous content, finalize it first
                        if let Some(thinking) = current_thinking.take() {
                            content_blocks.push(ContentBlock::Thinking {
                                thinking: thinking.0,
                                signature: thinking.1,
                            });
                        }
                        current_text = Some(text);
                    }
                }
                LlmStreamChunk::Thinking { thinking } => {
                    // Accumulate thinking content
                    if let Some(ref mut current) = current_thinking {
                        current.0.push_str(&thinking);
                    } else {
                        // If there was previous text, finalize it first
                        if let Some(text) = current_text.take() {
                            content_blocks.push(ContentBlock::Text {
                                text,
                                citation: None,
                            });
                        }
                        current_thinking = Some((thinking, None));
                    }
                }
                LlmStreamChunk::ToolUseStart { id, name } => {
                    // Finalize any pending content before starting a tool use
                    if let Some(text) = current_text.take() {
                        content_blocks.push(ContentBlock::Text {
                            text,
                            citation: None,
                        });
                    }
                    if let Some(thinking) = current_thinking.take() {
                        content_blocks.push(ContentBlock::Thinking {
                            thinking: thinking.0,
                            signature: thinking.1,
                        });
                    }

                    // Start a new tool use
                    pending_tool_use = Some((id, name, String::new()));
                }
                LlmStreamChunk::ToolUseDelta {
                    id: _,
                    partial_json,
                } => {
                    // Accumulate JSON for the current tool use
                    if let Some(ref mut pending) = pending_tool_use {
                        pending.2.push_str(&partial_json);
                    }
                }
                LlmStreamChunk::ToolUseComplete { id, input } => {
                    // Complete the tool use with parsed input
                    // If we have accumulated JSON but the stream gives us parsed input, use the parsed input
                    if let Some((tool_id, name, _)) = pending_tool_use.take() {
                        content_blocks.push(ContentBlock::ToolUse {
                            id: tool_id,
                            name,
                            input,
                        });
                    } else {
                        // Stream provided complete info directly
                        content_blocks.push(ContentBlock::ToolUse {
                            id,
                            name: String::new(), // Will be filled from accumulated data if available
                            input,
                        });
                    }
                }
                LlmStreamChunk::Usage {
                    usage: stream_usage,
                } => {
                    usage = stream_usage;
                }
                LlmStreamChunk::Stop { reason } => {
                    stop_reason = reason;
                }
                LlmStreamChunk::Error { message } => {
                    // Convert error message to appropriate error type for fallback detection
                    let err_msg_lower = message.to_lowercase();
                    if err_msg_lower.contains("rate limit") || err_msg_lower.contains("429") {
                        return Err(QueryEngineError::LlmApi(LlmApiError::rate_limit(60)));
                    } else if err_msg_lower.contains("timeout") {
                        return Err(QueryEngineError::LlmApi(LlmApiError::timeout(30)));
                    } else if err_msg_lower.contains("model unavailable") || err_msg_lower.contains("503") {
                        return Err(QueryEngineError::LlmApi(LlmApiError::model_unavailable(model)));
                    } else if err_msg_lower.contains("context length") || err_msg_lower.contains("context too long") {
                        return Err(QueryEngineError::LlmApi(LlmApiError::context_length_exceeded(message.clone())));
                    } else {
                        return Err(QueryEngineError::Api { message });
                    }
                }
            }
        }

        // Finalize any pending content blocks
        if let Some(text) = current_text.take() {
            content_blocks.push(ContentBlock::Text {
                text,
                citation: None,
            });
        }
        if let Some(thinking) = current_thinking.take() {
            content_blocks.push(ContentBlock::Thinking {
                thinking: thinking.0,
                signature: thinking.1,
            });
        }

        // Handle incomplete tool use (if stream ended before ToolUseComplete)
        if let Some((id, name, json_str)) = pending_tool_use.take() {
            // Try to parse the accumulated JSON
            let input = if json_str.is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&json_str).unwrap_or_else(|_| {
                    // If parsing fails, wrap the raw string
                    serde_json::json!({ "raw": json_str })
                })
            };

            content_blocks.push(ContentBlock::ToolUse { id, name, input });
        }

        // If cancelled, we still return what we have
        if cancelled {
            stop_reason = StopReason::EndTurn;
        }

        // Create assistant message from accumulated content
        let assistant_msg = AssistantMessage {
            id: MessageId::new(),
            session_id: self.session_id,
            timestamp: chrono::Utc::now(),
            content: AssistantMessageContent {
                role: "assistant".to_string(),
                content: content_blocks,
                model: model.to_string(),
                stop_reason: Some(stop_reason.to_string()),
                usage: Some(usage),
            },
            parent_tool_use_id: None,
        };

        Ok((assistant_msg, stop_reason, usage))
    }

    /// Get available tools as JSON schemas.
    async fn get_available_tools(&self) -> Vec<serde_json::Value> {
        // This would enumerate the tool registry and get schemas
        // For now, return an empty list - tools are resolved by name at execution time
        Vec::new()
    }
}
