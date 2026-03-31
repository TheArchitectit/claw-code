//! State & Persistence crate for R.A.D Codicological 2.x
//!
//! This crate provides:
//! - Task persistence and recovery
//! - Session storage and resume
//! - Memory directory (memdir) management
//! - Settings management with validation
//! - Git worktree state tracking
//! - Plugin state management

pub mod error;
pub mod git_worktree;
pub mod memdir;
pub mod plugin;
pub mod session;
pub mod settings;
pub mod task;

pub use error::{StateError, StateResult};
pub use git_worktree::{GitWorktreeState, WorktreeManager};
pub use memdir::{MemDirConfig, MemDirManager, MemoryEntry, MemoryType};
pub use plugin::{PluginState, PluginStateManager};
pub use session::{
    CheckpointId, CheckpointMetadata, ConversationState, SessionInfo, SessionManager,
    SessionStorageConfig,
};
pub use settings::{SettingSource, Settings, SettingsManager};
pub use task::{Task, TaskPersistence, TaskStatus, TaskStorage};

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global state manager that coordinates all state subsystems
#[derive(Debug, Clone)]
pub struct StateManager {
    inner: Arc<RwLock<StateManagerInner>>,
}

#[derive(Debug)]
struct StateManagerInner {
    project_dir: PathBuf,
    session_manager: SessionManager,
    task_storage: TaskStorage,
    settings_manager: SettingsManager,
    memdir_manager: MemDirManager,
    worktree_manager: WorktreeManager,
    plugin_manager: PluginStateManager,
}

impl StateManager {
    /// Create a new state manager for the given project directory
    pub async fn new(project_dir: impl Into<PathBuf>) -> StateResult<Self> {
        let project_dir = project_dir.into();

        let state_dir = project_dir.join(".claude").join("state");
        tokio::fs::create_dir_all(&state_dir).await?;

        let session_manager = SessionManager::new(&state_dir).await?;
        let task_storage = TaskStorage::new(&state_dir).await?;
        let settings_manager = SettingsManager::new(&state_dir).await?;
        let memdir_manager = MemDirManager::new(&project_dir).await?;
        let worktree_manager = WorktreeManager::new(&project_dir).await?;
        let plugin_manager = PluginStateManager::new(&state_dir).await?;

        Ok(Self {
            inner: Arc::new(RwLock::new(StateManagerInner {
                project_dir,
                session_manager,
                task_storage,
                settings_manager,
                memdir_manager,
                worktree_manager,
                plugin_manager,
            })),
        })
    }

    /// Get the project directory
    pub async fn project_dir(&self) -> PathBuf {
        let inner = self.inner.read().await;
        inner.project_dir.clone()
    }

    /// Access the session manager
    pub async fn sessions(&self) -> SessionManager {
        let inner = self.inner.read().await;
        inner.session_manager.clone()
    }

    /// Access the task storage
    pub async fn tasks(&self) -> TaskStorage {
        let inner = self.inner.read().await;
        inner.task_storage.clone()
    }

    /// Access the settings manager
    pub async fn settings(&self) -> SettingsManager {
        let inner = self.inner.read().await;
        inner.settings_manager.clone()
    }

    /// Access the memory directory manager
    pub async fn memdir(&self) -> MemDirManager {
        let inner = self.inner.read().await;
        inner.memdir_manager.clone()
    }

    /// Access the worktree manager
    pub async fn worktrees(&self) -> WorktreeManager {
        let inner = self.inner.read().await;
        inner.worktree_manager.clone()
    }

    /// Access the plugin state manager
    pub async fn plugins(&self) -> PluginStateManager {
        let inner = self.inner.read().await;
        inner.plugin_manager.clone()
    }

    /// Save all state to disk
    pub async fn save_all(&self) -> StateResult<()> {
        let inner = self.inner.read().await;
        inner.task_storage.save_all().await?;
        inner.session_manager.save_current().await?;
        inner.plugin_manager.save_all().await?;
        Ok(())
    }

    /// Load all state from disk
    pub async fn load_all(&self) -> StateResult<()> {
        let inner = self.inner.read().await;
        inner.task_storage.load_all().await?;
        inner.session_manager.load_active().await?;
        inner.plugin_manager.load_all().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_state_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = StateManager::new(temp_dir.path()).await;
        assert!(manager.is_ok());
    }
}
