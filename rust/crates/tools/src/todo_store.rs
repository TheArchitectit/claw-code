//! Todo Store - Shared in-memory storage for session-level todos.
//!
//! This module provides a thread-safe, shared todo store that persists
//! across tool invocations during a session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Todo status enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl TodoStatus {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
            TodoStatus::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for TodoStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TodoStatus::Pending),
            "in_progress" => Ok(TodoStatus::InProgress),
            "completed" => Ok(TodoStatus::Completed),
            "cancelled" => Ok(TodoStatus::Cancelled),
            _ => Err(format!("Invalid status: {s}")),
        }
    }
}

impl Default for TodoStatus {
    fn default() -> Self {
        TodoStatus::Pending
    }
}

/// Todo priority levels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TodoPriority {
    #[serde(rename = "low")]
    Low,
    #[default]
    #[serde(rename = "medium")]
    Medium,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "urgent")]
    Urgent,
}

impl TodoPriority {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoPriority::Low => "low",
            TodoPriority::Medium => "medium",
            TodoPriority::High => "high",
            TodoPriority::Urgent => "urgent",
        }
    }
}

impl std::str::FromStr for TodoPriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "low" => Ok(TodoPriority::Low),
            "medium" => Ok(TodoPriority::Medium),
            "high" => Ok(TodoPriority::High),
            "urgent" => Ok(TodoPriority::Urgent),
            _ => Err(format!("Invalid priority: {s}")),
        }
    }
}

/// A todo item in the session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    /// Unique identifier for the todo.
    pub id: String,
    /// The todo content/description.
    pub content: String,
    /// Current status.
    pub status: TodoStatus,
    /// Priority level.
    #[serde(default)]
    pub priority: TodoPriority,
    /// Optional active form (present continuous description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_form: Option<String>,
    /// Creation timestamp.
    pub created_at: u64,
    /// Last update timestamp.
    pub updated_at: u64,
    /// Optional metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Todo {
    /// Create a new todo.
    pub fn new(content: impl Into<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_millis = now.as_millis();
        let random_suffix = rand::random::<u16>();
        let id = format!("todo-{:x}-{:x}", now_millis, random_suffix);

        Self {
            id,
            content: content.into(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Medium,
            active_form: None,
            created_at: now_secs,
            updated_at: now_secs,
            metadata: None,
        }
    }

    /// Set the status.
    pub fn set_status(&mut self, status: TodoStatus) {
        self.status = status;
        self.touch();
    }

    /// Set the priority.
    pub fn set_priority(&mut self, priority: TodoPriority) {
        self.priority = priority;
        self.touch();
    }

    /// Set the content.
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
        self.touch();
    }

    /// Update the timestamp.
    pub fn touch(&mut self) {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// Todo summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoSummary {
    pub id: String,
    pub content: String,
    pub status: String,
    pub priority: String,
}

impl From<Todo> for TodoSummary {
    fn from(todo: Todo) -> Self {
        Self {
            id: todo.id,
            content: todo.content,
            status: todo.status.as_str().to_string(),
            priority: todo.priority.as_str().to_string(),
        }
    }
}

impl From<&Todo> for TodoSummary {
    fn from(todo: &Todo) -> Self {
        Self {
            id: todo.id.clone(),
            content: todo.content.clone(),
            status: todo.status.as_str().to_string(),
            priority: todo.priority.as_str().to_string(),
        }
    }
}

/// The todo store - thread-safe shared storage.
#[derive(Debug, Default)]
pub struct TodoStore {
    todos: HashMap<String, Todo>,
}

impl TodoStore {
    /// Create a new empty todo store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            todos: HashMap::new(),
        }
    }

    /// Insert or update a todo.
    pub fn set(&mut self, todo: Todo) {
        self.todos.insert(todo.id.clone(), todo);
    }

    /// Get a todo by ID.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Todo> {
        self.todos.get(id)
    }

    /// Remove a todo by ID.
    pub fn remove(&mut self, id: &str) -> Option<Todo> {
        self.todos.remove(id)
    }

    /// Get all todos as a vector.
    #[must_use]
    pub fn list(&self) -> Vec<&Todo> {
        self.todos.values().collect()
    }

    /// Get all todos sorted by status and priority.
    #[must_use]
    pub fn list_sorted(&self) -> Vec<&Todo> {
        let mut todos: Vec<&Todo> = self.todos.values().collect();
        // Sort by: pending/in_progress first, then by priority (urgent > high > medium > low)
        todos.sort_by(|a, b| {
            let status_order = |s: &TodoStatus| match s {
                TodoStatus::InProgress => 0,
                TodoStatus::Pending => 1,
                TodoStatus::Completed => 2,
                TodoStatus::Cancelled => 3,
            };
            let priority_order = |p: &TodoPriority| match p {
                TodoPriority::Urgent => 0,
                TodoPriority::High => 1,
                TodoPriority::Medium => 2,
                TodoPriority::Low => 3,
            };
            let a_order = (status_order(&a.status), priority_order(&a.priority));
            let b_order = (status_order(&b.status), priority_order(&b.priority));
            a_order.cmp(&b_order)
        });
        todos
    }

    /// Replace all todos (batch update).
    pub fn replace_all(&mut self, todos: Vec<Todo>) {
        self.todos.clear();
        for todo in todos {
            self.todos.insert(todo.id.clone(), todo);
        }
    }

    /// Clear all completed todos.
    pub fn clear_completed(&mut self) {
        self.todos.retain(|_, todo| todo.status != TodoStatus::Completed);
    }

    /// Clear all todos.
    pub fn clear(&mut self) {
        self.todos.clear();
    }

    /// Get count of todos by status.
    #[must_use]
    pub fn count_by_status(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for todo in self.todos.values() {
            *counts.entry(todo.status.as_str().to_string()).or_insert(0) += 1;
        }
        counts
    }

    /// Check if all todos are completed.
    #[must_use]
    pub fn all_completed(&self) -> bool {
        !self.todos.is_empty() && self.todos.values().all(|t| t.status == TodoStatus::Completed)
    }
}

