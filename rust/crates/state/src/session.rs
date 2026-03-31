//! Session storage and resume functionality
//!
//! Provides:
//! - Session persistence
//! - Session resumption
//! - Session metadata tracking
//! - Session history

use crate::error::{StateError, StateResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Unique identifier for a session
pub type SessionId = String;

/// Unique identifier for a checkpoint
pub type CheckpointId = String;

/// Session status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session is active
    Active,
    /// Session is suspended (can be resumed)
    Suspended,
    /// Session ended normally
    Ended,
    /// Session was terminated
    Terminated,
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub status: SessionStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    /// Working directory when session started
    pub working_dir: PathBuf,
    /// Initial prompt or command
    pub initial_prompt: Option<String>,
    /// Session title (user-defined or auto-generated)
    pub title: Option<String>,
    /// Session metadata (model used, settings, etc.)
    pub metadata: HashMap<String, serde_json::Value>,
    /// Associated task IDs
    pub task_ids: Vec<String>,
    /// Transcript file path
    pub transcript_path: Option<PathBuf>,
    /// Whether this session is resumable
    pub is_resumable: bool,
}

impl SessionInfo {
    /// Create a new session
    pub fn new(id: impl Into<SessionId>, working_dir: impl Into<PathBuf>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            status: SessionStatus::Active,
            created_at: now,
            updated_at: now,
            ended_at: None,
            working_dir: working_dir.into(),
            initial_prompt: None,
            title: None,
            metadata: HashMap::new(),
            task_ids: Vec::new(),
            transcript_path: None,
            is_resumable: true,
        }
    }

    /// Set the session title
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.updated_at = Utc::now();
    }

    /// Set the initial prompt
    pub fn set_prompt(&mut self, prompt: impl Into<String>) {
        self.initial_prompt = Some(prompt.into());
        self.updated_at = Utc::now();
    }

    /// Add a task to the session
    pub fn add_task(&mut self, task_id: impl Into<String>) {
        self.task_ids.push(task_id.into());
        self.updated_at = Utc::now();
    }

    /// Set session status
    pub fn set_status(&mut self, status: SessionStatus) {
        self.status = status;
        self.updated_at = Utc::now();
        if matches!(status, SessionStatus::Ended | SessionStatus::Terminated) {
            self.ended_at = Some(Utc::now());
        }
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Serialize) -> StateResult<()> {
        let value = serde_json::to_value(value)?;
        self.metadata.insert(key.into(), value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Get session duration (if ended) or current duration
    pub fn duration(&self) -> chrono::Duration {
        let end = self.ended_at.unwrap_or_else(Utc::now);
        end.signed_duration_since(self.created_at)
    }
}

/// Metadata for a conversation checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub id: CheckpointId,
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub description: Option<String>,
    pub message_count: usize,
    pub turn_count: u32,
}

