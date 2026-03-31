//! Git worktree state tracking
//!
//! Provides:
//! - Worktree creation and management
//! - Worktree state persistence
//! - Worktree list tracking
//! - Symlink management for shared directories

use crate::error::{StateError, StateResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Valid worktree name pattern
const VALID_WORKTREE_SLUG: &str = r"^[a-zA-Z0-9._-]+$";

/// Maximum worktree name length
const MAX_WORKTREE_LENGTH: usize = 64;

/// Worktree information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeInfo {
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub is_active: bool,
    /// Directories symlinked from main repo
    pub symlinked_dirs: Vec<String>,
    /// Git worktree ID (from git worktree list)
    pub worktree_id: Option<String>,
}

impl WorktreeInfo {
    /// Create new worktree info
    pub fn new(name: impl Into<String>, path: impl Into<PathBuf>, branch: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            branch: branch.into(),
            created_at: chrono::Utc::now(),
            is_active: true,
            symlinked_dirs: Vec::new(),
            worktree_id: None,
        }
    }

    /// Validate a worktree name
    pub fn validate_name(name: &str) -> StateResult<()> {
        if name.len() > MAX_WORKTREE_LENGTH {
            return Err(StateError::Validation(format!(
                "Worktree name must be {} characters or fewer (got {})",
                MAX_WORKTREE_LENGTH,
                name.len()
            )));
        }

        // Check for path traversal attempts
        if name.contains("..") || name.starts_with('/') || name.starts_with('\\') {
            return Err(StateError::Validation(
                "Worktree name must not contain path traversal sequences".to_string()
            ));
        }

        // Validate each segment
        for segment in name.split('/') {
            if segment.is_empty() {
                return Err(StateError::Validation(
                    "Worktree name segments must not be empty".to_string()
                ));
            }
            if segment == "." || segment == ".." {
                return Err(StateError::Validation(
                    "Worktree name must not contain '.' or '..' segments".to_string()
                ));
            }
            if !regex::Regex::new(VALID_WORKTREE_SLUG).unwrap().is_match(segment) {
                return Err(StateError::Validation(format!(
                    "Invalid worktree name segment '{}': must contain only letters, digits, dots, underscores, and dashes",
                    segment
                )));
            }
        }

        Ok(())
    }
}

/// Git worktree state
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitWorktreeState {
    pub worktrees: HashMap<String, WorktreeInfo>,
    pub main_repo_path: Option<PathBuf>,
}

/// Worktree manager
#[derive(Debug, Clone)]
pub struct WorktreeManager {
    state_dir: PathBuf,
    state: Arc<RwLock<GitWorktreeState>>,
    project_dir: PathBuf,
}

impl WorktreeManager {
    /// Create a new worktree manager
    pub async fn new(project_dir: &Path) -> StateResult<Self> {
        let worktrees_dir = project_dir.join(".claude").join("worktrees");
        fs::create_dir_all(&worktrees_dir).await?;

        let state_path = worktrees_dir.join("state.json");
        let state = if state_path.exists() {
            let content = fs::read_to_string(&state_path).await?;
            serde_json::from_str(&content)?
        } else {
            GitWorktreeState::default()
        };

        Ok(Self {
            state_dir: worktrees_dir,
            state: Arc::new(RwLock::new(state)),
            project_dir: project_dir.to_path_buf(),
        })
    }

    /// Get the worktrees directory
    pub fn worktrees_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Get the project directory
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Create a new worktree
    pub async fn create_worktree(
        &self,
        name: impl Into<String>,
        branch: impl Into<String>,
        symlink_dirs: Option<Vec<String>>,
    ) -> StateResult<WorktreeInfo> {
        let name = name.into();
        let branch = branch.into();

        // Validate name
        WorktreeInfo::validate_name(&name)?;

        // Check if already exists
        let state = self.state.read().await;
        if state.worktrees.contains_key(&name) {
            return Err(StateError::Validation(format!(
                "Worktree '{}' already exists",
                name
            )));
        }
        drop(state);

        let worktree_path = self.state_dir.join(&name);

        // Create the worktree
        let mut info = WorktreeInfo::new(&name, &worktree_path, &branch);

        if let Some(dirs) = symlink_dirs {
            info.symlinked_dirs = dirs;
        }

        // Save to state
        let mut state = self.state.write().await;
        state.worktrees.insert(name.clone(), info.clone());
        self.persist_state(&state).await?;

        Ok(info)
    }

    /// Remove a worktree
    pub async fn remove_worktree(&self, name: &str) -> StateResult<()> {
        let mut state = self.state.write().await;

        if let Some(info) = state.worktrees.get(name) {
            // Remove from filesystem
            if info.path.exists() {
                fs::remove_dir_all(&info.path).await?;
            }

            // Remove from state
            state.worktrees.remove(name);
            self.persist_state(&state).await?;
        }

        Ok(())
    }

    /// Get a worktree by name
    pub async fn get_worktree(&self, name: &str) -> StateResult<Option<WorktreeInfo>> {
        let state = self.state.read().await;
        Ok(state.worktrees.get(name).cloned())
    }

    /// List all worktrees
    pub async fn list_worktrees(&self) -> StateResult<Vec<WorktreeInfo>> {
        let state = self.state.read().await;
        Ok(state.worktrees.values().cloned().collect())
    }

    /// List active worktrees
    pub async fn list_active(&self) -> StateResult<Vec<WorktreeInfo>> {
        let all = self.list_worktrees().await?;
        Ok(all.into_iter().filter(|w| w.is_active).collect())
    }