// Global todo store singleton
static TODO_STORE: std::sync::OnceLock<Arc<Mutex<TodoStore>>> = std::sync::OnceLock::new();

/// Get the global todo store instance.
pub fn get_todo_store() -> Arc<Mutex<TodoStore>> {
    TODO_STORE
        .get_or_init(|| Arc::new(Mutex::new(TodoStore::new())))
        .clone()
}

/// Reset the todo store (useful for testing).
pub fn reset_todo_store() {
    if let Ok(mut store) = get_todo_store().lock() {
        store.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_creation() {
        let todo = Todo::new("Test todo");
        assert_eq!(todo.content, "Test todo");
        assert_eq!(todo.status, TodoStatus::Pending);
        assert_eq!(todo.priority, TodoPriority::Medium);
        assert!(todo.id.starts_with("todo-"));
    }

    #[test]
    fn test_todo_status_update() {
        let mut todo = Todo::new("Test");
        let original_updated = todo.updated_at;
        // Sleep long enough to guarantee a timestamp change (timestamps are in seconds)
        std::thread::sleep(std::time::Duration::from_millis(1100));
        todo.set_status(TodoStatus::InProgress);
        assert_eq!(todo.status, TodoStatus::InProgress);
        assert!(todo.updated_at > original_updated);
    }

    #[test]
    fn test_todo_priority() {
        let mut todo = Todo::new("Test");
        todo.set_priority(TodoPriority::High);
        assert_eq!(todo.priority, TodoPriority::High);
    }

    #[test]
    fn test_todo_store() {
        let mut store = TodoStore::new();
        let todo = Todo::new("Test");
        let id = todo.id.clone();
        store.set(todo);

        assert!(store.get(&id).is_some());
        assert_eq!(store.list().len(), 1);

        let removed = store.remove(&id);
        assert!(removed.is_some());
        assert!(store.get(&id).is_none());
    }

    #[test]
    fn test_todo_store_sorted() {
        let mut store = TodoStore::new();

        let mut todo1 = Todo::new("Low priority pending");
        todo1.set_priority(TodoPriority::Low);
        store.set(todo1);

        let mut todo2 = Todo::new("High priority pending");
        todo2.set_priority(TodoPriority::High);
        store.set(todo2);

        let mut todo3 = Todo::new("Medium priority in_progress");
        todo3.set_status(TodoStatus::InProgress);
        todo3.set_priority(TodoPriority::Medium);
        store.set(todo3);

        let sorted = store.list_sorted();
        assert_eq!(sorted.len(), 3);
        // In-progress should be first
        assert_eq!(sorted[0].content, "Medium priority in_progress");
        // Then pending sorted by priority
        assert_eq!(sorted[1].content, "High priority pending");
        assert_eq!(sorted[2].content, "Low priority pending");
    }

    #[test]
    fn test_todo_store_replace_all() {
        let mut store = TodoStore::new();
        store.set(Todo::new("Old 1"));
        store.set(Todo::new("Old 2"));

        let new_todos = vec![Todo::new("New 1"), Todo::new("New 2")];
        store.replace_all(new_todos);

        assert_eq!(store.list().len(), 2);
        assert!(store.list().iter().all(|t| t.content.starts_with("New")));
    }

    #[test]
    fn test_todo_store_clear_completed() {
        let mut store = TodoStore::new();

        let mut todo1 = Todo::new("Pending");
        todo1.set_status(TodoStatus::Pending);
        store.set(todo1);

        let mut todo2 = Todo::new("Completed");
        todo2.set_status(TodoStatus::Completed);
        store.set(todo2);

        store.clear_completed();

        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].content, "Pending");
    }

    #[test]
    fn test_todo_store_all_completed() {
        let mut store = TodoStore::new();
        assert!(!store.all_completed()); // Empty store

        let mut todo = Todo::new("Test");
        todo.set_status(TodoStatus::Completed);
        store.set(todo);

        assert!(store.all_completed());

        let mut todo2 = Todo::new("Test 2");
        todo2.set_status(TodoStatus::Pending);
        store.set(todo2);

        assert!(!store.all_completed());
    }

    #[test]
    fn test_status_from_str() {
        assert_eq!("pending".parse::<TodoStatus>().unwrap(), TodoStatus::Pending);
        assert_eq!("in_progress".parse::<TodoStatus>().unwrap(), TodoStatus::InProgress);
        assert_eq!("completed".parse::<TodoStatus>().unwrap(), TodoStatus::Completed);
        assert_eq!("cancelled".parse::<TodoStatus>().unwrap(), TodoStatus::Cancelled);
        assert!("invalid".parse::<TodoStatus>().is_err());
    }

    #[test]
    fn test_priority_from_str() {
        assert_eq!("low".parse::<TodoPriority>().unwrap(), TodoPriority::Low);
        assert_eq!("medium".parse::<TodoPriority>().unwrap(), TodoPriority::Medium);
        assert_eq!("high".parse::<TodoPriority>().unwrap(), TodoPriority::High);
        assert_eq!("urgent".parse::<TodoPriority>().unwrap(), TodoPriority::Urgent);
        assert!("invalid".parse::<TodoPriority>().is_err());
    }
}
