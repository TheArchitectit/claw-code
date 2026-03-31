//! `/tasks` - Task list management command.
//!
//! This command provides task list management functionality including:
//! - List all tasks with optional status/owner filters
//! - Show task details
//! - Create, update, and delete tasks

use async_trait::async_trait;

use crate::{Command, CommandContext, CommandError, CommandMetadata, CommandOutput, CommandResult};
use tools::task_store::{get_task_store, TaskStatus};

/// `/tasks` - Task list management command
pub struct TasksCommand {
    metadata: CommandMetadata,
}

impl TasksCommand {
    /// Create a new tasks command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("tasks", "Manage tasks and todo lists")
                .with_aliases(vec!["task".to_string(), "t".to_string()])
                .with_usage("/tasks [list|show|create|update|delete] [options]"),
        }
    }

    /// List tasks with optional filters
    fn list_tasks(&self, status: Option<&str>, owner: Option<&str>) -> CommandResult {
        let store = get_task_store();
        let tasks = store.list(status, owner);

        if tasks.is_empty() {
            return Ok(CommandOutput::Text("No tasks found.".to_string()));
        }

        let mut output = format!("Found {} task(s):\n\n", tasks.len());
        for task in tasks {
            let status_icon = match task.status.as_str() {
                "completed" => "✓",
                "in_progress" => "▶",
                "cancelled" => "✗",
                _ => "○",
            };
            let owner_str = task
                .owner
                .map(|o| format!(" [@{o}]"))
                .unwrap_or_default();
            output.push_str(&format!(
                "{} {}: {}{}\n",
                status_icon, task.task_id, task.subject, owner_str
            ));
        }

        Ok(CommandOutput::Text(output))
    }

    /// Show task details
    fn show_task(&self, task_id: &str) -> CommandResult {
        let store = get_task_store();
        let task = store
            .get(task_id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("Task not found: {task_id}")))?;

        let details = serde_json::json!({
            "id": task.id,
            "subject": task.subject,
            "description": task.description,
            "status": task.status.as_str(),
            "owner": task.owner,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
            "metadata": task.metadata,
        });

        Ok(CommandOutput::Json(details))
    }

    /// Create a new task
    fn create_task(&self, subject: &str, description: Option<String>) -> CommandResult {
        let store = get_task_store();
        let task = store.create(subject, description);

        Ok(CommandOutput::Text(format!(
            "Created task {}: {}",
            task.id, task.subject
        )))
    }

    /// Update task status
    fn update_task(&self, task_id: &str, status: &str) -> CommandResult {
        let store = get_task_store();

        let new_status = match status {
            "pending" => TaskStatus::Pending,
            "in_progress" => TaskStatus::InProgress,
            "completed" => TaskStatus::Completed,
            "cancelled" => TaskStatus::Cancelled,
            _ => {
                return Err(CommandError::InvalidArguments(format!(
                    "Invalid status: {status}"
                )))
            }
        };

        let task = store
            .update(task_id, |t| t.set_status(new_status))
            .ok_or_else(|| CommandError::ExecutionFailed(format!("Task not found: {task_id}")))?;

        Ok(CommandOutput::Text(format!(
            "Updated task {}: status -> {}",
            task.id,
            task.status.as_str()
        )))
    }

    /// Delete a task
    fn delete_task(&self, task_id: &str) -> CommandResult {
        let store = get_task_store();
        let deleted = store.delete(task_id);

        if deleted {
            Ok(CommandOutput::Text(format!("Deleted task: {task_id}")))
        } else {
            Err(CommandError::ExecutionFailed(format!(
                "Task not found: {task_id}"
            )))
        }
    }
}

impl Default for TasksCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TasksCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // Parse subcommand from args
        let subcommand = ctx.args.first().map(|s| s.as_str()).unwrap_or("list");

        match subcommand {
            "list" | "ls" => {
                let status = ctx.flags.get("status").map(|s| s.as_str());
                let owner = ctx.flags.get("owner").map(|s| s.as_str());
                self.list_tasks(status, owner)
            }
            "show" | "get" | "detail" => {
                let task_id = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Task ID required".to_string()))?;
                self.show_task(task_id)
            }
            "create" | "new" | "add" => {
                let subject = ctx
                    .args
                    .get(1)
                    .cloned()
                    .unwrap_or_else(|| "New task".to_string());
                let description = ctx.args.get(2).cloned();
                self.create_task(&subject, description)
            }
            "update" | "set" => {
                let task_id = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Task ID required".to_string()))?;
                let status = ctx
                    .flags
                    .get("status")
                    .map(|s| s.as_str())
                    .unwrap_or("completed");
                self.update_task(task_id, status)
            }
            "delete" | "rm" | "remove" => {
                let task_id = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Task ID required".to_string()))?;
                self.delete_task(task_id)
            }
            _ => Err(CommandError::InvalidArguments(format!(
                "Unknown subcommand: {subcommand}. Use: list, show, create, update, delete"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandContext;

    #[test]
    fn test_tasks_command_metadata() {
        let cmd = TasksCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "tasks");
        assert!(meta.aliases.contains(&"task".to_string()));
        assert!(meta.aliases.contains(&"t".to_string()));
    }

    #[test]
    fn test_tasks_command_matches() {
        let cmd = TasksCommand::new();
        assert!(cmd.matches("tasks"));
        assert!(cmd.matches("task"));
        assert!(cmd.matches("t"));
        assert!(!cmd.matches("skill"));
    }

    #[tokio::test]
    async fn test_tasks_list_empty() {
        let cmd = TasksCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("No tasks"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_tasks_create() {
        let cmd = TasksCommand::new();
        let ctx = CommandContext::new("/tmp").with_args(vec![
            "create".to_string(),
            "Test task".to_string(),
            "Test description".to_string(),
        ]);

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Created task"));
                assert!(text.contains("Test task"));
            }
            _ => panic!("Expected text output"),
        }
    }
}
