//! Stream types for LLM responses.
//!
//! This module provides types for handling streaming LLM responses,
//! including individual chunks and the stream itself.

use crate::messages::ContentBlock;
use crate::types::{ToolUseId, Usage};

/// A stream of LLM response chunks.
pub struct LlmStream {
    /// The receiver for stream chunks.
    receiver: tokio::sync::mpsc::Receiver<LlmStreamChunk>,
}

impl LlmStream {
    /// Create a new LLM stream from a receiver.
    #[must_use]
    pub fn new(receiver: tokio::sync::mpsc::Receiver<LlmStreamChunk>) -> Self {
        Self { receiver }
    }

    /// Receive the next chunk from the stream.
    pub async fn next(&mut self) -> Option<LlmStreamChunk> {
        self.receiver.recv().await
    }

    /// Collect all chunks into a complete response.
    pub async fn collect(mut self) -> Vec<LlmStreamChunk> {
        let mut chunks = Vec::new();
        while let Some(chunk) = self.next().await {
            chunks.push(chunk);
        }
        chunks
    }
}

/// A chunk from an LLM stream.
#[derive(Debug, Clone)]
pub enum LlmStreamChunk {
    /// Text content chunk.
    Text {
        /// The text content.
        text: String,
    },

    /// Thinking content chunk.
    Thinking {
        /// The thinking content.
        thinking: String,
    },

    /// Tool use start (name and ID).
    ToolUseStart {
        /// The tool use ID.
        id: ToolUseId,
        /// The tool name.
        name: String,
    },

    /// Tool use input JSON delta.
    ToolUseDelta {
        /// The tool use ID.
        id: ToolUseId,
        /// The partial JSON input.
        partial_json: String,
    },

    /// Tool use complete.
    ToolUseComplete {
        /// The tool use ID.
        id: ToolUseId,
        /// The complete input.
        input: serde_json::Value,
    },

    /// Usage statistics.
    Usage {
        /// The token usage.
        usage: Usage,
    },

    /// Stop reason.
    Stop {
        /// The stop reason.
        reason: StopReason,
    },

    /// Error occurred.
    Error {
        /// The error message.
        message: String,
    },
}

/// The reason the LLM stopped generating.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// Natural end of turn.
    EndTurn,
    /// Maximum tokens reached.
    MaxTokens,
    /// Stop sequence encountered.
    StopSequence,
    /// Tool use (will continue after tool execution).
    ToolUse,
    /// Error occurred during generation.
    Error,
    /// Content was filtered.
    ContentFiltered,
}

impl std::fmt::Display for StopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StopReason::EndTurn => write!(f, "end_turn"),
            StopReason::MaxTokens => write!(f, "max_tokens"),
            StopReason::StopSequence => write!(f, "stop_sequence"),
            StopReason::ToolUse => write!(f, "tool_use"),
            StopReason::Error => write!(f, "error"),
            StopReason::ContentFiltered => write!(f, "content_filtered"),
        }
    }
}
