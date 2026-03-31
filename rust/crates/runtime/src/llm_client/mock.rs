//! Mock LLM client for testing.
//!
//! This module provides a mock implementation of the LlmClient trait
//! for use in tests, allowing pre-programmed responses and request recording.

use async_trait::async_trait;

use crate::messages::ContentBlock;
use crate::llm_client::client::LlmClient;
use crate::llm_client::stream::{LlmStream, LlmStreamChunk, StopReason};
use crate::llm_client::types::{LlmRequest, LlmResponse};
use crate::types::{QueryResult, ToolUseId, Usage};

/// A mock response configuration for the MockLlmClient.
#[derive(Debug, Clone)]
pub struct MockResponse {
    /// The content blocks to return.
    pub content: Vec<ContentBlock>,
    /// The stop reason.
    pub stop_reason: StopReason,
    /// The token usage.
    pub usage: Usage,
    /// Optional delay to simulate network latency (in milliseconds).
    pub delay_ms: u64,
    /// Optional error to return instead of success.
    pub error: Option<String>,
}

impl MockResponse {
    /// Create a new mock response with text content.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content: vec![ContentBlock::Text {
                text: text.into(),
                citation: None,
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            delay_ms: 0,
            error: None,
        }
    }

    /// Create a new mock response with a tool call.
    #[must_use]
    pub fn tool_call(name: impl Into<String>, input: serde_json::Value) -> Self {
        Self {
            content: vec![ContentBlock::ToolUse {
                id: ToolUseId::generate(),
                name: name.into(),
                input,
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage::default(),
            delay_ms: 0,
            error: None,
        }
    }

    /// Create a mock response with thinking content.
    #[must_use]
    pub fn with_thinking(text: impl Into<String>, thinking: impl Into<String>) -> Self {
        Self {
            content: vec![
                ContentBlock::Thinking {
                    thinking: thinking.into(),
                    signature: None,
                },
                ContentBlock::Text {
                    text: text.into(),
                    citation: None,
                },
            ],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            delay_ms: 0,
            error: None,
        }
    }

    /// Create an error response.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            content: vec![],
            stop_reason: StopReason::Error,
            usage: Usage::default(),
            delay_ms: 0,
            error: Some(message.into()),
        }
    }

    /// Set the usage statistics for this response.
    #[must_use]
    pub fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = usage;
        self
    }

    /// Set a delay for this response to simulate network latency.
    #[must_use]
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = delay_ms;
        self
    }

    /// Convert this mock response to an LlmResponse.
    #[must_use]
    pub fn to_llm_response(&self, model: &str) -> LlmResponse {
        LlmResponse {
            content: self.content.clone(),
            stop_reason: self.stop_reason.clone(),
            usage: self.usage,
            model: model.to_string(),
        }
    }
}

/// A mock LLM client for testing.
pub struct MockLlmClient {
    /// Pre-programmed responses.
    responses: std::sync::Mutex<Vec<LlmResponse>>,
    /// Response index.
    response_index: std::sync::Mutex<usize>,
    /// Recorded requests for verification.
    recorded_requests: std::sync::Mutex<Vec<LlmRequest>>,
}

