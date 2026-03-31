//! TaskGetTool - Get task details.
//!
//! This tool retrieves detailed information about a specific task
//! from the shared task store.

use crate::task_store::{get_task_store, TaskDetails};
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the TaskGetTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskGetInput {
    /// The ID of the task to retrieve.
    pub task_id: String,
}

/// The TaskGetTool retrieves task details.
#[derive(Debug, Clone, Default)]
pub struct TaskGetTool;

impl TaskGetTool {
    /// Create a new TaskGetTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskGetTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TaskGetTool",
                "Get detailed information about a specific task",
            )
            .read_only()
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

        // Retrieve from store
        let store = get_task_store();
        match store.get(&task_id) {
            Some(task) => {
                let details = TaskDetails::from(task);
                ToolOutput::new()
                    .with_field("task_id", &details.task_id)
                    .with_field("subject", &details.subject)
                    .with_field("status", &details.status)
                    .with_field("created_at", details.created_at)
                    .with_field("updated_at", details.updated_at)
                    .with_field("type", "found")
                    .with_field(
                        "description",
                        details.description.unwrap_or_default(),
                    )
                    .with_field(
                        "owner",
                        details.owner.unwrap_or_default(),
                    )
            }
            None => ToolOutput::new()
                .with_field("error", format!("Task not found: {task_id}"))
                .with_field("type", "not_found"),
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
    async fn test_task_get_validation() {
        let tool = TaskGetTool::new();

        // Valid input
        let input = ToolInput::new().with_arg("task_id", "task-123");
        assert!(tool.validate(&input).await.is_ok());

        // Missing task_id
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Empty task_id
        let input = ToolInput::new().with_arg("task_id", "");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_task_get_execution_found() {
        let _guard = setup();
        let get_tool = TaskGetTool::new();
        let create_tool = crate::task_create::TaskCreateTool::new();

        // Create a task first
        let create_input = ToolInput::new()
            .with_arg("subject", "Test task to get")
            .with_arg("description", "Test description");
        let create_output = create_tool.execute(create_input).await;
        let task_id = create_output
            .data
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Now get it
        let get_input = ToolInput::new().with_arg("task_id", &task_id);
        let get_output = get_tool.execute(get_input).await;

        assert_eq!(get_output.data.get("type").and_then(|v| v.as_str()), Some("found"));
        assert_eq!(
            get_output.data.get("subject").and_then(|v| v.as_str()),
            Some("Test task to get")
        );
    }

    #[tokio::test]
    async fn test_task_get_execution_not_found() {
        let tool = TaskGetTool::new();

        let input = ToolInput::new().with_arg("task_id", "nonexistent-task");
        let output = tool.execute(input).await;

        assert_eq!(output.data.get("type").and_then(|v| v.as_str()), Some("not_found"));
        assert!(output.data.get("error").is_some());
    }

    #[test]
    fn test_task_get_metadata() {
        let tool = TaskGetTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "TaskGetTool");
        assert!(meta.is_read_only);
    }
}
