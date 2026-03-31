//! TaskUpdateTool - Update task status and properties.
//!
//! This tool updates existing tasks in the task management system
//! using the shared in-memory store.

use crate::task_store::{get_task_store, TaskStatus};
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the TaskUpdateTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskUpdateInput {
    /// The ID of the task to update.
    pub task_id: String,
    /// The new status for the task.
    #[serde(default)]
    pub status: Option<String>,
    /// Updated subject/title.
    #[serde(default)]
    pub subject: Option<String>,
    /// Updated description.
    #[serde(default)]
    pub description: Option<String>,
    /// New owner to assign.
    #[serde(default)]
    pub owner: Option<String>,
    /// Updated metadata.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Output schema for task update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdateOutput {
    pub task_id: String,
    pub updated_fields: Vec<String>,
    pub new_status: Option<String>,
}

/// The TaskUpdateTool updates existing tasks.
#[derive(Debug, Clone, Default)]
pub struct TaskUpdateTool;

impl TaskUpdateTool {
    /// Create a new TaskUpdateTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskUpdateTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TaskUpdateTool",
                "Update task status and properties",
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

        // Validate status if provided
        if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
            let valid_statuses = ["pending", "in_progress", "completed", "cancelled"];
            if !valid_statuses.contains(&status) {
                return Err(ToolError::ValidationFailed {
                    message: format!("Invalid status: {status}. Valid statuses are: pending, in_progress, completed, cancelled"),
                    error_code: Some(3),
                });
            }
        }

        // Check that at least one update field is provided
        let has_update = input.get("status").is_some()
            || input.get("subject").is_some()
            || input.get("description").is_some()
            || input.get("owner").is_some()
            || input.get("metadata").is_some();

        if !has_update {
            return Err(ToolError::ValidationFailed {
                message: "At least one field to update must be provided (status, subject, description, owner, or metadata)".to_string(),
                error_code: Some(4),
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

        // Check if task exists
        let store = get_task_store();
        if store.get(&task_id).is_none() {
            return ToolOutput::new()
                .with_field("error", format!("Task not found: {task_id}"))
                .with_field("type", "not_found");
        }

        let mut updated_fields = Vec::new();
        let mut new_status = None;

        // Update status
        if let Some(status_str) = input.get("status").and_then(|v| v.as_str()) {
            if let Ok(status) = status_str.parse::<TaskStatus>() {
                store.update(&task_id, |task| {
                    task.set_status(status);
                });
                updated_fields.push("status".to_string());
                new_status = Some(status_str.to_string());
            }
        }

        // Update subject
        if let Some(subject) = input.get("subject").and_then(|v| v.as_str()) {
            store.update(&task_id, |task| {
                task.subject = subject.to_string();
                task.touch();
            });
            updated_fields.push("subject".to_string());
        }

        // Update description
        if let Some(desc) = input.get("description") {
            let desc_val = desc.as_str().map(|s| s.to_string());
            store.update(&task_id, |task| {
                task.description = desc_val.clone();
                task.touch();
            });
            updated_fields.push("description".to_string());
        }

        // Update owner
        if let Some(owner) = input.get("owner").and_then(|v| v.as_str()) {
            store.update(&task_id, |task| {
                task.set_owner(owner.to_string());
            });
            updated_fields.push("owner".to_string());
        }

        // Update metadata
        if let Some(metadata) = input.get("metadata") {
            let meta = metadata.clone();
            store.update(&task_id, |task| {
                task.metadata = Some(meta.clone());
                task.touch();
            });
            updated_fields.push("metadata".to_string());
        }

        ToolOutput::new()
            .with_field("task_id", &task_id)
            .with_field("type", "updated")
            .with_field("updated_fields", updated_fields)
            .with_field("new_status", new_status.unwrap_or_default())
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
    async fn test_task_update_validation() {
        let tool = TaskUpdateTool::new();

        // Valid input
        let input = ToolInput::new()
            .with_arg("task_id", "task-123")
            .with_arg("status", "completed");
        assert!(tool.validate(&input).await.is_ok());

        // Missing task_id
        let input = ToolInput::new().with_arg("status", "completed");
        assert!(tool.validate(&input).await.is_err());

        // Invalid status
        let input = ToolInput::new()
            .with_arg("task_id", "task-123")
            .with_arg("status", "invalid_status");
        assert!(tool.validate(&input).await.is_err());

        // No update fields
        let input = ToolInput::new().with_arg("task_id", "task-123");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_task_update_not_found() {
        let _guard = setup();
        let tool = TaskUpdateTool::new();

        let input = ToolInput::new()
            .with_arg("task_id", "nonexistent-task-99999")
            .with_arg("status", "completed");
        let output = tool.execute(input).await;

        assert_eq!(output.data.get("type").and_then(|v| v.as_str()), Some("not_found"));
        assert!(output.data.get("error").is_some());
    }
}
