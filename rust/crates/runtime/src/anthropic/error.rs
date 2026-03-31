//! Anthropic API error types.

/// Errors that can occur when interacting with the Anthropic API.
#[derive(Debug, thiserror::Error)]
pub enum AnthropicError {
    /// HTTP request failed.
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// API returned an error.
    #[error("API error: {message} (type: {error_type})")]
    Api {
        /// Error message.
        message: String,
        /// Error type.
        error_type: String,
    },

    /// Failed to parse response.
    #[error("Failed to parse response: {0}")]
    Parse(String),

    /// Streaming error.
    #[error("Streaming error: {0}")]
    Stream(String),

    /// Missing API key.
    #[error("ANTHROPIC_API_KEY environment variable not set")]
    MissingApiKey,

    /// Rate limited.
    #[error("Rate limited. Retry after {retry_after}s")]
    RateLimited {
        /// Seconds to wait before retrying.
        retry_after: u64,
    },

    /// Request timeout.
    #[error("Request timed out after {0}s")]
    Timeout(u64),
}
