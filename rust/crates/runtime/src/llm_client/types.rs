//! Request and response types for LLM API.
//!
//! This module provides the core types for making requests to and
//! receiving responses from LLM APIs.

use crate::messages::{ContentBlock, Message};
use crate::llm_client::StopReason;
use crate::types::Usage;

/// Request to the LLM API.
#[derive(Debug, Clone)]
pub struct LlmRequest {
    /// The model to use.
    pub model: String,
    /// The system prompt.
    pub system: Option<String>,
    /// The conversation messages.
    pub messages: Vec<Message>,
    /// Available tools.
    pub tools: Vec<serde_json::Value>,
    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,
    /// Temperature for sampling.
    pub temperature: Option<f32>,
}

impl LlmRequest {
    /// Create a new LLM request with the given model.
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system: None,
            messages: Vec::new(),
            tools: Vec::new(),
            max_tokens: None,
            temperature: None,
        }
    }

    /// Set the system prompt.
    #[must_use]
    pub fn with_system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(system.into());
        self
    }

    /// Set the messages.
    #[must_use]
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Set the tools.
    #[must_use]
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = tools;
        self
    }

    /// Set the max tokens.
    #[must_use]
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// A complete LLM response (non-streaming).
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// The content blocks.
    pub content: Vec<ContentBlock>,
    /// The stop reason.
    pub stop_reason: StopReason,
    /// Token usage.
    pub usage: Usage,
    /// The model used.
    pub model: String,
}
