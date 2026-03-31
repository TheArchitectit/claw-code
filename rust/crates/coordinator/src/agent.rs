//! Agent lifecycle management.
//!
//! This module provides the core agent abstraction, handling:
//! - Agent spawning and initialization
//! - Health monitoring and heartbeats
//! - Graceful termination
//! - State management and persistence
//! - Reconnection handling

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Unique identifier for an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub Uuid);

impl AgentId {
    /// Generate a new unique agent ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Human-readable name for the agent.
    pub name: String,

    /// Model to use for this agent.
    pub model: String,

    /// Agent capabilities.
    pub capabilities: Vec<Capability>,

    /// Maximum number of retries on failure.
    pub max_retries: u32,

    /// Timeout for agent operations.
    pub timeout_seconds: u64,

    /// Heartbeat interval in seconds.
    pub heartbeat_interval_seconds: u64,

    /// Reconnection policy.
    pub reconnection_policy: ReconnectionPolicy,

    /// Whether the agent should auto-restart on failure.
    pub auto_restart: bool,

    /// Maximum memory (in MB) the agent can use.
    pub max_memory_mb: u64,

    /// Environment variables for the agent.
    pub environment: HashMap<String, String>,

    /// System prompt override.
    pub system_prompt: Option<String>,
}

impl AgentConfig {
    /// Create a new agent config with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            model: "claude-3-5-sonnet".to_string(),
            capabilities: Vec::new(),
            max_retries: 3,
            timeout_seconds: 300,
            heartbeat_interval_seconds: 30,
            reconnection_policy: ReconnectionPolicy::default(),
            auto_restart: true,
            max_memory_mb: 512,
            environment: HashMap::new(),
            system_prompt: None,
        }
    }

    /// Set the model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Add a capability.
    #[must_use]
    pub fn with_capability(mut self, capability: Capability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Set max retries.
    #[must_use]
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set timeout.
    #[must_use]
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout_seconds = seconds;
        self
    }

    /// Set reconnection policy.
    #[must_use]
    pub fn with_reconnection_policy(mut self, policy: ReconnectionPolicy) -> Self {
        self.reconnection_policy = policy;
        self
    }
}

/// Agent capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Capability {
    /// Can read files.
    FileRead,
    /// Can write files.
    FileWrite,
    /// Can execute bash commands.
    BashExecute,
    /// Can search code.
    CodeSearch,
    /// Can edit code.
    CodeEdit,
    /// Can use the web.
    WebAccess,
    /// Can spawn child agents.
    AgentSpawning,
    /// Can coordinate teams.
    TeamCoordination,
    /// Custom capability.
    Custom(String),
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::FileRead => write!(f, "file_read"),
            Capability::FileWrite => write!(f, "file_write"),
            Capability::BashExecute => write!(f, "bash_execute"),
            Capability::CodeSearch => write!(f, "code_search"),
            Capability::CodeEdit => write!(f, "code_edit"),
            Capability::WebAccess => write!(f, "web_access"),
            Capability::AgentSpawning => write!(f, "agent_spawning"),
            Capability::TeamCoordination => write!(f, "team_coordination"),
            Capability::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Reconnection policy for agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconnectionPolicy {
    /// Maximum number of reconnection attempts.
    pub max_attempts: u32,
    /// Initial backoff delay in milliseconds.
    pub initial_backoff_ms: u64,
    /// Maximum backoff delay in milliseconds.
    pub max_backoff_ms: u64,
    /// Backoff multiplier.
    pub backoff_multiplier: f64,
}

impl Default for ReconnectionPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_backoff_ms: 1000,
            max_backoff_ms: 60000,
            backoff_multiplier: 2.0,
        }
    }
}

