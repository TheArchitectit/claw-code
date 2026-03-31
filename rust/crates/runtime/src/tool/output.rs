//! Tool output types.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::context::ToolUseContext;
use crate::messages::Message;

/// The output of a tool execution.
pub struct ToolOutput {
    /// The output data.
    pub data: serde_json::Value,

    /// Optional new messages to add to the conversation.
    pub new_messages: Vec<Message>,

    /// Optional context modifier function.
    ///
    /// This is only honored for tools that aren't concurrency safe.
    /// Wrapped in Arc<Mutex<...>> to make ToolOutput Sync + Send.
    pub context_modifier: Option<Arc<Mutex<Option<Box<dyn FnOnce(&mut ToolUseContext) + Send>>>>>,

    /// MCP protocol metadata to pass through.
    pub mcp_meta: Option<McpMeta>,
}

impl Clone for ToolOutput {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            new_messages: self.new_messages.clone(),
            // Context modifier cannot be cloned (it's a FnOnce), so we lose it on clone.
            // Since it's wrapped in Arc, we just don't clone it.
            context_modifier: None,
            mcp_meta: self.mcp_meta.clone(),
        }
    }
}

impl std::fmt::Debug for ToolOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolOutput")
            .field("data", &self.data)
            .field("new_messages", &self.new_messages)
            .field("context_modifier", &self.context_modifier.is_some())
            .field("mcp_meta", &self.mcp_meta)
            .finish()
    }
}

impl ToolOutput {
    /// Create a new tool output with the given data.
    #[must_use]
    pub fn new(data: impl Serialize) -> Self {
        Self {
            data: serde_json::to_value(data).unwrap_or_default(),
            new_messages: Vec::new(),
            context_modifier: None,
            mcp_meta: None,
        }
    }

    /// Create a new error tool output with an error message.
    #[must_use]
    pub fn new_error(message: impl Into<String>) -> Self {
        Self {
            data: serde_json::json!({"error": message.into()}),
            new_messages: Vec::new(),
            context_modifier: None,
            mcp_meta: None,
        }
    }

    /// Add new messages to the output.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.new_messages = messages;
        self
    }

    /// Set the MCP metadata.
    pub fn with_mcp_meta(mut self, meta: McpMeta) -> Self {
        self.mcp_meta = Some(meta);
        self
    }

    /// Set a context modifier.
    pub fn with_context_modifier(
        mut self,
        modifier: impl FnOnce(&mut ToolUseContext) + Send + 'static,
    ) -> Self {
        self.context_modifier = Some(Arc::new(Mutex::new(Some(Box::new(modifier)))));
        self
    }
}

/// MCP metadata for tool results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpMeta {
    /// Additional metadata.
    #[serde(skip_serializing_if = "Option::is_none", rename = "_meta")]
    pub meta: Option<HashMap<String, serde_json::Value>>,

    /// Structured content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_content: Option<HashMap<String, serde_json::Value>>,
}
