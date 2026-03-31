//! Task persistence and storage
//!
//! Provides functionality for:
//! - Saving task state to disk
//! - Loading task state from disk
//! - Task status tracking
//! - Task output persistence

use crate::error::{StateError, StateResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Unique identifier for a task
pub type TaskId = String;

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is pending (not started)
    Pending,
    /// Task is running
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed,
    /// Task was stopped/cancelled
    Stopped,
    /// Task is paused
    Paused,
}

impl TaskStatus {
    /// Check if the task is in a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Stopped)
    }
}

/// Task type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Local shell task
    LocalShell,
    /// Local agent task
    LocalAgent,
    /// Remote agent task
    RemoteAgent,
    /// Dream task (background processing)
    Dream,
    /// Workflow task
    Workflow,
    /// Monitor task
    Monitor,
}

/// Task representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub task_type: TaskType,
    pub status: TaskStatus,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub output: Option<String>,
    pub output_path: Option<PathBuf>,
    pub error_message: Option<String>,
    pub metadata: HashMap<String, serde_json::Value>,
    /// Parent task ID (for subtasks)
    pub parent_id: Option<TaskId>,
    /// Child task IDs
    pub child_ids: Vec<TaskId>,
    /// Tool use ID associated with this task
    pub tool_use_id: Option<String>,
}

impl Task {
    /// Create a new task
    pub fn new(id: impl Into<TaskId>, task_type: TaskType, description: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            task_type,
            status: TaskStatus::Pending,
            description: description.into(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            output: None,
            output_path: None,
            error_message: None,
            metadata: HashMap::new(),
            parent_id: None,
            child_ids: Vec::new(),
            tool_use_id: None,
        }
    }

    /// Update the task status
    pub fn set_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        if status.is_terminal() && self.completed_at.is_none() {
            self.completed_at = Some(Utc::now());
        }
    }

    /// Set task output
    pub fn set_output(&mut self, output: impl Into<String>) {
        self.output = Some(output.into());
        self.updated_at = Utc::now();
    }

    /// Set error message
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error_message = Some(error.into());
        self.updated_at = Utc::now();
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Serialize) -> StateResult<()> {
        let value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Set parent task
    pub fn set_parent(&mut self, parent_id: impl Into<TaskId>) {
        self.parent_id = Some(parent_id.into());
        self.updated_at = Utc::now();
    }

    /// Add child task
    pub fn add_child(&mut self, child_id: impl Into<TaskId>) {
        self.child_ids.push(child_id.into());
        self.updated_at = Utc::now();
    }
}

/// Trait for task persistence operations
#[async_trait::async_trait]
pub trait TaskPersistence: Send + Sync {
    /// Save a task to persistence
    async fn save_task(&self, task: &Task) -> StateResult<()>;

    /// Load a task from persistence
    async fn load_task(&self, task_id: &TaskId) -> StateResult<Option<Task>>;

    /// Delete a task from persistence
    async fn delete_task(&self, task_id: &TaskId) -> StateResult<()>;

    /// List all persisted task IDs
    async fn list_tasks(&self) -> StateResult<Vec<TaskId>>;

    /// Load all tasks
    async fn load_all_tasks(&self) -> StateResult<Vec<Task>>;
}

/// File-based task storage
#[derive(Debug, Clone)]
pub struct TaskStorage {
    tasks_dir: PathBuf,
    tasks: Arc<RwLock<HashMap<TaskId, Task>>>,
}

impl TaskStorage {
    /// Create a new task storage at the given state directory
    pub async fn new(state_dir: &Path) -> StateResult<Self> {
        let tasks_dir = state_dir.join("tasks");
        fs::create_dir_all(&tasks_dir).await?;

        Ok(Self {
            tasks_dir,
            tasks: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register a new task (in-memory only, use save_all to persist)
    pub async fn register(&self, task: Task) -> StateResult<()> {
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    /// Update an existing task
    pub async fn update(&self, task_id: &TaskId, updater: impl FnOnce(&mut Task)) -> StateResult<()> {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(task_id) {
            updater(task);
            Ok(())
        } else {
            Err(StateError::TaskNotFound(task_id.clone()))
        }
    }

    /// Get a task by ID
    pub async fn get(&self, task_id: &TaskId) -> StateResult<Option<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.get(task_id).cloned())
    }

    /// Remove a task
    pub async fn remove(&self, task_id: &TaskId) -> StateResult<Option<Task>> {
        let mut tasks = self.tasks.write().await;
        let task = tasks.remove(task_id);

        // Also remove from disk
        let task_file = self.task_file_path(task_id);
        if task_file.exists() {
            fs::remove_file(task_file).await?;
        }

        Ok(task)
    }

    /// List all task IDs
    pub async fn list(&self) -> StateResult<Vec<TaskId>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.keys().cloned().collect())
    }

