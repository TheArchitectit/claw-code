//! ID types for the runtime crate.
//!
//! This module provides unique identifier types used throughout the runtime
//! for messages, sessions, tool uses, and checkpoints.

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unique identifier for messages in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Generate a new random message ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for MessageId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A unique identifier for tool uses within a conversation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolUseId(pub String);

impl ToolUseId {
    /// Create a new tool use ID with the given string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new random tool use ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(format!("toolu_{}", Uuid::new_v4().simple()))
    }
}

impl Display for ToolUseId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A unique identifier for a conversation session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub Uuid);

impl SessionId {
    /// Generate a new random session ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A unique identifier for a conversation checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    /// Create a new checkpoint ID with the given string.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a new unique checkpoint ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(format!("chk-{}", Uuid::new_v4().simple()))
    }
}

impl Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_id_generation() {
        let id1 = MessageId::new();
        let id2 = MessageId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_session_id_creation() {
        let id1 = SessionId::new();
        let id2 = SessionId::new();
        assert_ne!(id1, id2);

        // Test default
        let id3: SessionId = Default::default();
        let _ = id3.to_string();
    }

    #[test]
    fn test_tool_use_id_creation() {
        let id1 = ToolUseId::generate();
        let id2 = ToolUseId::generate();
        assert_ne!(id1, id2);

        let custom = ToolUseId::new("custom-id");
        assert_eq!(custom.0, "custom-id");
    }

    #[test]
    fn test_checkpoint_id_creation() {
        let id1 = CheckpointId::generate();
        let id2 = CheckpointId::generate();
        assert_ne!(id1.0, id2.0);

        let custom = CheckpointId::new("custom-id");
        assert_eq!(custom.0, "custom-id");

        let display = format!("{}", id1);
        assert!(display.starts_with("chk-"));
    }

    #[test]
    fn test_message_id_default() {
        let id: MessageId = Default::default();
        let id2 = MessageId::new();
        // Just verify they are different UUIDs
        assert_ne!(id.0, id2.0);
    }
}
