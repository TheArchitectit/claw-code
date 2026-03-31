//! Core tool trait and types for the R.A.D Codicological tool system.
//!
//! This module provides the foundational types and traits that all tools must implement
//! to be usable within the system.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum ToolError {
    /// The tool input failed validation.
    #[error("Validation failed: {message}")]
    ValidationFailed {
        message: String,
        error_code: Option<u32>,
    },

    /// The tool execution failed.
    #[error("Execution failed: {message}")]
    ExecutionFailed { message: String },

    /// Permission denied for the operation.
    #[error("Permission denied: {message}")]
    PermissionDenied { message: String },

    /// The requested resource was not found.
    #[error("Not found: {message}")]
    NotFound { message: String },

    /// The operation timed out.
    #[error("Operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// An internal error occurred.
    #[error("Internal error: {message}")]
    Internal { message: String },

    /// The operation was cancelled.
    #[error("Operation was cancelled")]
    Cancelled,
}

/// The result type for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

/// Input to a tool invocation.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct ToolInput {
    /// The arguments passed to the tool.
    #[serde(flatten)]
    pub args: HashMap<String, serde_json::Value>,
}

impl ToolInput {
    /// Create a new empty tool input.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an argument to the input.
    #[must_use]
    pub fn with_arg(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let key = key.into();
        let value = serde_json::to_value(value).unwrap_or_default();
        self.args.insert(key, value);
        self
    }

    /// Get an argument by name.
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.args.get(key)
    }

    /// Get a required argument by name.
    pub fn require(&self, key: &str) -> ToolResult<&serde_json::Value> {
        self.args
            .get(key)
            .ok_or_else(|| ToolError::ValidationFailed {
                message: format!("Missing required argument: {key}"),
                error_code: Some(1),
            })
    }
}

/// Output from a tool invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ToolOutput {
    /// The result data from the tool.
    #[serde(flatten)]
    pub data: HashMap<String, serde_json::Value>,

    /// Whether the output was truncated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<bool>,
}

impl ToolOutput {
    /// Create a new empty tool output.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a data field to the output.
    #[must_use]
    pub fn with_field(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        let key = key.into();
        let value = serde_json::to_value(value).unwrap_or_default();
        self.data.insert(key, value);
        self
    }

    /// Set the truncated flag.
    #[must_use]
    pub fn with_truncated(mut self, truncated: bool) -> Self {
        self.truncated = Some(truncated);
        self
    }
}

/// Metadata about a tool.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolMetadata {
    /// The unique name of the tool.
    pub name: String,

    /// A brief description of what the tool does.
    pub description: String,

    /// Whether the tool is read-only (doesn't modify the filesystem).
    pub is_read_only: bool,

    /// Whether the tool can be run concurrently with other tool instances.
    pub is_concurrency_safe: bool,
}

impl ToolMetadata {
    /// Create new tool metadata.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            is_read_only: false,
            is_concurrency_safe: true,
        }
    }

    /// Set whether the tool is read-only.
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.is_read_only = true;
        self
    }

    /// Set whether the tool is concurrency-safe.
    #[must_use]
    pub fn concurrency_safe(mut self, safe: bool) -> Self {
        self.is_concurrency_safe = safe;
        self
    }
}

/// The core trait that all tools must implement.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the metadata for this tool.
    fn metadata(&self) -> &ToolMetadata;

    /// Validate the input before execution.
    ///
    /// This should check that all required arguments are present and valid,
    /// without actually performing any I/O operations.
    async fn validate(&self, input: &ToolInput) -> ToolResult<()>;

    /// Execute the tool with the given input.
    ///
    /// This is the main entry point for tool execution. It should perform
    /// the actual operation and return the result.
    async fn execute(&self, input: ToolInput) -> ToolOutput;

    /// Get the name of the tool.
    fn name(&self) -> &str {
        &self.metadata().name
    }
}

/// A boxed tool trait object for dynamic dispatch.
pub type BoxedTool = Box<dyn Tool>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_input_creation() {
        let input = ToolInput::new()
            .with_arg("file_path", "/test/path")
            .with_arg("limit", 100u32);

        assert!(input.get("file_path").is_some());
        assert!(input.get("limit").is_some());
        assert!(input.get("nonexistent").is_none());
    }

    #[test]
    fn test_tool_output_creation() {
        let output = ToolOutput::new()
            .with_field("content", "test content")
            .with_field("lines", 42u32)
            .with_truncated(true);

        assert!(output.truncated.unwrap());
        assert_eq!(output.data.len(), 2);
    }

    #[test]
    fn test_tool_metadata() {
        let meta = ToolMetadata::new("test_tool", "A test tool")
            .read_only()
            .concurrency_safe(false);

        assert_eq!(meta.name, "test_tool");
        assert_eq!(meta.description, "A test tool");
        assert!(meta.is_read_only);
        assert!(!meta.is_concurrency_safe);
    }
}