    /// Get all tasks
    pub async fn get_all(&self) -> StateResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks.values().cloned().collect())
    }

    /// Get tasks by status
    pub async fn get_by_status(&self, status: TaskStatus) -> StateResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks
            .values()
            .filter(|t| t.status == status)
            .cloned()
            .collect())
    }

    /// Get active (non-terminal) tasks
    pub async fn get_active(&self) -> StateResult<Vec<Task>> {
        let tasks = self.tasks.read().await;
        Ok(tasks
            .values()
            .filter(|t| !t.status.is_terminal())
            .cloned()
            .collect())
    }

    /// Save all tasks to disk
    pub async fn save_all(&self) -> StateResult<()> {
        let tasks = self.tasks.read().await;
        for (id, task) in tasks.iter() {
            self.save_task_to_disk(id, task).await?;
        }
        Ok(())
    }

    /// Load all tasks from disk
    pub async fn load_all(&self) -> StateResult<()> {
        let mut entries = fs::read_dir(&self.tasks_dir).await?;
        let mut loaded = HashMap::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                    let content = fs::read_to_string(&path).await?;
                    let task: Task = serde_json::from_str(&content)?;
                    loaded.insert(id.to_string(), task);
                }
            }
        }

        let mut tasks = self.tasks.write().await;
        *tasks = loaded;

        Ok(())
    }

    /// Save a specific task to disk
    async fn save_task_to_disk(&self, id: &TaskId, task: &Task) -> StateResult<()> {
        let path = self.task_file_path(id);
        let content = serde_json::to_string_pretty(task)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Get the file path for a task
    fn task_file_path(&self, task_id: &TaskId) -> PathBuf {
        self.tasks_dir.join(format!("{}.json", task_id))
    }
}

#[async_trait::async_trait]
impl TaskPersistence for TaskStorage {
    async fn save_task(&self, task: &Task) -> StateResult<()> {
        self.save_task_to_disk(&task.id, task).await
    }

    async fn load_task(&self, task_id: &TaskId) -> StateResult<Option<Task>> {
        self.get(task_id).await
    }

    async fn delete_task(&self, task_id: &TaskId) -> StateResult<()> {
        self.remove(task_id).await?;
        Ok(())
    }

    async fn list_tasks(&self) -> StateResult<Vec<TaskId>> {
        self.list().await
    }

    async fn load_all_tasks(&self) -> StateResult<Vec<Task>> {
        self.get_all().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_task_creation() {
        let task = Task::new("task-1", TaskType::LocalShell, "Test task");
        assert_eq!(task.id, "task-1");
        assert_eq!(task.status, TaskStatus::Pending);
        assert!(task.output.is_none());
    }

    #[test]
    fn test_task_status_transitions() {
        let mut task = Task::new("task-1", TaskType::LocalShell, "Test task");

        assert!(!task.status.is_terminal());

        task.set_status(TaskStatus::Running);
        assert_eq!(task.status, TaskStatus::Running);
        assert!(!task.status.is_terminal());

        task.set_status(TaskStatus::Completed);
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.status.is_terminal());
        assert!(task.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_task_storage() {
        let temp_dir = TempDir::new().unwrap();
        let storage = TaskStorage::new(temp_dir.path()).await.unwrap();

        let task = Task::new("task-1", TaskType::LocalAgent, "Test task");
        storage.register(task.clone()).await.unwrap();

        let retrieved = storage.get(&"task-1".to_string()).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "Test task");
    }

    #[tokio::test]
    async fn test_task_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create and save
        {
            let storage = TaskStorage::new(temp_dir.path()).await.unwrap();
            let task = Task::new("task-1", TaskType::LocalShell, "Test task");
            storage.register(task).await.unwrap();
            storage.save_all().await.unwrap();
        }

        // Load in new instance
        {
            let storage = TaskStorage::new(temp_dir.path()).await.unwrap();
            storage.load_all().await.unwrap();

            let task = storage.get(&"task-1".to_string()).await.unwrap();
            assert!(task.is_some());
            assert_eq!(task.unwrap().task_type, TaskType::LocalShell);
        }
    }
}
