/// Error handling and retry logic for QueryExecution.
///
/// This impl block provides:
/// - Retry logic with exponential backoff for transient errors
/// - LLM API error categorization and handling
/// - Tool execution with panic catching
/// - Conversation state recovery
impl QueryExecution {
    /// Execute an operation with retry logic using exponential backoff.
    ///
    /// This function will retry the operation up to `max_retries` times if it
    /// fails with a transient error. The delay between retries doubles each time
    /// (exponential backoff).
    ///
    /// # Arguments
    /// * `operation` - The async operation to execute
    /// * `max_retries` - Maximum number of retry attempts
    /// * `base_delay_ms` - Initial delay in milliseconds (doubles each retry)
    ///
    /// # Returns
    /// * `QueryResult<T>` - The result of the operation
    ///
    /// # Errors
    /// Returns `QueryEngineError::MaxRetriesExceeded` if all retries fail.
    pub async fn with_retry<F, Fut, T>(
        &self,
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
                    // Check if error is retryable
                    if !e.is_retryable() {
                        debug!("Non-retryable error, failing fast: {}", e);
                        return Err(e);
                    }

                    // If this was the last attempt, return the error
                    if attempt >= max_retries {
                        error!("Max retries ({}) exceeded. Last error: {}", max_retries, e);
                        return Err(QueryEngineError::MaxRetriesExceeded {
                            max_retries,
                            last_error: e.to_string(),
                        });
                    }

                    // Calculate exponential backoff delay: base_delay * 2^attempt
                    let delay_ms = base_delay_ms * (1_u64 << attempt).min(60000); // Cap at 60s
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

        // This should never be reached due to the return in the loop
        Err(QueryEngineError::MaxRetriesExceeded {
            max_retries,
            last_error: last_error.map_or_else(|| "Unknown error".to_string(), |e| e.to_string()),
        })
    }

    /// Get LLM response with retry logic.
    ///
    /// This method wraps `get_llm_response` with automatic retry for transient
    /// errors like rate limits, timeouts, and server errors.
    ///
    /// # Arguments
    /// * `max_retries` - Maximum number of retry attempts
    /// * `base_delay_ms` - Initial delay between retries
    ///
    /// # Returns
    /// * `QueryResult<(AssistantMessage, StopReason, RuntimeUsage)>` - The LLM response
    async fn get_llm_response_with_retry(
        &self,
        max_retries: u32,
        base_delay_ms: u64,
    ) -> QueryResult<(AssistantMessage, StopReason, RuntimeUsage)> {
        self.with_retry(
            || async { self.get_llm_response().await },
            max_retries,
            base_delay_ms,
        )
        .await
    }

    /// Execute a tool with panic catching and error handling.
    ///
    /// This method wraps tool execution to catch any panics and convert them
    /// into proper error results. This prevents a single misbehaving tool from
    /// crashing the entire conversation.
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool to execute
    /// * `input` - Tool input parameters
    /// * `tool_use_id` - Unique ID for this tool use
    ///
    /// # Returns
    /// * `QueryResult<ToolOutput>` - Tool output or error
    pub async fn execute_tool_with_error_handling(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        tool_use_id: ToolUseId,
    ) -> QueryResult<ToolOutput> {
        // Log the tool execution attempt
        info!("Executing tool '{}' with ID {}", tool_name, tool_use_id);

        // Wrap the tool execution to catch panics
        let result = self.execute_tool_with_panic_catch(tool_name, input, tool_use_id.clone()).await;

        match &result {
            Ok(output) => {
                debug!("Tool '{}' executed successfully", tool_name);
                Ok(output.clone())
            }
            Err(e) => {
                error!("Tool '{}' execution failed: {}", tool_name, e);
                Err(e.clone())
            }
        }
    }

