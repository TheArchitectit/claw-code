//! Core message types for conversations.
//!
//! This module defines the main message structures used in the conversation flow
//! between the user, assistant, and tools.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{MessageId, SessionId, ToolUseId, Usage as RuntimeUsage};

/// A message from the user.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The message content.
    pub content: UserMessageContent,

    /// Whether this is a meta message (e.g., a caveat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta: Option<bool>,

    /// Whether this message is only visible in the transcript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_visible_in_transcript_only: Option<bool>,

    /// Tool use result if this message is a tool result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_result: Option<super::ToolUseResult>,

    /// Parent tool use ID if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<ToolUseId>,

    /// Whether this is a replay message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,

    /// Whether this is a synthetic message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_synthetic: Option<bool>,
}

/// Content for a user message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserMessageContent {
    /// The role (always "user").
    pub role: String,

    /// The content blocks.
    pub content: Vec<super::ContentBlock>,
}

/// A message from the assistant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The message content.
    pub content: AssistantMessageContent,

    /// Parent tool use ID if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<ToolUseId>,
}

/// Content for an assistant message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssistantMessageContent {
    /// The role (always "assistant").
    pub role: String,

    /// The content blocks.
    pub content: Vec<super::ContentBlock>,

    /// The model that generated this message.
    pub model: String,

    /// The stop reason if finished.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RuntimeUsage>,
}

/// A system message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The system message subtype.
    pub subtype: SystemMessageSubtype,
}

/// System message subtypes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum SystemMessageSubtype {
    /// A compact boundary message.
    CompactBoundary {
        /// Metadata about the compaction.
        compact_metadata: CompactMetadata,
    },

    /// An API error.
    ApiError {
        /// The error details.
        error: ApiErrorDetails,

        /// Current retry attempt.
        retry_attempt: u32,

        /// Maximum retry attempts.
        max_retries: u32,

        /// Time to wait before retry.
        retry_in_ms: u64,
    },

    /// API metrics.
    ApiMetrics {
        /// Time to first token in milliseconds.
        ttft_ms: u64,
    },

    /// A local command output.
    LocalCommand {
        /// The command output.
        content: String,
    },

    /// A permission retry message.
    PermissionRetry {
        /// The tool use ID.
        tool_use_id: ToolUseId,
    },

    /// An informational message.
    Informational {
        /// The message level.
        level: MessageLevel,

        /// The message content.
        content: String,
    },

    /// A memory saved notification.
    MemorySaved {
        /// The path to the saved memory.
        path: String,
    },

    /// A turn duration notification.
    TurnDuration {
        /// The duration in milliseconds.
        duration_ms: u64,
    },
}

/// Message severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    /// Info level.
    Info,

    /// Warning level.
    Warn,

    /// Error level.
    Error,
}

/// API error details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiErrorDetails {
    /// The error status code.
    pub status: Option<u16>,

    /// The error message.
    pub message: String,

    /// The error type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_type: Option<String>,
}

/// Metadata for compaction boundaries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompactMetadata {
    /// The preserved segment information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserved_segment: Option<PreservedSegment>,

    /// The compaction strategy used.
    pub strategy: String,

    /// Tokens before compaction.
    pub tokens_before: u32,

    /// Tokens after compaction.
    pub tokens_after: u32,
}

/// A preserved segment during compaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreservedSegment {
    /// The head UUID of the preserved segment.
    pub head_uuid: MessageId,

    /// The tail UUID of the preserved segment.
    pub tail_uuid: MessageId,

    /// The number of messages preserved.
    pub message_count: u32,
}

/// An attachment message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachmentMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The attachment content.
    pub attachment: Attachment,
}

/// Attachment types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Attachment {
    /// Structured output data.
    StructuredOutput {
        /// The output data.
        data: serde_json::Value,
    },

    /// Max turns reached notification.
    MaxTurnsReached {
        /// The current turn count.
        turn_count: u32,

        /// The maximum turns allowed.
        max_turns: u32,
    },

    /// A queued command.
    QueuedCommand {
        /// The prompt for the command.
        prompt: String,

        /// The source UUID.
        #[serde(skip_serializing_if = "Option::is_none")]
        source_uuid: Option<String>,
    },

    /// A hook attachment.
    Hook {
        /// The hook name.
        hook_name: String,

        /// The hook data.
        data: serde_json::Value,
    },

    /// Memory file content.
    Memory {
        /// The memory file path.
        path: String,

        /// The memory content.
        content: String,
    },
}

/// A tombstone message for removing messages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TombstoneMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The IDs of messages to remove.
    pub remove_message_ids: Vec<MessageId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_message_subtypes() {
        // Test creating system message content with various content blocks
        let _local_cmd = SystemMessageSubtype::LocalCommand {
            content: "output".to_string(),
        };

        let _info = SystemMessageSubtype::Informational {
            level: MessageLevel::Info,
            content: "info".to_string(),
        };

        let _memory = SystemMessageSubtype::MemorySaved {
            path: "/tmp/memory".to_string(),
        };

        let _turn_duration = SystemMessageSubtype::TurnDuration { duration_ms: 1000 };

        // Note: SystemMessage uses subtype field
        let msg = SystemMessage {
            id: MessageId::new(),
            session_id: SessionId::new(),
            timestamp: Utc::now(),
            subtype: SystemMessageSubtype::Informational {
                level: MessageLevel::Info,
                content: "test".to_string(),
            },
        };
        let _ = serde_json::to_string(&msg);
    }

    #[test]
    fn test_attachment_variants() {
        let structured = Attachment::StructuredOutput {
            data: serde_json::json!({"key": "value"}),
        };
        let max_turns = Attachment::MaxTurnsReached {
            turn_count: 10,
            max_turns: 50,
        };
        let queued = Attachment::QueuedCommand {
            prompt: "command".to_string(),
            source_uuid: None,
        };
        let hook = Attachment::Hook {
            hook_name: "test".to_string(),
            data: serde_json::json!({}),
        };

        let _ = serde_json::to_string(&structured).unwrap();
        let _ = serde_json::to_string(&max_turns).unwrap();
        let _ = serde_json::to_string(&queued).unwrap();
        let _ = serde_json::to_string(&hook).unwrap();
    }
}
