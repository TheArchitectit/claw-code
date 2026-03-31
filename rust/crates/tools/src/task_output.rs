//! TaskOutputTool - Store and retrieve task output.
//!
//! This tool stores and retrieves output content associated with tasks
//! using the shared in-memory store.

use crate::task_store::get_task_store;
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the TaskOutputTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskOutputInput {
    /// The ID of the task.
    pub task_id: String,
    /// The output content to store (optional for retrieval).
    #[serde(default)]
    pub content: Option<String>,
    /// Whether to retrieve instead of store.
    #[serde(default)]
    pub retrieve: Option<bool>,
    /// Whether to append to existing output.
    #[serde(default)]
    pub append: Option<bool>,
}

/// Output schema for storing task output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStoreOutput {
    pub task_id: String,
    pub stored: bool,
    pub bytes_written: usize,
}

/// Output schema for retrieving task output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRetrieveOutput {
    pub task_id: String,
    pub content: String,
    pub has_output: bool,
}

/// The TaskOutputTool stores and retrieves task output.
#[derive(Debug, Clone, Default)]
pub struct TaskOutputTool;

impl TaskOutputTool {
    /// Create a new TaskOutputTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskOutputTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TaskOutputTool",
                "Store and retrieve task output",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let task_id = input
            .require("task_id")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "task_id must be a string".to_string(),
                error_code: Some(1),
            })?;

        if task_id.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "task_id cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let task_id = match input.get("task_id") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "task_id must be a string")
                        .with_field("type", "validation_error");
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: task_id")
                    .with_field("type", "validation_error");
            }
        };

        let retrieve = input
            .get("retrieve")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let store = get_task_store();

        // Check if task exists
        if store.get(&task_id).is_none() {
            return ToolOutput::new()
                .with_field("error", format!("Task not found: {task_id}"))
                .with_field("type", "not_found");
        }

        if retrieve {
            // Retrieve output
            let content = store.get_output(&task_id).unwrap_or_default();
            let has_output = !content.is_empty();

            ToolOutput::new()
                .with_field("task_id", &task_id)
                .with_field("content", content)
                .with_field("has_output", has_output)
                .with_field("type", "retrieved")
        } else {
            // Store output
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let append = input
                .get("append")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let bytes_written = content.len();

            if append {
                // Append to existing output
                let existing = store.get_output(&task_id).unwrap_or_default();
                let combined = format!("{}{}", existing, content);
                store.set_output(&task_id, combined);
            } else {
                // Replace existing output
                store.set_output(&task_id, &content);
            }

            ToolOutput::new()
                .with_field("task_id", &task_id)
                .with_field("stored", true)
                .with_field("bytes_written", bytes_written)
                .with_field("type", "stored")
                .with_field("appended", append)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_store::reset_task_store;
    use crate::task_store::TEST_MUTEX;

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_task_store();
        guard
    }

    #[tokio::test]
    async fn test_task_output_validation() {
        let tool = TaskOutputTool::new();

        // Valid input
        let input = ToolInput::new().with_arg("task_id", "task-123");
        assert!(tool.validate(&input).await.is_ok());

        // Missing task_id
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_task_output_not_found() {
        let _guard = setup();
        let tool = TaskOutputTool::new();

        let input = ToolInput::new()
            .with_arg("task_id", "nonexistent-task-99999")
            .with_arg("content", "Some content");
        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("not_found")
        );
    }
}
