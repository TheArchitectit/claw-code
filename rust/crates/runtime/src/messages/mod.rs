//! Message types for conversations.
//!
//! This module defines all message types used in the conversation flow
//! between the user, assistant, and tools.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{MessageId, SessionId, ToolUseId};

pub mod content;
pub mod normalized;
pub mod tools;
pub mod types;

// Re-export all types for backward compatibility
pub use content::{ContentBlock, ImageSource, ToolResultContent};
pub use normalized::{
    ContentDelta, MessageDelta, NormalizedAssistantMessage, NormalizedMessage,
    NormalizedUserMessage, PermissionDenial, QueryResult, StreamEvent, StreamEventData,
};

// Type alias for backward compatibility
pub type QueryResultMessage = QueryResult;

pub use tools::{
    ProgressData, ProgressMessage, ToolUseResult, ToolUseSummaryMessage,
};
pub use types::{
    ApiErrorDetails, AssistantMessage, AssistantMessageContent, Attachment,
    AttachmentMessage, CompactMetadata, MessageLevel, PreservedSegment, SystemMessage,
    SystemMessageSubtype, TombstoneMessage, UserMessage, UserMessageContent,
};

/// A message in the conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// A message from the user.
    User(UserMessage),

    /// A message from the assistant.
    Assistant(AssistantMessage),

    /// A progress update from a tool.
    Progress(ProgressMessage),

    /// An attachment to the conversation.
    Attachment(AttachmentMessage),

    /// A system message.
    System(SystemMessage),

    /// A tombstone for removing messages.
    Tombstone(TombstoneMessage),

    /// A tool use summary.
    ToolUseSummary(ToolUseSummaryMessage),
}

impl Message {
    /// Get the message ID.
    #[must_use]
    pub fn id(&self) -> MessageId {
        match self {
            Self::User(m) => m.id,
            Self::Assistant(m) => m.id,
            Self::Progress(m) => m.id,
            Self::Attachment(m) => m.id,
            Self::System(m) => m.id,
            Self::Tombstone(m) => m.id,
            Self::ToolUseSummary(m) => m.id,
        }
    }

    /// Get the timestamp of the message.
    #[must_use]
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::User(m) => m.timestamp,
            Self::Assistant(m) => m.timestamp,
            Self::Progress(m) => m.timestamp,
            Self::Attachment(m) => m.timestamp,
            Self::System(m) => m.timestamp,
            Self::Tombstone(m) => m.timestamp,
            Self::ToolUseSummary(m) => m.timestamp,
        }
    }

    /// Get the session ID of the message.
    #[must_use]
    pub fn session_id(&self) -> SessionId {
        match self {
            Self::User(m) => m.session_id,
            Self::Assistant(m) => m.session_id,
            Self::Progress(m) => m.session_id,
            Self::Attachment(m) => m.session_id,
            Self::System(m) => m.session_id,
            Self::Tombstone(m) => m.session_id,
            Self::ToolUseSummary(m) => m.session_id,
        }
    }

    /// Get the parent tool use ID if applicable.
    #[must_use]
    pub fn parent_tool_use_id(&self) -> Option<ToolUseId> {
        match self {
            Self::User(m) => m.parent_tool_use_id.clone(),
            Self::Assistant(m) => m.parent_tool_use_id.clone(),
            Self::Progress(m) => Some(m.tool_use_id.clone()),
            Self::Attachment(_) => None,
            Self::System(_) => None,
            Self::Tombstone(_) => None,
            Self::ToolUseSummary(_) => None,
        }
    }

    /// Check if this is a user message.
    #[must_use]
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }

    /// Check if this is an assistant message.
    #[must_use]
    pub fn is_assistant(&self) -> bool {
        matches!(self, Self::Assistant(_))
    }

    /// Check if this is a system message.
    #[must_use]
    pub fn is_system(&self) -> bool {
        matches!(self, Self::System(_))
    }

    /// Check if this is a progress message.
    #[must_use]
    pub fn is_progress(&self) -> bool {
        matches!(self, Self::Progress(_))
    }

    /// Check if this is a tool result message.
    #[must_use]
    pub fn is_tool_result(&self) -> bool {
        match self {
            Self::User(m) => m.tool_use_result.is_some(),
            _ => false,
        }
    }

    /// Serialize to JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize from JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if deserialization fails.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Normalize this message to a standard representation.
    #[must_use]
    pub fn normalize(&self) -> Option<NormalizedMessage> {
        match self {
            Self::User(msg) => Some(msg.normalize()),
            Self::Assistant(msg) => Some(msg.normalize()),
            Self::System(msg) => Some(msg.normalize()),
            Self::Progress(msg) => Some(msg.normalize()),
            Self::Attachment(msg) => msg.normalize(),
            Self::Tombstone(_) => None,
            Self::ToolUseSummary(msg) => msg.normalize(),
        }
    }
}