    /// Activate/deactivate a worktree
    pub async fn set_active(&self, name: &str, active: bool) -> StateResult<()> {
        let mut state = self.state.write().await;

        if let Some(info) = state.worktrees.get_mut(name) {
            info.is_active = active;
            self.persist_state(&state).await?;
            Ok(())
        } else {
            Err(StateError::WorktreeNotFound(name.to_string()))
        }
    }

    /// Update worktree info
    pub async fn update_worktree<F>(&self, name: &str, updater: F) -> StateResult<()>
    where
        F: FnOnce(&mut WorktreeInfo),
    {
        let mut state = self.state.write().await;

        if let Some(info) = state.worktrees.get_mut(name) {
            updater(info);
            self.persist_state(&state).await?;
            Ok(())
        } else {
            Err(StateError::WorktreeNotFound(name.to_string()))
        }
    }

    /// Persist state to disk
    async fn persist_state(&self, state: &GitWorktreeState) -> StateResult<()> {
        let path = self.state_dir.join("state.json");
        let content = serde_json::to_string_pretty(state)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Check if a worktree exists
    pub async fn exists(&self, name: &str) -> bool {
        let state = self.state.read().await;
        state.worktrees.contains_key(name)
    }

    /// Get worktree path
    pub fn worktree_path(&self, name: &str) -> PathBuf {
        self.state_dir.join(name)
    }

    /// Symlink directories from main repo to worktree
    pub async fn symlink_directories(
        &self,
        worktree_name: &str,
        dirs: &[String],
    ) -> StateResult<()> {
        let state = self.state.read().await;
        let info = state
            .worktrees
            .get(worktree_name)
            .ok_or_else(|| StateError::WorktreeNotFound(worktree_name.to_string()))?;

        let worktree_path = info.path.clone();
        drop(state);

        for dir in dirs {
            let source = self.project_dir.join(dir);
            let target = worktree_path.join(dir);

            if source.exists() {
                // Remove target if it exists
                if target.exists() {
                    fs::remove_file(&target).await.ok();
                }

                // Create symlink (Unix) or junction (Windows)
                #[cfg(unix)]
                {
                    std::os::unix::fs::symlink(&source, &target)?;
                }
                #[cfg(windows)]
                {
                    // On Windows, use junction for directories
                    tokio::task::spawn_blocking(move || {
                        std::os::windows::fs::symlink_dir(&source, &target)
                    })
                    .await??;
                }
            }
        }

        // Update state
        self.update_worktree(worktree_name, |info| {
            for dir in dirs {
                if !info.symlinked_dirs.contains(dir) {
                    info.symlinked_dirs.push(dir.clone());
                }
            }
        })
        .await
    }

    /// Remove symlinks from worktree
    pub async fn remove_symlinks(&self, worktree_name: &str, dirs: &[String]) -> StateResult<()> {
        let state = self.state.read().await;
        let info = state
            .worktrees
            .get(worktree_name)
            .ok_or_else(|| StateError::WorktreeNotFound(worktree_name.to_string()))?;

        let worktree_path = info.path.clone();
        drop(state);

        for dir in dirs {
            let target = worktree_path.join(dir);
            if target.exists() {
                fs::remove_file(&target).await.ok();
            }
        }

        // Update state
        self.update_worktree(worktree_name, |info| {
            info.symlinked_dirs.retain(|d| !dirs.contains(d));
        })
        .await
    }

    /// Sync state with actual git worktrees
    pub async fn sync_with_git(&self) -> StateResult<()> {
        // This would run git worktree list and update state
        // For now, just a placeholder
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validate_worktree_name() {
        // Valid names
        assert!(WorktreeInfo::validate_name("feature-abc").is_ok());
        assert!(WorktreeInfo::validate_name("user/feature").is_ok());
        assert!(WorktreeInfo::validate_name("fix_123").is_ok());

        // Invalid names
        assert!(WorktreeInfo::validate_name("../escape").is_err());
        assert!(WorktreeInfo::validate_name("/absolute").is_err());
        assert!(WorktreeInfo::validate_name("").is_err());
        assert!(WorktreeInfo::validate_name("a/../b").is_err());
    }

    #[tokio::test]
    async fn test_worktree_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::new(temp_dir.path()).await.unwrap();

        // Create worktree
        let info = manager
            .create_worktree("test-wt", "main", None)
            .await
            .unwrap();
        assert_eq!(info.name, "test-wt");
        assert!(info.is_active);

        // Check exists
        assert!(manager.exists("test-wt").await);

        // Get worktree
        let retrieved = manager.get_worktree("test-wt").await.unwrap();
        assert!(retrieved.is_some());

        // List worktrees
        let list = manager.list_worktrees().await.unwrap();
        assert_eq!(list.len(), 1);

        // Set inactive
        manager.set_active("test-wt", false).await.unwrap();
        let info = manager.get_worktree("test-wt").await.unwrap().unwrap();
        assert!(!info.is_active);
    }

    #[tokio::test]
    async fn test_worktree_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create worktree
        {
            let manager = WorktreeManager::new(temp_dir.path()).await.unwrap();
            manager
                .create_worktree("test-wt", "main", None)
                .await
                .unwrap();
        }

        // Verify in new instance
        {
            let manager = WorktreeManager::new(temp_dir.path()).await.unwrap();
            assert!(manager.exists("test-wt").await);
        }
    }

    #[tokio::test]
    async fn test_duplicate_worktree() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorktreeManager::new(temp_dir.path()).await.unwrap();

        manager.create_worktree("test", "main", None).await.unwrap();

        let result = manager.create_worktree("test", "main", None).await;
        assert!(result.is_err());
    }
}