/// Agent status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    /// Agent is starting up.
    Starting,
    /// Agent is active and ready.
    Active,
    /// Agent is currently processing work.
    Busy,
    /// Agent is reconnecting after a failure.
    Reconnecting,
    /// Agent is shutting down.
    ShuttingDown,
    /// Agent has terminated.
    Terminated,
    /// Agent failed and cannot recover.
    Failed,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Starting => write!(f, "starting"),
            AgentStatus::Active => write!(f, "active"),
            AgentStatus::Busy => write!(f, "busy"),
            AgentStatus::Reconnecting => write!(f, "reconnecting"),
            AgentStatus::ShuttingDown => write!(f, "shutting_down"),
            AgentStatus::Terminated => write!(f, "terminated"),
            AgentStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Agent state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Current status.
    pub status: AgentStatus,
    /// When the agent was created.
    pub created_at: DateTime<Utc>,
    /// When the agent was last active.
    pub last_active: DateTime<Utc>,
    /// Number of tasks completed.
    pub tasks_completed: u64,
    /// Number of tasks failed.
    pub tasks_failed: u64,
    /// Current task ID if busy.
    pub current_task: Option<String>,
    /// Memory usage in MB.
    pub memory_usage_mb: u64,
    /// CPU usage percentage.
    pub cpu_usage_percent: f32,
    /// Last error message if any.
    pub last_error: Option<String>,
    /// Reconnection attempt count.
    pub reconnection_attempts: u32,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            status: AgentStatus::Starting,
            created_at: Utc::now(),
            last_active: Utc::now(),
            tasks_completed: 0,
            tasks_failed: 0,
            current_task: None,
            memory_usage_mb: 0,
            cpu_usage_percent: 0.0,
            last_error: None,
            reconnection_attempts: 0,
        }
    }
}

/// Handle to a running agent.
pub struct AgentHandle {
    /// Agent ID.
    pub id: AgentId,
    /// Agent configuration.
    pub config: AgentConfig,
    /// Channel to send commands to the agent.
    command_tx: mpsc::UnboundedSender<AgentCommand>,
    /// Current state (shared).
    state: Arc<RwLock<AgentState>>,
    /// Task handle for the agent loop.
    task_handle: Option<JoinHandle<()>>,
}

impl Clone for AgentHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            config: self.config.clone(),
            command_tx: self.command_tx.clone(),
            state: Arc::clone(&self.state),
            task_handle: None,
        }
    }
}

impl AgentHandle {
    /// Get the agent ID.
    #[must_use]
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Get the current state.
    pub async fn state(&self) -> AgentState {
        self.state.read().await.clone()
    }

    /// Check if the agent is alive.
    pub async fn is_alive(&self) -> bool {
        let state = self.state.read().await;
        matches!(
            state.status,
            AgentStatus::Starting | AgentStatus::Active | AgentStatus::Busy | AgentStatus::Reconnecting
        )
    }

    /// Request graceful shutdown.
    pub async fn shutdown(&mut self) -> Result<(), AgentError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(AgentCommand::Shutdown { respond_to: tx })
            .map_err(|_| AgentError::CommandFailed)?;

        rx.await.map_err(|_| AgentError::CommandFailed)
    }

    /// Terminate the agent immediately.
    pub async fn terminate(&mut self) {
        if let Some(handle) = self.task_handle.take() {
            handle.abort();
        }

        let mut state = self.state.write().await;
        state.status = AgentStatus::Terminated;
    }

    /// Send a ping to check agent health.
    pub async fn ping(&self) -> Result<Duration, AgentError> {
        let start = Instant::now();
        let (tx, rx) = oneshot::channel();

        self.command_tx
            .send(AgentCommand::Ping { respond_to: tx })
            .map_err(|_| AgentError::CommandFailed)?;

        rx.await.map_err(|_| AgentError::CommandFailed)?;
        Ok(start.elapsed())
    }

    /// Assign work to the agent.
    pub async fn assign_work(
        &self,
        task_id: String,
        work: WorkAssignment,
    ) -> Result<(), AgentError> {
        let (tx, rx) = oneshot::channel();
        self.command_tx
            .send(AgentCommand::AssignWork { task_id, work, respond_to: tx })
            .map_err(|_| AgentError::CommandFailed)?;
        rx.await.map_err(|_| AgentError::CommandFailed)
    }
}

/// Work assignment for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkAssignment {
    /// Task description.
    pub description: String,
    /// Context data.
    pub context: serde_json::Value,
    /// Priority level (1-10, 10 being highest).
    pub priority: u8,
    /// Maximum time allowed for completion.
    pub deadline: Option<DateTime<Utc>>,
}

/// Internal agent command.
enum AgentCommand {
    Shutdown {
        respond_to: oneshot::Sender<()>,
    },
    Ping {
        respond_to: oneshot::Sender<()>,
    },
    AssignWork {
        task_id: String,
        work: WorkAssignment,
        respond_to: oneshot::Sender<()>,
    },
    GetState {
        respond_to: oneshot::Sender<AgentState>,
    },
}

/// Agent errors.
#[derive(Error, Debug, Clone)]
pub enum AgentError {
    /// Agent initialization failed.
    #[error("Agent initialization failed: {0}")]
    InitializationFailed(String),

    /// Command failed to send/receive.
    #[error("Command failed")]
    CommandFailed,

