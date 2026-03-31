//! Configuration types for agents and teams.

use super::types::Capability;

/// Agent configuration type.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// The name of the agent
    pub name: String,
    /// The capabilities of the agent
    pub capabilities: Vec<Capability>,
}

impl AgentConfig {
    /// Create a new agent config
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            capabilities: Vec::new(),
        }
    }

    /// Set the capabilities
    pub fn with_capabilities(mut self, capabilities: Vec<Capability>) -> Self {
        self.capabilities = capabilities;
        self
    }
}

/// Team configuration type.
#[derive(Debug, Clone)]
pub struct TeamConfig {
    /// The name of the team
    pub name: String,
    /// The max number of agents
    pub max_agents: usize,
}

impl TeamConfig {
    /// Create a new team config
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            max_agents: 10,
        }
    }

    /// Set the max agents
    pub fn with_max_agents(mut self, max_agents: usize) -> Self {
        self.max_agents = max_agents;
        self
    }
}
