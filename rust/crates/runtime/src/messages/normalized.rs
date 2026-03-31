//! Normalized message types for consistent conversation representation.
//!
//! This module defines normalized message formats used for serialization
//! and conversation replay.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{MessageId, SessionId, ToolUseId, Usage as RuntimeUsage};

/// A normalized message for consistent conversation representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NormalizedMessage {
    /// A normalized user message.
    User(NormalizedUserMessage),

    /// A normalized assistant message.
    Assistant(NormalizedAssistantMessage),

    /// A normalized system message.
    System {
        /// The message ID.
        id: MessageId,
        /// The content (string representation).
        content: String,
        /// The timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A normalized progress message.
    Progress {
        /// The message ID.
        id: MessageId,
        /// The progress data.
        data: super::ProgressData,
        /// The timestamp.
        timestamp: DateTime<Utc>,
    },

    /// A normalized tool result message.
    ToolResult {
        /// The tool use ID this result is for.
        tool_use_id: ToolUseId,
        /// The result content.
        content: Vec<super::ContentBlock>,
        /// Whether this is an error result.
        is_error: bool,
    },

    /// A stream event (kept for SDK compatibility).
    StreamEvent(StreamEvent),

    /// A query result (kept for SDK compatibility).
    Result(QueryResult),
}

impl NormalizedMessage {
    /// Get the message ID if applicable.
    #[must_use]
    pub fn id(&self) -> Option<MessageId> {
        match self {
            Self::User(u) => Some(u.uuid),
            Self::Assistant(a) => Some(a.uuid),
            Self::System { id, .. } => Some(*id),
            Self::Progress { id, .. } => Some(*id),
            Self::ToolResult { .. } | Self::StreamEvent(_) | Self::Result(_) => None,
        }
    }

    /// Get the timestamp if applicable.
    #[must_use]
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            Self::User(u) => Some(u.timestamp),
            Self::Assistant(a) => Some(a.timestamp),
            Self::System { timestamp, .. } => Some(*timestamp),
            Self::Progress { timestamp, .. } => Some(*timestamp),
            Self::ToolResult { .. } | Self::StreamEvent(_) | Self::Result(_) => None,
        }
    }

    /// Get the content blocks if applicable.
    #[must_use]
    pub fn content(&self) -> Option<&[super::ContentBlock]> {
        match self {
            Self::User(u) => Some(&u.message.content),
            Self::Assistant(a) => Some(&a.content),
            Self::ToolResult { content, .. } => Some(content),
            _ => None,
        }
    }

    /// Serialize to JSON for conversation replay.
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
}

/// Full user message structure for serialization compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedUserMessage {
    /// The message type.
    #[serde(rename = "type")]
    pub r#type: String,
    /// The message content.
    pub message: super::UserMessageContent,
    /// The session ID.
    pub session_id: SessionId,
    /// Parent tool use ID if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<ToolUseId>,
    /// The message UUID.
    pub uuid: MessageId,
    /// The timestamp.
    pub timestamp: DateTime<Utc>,
    /// Whether this is a replay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_replay: Option<bool>,
    /// Whether this is synthetic.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_synthetic: Option<bool>,
}

/// Full assistant message structure for serialization compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NormalizedAssistantMessage {
    /// The message type.
    #[serde(rename = "type")]
    pub r#type: String,
    /// The message content.
    pub content: Vec<super::ContentBlock>,
    /// The session ID.
    pub session_id: SessionId,
    /// Parent tool use ID if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_tool_use_id: Option<ToolUseId>,
    /// The message UUID.
    pub uuid: MessageId,
    /// The timestamp.
    pub timestamp: DateTime<Utc>,
    /// Usage statistics.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RuntimeUsage>,
    /// Stop reason if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

/// A stream event message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamEvent {
    /// The type.
    pub r#type: String,

    /// The event data.
    pub event: StreamEventData,

    /// The session ID.
    pub session_id: SessionId,

    /// The parent tool use ID.
    pub parent_tool_use_id: Option<ToolUseId>,

    /// The UUID.
    pub uuid: MessageId,
}

/// Stream event data types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEventData {
    /// Message start event.
    MessageStart {
        /// The message ID.
        message_id: String,

        /// Usage at start.
        usage: RuntimeUsage,
    },

    /// Content block start event.
    ContentBlockStart {
        /// The index.
        index: u32,

        /// The content block.
        content_block: super::ContentBlock,
    },

    /// Content block delta event.
    ContentBlockDelta {
        /// The index.
        index: u32,

        /// The delta.
        delta: ContentDelta,
    },

    /// Content block stop event.
    ContentBlockStop {
        /// The index.
        index: u32,
    },

    /// Message delta event.
    MessageDelta {
        /// The delta.
        delta: MessageDelta,

        /// Usage delta.
        usage: RuntimeUsage,
    },

    /// Message stop event.
    MessageStop,
}

