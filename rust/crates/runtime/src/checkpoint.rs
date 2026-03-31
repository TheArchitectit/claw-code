//! Checkpoint persistence for conversation state
//!
//! This module provides checkpoint save/restore functionality for QueryEngine,
//! enabling conversation persistence and recovery during long-running sessions.

use std::time::Duration;

use state::{CheckpointId, CheckpointMetadata, ConversationState, SessionManager};

/// Extension trait for checkpoint persistence on QueryEngine
#[async_trait::async_trait]
pub trait QueryEngineCheckpoint {
    /// Save a checkpoint of the current conversation state
    async fn save_checkpoint(
        &self,
        session_manager: &SessionManager,
        description: Option<String>,
    ) -> Result<CheckpointId, crate::types::QueryEngineError>;

    /// Restore conversation state from a checkpoint
    async fn restore_from_checkpoint(
        &mut self,
        session_manager: &SessionManager,
        checkpoint_id: CheckpointId,
    ) -> Result<(), crate::types::QueryEngineError>;

    /// List all available checkpoints for this session
    async fn list_checkpoints(
        &self,
        session_manager: &SessionManager,
    ) -> Result<Vec<CheckpointMetadata>, crate::types::QueryEngineError>;

    /// Delete a specific checkpoint
    async fn delete_checkpoint(
        &self,
        session_manager: &SessionManager,
        checkpoint_id: &CheckpointId,
    ) -> Result<(), crate::types::QueryEngineError>;

    /// Clean up old checkpoints, keeping only the most recent N
    async fn cleanup_old_checkpoints(
        &self,
        session_manager: &SessionManager,
        keep_count: usize,
    ) -> Result<usize, crate::types::QueryEngineError>;

    /// Attempt to recover from the most recent checkpoint
    async fn attempt_recovery(
        &mut self,
        session_manager: &SessionManager,
    ) -> Result<RecoveryResult, crate::types::QueryEngineError>;
}

/// Auto-save checkpoint manager
#[derive(Debug)]
pub struct AutoSaveManager {
    /// Last checkpoint save time
    last_save_time: std::time::Instant,
    /// Auto-save interval in seconds
    interval_secs: u64,
    /// Whether auto-save is enabled
    enabled: bool,
}

impl AutoSaveManager {
    /// Create a new auto-save manager with default interval (5 minutes)
    pub fn new() -> Self {
        Self {
            last_save_time: std::time::Instant::now(),
            interval_secs: 300, // 5 minutes default
            enabled: true,
        }
    }

    /// Create with custom interval
    pub fn with_interval(interval_secs: u64) -> Self {
        Self {
            last_save_time: std::time::Instant::now(),
            interval_secs,
            enabled: true,
        }
    }

    /// Check if auto-save should trigger
    pub fn should_save(&self) -> bool {
        if !self.enabled {
            return false;
        }
        self.last_save_time.elapsed() >= Duration::from_secs(self.interval_secs)
    }

    /// Mark that a save has occurred
    pub fn mark_saved(&mut self) {
        self.last_save_time = std::time::Instant::now();
    }

    /// Enable auto-save
    pub fn enable(&mut self) {
        self.enabled = true;
    }

    /// Disable auto-save
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Check if auto-save is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get the interval
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }

    /// Set the interval
    pub fn set_interval(&mut self, secs: u64) {
        self.interval_secs = secs;
    }
}

impl Default for AutoSaveManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Partial conversation recovery result
#[derive(Debug, Clone)]
pub enum RecoveryResult {
    /// Successfully recovered from checkpoint
    Recovered {
        /// The checkpoint ID used for recovery
        checkpoint_id: CheckpointId,
        /// How many messages were restored
        message_count: usize,
        /// Notification message for user
        notification: String,
    },
    /// No recovery needed - conversation is fresh
    NoRecoveryNeeded,
    /// Recovery failed
    Failed {
        /// Error message
        error: String,
    },
}

/// Helper to attempt recovery from the most recent checkpoint
pub async fn attempt_recovery(
    session_manager: &SessionManager,
    session_id: &str,
) -> Result<RecoveryResult, state::StateError> {
    let checkpoints = session_manager.list_checkpoints(&session_id.to_string()).await?;

    if checkpoints.is_empty() {
        return Ok(RecoveryResult::NoRecoveryNeeded);
    }

    // Get the most recent checkpoint
    let most_recent = &checkpoints[0];
    let checkpoint_id = most_recent.id.clone();

    Ok(RecoveryResult::Recovered {
        checkpoint_id,
        message_count: most_recent.message_count,
        notification: format!(
            "Conversation recovered from checkpoint ({} messages, {} turns)",
            most_recent.message_count, most_recent.turn_count
        ),
    })
}

/// Generate a unique checkpoint ID
pub fn generate_checkpoint_id() -> CheckpointId {
    format!(
        "chk-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Helper to create usage stats JSON
pub fn create_usage_stats(
    input_tokens: u32,
    output_tokens: u32,
    cache_read_input_tokens: u32,
    cache_creation_input_tokens: u32,
) -> serde_json::Value {
    serde_json::json!({
        "input_tokens": input_tokens,
        "output_tokens": output_tokens,
        "cache_read_input_tokens": cache_read_input_tokens,
        "cache_creation_input_tokens": cache_creation_input_tokens,
    })
}

// Internal helper to capture conversation state from QueryEngine
#[doc(hidden)]
pub async fn capture_conversation_state(
    engine: &crate::query_engine::QueryEngine,
    description: Option<String>,
) -> ConversationState {
    let checkpoint_id = generate_checkpoint_id();
    let session_id = engine.session_id().to_string();

    // Capture current state
    let messages = engine.get_messages().await;
    let total_cost = engine.total_cost().await;
    let total_usage = engine.total_usage().await;

    // Convert messages to JSON
    let messages_json: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null))
        .collect();

    // Note: Permission denials, turn count, and usage are accessed via internal
    // synchronization in the actual implementation. Here we use what's available
    // through public methods.

    let usage_stats = create_usage_stats(
        total_usage.input_tokens,
        total_usage.output_tokens,
        total_usage.cache_read_input_tokens,
        total_usage.cache_creation_input_tokens,
    );

    ConversationState::new(session_id, checkpoint_id)
        .with_description(description.unwrap_or_else(|| "Conversation checkpoint".to_string()))
        .with_messages(messages_json)
        .with_usage_stats(usage_stats)
        .with_total_cost(total_cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_save_manager() {
        let mut manager = AutoSaveManager::new();
        assert!(manager.is_enabled());
        assert!(!manager.should_save()); // Should not trigger immediately

        // Disable and check
        manager.disable();
        assert!(!manager.should_save());

        // Enable with very short interval
        manager.enable();
        manager.set_interval(0);
        std::thread::sleep(Duration::from_millis(10)); // Small delay
        assert!(manager.should_save());

        // Mark saved and check again
        manager.mark_saved();
        assert!(!manager.should_save());
    }

    #[test]
    fn test_generate_checkpoint_id() {
        let id1 = generate_checkpoint_id();
        let id2 = generate_checkpoint_id();
        assert_ne!(id1, id2);
        assert!(id1.starts_with("chk-"));
    }

    #[test]
    fn test_create_usage_stats() {
        let stats = create_usage_stats(100, 50, 25, 10);
        assert_eq!(stats["input_tokens"], 100);
        assert_eq!(stats["output_tokens"], 50);
        assert_eq!(stats["cache_read_input_tokens"], 25);
        assert_eq!(stats["cache_creation_input_tokens"], 10);
    }
}
