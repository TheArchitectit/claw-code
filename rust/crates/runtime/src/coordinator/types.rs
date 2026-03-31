//! Core types for coordinator integration.

use serde_json::Value;

/// Agent ID type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AgentId(pub uuid::Uuid);

impl AgentId {
    /// Create a new agent ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
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

/// Team ID type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TeamId(pub uuid::Uuid);

impl TeamId {
    /// Create a new team ID
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for TeamId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TeamId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Capability type.
#[derive(Debug, Clone)]
pub struct Capability {
    /// The name of the capability
    pub name: String,
    /// The description of the capability
    pub description: String,
}

impl Capability {
    /// Create a new capability
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Agent state type.
#[derive(Debug, Clone, Default)]
pub struct AgentState {
    /// The status of the agent
    pub status: AgentStatus,
}

impl AgentState {
    /// Check if the agent is active
    pub fn is_active(&self) -> bool {
        matches!(self.status, AgentStatus::Active | AgentStatus::Busy)
    }
}

/// Agent status enum.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AgentStatus {
    /// Agent is starting
    Starting,
    /// Agent is active
    #[default]
    Active,
    /// Agent is busy
    Busy,
    /// Agent is stopping
    Stopping,
    /// Agent is stopped
    Stopped,
}

/// Message priority enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagePriority {
    /// Low priority
    Low = 1,
    /// Normal priority
    Normal = 5,
    /// High priority
    High = 10,
}

impl Default for MessagePriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Inter-agent message type.
#[derive(Debug, Clone)]
pub struct InterAgentMessage {
    /// The sender ID
    pub sender: AgentId,
    /// The recipient ID (None for broadcast)
    pub recipient: Option<AgentId>,
    /// The payload
    pub payload: Value,
    /// The priority
    pub priority: MessagePriority,
}

impl InterAgentMessage {
    /// Create a direct message
    pub fn direct(
        sender: AgentId,
        recipient: AgentId,
        payload: Value,
    ) -> Self {
        Self {
            sender,
            recipient: Some(recipient),
            payload,
            priority: MessagePriority::Normal,
        }
    }

    /// Create a team message
    pub fn team(
        sender: AgentId,
        _team_id: TeamId,
        payload: Value,
    ) -> Self {
        Self {
            sender,
            recipient: None,
            payload,
            priority: MessagePriority::Normal,
        }
    }

    /// Create a broadcast message
    pub fn broadcast(sender: AgentId, payload: Value) -> Self {
        Self {
            sender,
            recipient: None,
            payload,
            priority: MessagePriority::Normal,
        }
    }

    /// Set the priority
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Work assignment type.
#[derive(Debug, Clone)]
pub struct WorkAssignment {
    /// The description of the work
    pub description: String,
    /// The context for the work
    pub context: Value,
    /// The priority
    pub priority: u8,
    /// The deadline
    pub deadline: Option<chrono::DateTime<chrono::Utc>>,
}

/// Coordinator integration errors.
#[derive(Debug, Clone)]
pub enum CoordinatorIntegrationError {
    /// Not registered as an agent.
    NotRegistered,
    /// No team context set.
    NoTeamContext,
    /// Invalid ID format.
    InvalidId(String),
    /// Message bus error.
    MessageBusError(String),
    /// Agent spawn failed.
    AgentSpawnFailed(String),
    /// Agent termination failed.
    AgentTerminationFailed(String),
    /// Agent not found.
    AgentNotFound(String),
    /// Work assignment failed.
    WorkAssignmentFailed(String),
    /// Team creation failed.
    TeamCreationFailed(String),
}

impl std::fmt::Display for CoordinatorIntegrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinatorIntegrationError::NotRegistered => write!(f, "Not registered as an agent"),
            CoordinatorIntegrationError::NoTeamContext => write!(f, "No team context set"),
            CoordinatorIntegrationError::InvalidId(s) => write!(f, "Invalid ID: {}", s),
            CoordinatorIntegrationError::MessageBusError(s) => write!(f, "Message bus error: {}", s),
            CoordinatorIntegrationError::AgentSpawnFailed(s) => write!(f, "Agent spawn failed: {}", s),
            CoordinatorIntegrationError::AgentTerminationFailed(s) => write!(f, "Agent termination failed: {}", s),
            CoordinatorIntegrationError::AgentNotFound(s) => write!(f, "Agent not found: {}", s),
            CoordinatorIntegrationError::WorkAssignmentFailed(s) => write!(f, "Work assignment failed: {}", s),
            CoordinatorIntegrationError::TeamCreationFailed(s) => write!(f, "Team creation failed: {}", s),
        }
    }
}

impl std::error::Error for CoordinatorIntegrationError {}

/// Extension trait for AgentStatus to check if agent is active.
pub trait AgentStatusExt {
    /// Check if the agent status indicates it is active.
    fn is_active(&self) -> bool;
}

impl AgentStatusExt for AgentStatus {
    fn is_active(&self) -> bool {
        matches!(self, AgentStatus::Active | AgentStatus::Busy)
    }
}