/// Content block delta types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// Text delta.
    TextDelta {
        /// The text delta.
        text: String,
    },

    /// Thinking delta.
    ThinkingDelta {
        /// The thinking delta.
        thinking: String,
    },

    /// Input JSON delta.
    InputJsonDelta {
        /// The partial JSON.
        partial_json: String,
    },
}

/// Message delta data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageDelta {
    /// The stop reason.
    pub stop_reason: Option<String>,

    /// The stop sequence.
    pub stop_sequence: Option<String>,
}

/// Query result types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "subtype", rename_all = "snake_case")]
pub enum QueryResult {
    /// A successful result.
    Success {
        /// Duration in milliseconds.
        duration_ms: u64,

        /// API duration in milliseconds.
        duration_api_ms: u64,

        /// Whether this is an error.
        is_error: bool,

        /// Number of turns.
        num_turns: u32,

        /// The result text.
        result: String,

        /// The stop reason.
        stop_reason: Option<String>,

        /// The session ID.
        session_id: SessionId,

        /// Total cost in USD.
        total_cost_usd: f64,

        /// Usage statistics.
        usage: RuntimeUsage,

        /// Model usage statistics.
        #[serde(skip_serializing_if = "Option::is_none")]
        model_usage: Option<HashMap<String, RuntimeUsage>>,

        /// Permission denials.
        permission_denials: Vec<PermissionDenial>,

        /// Fast mode state.
        #[serde(skip_serializing_if = "Option::is_none")]
        fast_mode_state: Option<serde_json::Value>,

        /// The UUID.
        uuid: MessageId,
    },

    /// An error result.
    Error {
        /// Duration in milliseconds.
        duration_ms: u64,

        /// API duration in milliseconds.
        duration_api_ms: u64,

        /// Whether this is an error.
        is_error: bool,

        /// Number of turns.
        num_turns: u32,

        /// The stop reason.
        stop_reason: Option<String>,

        /// The session ID.
        session_id: SessionId,

        /// Total cost in USD.
        total_cost_usd: f64,

        /// Usage statistics.
        usage: RuntimeUsage,

        /// Model usage statistics.
        #[serde(skip_serializing_if = "Option::is_none")]
        model_usage: Option<HashMap<String, RuntimeUsage>>,

        /// Permission denials.
        permission_denials: Vec<PermissionDenial>,

        /// Fast mode state.
        #[serde(skip_serializing_if = "Option::is_none")]
        fast_mode_state: Option<serde_json::Value>,

        /// The UUID.
        uuid: MessageId,

        /// Error messages.
        errors: Vec<String>,
    },
}

/// A permission denial record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDenial {
    /// The tool name.
    pub tool_name: String,

    /// The tool use ID.
    pub tool_use_id: ToolUseId,

    /// The tool input.
    pub tool_input: serde_json::Value,

    /// The reason for the denial.
    pub reason: String,

    /// The timestamp when the denial occurred.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_delta_variants() {
        let text = ContentDelta::TextDelta {
            text: "hello".to_string(),
        };
        let thinking = ContentDelta::ThinkingDelta {
            thinking: "thinking".to_string(),
        };
        let json = ContentDelta::InputJsonDelta {
            partial_json: "{\"key\": \"val\"}".to_string(),
        };

        let _ = serde_json::to_string(&text).unwrap();
        let _ = serde_json::to_string(&thinking).unwrap();
        let _ = serde_json::to_string(&json).unwrap();
    }

    #[test]
    fn test_stream_event_variants() {
        let start = StreamEvent {
            r#type: "message_start".to_string(),
            event: StreamEventData::MessageStart {
                message_id: MessageId::new().to_string(),
                usage: RuntimeUsage::default(),
            },
            session_id: SessionId::new(),
            parent_tool_use_id: None,
            uuid: MessageId::new(),
        };

        let stop = StreamEvent {
            r#type: "message_stop".to_string(),
            event: StreamEventData::MessageStop,
            session_id: SessionId::new(),
            parent_tool_use_id: None,
            uuid: MessageId::new(),
        };

        let _ = serde_json::to_string(&start).unwrap();
        let _ = serde_json::to_string(&stop).unwrap();
    }
}