/// Conversation state for checkpoint persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationState {
    /// The session ID this checkpoint belongs to
    pub session_id: SessionId,

    /// The checkpoint ID
    pub checkpoint_id: CheckpointId,

    /// When the checkpoint was created
    pub created_at: DateTime<Utc>,

    /// Optional description of the checkpoint
    pub description: Option<String>,

    /// All conversation messages (serialized as JSON Value for flexibility)
    pub messages: Vec<serde_json::Value>,

    /// Tool use context state (serialized)
    pub tool_use_context: Option<serde_json::Value>,

    /// Usage statistics
    pub usage_stats: serde_json::Value,

    /// Permission denials recorded during the conversation
    pub permission_denials: Vec<serde_json::Value>,

    /// Current turn count
    pub turn_count: u32,

    /// Total cost in USD
    pub total_cost: f64,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ConversationState {
    /// Create a new conversation state for checkpointing
    pub fn new(
        session_id: impl Into<SessionId>,
        checkpoint_id: impl Into<CheckpointId>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            checkpoint_id: checkpoint_id.into(),
            created_at: Utc::now(),
            description: None,
            messages: Vec::new(),
            tool_use_context: None,
            usage_stats: serde_json::Value::Object(serde_json::Map::new()),
            permission_denials: Vec::new(),
            turn_count: 0,
            total_cost: 0.0,
            metadata: HashMap::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Set the messages
    pub fn with_messages(mut self, messages: Vec<serde_json::Value>) -> Self {
        self.messages = messages;
        self
    }

    /// Set the tool use context
    pub fn with_tool_use_context(mut self, ctx: serde_json::Value) -> Self {
        self.tool_use_context = Some(ctx);
        self
    }

    /// Set usage stats
    pub fn with_usage_stats(mut self, stats: serde_json::Value) -> Self {
        self.usage_stats = stats;
        self
    }

    /// Set permission denials
    pub fn with_permission_denials(mut self, denials: Vec<serde_json::Value>) -> Self {
        self.permission_denials = denials;
        self
    }

    /// Set turn count
    pub fn with_turn_count(mut self, count: u32) -> Self {
        self.turn_count = count;
        self
    }

    /// Set total cost
    pub fn with_total_cost(mut self, cost: f64) -> Self {
        self.total_cost = cost;
        self
    }

    /// Get metadata for the checkpoint
    pub fn metadata(&self) -> CheckpointMetadata {
        CheckpointMetadata {
            id: self.checkpoint_id.clone(),
            session_id: self.session_id.clone(),
            created_at: self.created_at,
            description: self.description.clone(),
            message_count: self.messages.len(),
            turn_count: self.turn_count,
        }
    }
}

/// Storage configuration for sessions
#[derive(Debug, Clone)]
pub struct SessionStorageConfig {
    pub sessions_dir: PathBuf,
    pub checkpoints_dir: PathBuf,
    pub max_sessions: usize,
    pub max_checkpoints_per_session: usize,
    pub auto_save_interval_secs: u64,
}

impl SessionStorageConfig {
    pub fn new(state_dir: &Path) -> Self {
        Self {
            sessions_dir: state_dir.join("sessions"),
            checkpoints_dir: state_dir.join("checkpoints"),
            max_sessions: 100,
            max_checkpoints_per_session: 50,
            auto_save_interval_secs: 60,
        }
    }
}

/// Session storage manager
#[derive(Debug, Clone)]
pub struct SessionManager {
    config: SessionStorageConfig,
    current_session: Arc<RwLock<Option<SessionInfo>>>,
    sessions: Arc<RwLock<HashMap<SessionId, SessionInfo>>>,
}

impl SessionManager {
    /// Create a new session manager
    pub async fn new(state_dir: &Path) -> StateResult<Self> {
        let config = SessionStorageConfig::new(state_dir);
        fs::create_dir_all(&config.sessions_dir).await?;
        fs::create_dir_all(&config.checkpoints_dir).await?;

        Ok(Self {
            config,
            current_session: Arc::new(RwLock::new(None)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Start a new session
    pub async fn start_session(&self, working_dir: impl Into<PathBuf>) -> StateResult<SessionInfo> {
        let id = generate_session_id();
        let session = SessionInfo::new(&id, working_dir);

        let mut current = self.current_session.write().await;
        *current = Some(session.clone());

        let mut sessions = self.sessions.write().await;
        sessions.insert(id, session.clone());

        Ok(session)
    }

    /// Get the current session
    pub async fn current_session(&self) -> StateResult<Option<SessionInfo>> {
        let current = self.current_session.read().await;
        Ok(current.clone())
    }

    /// Update the current session
    pub async fn update_current<F>(&self, updater: F) -> StateResult<()>
    where
        F: FnOnce(&mut SessionInfo),
    {
        let mut current = self.current_session.write().await;
        if let Some(ref mut session) = *current {
            updater(session);
            Ok(())
        } else {
            Err(StateError::SessionNotFound("No active session".to_string()))
        }
    }

    /// End the current session
    pub async fn end_current(&self) -> StateResult<()> {
        let mut current = self.current_session.write().await;
        if let Some(ref mut session) = *current {
            session.set_status(SessionStatus::Ended);
            self.save_session(session).await?;
            *current = None;
            Ok(())
        } else {
            Err(StateError::SessionNotFound("No active session".to_string()))
        }
    }

    /// Suspend the current session (for resume later)
    pub async fn suspend_current(&self) -> StateResult<()> {
        let mut current = self.current_session.write().await;
        if let Some(ref mut session) = *current {
            session.set_status(SessionStatus::Suspended);
            let session_id = session.id.clone();
            let session_clone = session.clone();
            drop(current);

            // Update in sessions map
            let mut sessions = self.sessions.write().await;
            sessions.insert(session_id, session_clone.clone());

            // Save to disk
            self.save_session(&session_clone).await?;
            Ok(())
        } else {
            Err(StateError::SessionNotFound("No active session".to_string()))
        }
    }

    /// Resume a suspended session
    pub async fn resume_session(&self, session_id: &SessionId) -> StateResult<SessionInfo> {
        let session = self.load_session(session_id).await?;

        if let Some(mut session) = session {
            if session.status == SessionStatus::Suspended && session.is_resumable {
                session.set_status(SessionStatus::Active);

                let mut current = self.current_session.write().await;
                *current = Some(session.clone());

                self.save_session(&session).await?;

                return Ok(session);
            } else {
                return Err(StateError::SessionNotFound(format!(
                    "Session {} cannot be resumed (status: {:?}, resumable: {})",
                    session_id, session.status, session.is_resumable
                )));
            }
        }

        Err(StateError::SessionNotFound(session_id.clone()))
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> StateResult<Vec<SessionInfo>> {
        let sessions = self.sessions.read().await;
        let mut list: Vec<_> = sessions.values().cloned().collect();
        // Sort by updated_at descending
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(list)
    }

    /// List resumable sessions
    pub async fn list_resumable(&self) -> StateResult<Vec<SessionInfo>> {
        let all = self.list_sessions().await?;
        Ok(all
            .into_iter()
            .filter(|s| s.is_resumable && s.status == SessionStatus::Suspended)
            .collect())
    }

    /// Get a session by ID
    pub async fn get_session(&self, session_id: &SessionId) -> StateResult<Option<SessionInfo>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    /// Delete a session and all its checkpoints
    pub async fn delete_session(&self, session_id: &SessionId) -> StateResult<()> {
        // Remove from memory
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);

        // Remove from disk
        let path = self.session_file_path(session_id);
        if path.exists() {
            fs::remove_file(path).await?;
        }

        // Clean up checkpoints
        self.delete_session_checkpoints(session_id).await?;

        Ok(())
    }

    /// Save the current session to disk
    pub async fn save_current(&self) -> StateResult<()> {
        let current = self.current_session.read().await;
        if let Some(ref session) = *current {
            self.save_session(session).await?;
        }
        Ok(())
    }

    /// Load active session from disk (on startup)
    pub async fn load_active(&self) -> StateResult<()> {
        let mut entries = fs::read_dir(&self.config.sessions_dir).await?;
        let mut loaded = HashMap::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                    let content = fs::read_to_string(&path).await?;
                    let session: SessionInfo = serde_json::from_str(&content)?;
                    loaded.insert(id.to_string(), session);
                }
            }
        }

        let mut sessions = self.sessions.write().await;
        *sessions = loaded;

        Ok(())
    }

    /// Save a session to disk
    async fn save_session(&self, session: &SessionInfo) -> StateResult<()> {
        let path = self.session_file_path(&session.id);
        let content = serde_json::to_string_pretty(session)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load a session from disk
    async fn load_session(&self, session_id: &SessionId) -> StateResult<Option<SessionInfo>> {
        let path = self.session_file_path(session_id);
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).await?;
        let session: SessionInfo = serde_json::from_str(&content)?;
        Ok(Some(session))
    }

    /// Save a checkpoint to disk
    pub async fn save_checkpoint(&self, state: &ConversationState) -> StateResult<CheckpointId> {
        let checkpoint_id = state.checkpoint_id.clone();

        // Ensure checkpoints directory exists
        let session_checkpoints_dir = self.config.checkpoints_dir.join(&state.session_id);
        fs::create_dir_all(&session_checkpoints_dir).await?;

        // Save the checkpoint
        let path = self.checkpoint_file_path(&state.session_id, &checkpoint_id);
        let content = serde_json::to_string_pretty(state)?;
        fs::write(path, content).await?;

        Ok(checkpoint_id)
    }

    /// Load a checkpoint from disk
    pub async fn load_checkpoint(
        &self,
        session_id: &SessionId,
        checkpoint_id: &CheckpointId,
    ) -> StateResult<Option<ConversationState>> {
        let path = self.checkpoint_file_path(session_id, checkpoint_id);

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).await?;
        let state: ConversationState = serde_json::from_str(&content)?;
        Ok(Some(state))
    }

    /// List all checkpoints for a session
    pub async fn list_checkpoints(
        &self,
        session_id: &SessionId,
    ) -> StateResult<Vec<CheckpointMetadata>> {
        let session_checkpoints_dir = self.config.checkpoints_dir.join(session_id);

        if !session_checkpoints_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = fs::read_dir(&session_checkpoints_dir).await?;
        let mut checkpoints = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Some(content) = fs::read_to_string(&path).await.ok() {
                    if let Ok(state) = serde_json::from_str::<ConversationState>(&content) {
                        checkpoints.push(state.metadata());
                    }
                }
            }
        }

        // Sort by created_at descending (newest first)
        checkpoints.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(checkpoints)
    }

    /// Delete a checkpoint
    pub async fn delete_checkpoint(
        &self,
        session_id: &SessionId,
        checkpoint_id: &CheckpointId,
    ) -> StateResult<()> {
        let path = self.checkpoint_file_path(session_id, checkpoint_id);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    /// Delete all checkpoints for a session
    pub async fn delete_session_checkpoints(&self, session_id: &SessionId) -> StateResult<()> {
        let session_checkpoints_dir = self.config.checkpoints_dir.join(session_id);
        if session_checkpoints_dir.exists() {
            fs::remove_dir_all(&session_checkpoints_dir).await?;
        }
        Ok(())
    }

    /// Get the file path for a checkpoint
    fn checkpoint_file_path(
        &self,
        session_id: &SessionId,
        checkpoint_id: &CheckpointId,
    ) -> PathBuf {
        self.config
            .checkpoints_dir
            .join(session_id)
            .join(format!("{}.json", checkpoint_id))
    }

    /// Get the file path for a session
    fn session_file_path(&self, session_id: &SessionId) -> PathBuf {
        self.config.sessions_dir.join(format!("{}.json", session_id))
    }

    pub async fn cleanup_old_checkpoints(
        &self,
        session_id: &SessionId,
        keep_count: usize,
    ) -> StateResult<usize> {
        let mut checkpoints = self.list_checkpoints(session_id).await?;

        if checkpoints.len() <= keep_count {
            return Ok(0);
        }

        // Sort oldest first (reverse of default)
        checkpoints.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        // Delete oldest checkpoints beyond keep_count
        let to_delete = &checkpoints[..checkpoints.len() - keep_count];
        let mut deleted = 0;

        for meta in to_delete {
            self.delete_checkpoint(session_id, &meta.id).await?;
            deleted += 1;
        }

        Ok(deleted)
    }
}

