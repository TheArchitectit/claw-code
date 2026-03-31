//! Handle types for agents and teams.

use super::types::{AgentId, AgentState, WorkAssignment, TeamId};

/// Agent handle type.
#[derive(Debug, Clone)]
pub struct AgentHandle {
    /// The ID of the agent
    id: AgentId,
}

impl AgentHandle {
    /// Create a new agent handle
    pub fn new(id: AgentId) -> Self {
        Self { id }
    }

    /// Get the agent ID
    pub fn id(&self) -> AgentId {
        self.id
    }

    /// Get the state of the agent
    pub async fn state(&self) -> AgentState {
        AgentState::default()
    }

    /// Assign work to the agent
    pub async fn assign_work(
        &self,
        _work_id: String,
        _work: WorkAssignment,
    ) -> Result<(), String> {
        Ok(())
    }
}

/// Team handle type.
#[derive(Debug, Clone)]
pub struct TeamHandle {
    /// The ID of the team
    pub id: TeamId,
}
