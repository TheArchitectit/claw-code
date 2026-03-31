//! TaskCreateTool - Create new tasks.
//!
//! This tool creates new tasks in the task management system using
//! a shared in-memory store that persists across tool invocations.

use crate::task_store::get_task_store;
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the TaskCreateTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskCreateInput {
    /// Brief subject/title for the task.
    pub subject: String,
    /// Detailed description of what needs to be done.
    #[serde(default)]
    pub description: Option<String>,
    /// Initial status (defaults to "pending").
    #[serde(default)]
    pub status: Option<String>,
    /// Owner/agent to assign the task to.
    #[serde(default)]
    pub owner: Option<String>,
    /// Task metadata.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Output schema for task creation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreateOutput {
    /// The ID of the created task.
    pub task_id: String,
    /// The subject of the task.
    pub subject: String,
    /// The status of the task.
    pub status: String,
    /// The created timestamp.
    pub created_at: u64,
}

/// The TaskCreateTool creates new tasks.
#[derive(Debug, Clone, Default)]
pub struct TaskCreateTool;

impl TaskCreateTool {
    /// Create a new TaskCreateTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for TaskCreateTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TaskCreateTool",
                "Create new tasks in the task management system",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let subject = input
            .require("subject")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "subject must be a string".to_string(),
                error_code: Some(1),
            })?;

        if subject.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "subject cannot be empty".to_string(),
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

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let subject = match input.get("subject") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "subject must be a string")
                        .with_field("type", "validation_error");
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: subject")
                    .with_field("type", "validation_error");
            }
        };

        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let status = input
            .get("status")
            .and_then(|v| v.as_str());

        let owner = input
            .get("owner")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let metadata = input.get("metadata").cloned();

        // Create the task
        let store = get_task_store();
        let mut task = store.create(&subject, description);

        // Apply optional initial status
        if let Some(s) = status {
            if let Ok(new_status) = s.parse() {
                task.set_status(new_status);
            }
        }

        // Apply optional owner
        if let Some(o) = owner {
            task.set_owner(o);
        }

        // Apply optional metadata
        if let Some(m) = metadata {
            task.metadata = Some(m);
        }

        // Re-insert the updated task
        store.update(&task.id, |t| {
            t.status = task.status.clone();
            t.owner = task.owner.clone();
            t.metadata = task.metadata.clone();
        });

        ToolOutput::new()
            .with_field("task_id", &task.id)
            .with_field("subject", &task.subject)
            .with_field("status", task.status.as_str())
            .with_field("created_at", task.created_at)
            .with_field("type", "created")
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
    async fn test_task_create_validation() {
        let tool = TaskCreateTool::new();

        // Valid input
        let input = ToolInput::new().with_arg("subject", "Test task");
        assert!(tool.validate(&input).await.is_ok());

        // Missing subject
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Empty subject
        let input = ToolInput::new().with_arg("subject", "");
        assert!(tool.validate(&input).await.is_err());

        // Invalid status
        let input = ToolInput::new()
            .with_arg("subject", "Test")
            .with_arg("status", "invalid");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_task_create_execution() {
        let _guard = setup();
        let tool = TaskCreateTool::new();

        let input = ToolInput::new()
            .with_arg("subject", "Implement feature X")
            .with_arg("description", "Detailed description here");

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("type").and_then(|v| v.as_str()), Some("created"));
        assert!(output.data.contains_key("task_id"));
        assert_eq!(
            output.data.get("subject").and_then(|v| v.as_str()),
            Some("Implement feature X")
        );
        assert_eq!(
            output.data.get("status").and_then(|v| v.as_str()),
            Some("pending")
        );
    }

    #[tokio::test]
    async fn test_task_create_with_status() {
        let _guard = setup();
        let tool = TaskCreateTool::new();

        let input = ToolInput::new()
            .with_arg("subject", "Test task")
            .with_arg("status", "in_progress")
            .with_arg("owner", "agent1");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("status").and_then(|v| v.as_str()),
            Some("in_progress")
        );
    }
}
