//! Error types for the runtime crate.
//!
//! This module provides error types and error handling patterns used
//! throughout the runtime including tool errors, LLM API errors, and
//! query engine errors.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Error, Debug, Clone)]
pub enum ToolError {
    /// The tool execution was cancelled.
    #[error("Tool execution was cancelled")]
    Cancelled,

    /// The tool input was invalid.
    #[error("Invalid tool input: {message}")]
    InvalidInput { message: String },

    /// The tool execution failed with an error.
    #[error("Tool execution failed: {message}")]
    ExecutionFailed { message: String },

    /// The tool was not found.
    #[error("Tool not found: {name}")]
    NotFound { name: String },

    /// Permission was denied for the tool execution.
    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    /// A timeout occurred during tool execution.
    #[error("Tool execution timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    /// The tool execution panicked.
    #[error("Tool execution panicked: {message}")]
    Panic { message: String },

    /// An internal error occurred.
    #[error("Internal error: {message}")]
    Internal { message: String },
}

impl ToolError {
    /// Create an invalid input error.
    #[must_use]
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create an execution failed error.
    #[must_use]
    pub fn execution_failed(message: impl Into<String>) -> Self {
        Self::ExecutionFailed {
            message: message.into(),
        }
    }

    /// Create a not found error.
    #[must_use]
    pub fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound {
            name: name.into(),
        }
    }

    /// Create a permission denied error.
    #[must_use]
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::PermissionDenied {
            message: message.into(),
        }
    }

    /// Create a timeout error.
    #[must_use]
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    /// Create a panic error.
    #[must_use]
    pub fn panic(message: impl Into<String>) -> Self {
        Self::Panic {
            message: message.into(),
        }
    }

    /// Create an internal error.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }

    /// Create a cancelled error.
    #[must_use]
    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    /// Create an other/generic error (alias for internal).
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

/// Category of error for determining retry behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Transient errors that may succeed on retry.
    Transient,
    /// Permanent errors that should fail fast.
    Permanent,
}

/// Specific LLM API error types.
#[derive(Error, Debug, Clone)]
pub enum LlmApiError {
    /// Rate limit exceeded (429) - retry with backoff.
    #[error("Rate limit exceeded. Retry after {retry_after_secs}s")]
    RateLimit { retry_after_secs: u64 },

    /// Authentication failed (401/403) - fail fast.
    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    /// Server error (5xx) - retry with backoff.
    #[error("Server error (status {status_code}): {message}")]
    ServerError { status_code: u16, message: String },

    /// Request timeout - retry with backoff.
    #[error("Request timeout after {timeout_secs}s")]
    Timeout { timeout_secs: u64 },

    /// Failed to parse response.
    #[error("Failed to parse LLM response: {message}")]
    ParseError { message: String },

    /// Network error - retry with backoff.
    #[error("Network error: {message}")]
    Network { message: String },

    /// Model temporarily unavailable - retry with fallback.
    #[error("Model temporarily unavailable: {model}")]
    ModelUnavailable { model: String },

    /// Context length exceeded - retry with larger context model.
    #[error("Context length exceeded: {message}")]
    ContextLengthExceeded { message: String },

    /// Generic API error.
    #[error("API error: {message}")]
    Other { message: String },
}

impl LlmApiError {
    /// Get the error category for determining retry behavior.
    #[must_use]
    pub fn category(&self) -> ErrorCategory {
        match self {
            // Transient errors - should retry
            Self::RateLimit { .. }
            | Self::ServerError { .. }
            | Self::Timeout { .. }
            | Self::Network { .. }
            | Self::ModelUnavailable { .. }
            | Self::ContextLengthExceeded { .. } => ErrorCategory::Transient,
            // Permanent errors - fail fast
            Self::Authentication { .. } | Self::ParseError { .. } | Self::Other { .. } => {
                ErrorCategory::Permanent
            }
        }
    }

    /// Check if this error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self.category(), ErrorCategory::Transient)
    }

    /// Check if this error should trigger model fallback.
    #[must_use]
    pub fn is_fallback_trigger(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. }
                | Self::ModelUnavailable { .. }
                | Self::Timeout { .. }
                | Self::ContextLengthExceeded { .. }
        )
    }

    /// Create a rate limit error.
    #[must_use]
    pub fn rate_limit(retry_after_secs: u64) -> Self {
        Self::RateLimit { retry_after_secs }
    }

    /// Create an authentication error.
    #[must_use]
    pub fn authentication(message: impl Into<String>) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a server error.
    #[must_use]
    pub fn server_error(status_code: u16, message: impl Into<String>) -> Self {
        Self::ServerError {
            status_code,
            message: message.into(),
        }
    }

    /// Create a timeout error.
    #[must_use]
    pub fn timeout(timeout_secs: u64) -> Self {
        Self::Timeout { timeout_secs }
    }

    /// Create a parse error.
    #[must_use]
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::ParseError {
            message: message.into(),
        }
    }

    /// Create a network error.
    #[must_use]
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
        }
    }

    /// Create a model unavailable error.
    #[must_use]
    pub fn model_unavailable(model: impl Into<String>) -> Self {
        Self::ModelUnavailable {
            model: model.into(),
        }
    }

    /// Create a context length exceeded error.
    #[must_use]
    pub fn context_length_exceeded(message: impl Into<String>) -> Self {
        Self::ContextLengthExceeded {
            message: message.into(),
        }
    }
}