impl MockLlmClient {
    /// Create a new mock LLM client with programmed responses.
    #[must_use]
    pub fn new(responses: Vec<LlmResponse>) -> Self {
        Self {
            responses: std::sync::Mutex::new(responses),
            response_index: std::sync::Mutex::new(0),
            recorded_requests: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Create a simple mock that returns text responses.
    #[must_use]
    pub fn simple_text_response(text: impl Into<String>) -> Self {
        let response = LlmResponse {
            content: vec![ContentBlock::Text {
                text: text.into(),
                citation: None,
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            model: "mock-model".to_string(),
        };
        Self::new(vec![response])
    }

    /// Create a mock that simulates a tool call.
    #[must_use]
    pub fn with_tool_call(tool_name: impl Into<String>, tool_input: serde_json::Value) -> Self {
        let response = LlmResponse {
            content: vec![ContentBlock::ToolUse {
                id: ToolUseId::generate(),
                name: tool_name.into(),
                input: tool_input,
            }],
            stop_reason: StopReason::ToolUse,
            usage: Usage::default(),
            model: "mock-model".to_string(),
        };
        Self::new(vec![response])
    }

    /// Create a mock with a sequence of responses (for multi-turn conversations).
    #[must_use]
    pub fn with_responses(responses: Vec<LlmResponse>) -> Self {
        Self::new(responses)
    }

    /// Create a mock with MockResponse configurations.
    #[must_use]
    pub fn with_mock_responses(responses: Vec<MockResponse>) -> Self {
        let llm_responses: Vec<LlmResponse> = responses
            .iter()
            .map(|r| r.to_llm_response("mock-model"))
            .collect();
        Self::new(llm_responses)
    }

    /// Create a mock that returns an error response.
    #[must_use]
    pub fn with_error_response(message: impl Into<String>) -> Self {
        let response = LlmResponse {
            content: vec![ContentBlock::Text {
                text: message.into(),
                citation: None,
            }],
            stop_reason: StopReason::Error,
            usage: Usage::default(),
            model: "mock-model".to_string(),
        };
        Self::new(vec![response])
    }

    /// Get the recorded requests for verification.
    #[must_use]
    pub fn get_recorded_requests(&self) -> Vec<LlmRequest> {
        self.recorded_requests.lock().unwrap().clone()
    }

    /// Get the number of requests made.
    #[must_use]
    pub fn request_count(&self) -> usize {
        self.recorded_requests.lock().unwrap().len()
    }

    /// Clear recorded requests.
    pub fn clear_recorded_requests(&self) {
        self.recorded_requests.lock().unwrap().clear();
    }

    /// Reset the response index to start from the beginning.
    pub fn reset_response_index(&self) {
        *self.response_index.lock().unwrap() = 0;
    }

    /// Record a request manually (for external tracking).
    pub fn record_request_manually(&self, request: LlmRequest) {
        self.record_request(request);
    }

    fn record_request(&self, request: LlmRequest) {
        self.recorded_requests.lock().unwrap().push(request);
    }

    fn get_next_response(&self) -> LlmResponse {
        let index = *self.response_index.lock().unwrap();
        let responses = self.responses.lock().unwrap();
        let response = responses.get(index % responses.len()).cloned();
        *self.response_index.lock().unwrap() = index + 1;
        response.unwrap_or_else(|| LlmResponse {
            content: vec![ContentBlock::Text {
                text: "Mock response".to_string(),
                citation: None,
            }],
            stop_reason: StopReason::EndTurn,
            usage: Usage::default(),
            model: "mock-model".to_string(),
        })
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn stream(&self, request: LlmRequest) -> QueryResult<LlmStream> {
        self.record_request(request);
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let response = self.get_next_response();

        tokio::spawn(async move {
            for block in response.content {
                match block {
                    ContentBlock::Text { text, .. } => {
                        let _ = tx.send(LlmStreamChunk::Text { text }).await;
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        let _ = tx
                            .send(LlmStreamChunk::ToolUseStart {
                                id: id.clone(),
                                name: name.clone(),
                            })
                            .await;
                        let _ = tx
                            .send(LlmStreamChunk::ToolUseComplete { id, input })
                            .await;
                    }
                    _ => {}
                }
            }
            let _ = tx
                .send(LlmStreamChunk::Usage {
                    usage: response.usage,
                })
                .await;
            let _ = tx.send(LlmStreamChunk::Stop { reason: response.stop_reason }).await;
        });

        Ok(LlmStream::new(rx))
    }

    async fn complete(&self, request: LlmRequest) -> QueryResult<LlmResponse> {
        self.record_request(request);
        Ok(self.get_next_response())
    }
}

#[cfg(test)]
mod tests;