// Normalize implementations for each message type
impl UserMessage {
    /// Normalize this user message.
    #[must_use]
    pub fn normalize(&self) -> NormalizedMessage {
        // Check if this message has a tool use result - if so, return ToolResult variant
        if let Some(ref tool_result) = self.tool_use_result {
            let tool_use_id = tool_result.tool_use_id.clone();
            let content = vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: tool_result.content.clone(),
                is_error: Some(tool_result.is_error),
            }];
            return NormalizedMessage::ToolResult {
                tool_use_id,
                content,
                is_error: tool_result.is_error,
            };
        }

        NormalizedMessage::User(NormalizedUserMessage {
            r#type: "user".to_string(),
            message: self.content.clone(),
            session_id: self.session_id,
            parent_tool_use_id: self.parent_tool_use_id.clone(),
            uuid: self.id,
            timestamp: self.timestamp,
            is_replay: self.is_replay,
            is_synthetic: self.is_synthetic,
        })
    }
}

impl AssistantMessage {
    /// Normalize this assistant message.
    #[must_use]
    pub fn normalize(&self) -> NormalizedMessage {
        NormalizedMessage::Assistant(NormalizedAssistantMessage {
            r#type: "assistant".to_string(),
            content: self.content.content.clone(),
            session_id: self.session_id,
            parent_tool_use_id: self.parent_tool_use_id.clone(),
            uuid: self.id,
            timestamp: self.timestamp,
            usage: self.content.usage.clone(),
            stop_reason: self.content.stop_reason.clone(),
        })
    }
}

impl ProgressMessage {
    /// Normalize this progress message.
    #[must_use]
    pub fn normalize(&self) -> NormalizedMessage {
        NormalizedMessage::Progress {
            id: self.id,
            data: self.data.clone(),
            timestamp: self.timestamp,
        }
    }
}

impl AttachmentMessage {
    /// Normalize this attachment message.
    /// Attachments convert to system messages as they're informational.
    #[must_use]
    pub fn normalize(&self) -> Option<NormalizedMessage> {
        let content = match &self.attachment {
            Attachment::StructuredOutput { data } => {
                format!("Structured output: {}", data)
            }
            Attachment::MaxTurnsReached {
                turn_count,
                max_turns,
            } => {
                format!("Max turns reached: {}/{}", turn_count, max_turns)
            }
            Attachment::QueuedCommand {
                prompt,
                source_uuid,
            } => {
                if let Some(uuid) = source_uuid {
                    format!("Queued command [{}]: {}", uuid, prompt)
                } else {
                    format!("Queued command: {}", prompt)
                }
            }
            Attachment::Hook { hook_name, data } => {
                format!("Hook '{}': {}", hook_name, data)
            }
            Attachment::Memory { path, content } => {
                format!("Memory file '{}': {} bytes", path, content.len())
            }
        };

        Some(NormalizedMessage::System {
            id: self.id,
            content,
            timestamp: self.timestamp,
        })
    }
}

impl SystemMessage {
    /// Normalize this system message.
    #[must_use]
    pub fn normalize(&self) -> NormalizedMessage {
        let content = match &self.subtype {
            SystemMessageSubtype::CompactBoundary { compact_metadata } => {
                format!(
                    "Compact boundary: strategy={}, tokens_before={}, tokens_after={}",
                    compact_metadata.strategy,
                    compact_metadata.tokens_before,
                    compact_metadata.tokens_after
                )
            }
            SystemMessageSubtype::ApiError {
                error,
                retry_attempt,
                max_retries,
                ..
            } => {
                format!(
                    "API error: {} (attempt {}/{})",
                    error.message, retry_attempt, max_retries
                )
            }
            SystemMessageSubtype::ApiMetrics { ttft_ms } => {
                format!("API metrics: TTFT={ttft_ms}ms")
            }
            SystemMessageSubtype::LocalCommand { content } => {
                format!("Local command output: {content}")
            }
            SystemMessageSubtype::PermissionRetry { tool_use_id } => {
                format!("Permission retry requested for tool {tool_use_id}")
            }
            SystemMessageSubtype::Informational { level, content } => {
                format!("[{level:?}] {content}")
            }
            SystemMessageSubtype::MemorySaved { path } => {
                format!("Memory saved: {path}")
            }
            SystemMessageSubtype::TurnDuration { duration_ms } => {
                format!("Turn duration: {duration_ms}ms")
            }
        };

        NormalizedMessage::System {
            id: self.id,
            content,
            timestamp: self.timestamp,
        }
    }
}

impl ToolUseSummaryMessage {
    /// Normalize this tool use summary message.
    /// Summaries convert to system messages.
    #[must_use]
    pub fn normalize(&self) -> Option<NormalizedMessage> {
        let tool_count = self.preceding_tool_use_ids.len();
        let content = format!(
            "Tool use summary ({} tool{}): {}",
            tool_count,
            if tool_count == 1 { "" } else { "s" },
            self.summary
        );

        Some(NormalizedMessage::System {
            id: self.id,
            content,
            timestamp: self.timestamp,
        })
    }
}


#[cfg(test)]
mod tests;