    /// Agent is not responding.
    #[error("Agent not responding")]
    NotResponding,

    /// Max reconnection attempts exceeded.
    #[error("Max reconnection attempts exceeded")]
    MaxReconnectionAttempts,

    /// Agent is in invalid state for operation.
    #[error("Invalid agent state: {current}, expected: {expected}")]
    InvalidState {
        current: AgentStatus,
        expected: String,
    },

    /// Agent timed out.
    #[error("Agent timed out after {seconds}s")]
    Timeout {
        seconds: u64,
    },
}

/// Agent lifecycle manager.
pub struct AgentLifecycle {
    /// Active agents.
    agents: DashMap<AgentId, AgentHandle>,
    /// Agent state persistence.
    state_store: Arc<dyn AgentStateStore>,
}

impl AgentLifecycle {
    /// Create a new agent lifecycle manager.
    #[must_use]
    pub fn new(state_store: Arc<dyn AgentStateStore>) -> Self {
        Self {
            agents: DashMap::new(),
            state_store,
        }
    }

    /// Spawn a new agent.
    pub async fn spawn(&self, config: AgentConfig) -> Result<AgentHandle, AgentError> {
        let id = AgentId::new();
        let state = Arc::new(RwLock::new(AgentState::default()));
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();

        // Start the agent task
        let task_state = Arc::clone(&state);
        let task_handle = tokio::spawn(async move {
            // Set agent as active
            {
                let mut s = task_state.write().await;
                s.status = AgentStatus::Active;
            }

            // Agent main loop
            loop {
                tokio::select! {
                    Some(cmd) = command_rx.recv() => {
                        match cmd {
                            AgentCommand::Shutdown { respond_to } => {
                                let mut s = task_state.write().await;
                                s.status = AgentStatus::ShuttingDown;
                                let _ = respond_to.send(());
                                break;
                            }
                            AgentCommand::Ping { respond_to } => {
                                let mut s = task_state.write().await;
                                s.last_active = Utc::now();
                                let _ = respond_to.send(());
                            }
                            AgentCommand::AssignWork { task_id, work, respond_to } => {
                                let mut s = task_state.write().await;
                                s.status = AgentStatus::Busy;
                                s.current_task = Some(task_id);
                                // Work would be processed here
                                tracing::info!("Agent assigned work: {:?}", work);
                                let _ = respond_to.send(());
                            }
                            AgentCommand::GetState { respond_to } => {
                                let s = task_state.read().await;
                                let _ = respond_to.send(s.clone());
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        // Periodic heartbeat/update
                        let mut s = task_state.write().await;
                        s.last_active = Utc::now();
                    }
                }
            }

            // Mark as terminated
            let mut s = task_state.write().await;
            s.status = AgentStatus::Terminated;
        });

        let handle = AgentHandle {
            id,
            config: config.clone(),
            command_tx,
            state,
            task_handle: Some(task_handle),
        };

        self.agents.insert(id, handle);
        Ok(self.agents.get(&id).unwrap().clone())
    }

    /// Get an agent by ID.
    #[must_use]
    pub fn get(&self, id: AgentId) -> Option<AgentHandle> {
        self.agents.get(&id).map(|h| h.clone())
    }

    /// List all active agents.
    pub async fn list_active(&self) -> Vec<AgentId> {
        let mut active = Vec::new();
        for e in self.agents.iter() {
            let state = e.value().state.read().await;
            if matches!(
                state.status,
                AgentStatus::Starting | AgentStatus::Active | AgentStatus::Busy
            ) {
                active.push(*e.key());
            }
        }
        active
    }

    /// Terminate an agent.
    pub async fn terminate(&self, id: AgentId) -> Result<(), AgentError> {
        if let Some((_, mut handle)) = self.agents.remove(&id) {
            handle.terminate().await;
        }
        Ok(())
    }

    /// Get count of active agents.
    pub fn active_count(&self) -> usize {
        self.agents.len()
    }

    /// Shutdown all agents gracefully.
    pub async fn shutdown_all(&self) {
        let handles: Vec<_> = self
            .agents
            .iter_mut()
            .map(|mut e| (e.key().clone(), e.value().clone()))
            .collect();

        for (id, mut handle) in handles {
            tracing::info!("Shutting down agent {}", id);
            if let Err(e) = handle.shutdown().await {
                tracing::warn!("Failed to gracefully shutdown agent {}: {}", id, e);
                handle.terminate().await;
            }
        }

        self.agents.clear();
    }
}

/// Trait for agent state persistence.
#[async_trait::async_trait]
pub trait AgentStateStore: Send + Sync {
    /// Save agent state.
    async fn save(&self, id: AgentId, state: &AgentState) -> Result<(), AgentError>;
    /// Load agent state.
    async fn load(&self, id: AgentId) -> Result<Option<AgentState>, AgentError>;
    /// Delete agent state.
    async fn delete(&self, id: AgentId) -> Result<(), AgentError>;
}

/// In-memory agent state store.
pub struct InMemoryStateStore {
    states: DashMap<AgentId, AgentState>,
}

impl InMemoryStateStore {
    /// Create a new in-memory state store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentStateStore for InMemoryStateStore {
    async fn save(&self, id: AgentId, state: &AgentState) -> Result<(), AgentError> {
        self.states.insert(id, state.clone());
        Ok(())
    }

    async fn load(&self, id: AgentId) -> Result<Option<AgentState>, AgentError> {
        Ok(self.states.get(&id).map(|s| s.clone()))
    }

    async fn delete(&self, id: AgentId) -> Result<(), AgentError> {
        self.states.remove(&id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_id_generation() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
    }

    #[tokio::test]
    async fn test_agent_config_builder() {
        let config = AgentConfig::new("test-agent")
            .with_model("claude-3-opus")
            .with_capability(Capability::FileRead)
            .with_max_retries(5);

        assert_eq!(config.name, "test-agent");
        assert_eq!(config.model, "claude-3-opus");
        assert_eq!(config.capabilities.len(), 1);
        assert_eq!(config.max_retries, 5);
    }

    #[tokio::test]
    async fn test_agent_lifecycle_spawn() {
        let store = Arc::new(InMemoryStateStore::new());
        let lifecycle = AgentLifecycle::new(store);

        let config = AgentConfig::new("test-agent");
        let handle = lifecycle.spawn(config).await.unwrap();

        assert!(lifecycle.get(handle.id()).is_some());
        assert!(handle.is_alive().await);

        // Cleanup
        lifecycle.terminate(handle.id()).await.unwrap();
    }

    #[tokio::test]
    async fn test_agent_ping() {
        let store = Arc::new(InMemoryStateStore::new());
        let lifecycle = AgentLifecycle::new(store);

        let config = AgentConfig::new("test-agent");
        let handle = lifecycle.spawn(config).await.unwrap();

        let latency = handle.ping().await.unwrap();
        assert!(latency.as_millis() < 100); // Should be very fast

        // Cleanup
        lifecycle.terminate(handle.id()).await.unwrap();
    }

    #[tokio::test]
    async fn test_agent_shutdown() {
        let store = Arc::new(InMemoryStateStore::new());
        let lifecycle = AgentLifecycle::new(store);

        let config = AgentConfig::new("test-agent");
        let mut handle = lifecycle.spawn(config).await.unwrap();

        handle.shutdown().await.unwrap();

        // Give it a moment to shut down
        tokio::time::sleep(Duration::from_millis(100)).await;

        let state = handle.state().await;
        assert_eq!(state.status, AgentStatus::Terminated);
    }

    #[tokio::test]
    async fn test_agent_work_assignment() {
        let store = Arc::new(InMemoryStateStore::new());
        let lifecycle = AgentLifecycle::new(store);

        let config = AgentConfig::new("test-agent");
        let handle = lifecycle.spawn(config).await.unwrap();

        let work = WorkAssignment {
            description: "Test task".to_string(),
            context: serde_json::json!({}),
            priority: 5,
            deadline: None,
        };

        handle
            .assign_work("task-123".to_string(), work)
            .await
            .unwrap();

        // Check agent became busy
        let state = handle.state().await;
        assert_eq!(state.status, AgentStatus::Busy);
        assert_eq!(state.current_task, Some("task-123".to_string()));

        // Cleanup
        lifecycle.terminate(handle.id()).await.unwrap();
    }

    #[tokio::test]
    async fn test_reconnection_policy_default() {
        let policy = ReconnectionPolicy::default();
        assert_eq!(policy.max_attempts, 5);
        assert_eq!(policy.initial_backoff_ms, 1000);
        assert_eq!(policy.max_backoff_ms, 60000);
        assert_eq!(policy.backoff_multiplier, 2.0);
    }

    #[tokio::test]
    async fn test_agent_status_display() {
        assert_eq!(AgentStatus::Active.to_string(), "active");
        assert_eq!(AgentStatus::Busy.to_string(), "busy");
        assert_eq!(AgentStatus::Terminated.to_string(), "terminated");
    }
}
