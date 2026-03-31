//! TaskListTool - List all tasks.
//!
//! This tool lists all tasks in the task management system with optional
//! filtering by status or owner.

use crate::task_store::{get_task_store, TaskSummary};
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the TaskListTool.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct TaskListInput {
    /// Filter by status (pending, in_progress, completed, cancelled).
    #[serde(default)]
    pub status: Option<String>,
    /// Filter by owner/agent.
    #[serde(default)]
    pub owner: Option<String>,
}

/// Output schema for task listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListOutput {
    /// The list of tasks.
    pub tasks: Vec<TaskSummary>,
    /// Total number of tasks.
    pub total: usize,
}

/// The TaskListTool lists all tasks.
#[derive(Debug, Clone, Default)]
pub struct TaskListTool;

impl TaskListTool {
    /// Create a new TaskListTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskListTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TaskListTool",
                "List all tasks in the task management system",
            )
            .read_only()
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        // Validate status filter if provided
        if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
            if !status.is_empty() {
                let valid_statuses = ["pending", "in_progress", "completed", "cancelled"];
                if !valid_statuses.contains(&status) {
                    return Err(ToolError::ValidationFailed {
                        message: format!("Invalid status filter: {status}"),
                        error_code: Some(1),
                    });
                }
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let status = input
            .get("status")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let owner = input
            .get("owner")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        // Query the store
        let store = get_task_store();
        let tasks = store.list(status, owner);
        let total = tasks.len();

        ToolOutput::new()
            .with_field("tasks", tasks)
            .with_field("total", total)
            .with_field("type", "list")
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
    async fn test_task_list_validation() {
        let tool = TaskListTool::new();

        // Valid - no filters
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_ok());

        // Valid - with status filter
        let input = ToolInput::new().with_arg("status", "pending");
        assert!(tool.validate(&input).await.is_ok());

        // Invalid status filter
        let input = ToolInput::new().with_arg("status", "invalid");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_task_list_execution() {
        let _guard = setup();
        let list_tool = TaskListTool::new();
        let create_tool = crate::task_create::TaskCreateTool::new();

        // Create a task with a unique subject
        let unique_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let input = ToolInput::new().with_arg("subject", format!("ListTest-{}", unique_id));
        let out = create_tool.execute(input).await;
        let task_id = out.data.get("task_id").and_then(|v| v.as_str()).unwrap().to_string();

        // List all tasks - should include our new one
        let input = ToolInput::new();
        let output = list_tool.execute(input).await;
        let total = output.data.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(total >= 1, "Expected at least 1 task");

        // Cleanup
        crate::task_store::get_task_store().delete(&task_id);
    }

    #[test]
    fn test_task_list_metadata() {
        let tool = TaskListTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "TaskListTool");
        assert!(meta.is_read_only);
    }
}
