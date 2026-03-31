//! TodoWriteTool - Manage session-level todo lists.
//!
//! This tool provides todo list management during a session:
//! - Write/update todos with id, content, status, priority
//! - Support batch operations (multiple todos at once)
//! - Validation for required fields

use crate::todo_store::{get_todo_store, reset_todo_store, Todo, TodoPriority, TodoStatus};
use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input for a single todo in a batch operation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoInput {
    /// Unique identifier (optional for new todos).
    #[serde(default)]
    pub id: Option<String>,
    /// The todo content/description.
    pub content: String,
    /// Current status (defaults to pending).
    #[serde(default)]
    pub status: Option<String>,
    /// Priority level (defaults to medium).
    #[serde(default)]
    pub priority: Option<String>,
    /// Optional active form (present continuous description).
    #[serde(default)]
    pub active_form: Option<String>,
}

/// Input schema for the TodoWriteTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TodoWriteInput {
    /// List of todos to write/update.
    pub todos: Vec<TodoInput>,
    /// If true, replace all existing todos (defaults to false for merge mode).
    #[serde(default)]
    pub replace_all: Option<bool>,
}

/// A single todo item in the output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoOutputItem {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl From<Todo> for TodoOutputItem {
    fn from(todo: Todo) -> Self {
        Self {
            id: todo.id,
            content: todo.content,
            status: todo.status.as_str().to_string(),
            priority: todo.priority.as_str().to_string(),
            active_form: todo.active_form,
            created_at: todo.created_at,
            updated_at: todo.updated_at,
        }
    }
}

/// Output schema for the TodoWriteTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoWriteOutput {
    /// The todo list before the update.
    pub old_todos: Vec<TodoOutputItem>,
    /// The todo list after the update.
    pub new_todos: Vec<TodoOutputItem>,
    /// Number of todos added.
    pub added: usize,
    /// Number of todos updated.
    pub updated: usize,
    /// Number of todos removed.
    pub removed: usize,
    /// Whether all todos are now completed.
    pub all_completed: bool,
}

/// The TodoWriteTool manages session-level todo lists.
#[derive(Debug, Clone, Default)]
pub struct TodoWriteTool;

impl TodoWriteTool {
    /// Create a new TodoWriteTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse status string to TodoStatus.
    fn parse_status(status: Option<&str>) -> TodoStatus {
        status
            .and_then(|s| s.parse().ok())
            .unwrap_or_default()
    }

    /// Parse priority string to TodoPriority.
    fn parse_priority(priority: Option<&str>) -> TodoPriority {
        priority
            .and_then(|p| p.parse().ok())
            .unwrap_or_default()
    }

