//! Tool-related message types.
//!
//! This module defines types for tool use results, progress data,
//! and other tool-related message structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::{MessageId, SessionId, ToolUseId};

/// Result of a tool use operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseResult {
    /// The tool use ID this result is for.
    pub tool_use_id: ToolUseId,
    /// The result content blocks.
    pub content: Vec<super::ToolResultContent>,
    /// Whether this is an error result.
    pub is_error: bool,
}

impl ToolUseResult {
    /// Create a new successful tool result.
    #[must_use]
    pub fn new(tool_use_id: ToolUseId, content: Vec<super::ToolResultContent>) -> Self {
        Self {
            tool_use_id,
            content,
            is_error: false,
        }
    }

    /// Create a new error tool result.
    #[must_use]
    pub fn error(tool_use_id: ToolUseId, content: Vec<super::ToolResultContent>) -> Self {
        Self {
            tool_use_id,
            content,
            is_error: true,
        }
    }
}

/// A progress message from a tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProgressMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The tool use ID this progress is for.
    pub tool_use_id: ToolUseId,

    /// The progress data.
    pub data: ProgressData,
}

/// Progress data types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProgressData {
    /// Generic tool progress.
    ToolProgress {
        /// The progress message.
        message: String,
    },

    /// Bash command progress.
    Bash {
        /// The command being executed.
        command: String,

        /// Current output.
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
    },

    /// File read progress.
    FileRead {
        /// The file path.
        path: String,

        /// Bytes read so far.
        bytes_read: u64,

        /// Total bytes.
        total_bytes: u64,
    },

    /// MCP tool progress.
    Mcp {
        /// The server name.
        server_name: String,

        /// The operation.
        operation: String,
    },

    /// Agent tool progress.
    Agent {
        /// The agent name.
        agent_name: String,

        /// Current status.
        status: String,
    },

    /// Hook progress.
    Hook {
        /// The hook name.
        hook_name: String,

        /// The progress message.
        message: String,
    },
}

/// A tool use summary message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUseSummaryMessage {
    /// The message ID.
    pub id: MessageId,

    /// The session ID.
    pub session_id: SessionId,

    /// The timestamp.
    pub timestamp: DateTime<Utc>,

    /// The summary text.
    pub summary: String,

    /// The tool use IDs that preceded this summary.
    pub preceding_tool_use_ids: Vec<ToolUseId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_data_variants() {
        let tool = ProgressData::ToolProgress {
            message: "progress".to_string(),
        };
        let bash = ProgressData::Bash {
            command: "echo".to_string(),
            output: Some("hi".to_string()),
        };
        let file = ProgressData::FileRead {
            path: "/tmp".to_string(),
            bytes_read: 100,
            total_bytes: 200,
        };
        let mcp = ProgressData::Mcp {
            server_name: "test".to_string(),
            operation: "read".to_string(),
        };
        let agent = ProgressData::Agent {
            agent_name: "agent".to_string(),
            status: "working".to_string(),
        };
        let hook = ProgressData::Hook {
            hook_name: "hook".to_string(),
            message: "msg".to_string(),
        };

        let _ = serde_json::to_string(&tool).unwrap();
        let _ = serde_json::to_string(&bash).unwrap();
        let _ = serde_json::to_string(&file).unwrap();
        let _ = serde_json::to_string(&mcp).unwrap();
        let _ = serde_json::to_string(&agent).unwrap();
        let _ = serde_json::to_string(&hook).unwrap();
    }
}