    /// Execute tool with panic catching.
    ///
    /// Uses AssertUnwindSafe to catch any panics during tool execution
    /// and convert them into a ToolError::Panic.
    async fn execute_tool_with_panic_catch(
        &self,
        tool_name: &str,
        input: serde_json::Value,
        tool_use_id: ToolUseId,
    ) -> QueryResult<ToolOutput> {
        // Clone necessary data for the async block
        let tool_name = tool_name.to_string();
        let input_clone = input.clone();
        let tool_use_id_clone = tool_use_id.clone();

        // Use AssertUnwindSafe since we're controlling the unwind safety
        let result = std::panic::AssertUnwindSafe(async move {
            self.execute_tool(&tool_name, input_clone, tool_use_id_clone).await
        })
        .catch_unwind()
        .await;

        match result {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => Err(e),
            Err(panic_payload) => {
                // Convert panic into a ToolError
                let panic_msg = if let Some(s) = panic_payload.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_payload.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown panic".to_string()
                };

                error!("Tool '{}' panicked: {}", tool_name, panic_msg);
                Err(QueryEngineError::Tool(ToolError::panic(format!(
                    "Tool '{}' panicked: {}",
                    tool_name, panic_msg
                ))))
            }
        }
    }

    /// Save the current conversation state for recovery.
    ///
    /// This method captures the current messages and state so that the
    /// conversation can be resumed from this point if a failure occurs.
    ///
    /// # Returns
    /// * `ConversationCheckpoint` - The saved state
    async fn save_conversation_state(&self) -> ConversationCheckpoint {
        let messages = self.messages.read().await.clone();
        let usage = *self.total_usage.lock().await;
        let cost = *self.total_cost.lock().await;

        ConversationCheckpoint {
            messages,
            usage,
            cost,
            session_id: self.session_id,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Restore conversation state from a checkpoint.
    ///
    /// This method restores the conversation to a previously saved state.
    /// Useful for recovering from failures.
    ///
    /// # Arguments
    /// * `checkpoint` - The checkpoint to restore from
    async fn restore_conversation_state(&self, checkpoint: &ConversationCheckpoint) {
        let mut messages = self.messages.write().await;
        *messages = checkpoint.messages.clone();

        let mut usage = self.total_usage.lock().await;
        *usage = checkpoint.usage;

        let mut cost = self.total_cost.lock().await;
        *cost = checkpoint.cost;

        info!("Restored conversation state from checkpoint");
    }

    /// Execute a risky operation with state preservation.
    ///
    /// This method saves the conversation state before executing an operation
    /// and restores it if the operation fails with a transient error.
    ///
    /// # Arguments
    /// * `operation` - The operation to execute
    ///
    /// # Returns
    /// * `QueryResult<T>` - The result of the operation
    pub async fn with_state_preservation<F, Fut, T>(&self, operation: F) -> QueryResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = QueryResult<T>>,
    {
        // Save state before the operation
        let checkpoint = self.save_conversation_state().await;
        debug!("Saved conversation state before risky operation");

        match operation().await {
            Ok(result) => {
                debug!("Risky operation succeeded");
                Ok(result)
            }
            Err(e) => {
                if e.is_retryable() {
                    warn!(
                        "Risky operation failed with transient error, restoring state: {}",
                        e
                    );
                    self.restore_conversation_state(&checkpoint).await;
                } else {
                    error!("Risky operation failed with permanent error: {}", e);
                }
                Err(e)
            }
        }
    }

    /// Categorize an error for handling.
    ///
    /// This method determines if an error is transient (retryable) or
    /// permanent (fail fast).
    ///
    /// # Arguments
    /// * `error` - The error to categorize
    ///
    /// # Returns
    /// * `ErrorCategory` - Transient or Permanent
    pub fn categorize_error(&self, error: &QueryEngineError) -> ErrorCategory {
        error.category()
    }

    /// Convert an API error message into a structured LlmApiError.
    ///
    /// This method parses error messages from the LLM API and categorizes
    /// them into specific error types for appropriate handling.
    ///
    /// # Arguments
    /// * `message` - The error message from the API
    /// * `status_code` - Optional HTTP status code
    ///
    /// # Returns
    /// * `LlmApiError` - The structured error
    pub fn parse_llm_api_error(&self, message: &str, status_code: Option<u16>) -> LlmApiError {
        let msg_lower = message.to_lowercase();

        // Check for specific error patterns
        match status_code {
            Some(429) => {
                // Rate limit - try to extract retry-after if present
                let retry_secs = self.extract_retry_after(&msg_lower).unwrap_or(60);
                LlmApiError::rate_limit(retry_secs)
            }
            Some(401) | Some(403) => {
                // Authentication error - fail fast
                LlmApiError::authentication(message)
            }
            Some(500..=599) => {
                // Server error - retryable
                LlmApiError::server_error(status_code.unwrap_or(500), message)
            }
            Some(408) | Some(504) => {
                // Timeout errors - retryable
                LlmApiError::timeout(30)
            }
            _ => {
                // Check message content for specific error types
                if msg_lower.contains("rate limit") || msg_lower.contains("too many requests") {
                    let retry_secs = self.extract_retry_after(&msg_lower).unwrap_or(60);
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

    /// Extract retry-after duration from error message.
    ///
    /// Attempts to parse retry-after seconds from error messages.
    fn extract_retry_after(&self, message: &str) -> Option<u64> {
        // Look for patterns like "retry after 60 seconds" or "Retry-After: 60"
        let patterns = [
            r"retry after (\d+) seconds?",
            r"retry-after[:\s]*(\d+)",
            r"try again in (\d+) seconds?",
        ];

        for pattern in &patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if let Some(caps) = re.captures(message) {
                    if let Some(matched) = caps.get(1) {
                        if let Ok(secs) = matched.as_str().parse::<u64>() {
                            return Some(secs);
                        }
                    }
                }
            }
        }

        None
    }
}

/// A checkpoint for conversation state recovery.
///
/// This structure stores the conversation state at a point in time,
/// allowing for recovery from failures.
#[derive(Debug, Clone)]
pub struct ConversationCheckpoint {
    /// Messages at the time of checkpoint.
    pub messages: Vec<Message>,
    /// Token usage at checkpoint.
    pub usage: RuntimeUsage,
    /// Cost at checkpoint.
    pub cost: f64,
    /// Session ID.
    pub session_id: SessionId,
    /// Timestamp of checkpoint.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
