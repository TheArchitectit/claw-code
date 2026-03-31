//! Result and data types for the runtime crate.
//!
//! This module provides result types, usage tracking, cost tracking,
//! and model information types used throughout the runtime.

use serde::{Deserialize, Serialize};

use crate::types::errors::{ToolError, QueryEngineError};

/// Result type for tool operations.
pub type ToolResult<T> = std::result::Result<T, ToolError>;

/// Result type for query engine operations.
pub type QueryResult<T> = std::result::Result<T, QueryEngineError>;

/// Action to take after handling a stop reason in the conversation loop.
#[derive(Debug, Clone)]
pub enum ConversationAction {
    /// Continue the conversation for another turn.
    Continue,
    /// End the conversation with success.
    End {
        /// The final response text.
        result: String,
        /// The stop reason that caused the end.
        stop_reason: String,
    },
    /// End the conversation with an error.
    Error {
        /// The error message.
        message: String,
    },
}

/// Usage statistics for an API call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Number of input tokens used.
    pub input_tokens: u32,

    /// Number of output tokens used.
    pub output_tokens: u32,

    /// Number of tokens read from cache.
    pub cache_read_input_tokens: u32,

    /// Number of tokens written to cache.
    pub cache_creation_input_tokens: u32,
}

impl Usage {
    /// Create a new usage record.
    #[must_use]
    pub fn new(
        input_tokens: u32,
        output_tokens: u32,
        cache_read_input_tokens: u32,
        cache_creation_input_tokens: u32,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_read_input_tokens,
            cache_creation_input_tokens,
        }
    }

    /// Accumulate another usage record into this one.
    pub fn accumulate(&mut self, other: &Self) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_input_tokens += other.cache_read_input_tokens;
        self.cache_creation_input_tokens += other.cache_creation_input_tokens;
    }

    /// Returns the total number of tokens used.
    #[must_use]
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} total tokens ({} input, {} output)",
            self.total_tokens(),
            self.input_tokens,
            self.output_tokens
        )
    }
}

/// Cost information for an API call.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    /// Cost for input tokens in USD.
    pub input_cost: f64,
    /// Cost for output tokens in USD.
    pub output_cost: f64,
}

impl Cost {
    /// Create a new cost record.
    #[must_use]
    pub fn new(input_cost: f64, output_cost: f64) -> Self {
        Self { input_cost, output_cost }
    }

    /// Returns the total cost in USD.
    #[must_use]
    pub fn total_cost(&self) -> f64 {
        self.input_cost + self.output_cost
    }

    /// Accumulate another cost record into this one.
    pub fn accumulate(&mut self, other: &Self) {
        self.input_cost += other.input_cost;
        self.output_cost += other.output_cost;
    }
}

/// Supported model providers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelProvider {
    /// Anthropic models.
    Anthropic,
    /// OpenAI models.
    OpenAi,
    /// Google models.
    Google,
    /// Ollama models.
    Ollama,
    /// Other/custom providers.
    Other,
    /// Custom provider.
    Custom(String),
}

/// Model information for tracking which model was used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// The model identifier.
    pub id: String,
    /// The model name.
    pub model: String,
    /// The model provider.
    pub provider: ModelProvider,
    /// The context window size.
    pub context_window: u32,
}

impl ModelInfo {
    /// Create a new model info record.
    #[must_use]
    pub fn new(id: impl Into<String>, model: impl Into<String>, provider: ModelProvider, context_window: u32) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            provider,
            context_window,
        }
    }
}

impl std::fmt::Display for ModelInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.model, self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_accumulation() {
        let mut usage1 = Usage::new(100, 50, 25, 10);
        let usage2 = Usage::new(50, 25, 15, 5);
        usage1.accumulate(&usage2);

        assert_eq!(usage1.input_tokens, 150);
        assert_eq!(usage1.output_tokens, 75);
        assert_eq!(usage1.cache_read_input_tokens, 40);
        assert_eq!(usage1.cache_creation_input_tokens, 15);
    }

    #[test]
    fn test_usage_total_tokens() {
        let usage = Usage::new(100, 50, 0, 0);
        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_usage_default() {
        let usage: Usage = Default::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
    }

    #[test]
    fn test_usage_display() {
        let usage = Usage::new(100, 50, 25, 10);
        let display = format!("{}", usage);
        assert!(display.contains("150")); // total tokens
    }

    #[test]
    fn test_cost_creation() {
        let cost = Cost::new(0.01, 0.02);
        assert_eq!(cost.input_cost, 0.01);
        assert_eq!(cost.output_cost, 0.02);
        assert_eq!(cost.total_cost(), 0.03);
    }

    #[test]
    fn test_cost_total_cost() {
        let cost = Cost::new(1.5, 2.5);
        assert_eq!(cost.total_cost(), 4.0);

        let default: Cost = Default::default();
        assert_eq!(default.total_cost(), 0.0);
    }

    #[test]
    fn test_model_info_creation() {
        let info = ModelInfo {
            id: "claude-3-test".to_string(),
            provider: ModelProvider::Anthropic,
            model: "claude-3".to_string(),
            context_window: 200000,
        };

        assert!(matches!(info.provider, ModelProvider::Anthropic));
        assert_eq!(info.model, "claude-3");
        assert_eq!(info.context_window, 200000);
    }

    #[test]
    fn test_model_provider_variants() {
        let providers = vec![
            ModelProvider::Anthropic,
            ModelProvider::OpenAi,
            ModelProvider::Google,
            ModelProvider::Ollama,
            ModelProvider::Custom("custom".to_string()),
        ];

        for provider in providers {
            let info = ModelInfo {
                id: "test-id".to_string(),
                provider,
                model: "test".to_string(),
                context_window: 1000,
            };
            let display = format!("{}", info);
            assert!(!display.is_empty());
        }
    }
}
