//! Task Store - Shared in-memory storage for task management.
//!
//! This module provides a thread-safe, shared task store that persists
//! across tool invocations during a session.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Task status enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "in_progress")]
    InProgress,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "cancelled")]
    Cancelled,
}

impl TaskStatus {
    /// Get string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::InProgress => "in_progress",
            TaskStatus::Completed => "completed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for TaskStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TaskStatus::Pending),
            "in_progress" => Ok(TaskStatus::InProgress),
            "completed" => Ok(TaskStatus::Completed),
            "cancelled" => Ok(TaskStatus::Cancelled),
            _ => Err(format!("Invalid status: {s}")),
        }
    }
}

/// A task in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub subject: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub metadata: Option<serde_json::Value>,
}

impl Task {
    /// Create a new task.
    pub fn new(subject: impl Into<String>, description: Option<String>) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let now_secs = now.as_secs();
        let now_millis = now.as_millis();
        // Use timestamp + random suffix to avoid collisions in rapid creation
        let random_suffix = rand::random::<u16>();
        let id = format!("task-{:x}-{:x}", now_millis, random_suffix);

        Self {
            id,
            subject: subject.into(),
            description,
            status: TaskStatus::Pending,
            owner: None,
            created_at: now_secs,
            updated_at: now_secs,
            metadata: None,
        }
    }

    /// Set the status.
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Set the owner.
    pub fn set_owner(&mut self, owner: impl Into<String>) {
        self.owner = Some(owner.into());
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Update the timestamp.
    pub fn touch(&mut self) {
        self.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// Task summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub task_id: String,
    pub subject: String,
    pub status: String,
    pub owner: Option<String>,
}

impl From<Task> for TaskSummary {
    fn from(task: Task) -> Self {
        Self {
            task_id: task.id,
            subject: task.subject,
            status: task.status.as_str().to_string(),
            owner: task.owner,
        }
    }
}

impl From<&Task> for TaskSummary {
    fn from(task: &Task) -> Self {
        Self {
            task_id: task.id.clone(),
            subject: task.subject.clone(),
            status: task.status.as_str().to_string(),
            owner: task.owner.clone(),
        }
    }
}

/// Detailed task information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDetails {
    pub task_id: String,
    pub subject: String,
    pub description: Option<String>,
    pub status: String,
    pub owner: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub metadata: Option<serde_json::Value>,
}

impl From<Task> for TaskDetails {
    fn from(task: Task) -> Self {
        Self {
            task_id: task.id,
            subject: task.subject,
            description: task.description,
            status: task.status.as_str().to_string(),
            owner: task.owner,
            created_at: task.created_at,
            updated_at: task.updated_at,
            metadata: task.metadata,
        }
    }
}

impl From<&Task> for TaskDetails {
    fn from(task: &Task) -> Self {
        Self {
            task_id: task.id.clone(),
            subject: task.subject.clone(),
            description: task.description.clone(),
            status: task.status.as_str().to_string(),
            owner: task.owner.clone(),
            created_at: task.created_at,
            updated_at: task.updated_at,
            metadata: task.metadata.clone(),
        }
    }
}

/// The task store.
#[derive(Debug, Clone)]
pub struct TaskStore {
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    outputs: Arc<Mutex<HashMap<String, String>>>,
}

impl TaskStore {
    /// Create a new empty task store.
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            outputs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all tasks and outputs.
    pub fn clear(&self) {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.clear();
        let mut outputs = self.outputs.lock().unwrap_or_else(|e| e.into_inner());
        outputs.clear();
    }

    /// Create a new task and add it to the store.
    pub fn create(&self, subject: impl Into<String>, description: Option<String>) -> Task {
        let task = Task::new(subject, description);
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.insert(task.id.clone(), task.clone());
        task
    }

    /// Get a task by ID.
    pub fn get(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.get(task_id).cloned()
    }

    /// Update a task.
    pub fn update<F>(&self, task_id: &str, f: F) -> Option<Task>
    where
        F: FnOnce(&mut Task),
    {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(task) = tasks.get_mut(task_id) {
            f(task);
            task.updated_at = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Some(task.clone())
        } else {
            None
        }
    }

    /// Delete a task.
    pub fn delete(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.remove(task_id).is_some()
    }

