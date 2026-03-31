//! Error handling and recovery mechanisms for the QueryEngine.

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::context::ToolUseContext;
use crate::messages::{Message, PermissionDenial, ToolResultContent};
use crate::permissions::PermissionResult;
use crate::tool::ToolOutput;
use crate::types::{QueryEngineError, QueryResult, Usage, SessionId, ToolError, ToolUseId};

// Alias Usage as RuntimeUsage for compatibility
pub type RuntimeUsage = Usage;

/// A checkpoint for conversation state recovery.
#[derive(Debug, Clone)]
pub struct ConversationCheckpoint {
    pub messages: Vec<Message>,
    pub usage: RuntimeUsage,
    pub cost: f64,
    pub session_id: SessionId,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Retry configuration for operations.
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
        }
    }
}

/// Execute an operation with retry logic using exponential backoff.
///
/// This function will retry the operation up to `max_retries` times if it
/// fails with a transient error. The delay between retries doubles each time.
pub async fn with_retry<F, Fut, T>(
    operation: F,
    max_retries: u32,
    base_delay_ms: u64,
) -> QueryResult<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = QueryResult<T>>,
{
    let mut last_error: Option<QueryEngineError> = None;

    for attempt in 0..=max_retries {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!("Operation succeeded after {} retries", attempt);
                }
                return Ok(result);
            }
            Err(e) => {
                if !e.is_retryable() {
                    debug!("Non-retryable error, failing fast: {}", e);
                    return Err(e);
                }

                if attempt >= max_retries {
                    error!("Max retries ({}) exceeded. Last error: {}", max_retries, e);
                    return Err(QueryEngineError::MaxRetriesExceeded {
                        max_retries,
                        last_error: e.to_string(),
                    });
                }

                let delay_ms = base_delay_ms * (1_u64 << attempt).min(60000);
                warn!(
                    "Transient error on attempt {}: {}. Retrying after {}ms...",
                    attempt + 1,
                    e,
                    delay_ms
                );

                last_error = Some(e);
                sleep(Duration::from_millis(delay_ms)).await;
            }
        }
    }

    Err(QueryEngineError::MaxRetriesExceeded {
        max_retries,
        last_error: last_error.map_or_else(|| "Unknown error".to_string(), |e| e.to_string()),
    })
}

/// Extract retry-after duration from error message.
///
/// Attempts to parse retry-after seconds from error messages.
pub fn extract_retry_after(message: &str) -> Option<u64> {
    let msg_lower = message.to_lowercase();

    for pattern in ["retry after ", "retry-after:", "try again in "] {
        if let Some(pos) = msg_lower.find(pattern) {
            let after_pattern = &msg_lower[pos + pattern.len()..];
            let num_str: String = after_pattern
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(secs) = num_str.parse::<u64>() {
                return Some(secs);
            }
        }
    }

    None
}

/// Calculate delay for rate limit based on retry attempt.
///
/// Uses exponential backoff: base_delay * 2^attempt, capped at 60 seconds.
pub fn calculate_rate_limit_delay(attempt: u32, base_delay_ms: u64) -> u64 {
    base_delay_ms * (1_u64 << attempt).min(60000)
}

/// Convert HTTP status code to error type.
///
/// Maps common HTTP status codes to appropriate error categories.
pub fn http_status_to_error(status_code: u16, message: &str) -> crate::types::LlmApiError {
    use crate::types::LlmApiError;

    match status_code {
        429 => {
            let retry_secs = extract_retry_after(message).unwrap_or(60);
            LlmApiError::rate_limit(retry_secs)
        }
        401 | 403 => LlmApiError::authentication(message),
        500..=599 => LlmApiError::server_error(status_code, message),
        408 | 504 => LlmApiError::timeout(30),
        _ => {
            // Check message content for specific error types
            let msg_lower = message.to_lowercase();
            if msg_lower.contains("rate limit") || msg_lower.contains("too many requests") {
                let retry_secs = extract_retry_after(&msg_lower).unwrap_or(60);
                LlmApiError::rate_limit(retry_secs)
            } else if msg_lower.contains("timeout") || msg_lower.contains("timed out") {
                LlmApiError::timeout(30)
            } else if msg_lower.contains("authentication")
                || msg_lower.contains("unauthorized")
                || msg_lower.contains("api key")
            {
                LlmApiError::authentication(message)
            } else if msg_lower.contains("parse") || msg_lower.contains("invalid json") {
                LlmApiError::parse_error(message)
            } else if msg_lower.contains("network")
                || msg_lower.contains("connection")
                || msg_lower.contains("dns")
            {
                LlmApiError::network(message)
            } else {
                LlmApiError::Other {
                    message: message.to_string(),
                }
            }
        }
    }
}

/// Execute a function with panic catching.
///
/// This spawns the function in a new tokio task to catch any panics.
/// The function is suitable for wrapping tool executions.
pub async fn with_panic_catch<F, Fut, T>(f: F) -> QueryResult<T>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: Future<Output = QueryResult<T>> + Send,
    T: Send + 'static,
{
    let handle = tokio::spawn(async move { f().await });

    match handle.await {
        Ok(result) => result,
        Err(join_error) => {
            if join_error.is_panic() {
                let panic_msg = join_error.to_string();
                error!("Task panicked: {}", panic_msg);
                Err(QueryEngineError::Tool(ToolError::panic(format!(
                    "Task panicked: {}",
                    panic_msg
                ))))
            } else {
                Err(QueryEngineError::Tool(ToolError::other(format!(
                    "Task cancelled: {}",
                    join_error
                ))))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay_ms, 1000);
    }

    #[tokio::test]
    async fn test_with_retry_success_first_try() {
        let result: QueryResult<i32> = with_retry(|| async { Ok(42) }, 3, 100).await;
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_extract_retry_after() {
        assert_eq!(extract_retry_after("retry after 60 seconds"), Some(60));
        assert_eq!(extract_retry_after("Retry-After: 120"), Some(120));
        assert_eq!(extract_retry_after("try again in 30 seconds"), Some(30));
        assert_eq!(extract_retry_after("no retry info"), None);
    }
}