/// Errors that can occur in the query engine.
#[derive(Error, Debug)]
pub enum QueryEngineError {
    /// A tool error occurred.
    #[error(transparent)]
    Tool(#[from] ToolError),

    /// The query was aborted.
    #[error("Query was aborted")]
    Aborted,

    /// Maximum number of turns exceeded.
    #[error("Maximum number of turns ({max_turns}) exceeded")]
    MaxTurnsExceeded { max_turns: u32 },

    /// Maximum budget exceeded.
    #[error("Maximum budget (${max_budget:.2}) exceeded")]
    MaxBudgetExceeded { max_budget: f64 },

    /// An LLM API error occurred.
    #[error(transparent)]
    LlmApi(#[from] LlmApiError),

    /// A generic API error occurred.
    #[error("API error: {message}")]
    Api { message: String },

    /// A validation error occurred.
    #[error("Validation error: {message}")]
    Validation { message: String },

    /// Max retries exceeded for a retryable operation.
    #[error("Max retries ({max_retries}) exceeded. Last error: {last_error}")]
    MaxRetriesExceeded { max_retries: u32, last_error: String },

    /// Checkpoint not found.
    #[error("Checkpoint not found: {checkpoint_id}")]
    CheckpointNotFound { checkpoint_id: String },

    /// State persistence error.
    #[error("State persistence error: {message}")]
    StatePersistence { message: String },
}

impl QueryEngineError {
    /// Get the error category if applicable.
    #[must_use]
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::LlmApi(api_err) => api_err.category(),
            Self::Tool(tool_err) => match tool_err {
                // Tool panics and timeouts are transient
                ToolError::Panic { .. } | ToolError::Timeout { .. } => ErrorCategory::Transient,
                // Most other tool errors are permanent
                ToolError::InvalidInput { .. }
                | ToolError::NotFound { .. }
                | ToolError::PermissionDenied { .. }
                | ToolError::Cancelled
                | ToolError::Internal { .. }
                | ToolError::ExecutionFailed { .. } => ErrorCategory::Permanent,
            },
            // All other query engine errors are permanent
            _ => ErrorCategory::Permanent,
        }
    }

    /// Check if this error is retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self.category(), ErrorCategory::Transient)
    }

    /// Check if this error should trigger model fallback.
    #[must_use]
    pub fn is_fallback_trigger(&self) -> bool {
        match self {
            Self::LlmApi(api_err) => api_err.is_fallback_trigger(),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_error_variants() {
        let cancelled = ToolError::cancelled();
        assert!(matches!(cancelled, ToolError::Cancelled));

        let invalid = ToolError::invalid_input("test");
        assert!(matches!(invalid, ToolError::InvalidInput { .. }));

        let execution = ToolError::execution_failed("test");
        assert!(matches!(execution, ToolError::ExecutionFailed { .. }));

        let not_found = ToolError::not_found("tool");
        assert!(matches!(not_found, ToolError::NotFound { .. }));

        let permission = ToolError::permission_denied("test");
        assert!(matches!(permission, ToolError::PermissionDenied { .. }));

        let timeout = ToolError::timeout(5000);
        assert!(matches!(timeout, ToolError::Timeout { .. }));

        let internal = ToolError::internal("test");
        assert!(matches!(internal, ToolError::Internal { .. }));
    }

    #[test]
    fn test_query_engine_error_variants() {
        let tool_err = ToolError::not_found("test");
        let query_err = QueryEngineError::Tool(tool_err);
        assert!(matches!(query_err, QueryEngineError::Tool(_)));

        let aborted = QueryEngineError::Aborted;
        assert!(matches!(aborted, QueryEngineError::Aborted));

        let max_turns = QueryEngineError::MaxTurnsExceeded { max_turns: 50 };
        assert!(matches!(max_turns, QueryEngineError::MaxTurnsExceeded { .. }));

        let budget = QueryEngineError::MaxBudgetExceeded { max_budget: 10.0 };
        assert!(matches!(budget, QueryEngineError::MaxBudgetExceeded { .. }));

        let api = QueryEngineError::Api { message: "test".to_string() };
        assert!(matches!(api, QueryEngineError::Api { .. }));

        let validation = QueryEngineError::Validation { message: "test".to_string() };
        assert!(matches!(validation, QueryEngineError::Validation { .. }));

        let checkpoint_not_found = QueryEngineError::CheckpointNotFound {
            checkpoint_id: "chk-123".to_string()
        };
        assert!(matches!(checkpoint_not_found, QueryEngineError::CheckpointNotFound { .. }));

        let state_err = QueryEngineError::StatePersistence {
            message: "disk full".to_string()
        };
        assert!(matches!(state_err, QueryEngineError::StatePersistence { .. }));
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::invalid_input("missing field");
        let display = format!("{}", err);
        assert!(display.contains("missing field"));
    }
}