    /// List all tasks, optionally filtered.
    pub fn list(&self, status: Option<&str>, owner: Option<&str>) -> Vec<TaskSummary> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks
            .values()
            .filter(|t| {
                if let Some(s) = status {
                    t.status.as_str() == s
                } else {
                    true
                }
            })
            .filter(|t| {
                if let Some(o) = owner {
                    t.owner.as_ref().map(|v| v == o).unwrap_or(false)
                } else {
                    true
                }
            })
            .map(TaskSummary::from)
            .collect()
    }

    /// Get all tasks.
    pub fn all(&self) -> Vec<Task> {
        let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        tasks.values().cloned().collect()
    }

    /// Get task output by task ID.
    pub fn get_output(&self, task_id: &str) -> Option<String> {
        let outputs = self.outputs.lock().unwrap_or_else(|e| e.into_inner());
        outputs.get(task_id).cloned()
    }

    /// Set task output by task ID.
    pub fn set_output(&self, task_id: &str, output: impl Into<String>) -> Option<Task> {
        let output_str = output.into();
        let mut outputs = self.outputs.lock().unwrap_or_else(|e| e.into_inner());
        outputs.insert(task_id.to_string(), output_str);
        drop(outputs);
        self.get(task_id)
    }

    /// Append to existing task output.
    pub fn append_output(&self, task_id: &str, output: impl AsRef<str>) -> Option<Task> {
        let output_str = output.as_ref();
        let mut outputs = self.outputs.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = outputs.get_mut(task_id) {
            existing.push_str(output_str);
        } else {
            outputs.insert(task_id.to_string(), output_str.to_string());
        }
        drop(outputs);
        self.get(task_id)
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global task store instance
use std::sync::OnceLock;

static GLOBAL_TASK_STORE: OnceLock<TaskStore> = OnceLock::new();

/// Get the global task store instance.
pub fn get_task_store() -> TaskStore {
    GLOBAL_TASK_STORE.get_or_init(TaskStore::new).clone()
}

/// Reset the global task store (for testing).
pub fn reset_task_store() {
    // Clear the global store if it exists
    if let Some(store) = GLOBAL_TASK_STORE.get() {
        store.clear();
    }
}

/// Global mutex for test synchronization across all task tool tests.
#[cfg(test)]
pub static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_store_create() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_task_store();
        let store = get_task_store();
        let task = store.create("Test task", Some("Description".to_string()));
        assert!(!task.id.is_empty());
        assert_eq!(task.subject, "Test task");
        assert_eq!(task.status, TaskStatus::Pending);
    }

    #[test]
    fn test_task_store_get() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_task_store();
        let store = get_task_store();
        let task = store.create("Test", None);
        let retrieved = store.get(&task.id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, task.id);
        // Cleanup
        store.delete(&task.id);
    }

    #[test]
    fn test_task_store_update() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_task_store();
        let store = get_task_store();
        let task = store.create("Test", None);
        store.update(&task.id, |t| {
            t.set_status(TaskStatus::InProgress);
        });
        let updated = store.get(&task.id).unwrap();
        assert_eq!(updated.status, TaskStatus::InProgress);
    }

    #[test]
    fn test_task_store_list_filter() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_task_store();
        let store = get_task_store();
        let t1 = store.create("Task 1", None);
        eprintln!("Created t1: id={}", t1.id);
        let t2 = store.create("Task 2", None);
        eprintln!("Created t2: id={}", t2.id);
        store.update(&t1.id, |t| t.set_status(TaskStatus::Completed));

        let all = store.list(None, None);
        eprintln!("All tasks count: {}, tasks: {:?}", all.len(), all);
        assert_eq!(all.len(), 2, "Expected 2 tasks but got {}: {:?}", all.len(), all);

        let completed = store.list(Some("completed"), None);
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].task_id, t1.id);
    }

    #[test]
    fn test_task_status_parse() {
        let _guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        assert!("pending".parse::<TaskStatus>().is_ok());
        assert!("in_progress".parse::<TaskStatus>().is_ok());
        assert!("completed".parse::<TaskStatus>().is_ok());
        assert!("cancelled".parse::<TaskStatus>().is_ok());
        assert!("invalid".parse::<TaskStatus>().is_err());
    }
}