/// Generate a unique session ID
fn generate_session_id() -> SessionId {
    format!("sess-{}", chrono::Utc::now().timestamp_millis())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_session_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session = manager.start_session("/tmp").await.unwrap();
        assert_eq!(session.status, SessionStatus::Active);
        assert!(session.title.is_none());
    }

    #[tokio::test]
    async fn test_session_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Create and save
        let session_id = {
            let manager = SessionManager::new(temp_dir.path()).await.unwrap();
            let session = manager.start_session("/tmp").await.unwrap();
            let id = session.id.clone();
            manager.save_current().await.unwrap();
            id
        };

        // Load in new instance
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();
        manager.load_active().await.unwrap();

        let loaded = manager.get_session(&session_id).await.unwrap();
        assert!(loaded.is_some());
    }

    #[tokio::test]
    async fn test_session_suspend_resume() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session = manager.start_session("/tmp").await.unwrap();
        let id = session.id.clone();

        manager.suspend_current().await.unwrap();

        let resumable = manager.list_resumable().await.unwrap();
        assert_eq!(resumable.len(), 1);
        assert_eq!(resumable[0].id, id);

        let resumed = manager.resume_session(&id).await.unwrap();
        assert_eq!(resumed.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn test_session_update() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let _ = manager.start_session("/tmp").await.unwrap();

        manager
            .update_current(|s| {
                s.set_title("My Session");
            })
            .await
            .unwrap();

        let current = manager.current_session().await.unwrap();
        assert!(current.is_some());
        assert_eq!(current.unwrap().title, Some("My Session".to_string()));
    }

    #[tokio::test]
    async fn test_checkpoint_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session_id = "test-session".to_string();
        let checkpoint_id = "chk-123".to_string();

        // Create a conversation state
        let state = ConversationState::new(&session_id, &checkpoint_id)
            .with_description("Test checkpoint")
            .with_messages(vec![serde_json::json!({"role": "user", "content": "hello"})])
            .with_usage_stats(serde_json::json!({"tokens": 100}))
            .with_permission_denials(vec![])
            .with_turn_count(5)
            .with_total_cost(0.001);

        // Save checkpoint
        let saved_id = manager.save_checkpoint(&state).await.unwrap();
        assert_eq!(saved_id, checkpoint_id);

        // Load checkpoint
        let loaded = manager
            .load_checkpoint(&session_id, &checkpoint_id)
            .await
            .unwrap();
        assert!(loaded.is_some());

        let loaded_state = loaded.unwrap();
        assert_eq!(loaded_state.session_id, session_id);
        assert_eq!(loaded_state.checkpoint_id, checkpoint_id);
        assert_eq!(loaded_state.description, Some("Test checkpoint".to_string()));
        assert_eq!(loaded_state.turn_count, 5);
        assert_eq!(loaded_state.total_cost, 0.001);
        assert_eq!(loaded_state.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_checkpoint_list_and_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session_id = "test-session".to_string();

        // Create multiple checkpoints
        for i in 0..3 {
            let checkpoint_id = format!("chk-{}", i);
            let state = ConversationState::new(&session_id, &checkpoint_id)
                .with_description(format!("Checkpoint {}", i));
            manager.save_checkpoint(&state).await.unwrap();
        }

        // List checkpoints
        let checkpoints = manager.list_checkpoints(&session_id).await.unwrap();
        assert_eq!(checkpoints.len(), 3);

        // Delete one checkpoint
        manager
            .delete_checkpoint(&session_id, &"chk-0".to_string())
            .await
            .unwrap();

        // Verify deletion
        let remaining = manager.list_checkpoints(&session_id).await.unwrap();
        assert_eq!(remaining.len(), 2);

        // Verify the checkpoint is gone
        let loaded = manager.load_checkpoint(&session_id, &"chk-0".to_string()).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_checkpoint_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session_id = "test-session".to_string();

        // Create 5 checkpoints
        for i in 0..5 {
            let checkpoint_id = format!("chk-{}", i);
            let state = ConversationState::new(&session_id, &checkpoint_id);
            manager.save_checkpoint(&state).await.unwrap();
            // Small delay to ensure different timestamps
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Verify all exist
        let checkpoints = manager.list_checkpoints(&session_id).await.unwrap();
        assert_eq!(checkpoints.len(), 5);

        // Cleanup, keeping only 2
        let deleted = manager
            .cleanup_old_checkpoints(&session_id, 2)
            .await
            .unwrap();
        assert_eq!(deleted, 3);

        // Verify only 2 remain
        let remaining = manager.list_checkpoints(&session_id).await.unwrap();
        assert_eq!(remaining.len(), 2);
    }

    #[tokio::test]
    async fn test_checkpoint_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await.unwrap();

        let session_id = "test-session".to_string();
        let checkpoint_id = "chk-meta".to_string();

        let state = ConversationState::new(&session_id, &checkpoint_id)
            .with_description("Metadata test")
            .with_messages(vec![
                serde_json::json!({"role": "user", "content": "hello"}),
                serde_json::json!({"role": "assistant", "content": "hi"}),
            ])
            .with_turn_count(10);

        manager.save_checkpoint(&state).await.unwrap();

        // Get metadata via listing
        let checkpoints = manager.list_checkpoints(&session_id).await.unwrap();
        assert_eq!(checkpoints.len(), 1);

        let meta = &checkpoints[0];
        assert_eq!(meta.id, checkpoint_id);
        assert_eq!(meta.session_id, session_id);
        assert_eq!(meta.description, Some("Metadata test".to_string()));
        assert_eq!(meta.message_count, 2);
        assert_eq!(meta.turn_count, 10);
    }
}