    /// Convert TodoInput to Todo (creating new or updating existing).
    fn create_or_update_todo(&self, input: TodoInput) -> Todo {
        if let Some(id) = input.id {
            // Try to update existing todo
            if let Ok(store) = get_todo_store().lock() {
                if let Some(existing) = store.get(&id) {
                    let mut todo = existing.clone();
                    if !input.content.is_empty() {
                        todo.set_content(input.content);
                    }
                    todo.set_status(TodoWriteTool::parse_status(input.status.as_deref()));
                    todo.set_priority(TodoWriteTool::parse_priority(input.priority.as_deref()));
                    if input.active_form.is_some() {
                        todo.active_form = input.active_form;
                    }
                    return todo;
                }
            }
        }

        // Create new todo
        let mut todo = Todo::new(input.content);
        todo.set_status(TodoWriteTool::parse_status(input.status.as_deref()));
        todo.set_priority(TodoWriteTool::parse_priority(input.priority.as_deref()));
        todo.active_form = input.active_form;
        todo
    }
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "TodoWriteTool",
                "Manage session-level todo lists with batch operations",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let todos = input
            .require("todos")?
            .as_array()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "todos must be an array".to_string(),
                error_code: Some(1),
            })?;

        if todos.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "todos array cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Validate each todo
        for (i, todo) in todos.iter().enumerate() {
            let content = todo
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::ValidationFailed {
                    message: format!("Todo {}: content is required", i),
                    error_code: Some(3),
                })?;

            if content.is_empty() {
                return Err(ToolError::ValidationFailed {
                    message: format!("Todo {}: content cannot be empty", i),
                    error_code: Some(4),
                });
            }

            // Validate status if provided
            if let Some(status) = todo.get("status").and_then(|v| v.as_str()) {
                let valid_statuses = ["pending", "in_progress", "completed", "cancelled"];
                if !valid_statuses.contains(&status) {
                    return Err(ToolError::ValidationFailed {
                        message: format!(
                            "Todo {}: Invalid status '{}'. Valid: pending, in_progress, completed, cancelled",
                            i, status
                        ),
                        error_code: Some(5),
                    });
                }
            }

            // Validate priority if provided
            if let Some(priority) = todo.get("priority").and_then(|v| v.as_str()) {
                let valid_priorities = ["low", "medium", "high", "urgent"];
                if !valid_priorities.contains(&priority) {
                    return Err(ToolError::ValidationFailed {
                        message: format!(
                            "Todo {}: Invalid priority '{}'. Valid: low, medium, high, urgent",
                            i, priority
                        ),
                        error_code: Some(6),
                    });
                }
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let todos_array = match input.get("todos") {
            Some(v) => match v.as_array() {
                Some(arr) => arr.clone(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "todos must be an array")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: todos")
                    .with_field("success", false);
            }
        };

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get old todos before modification
        let old_todos: Vec<TodoOutputItem> = {
            match get_todo_store().lock() {
                Ok(store) => store.list_sorted().into_iter().map(|t| t.clone().into()).collect(),
                Err(_) => {
                    return ToolOutput::new()
                        .with_field("error", "Failed to access todo store")
                        .with_field("success", false);
                }
            }
        };

        let old_count = old_todos.len();

        // Parse and process new todos
        let mut new_todos: Vec<Todo> = Vec::new();
        let mut updated_count = 0;
        let mut added_count = 0;

        for todo_val in todos_array {
            let todo_input = TodoInput {
                id: todo_val.get("id").and_then(|v| v.as_str()).map(String::from),
                content: todo_val
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                status: todo_val.get("status").and_then(|v| v.as_str()).map(String::from),
                priority: todo_val.get("priority").and_then(|v| v.as_str()).map(String::from),
                active_form: todo_val.get("active_form").and_then(|v| v.as_str()).map(String::from),
            };

            let is_update = todo_input.id.is_some();
            let todo = self.create_or_update_todo(todo_input);
            new_todos.push(todo);

            if is_update {
                updated_count += 1;
            } else {
                added_count += 1;
            }
        }

        // Update the store
        let removed_count = if replace_all {
            let count = match get_todo_store().lock() {
                Ok(store) => store.list().len(),
                Err(_) => 0,
            };
            reset_todo_store();
            count
        } else {
            // In merge mode, we only count explicit removals as "removed"
            // which would need a separate todo remove tool
            0
        };

        // Store the new todos
        match get_todo_store().lock() {
            Ok(mut store) => {
                if replace_all {
                    store.replace_all(new_todos.clone());
                } else {
                    for todo in new_todos {
                        store.set(todo);
                    }
                }
            }
            Err(_) => {
                return ToolOutput::new()
                    .with_field("error", "Failed to update todo store")
                    .with_field("success", false);
            }
        }

        // Get final state
        let new_todos_output: Vec<TodoOutputItem> = {
            match get_todo_store().lock() {
                Ok(store) => store.list_sorted().into_iter().map(|t| t.clone().into()).collect(),
                Err(_) => Vec::new(),
            }
        };

        let all_completed = match get_todo_store().lock() {
            Ok(store) => store.all_completed(),
            Err(_) => false,
        };

        // Adjust counts if replacing
        let (final_added, final_updated, final_removed) = if replace_all {
            (new_todos_output.len(), 0, old_count.saturating_sub(new_todos_output.len()))
        } else {
            (added_count, updated_count, removed_count)
        };

        ToolOutput::new()
            .with_field("success", true)
            .with_field("old_todos", old_todos)
            .with_field("new_todos", new_todos_output)
            .with_field("added", final_added)
            .with_field("updated", final_updated)
            .with_field("removed", final_removed)
            .with_field("all_completed", all_completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolInput;
    use std::sync::Mutex;

    // Global mutex to ensure tests run serially
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_todo_store();
        guard
    }

    #[tokio::test]
    async fn test_todo_write_create_new() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({
                    "content": "Test todo 1",
                    "status": "pending",
                    "priority": "high"
                }),
                serde_json::json!({
                    "content": "Test todo 2",
                    "status": "in_progress"
                }),
            ]);

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(output.data.get("added").and_then(|v| v.as_u64()), Some(2));

        let new_todos = output.data.get("new_todos").and_then(|v| v.as_array()).unwrap();
        assert_eq!(new_todos.len(), 2);
        // Results are sorted by status (in_progress > pending), so "Test todo 2" comes first
        let contents: Vec<&str> = new_todos
            .iter()
            .map(|t| t.get("content").and_then(|v| v.as_str()).unwrap())
            .collect();
        assert!(contents.contains(&"Test todo 1"));
        assert!(contents.contains(&"Test todo 2"));
        // Check that Test todo 1 has high priority
        let todo1 = new_todos.iter().find(|t| {
            t.get("content").and_then(|v| v.as_str()) == Some("Test todo 1")
        }).unwrap();
        assert_eq!(todo1.get("priority").and_then(|v| v.as_str()), Some("high"));
    }

    #[tokio::test]
    async fn test_todo_write_update_existing() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        // First create a todo
        let input1 = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({
                    "content": "Original content",
                    "status": "pending"
                }),
            ]);

        let output1 = tool.execute(input1).await;
        let todo_id = output1
            .data
            .get("new_todos")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.get(0))
            .and_then(|t| t.get("id"))
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Now update it
        let input2 = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({
                    "id": todo_id,
                    "content": "Updated content",
                    "status": "completed",
                    "priority": "high"
                }),
            ]);

        let output2 = tool.execute(input2).await;

        assert_eq!(output2.data.get("updated").and_then(|v| v.as_u64()), Some(1));

        let new_todos = output2.data.get("new_todos").and_then(|v| v.as_array()).unwrap();
        assert_eq!(new_todos[0].get("content").and_then(|v| v.as_str()), Some("Updated content"));
        assert_eq!(new_todos[0].get("status").and_then(|v| v.as_str()), Some("completed"));
    }

    #[tokio::test]
    async fn test_todo_write_replace_all() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        // Create initial todos
        let input1 = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({"content": "Todo 1", "status": "pending"}),
                serde_json::json!({"content": "Todo 2", "status": "pending"}),
            ]);
        tool.execute(input1).await;

        // Replace all
        let input2 = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({"content": "New todo", "status": "completed"}),
            ])
            .with_arg("replace_all", true);

        let output2 = tool.execute(input2).await;

        assert_eq!(output2.data.get("added").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(output2.data.get("removed").and_then(|v| v.as_u64()), Some(1));

        let new_todos = output2.data.get("new_todos").and_then(|v| v.as_array()).unwrap();
        assert_eq!(new_todos.len(), 1);
        assert_eq!(new_todos[0].get("content").and_then(|v| v.as_str()), Some("New todo"));
    }

    #[tokio::test]
    async fn test_todo_write_all_completed() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({"content": "Todo 1", "status": "completed"}),
                serde_json::json!({"content": "Todo 2", "status": "completed"}),
            ]);

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("all_completed").and_then(|v| v.as_bool()), Some(true));
    }

    #[tokio::test]
    async fn test_todo_write_validation_empty_todos() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new().with_arg("todos", Vec::<serde_json::Value>::new());

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_todo_write_validation_missing_content() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![serde_json::json!({"status": "pending"})]);

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_todo_write_validation_invalid_status() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({"content": "Test", "status": "invalid_status"})
            ]);

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_todo_write_validation_invalid_priority() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({"content": "Test", "priority": "critical"})
            ]);

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_todo_write_with_active_form() {
        let _guard = setup();
        let tool = TodoWriteTool::new();

        let input = ToolInput::new()
            .with_arg("todos", vec![
                serde_json::json!({
                    "content": "Implement feature",
                    "status": "in_progress",
                    "active_form": "Implementing feature"
                }),
            ]);

        let output = tool.execute(input).await;

        let new_todos = output.data.get("new_todos").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            new_todos[0].get("active_form").and_then(|v| v.as_str()),
            Some("Implementing feature")
        );
    }

    #[test]
    fn test_parse_status() {
        assert_eq!(TodoWriteTool::parse_status(Some("pending")), TodoStatus::Pending);
        assert_eq!(TodoWriteTool::parse_status(Some("in_progress")), TodoStatus::InProgress);
        assert_eq!(TodoWriteTool::parse_status(Some("completed")), TodoStatus::Completed);
        assert_eq!(TodoWriteTool::parse_status(Some("cancelled")), TodoStatus::Cancelled);
        assert_eq!(TodoWriteTool::parse_status(None), TodoStatus::Pending);
        assert_eq!(TodoWriteTool::parse_status(Some("invalid")), TodoStatus::Pending);
    }

    #[test]
    fn test_parse_priority() {
        assert_eq!(TodoWriteTool::parse_priority(Some("low")), TodoPriority::Low);
        assert_eq!(TodoWriteTool::parse_priority(Some("medium")), TodoPriority::Medium);
        assert_eq!(TodoWriteTool::parse_priority(Some("high")), TodoPriority::High);
        assert_eq!(TodoWriteTool::parse_priority(Some("urgent")), TodoPriority::Urgent);
        assert_eq!(TodoWriteTool::parse_priority(None), TodoPriority::Medium);
        assert_eq!(TodoWriteTool::parse_priority(Some("invalid")), TodoPriority::Medium);
    }
}
