//! Swarm orchestration for multi-agent coordination.
//!
//! This module provides the high-level Coordinator API that ties together
//! agent lifecycle, team orchestration, messaging, and work distribution.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::agent::{
    AgentConfig, AgentHandle, AgentId, AgentLifecycle, AgentState, InMemoryStateStore,
};
use crate::message_bus::MessageBus;
use crate::result_aggregator::ResultAggregator;
use crate::team::{TeamConfig, TeamHandle, TeamId, TeamOrchestrator, TeamState};
use crate::work_distribution::TaskDistributor;

/// Coordinator configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinatorConfig {
    /// Maximum number of agents.
    pub max_agents: usize,
    /// Maximum number of teams.
    pub max_teams: usize,
    /// Default team size.
    pub default_team_size: usize,
    /// Enable auto-scaling.
    pub auto_scaling: bool,
    /// Agent heartbeat timeout in seconds.
    pub heartbeat_timeout_seconds: u64,
    /// Message retention period in hours.
    pub message_retention_hours: u64,
}

impl Default for CoordinatorConfig {
    fn default() -> Self {
        Self {
            max_agents: 100,
            max_teams: 20,
            default_team_size: 5,
            auto_scaling: true,
            heartbeat_timeout_seconds: 60,
            message_retention_hours: 24,
        }
    }
}

impl CoordinatorConfig {
    /// Create default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set max agents.
    #[must_use]
    pub fn with_max_agents(mut self, max: usize) -> Self {
        self.max_agents = max;
        self
    }

    /// Set max teams.
    #[must_use]
    pub fn with_max_teams(mut self, max: usize) -> Self {
        self.max_teams = max;
        self
    }
}

/// Swarm state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmState {
    /// Active agent count.
    pub active_agents: usize,
    /// Active team count.
    pub active_teams: usize,
    /// Pending work units.
    pub pending_work: usize,
    /// Completed work units.
    pub completed_work: u64,
    /// Failed work units.
    pub failed_work: u64,
    /// Last updated.
    pub last_updated: DateTime<Utc>,
}

/// Swarm event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwarmEvent {
    /// Agent spawned.
    AgentSpawned { agent_id: AgentId, config: AgentConfig },
    /// Agent terminated.
    AgentTerminated { agent_id: AgentId, reason: String },
    /// Agent failed.
    AgentFailed { agent_id: AgentId, error: String },
    /// Team created.
    TeamCreated { team_id: TeamId, config: TeamConfig },
    /// Team disbanded.
    TeamDisbanded { team_id: TeamId },
    /// Work assigned.
    WorkAssigned { work_id: String, agent_id: Option<AgentId> },
    /// Work completed.
    WorkCompleted { work_id: String, agent_id: AgentId },
    /// Work failed.
    WorkFailed { work_id: String, error: String },
    /// Message sent.
    MessageSent { message_id: crate::message_bus::MessageId, from: AgentId, to: Option<AgentId> },
    /// Agent reconnected.
    AgentReconnected { agent_id: AgentId },
    /// Swarm rebalancing.
    SwarmRebalancing { reason: String },
}

/// Swarm metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SwarmMetrics {
    /// Total agents spawned (lifetime).
    pub total_agents_spawned: u64,
    /// Total agents terminated (lifetime).
    pub total_agents_terminated: u64,
    /// Total teams created (lifetime).
    pub total_teams_created: u64,
    /// Total work units processed.
    pub total_work_units: u64,
    /// Average agent lifetime in seconds.
    pub avg_agent_lifetime_seconds: f64,
    /// Average work completion time in seconds.
    pub avg_work_completion_seconds: f64,
    /// Message throughput (messages per second).
    pub message_throughput: f64,
}

/// The main Coordinator for multi-agent swarm orchestration.
pub struct Coordinator {
    /// Configuration.
    pub config: CoordinatorConfig,
    /// Agent lifecycle manager.
    pub agent_lifecycle: Arc<AgentLifecycle>,
    /// Team orchestrator.
    pub team_orchestrator: Arc<TeamOrchestrator>,
    /// Message bus for inter-agent communication.
    pub message_bus: Arc<MessageBus>,
    /// Event subscribers.
    event_subscribers: Arc<RwLock<Vec<tokio::sync::mpsc::UnboundedSender<SwarmEvent>>>>,
    /// Swarm metrics.
    metrics: Arc<RwLock<SwarmMetrics>>,
    /// Current state.
    state: Arc<RwLock<SwarmState>>,
}

impl Coordinator {
    /// Create a new coordinator with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CoordinatorConfig::default())
    }

    /// Create a new coordinator with custom configuration.
    #[must_use]
    pub fn with_config(config: CoordinatorConfig) -> Self {
        let store = Arc::new(InMemoryStateStore::new());
        let agent_lifecycle = Arc::new(AgentLifecycle::new(store));
        let message_bus = Arc::new(MessageBus::new());
        let team_orchestrator = Arc::new(TeamOrchestrator::new(
            Arc::clone(&agent_lifecycle),
            Arc::clone(&message_bus),
        ));

        let state = Arc::new(RwLock::new(SwarmState {
            active_agents: 0,
            active_teams: 0,
            pending_work: 0,
            completed_work: 0,
            failed_work: 0,
            last_updated: Utc::now(),
        }));

        Self {
            config,
            agent_lifecycle,
            team_orchestrator,
            message_bus,
            event_subscribers: Arc::new(RwLock::new(Vec::new())),
            metrics: Arc::new(RwLock::new(SwarmMetrics::default())),
            state,
        }
    }

    /// Spawn a new agent.
    pub async fn spawn_agent(&self, config: AgentConfig) -> Result<AgentHandle, SwarmError> {
        // Check max agents limit
        let active_count = self.agent_lifecycle.active_count();
        if active_count >= self.config.max_agents {
            return Err(SwarmError::MaxAgentsExceeded {
                max: self.config.max_agents,
            });
        }

        let handle = self.agent_lifecycle.spawn(config.clone()).await.map_err(|e| {
            SwarmError::AgentSpawnFailed(format!("{:?}", e))
        })?;

        // Register agent with message bus
        self.message_bus.register_agent(handle.id());

        // Update state
        {
            let mut state = self.state.write().await;
            state.active_agents += 1;
            state.last_updated = Utc::now();
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_agents_spawned += 1;
        }

        // Emit event
        self.emit_event(SwarmEvent::AgentSpawned {
            agent_id: handle.id(),
            config,
        }).await;

        tracing::info!("Spawned agent {} (total: {})", handle.id(), active_count + 1);

        Ok(handle)
    }

    /// Terminate an agent.
    pub async fn terminate_agent(&self, agent_id: AgentId) -> Result<(), SwarmError> {
        self.agent_lifecycle.terminate(agent_id).await.map_err(|e| {
            SwarmError::AgentTerminationFailed(format!("{:?}", e))
        })?;

        // Unregister from message bus
        self.message_bus.unregister_agent(agent_id);

        // Update state
        {
            let mut state = self.state.write().await;
            state.active_agents = state.active_agents.saturating_sub(1);
            state.last_updated = Utc::now();
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_agents_terminated += 1;
        }

        // Emit event
        self.emit_event(SwarmEvent::AgentTerminated {
            agent_id,
            reason: "manual_termination".to_string(),
        }).await;

        tracing::info!("Terminated agent {}", agent_id);

        Ok(())
    }

    /// Create a new team.
    pub async fn create_team(&self, config: TeamConfig) -> Result<TeamHandle, SwarmError> {
        // Check max teams limit
        let team_count = self.team_orchestrator.team_count();
        if team_count >= self.config.max_teams {
            return Err(SwarmError::MaxTeamsExceeded {
                max: self.config.max_teams,
            });
        }

        let handle = self.team_orchestrator.create_team(config.clone()).await.map_err(|e| {
            SwarmError::TeamCreationFailed(format!("{:?}", e))
        })?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.active_teams += 1;
            state.last_updated = Utc::now();
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_teams_created += 1;
        }

        // Emit event
        self.emit_event(SwarmEvent::TeamCreated {
            team_id: handle.id,
            config,
        }).await;

        tracing::info!("Created team {} (total: {})", handle.id, team_count + 1);

        Ok(handle)
    }

    /// Disband a team.
    pub async fn disband_team(&self, team_id: TeamId) -> Result<(), SwarmError> {
        self.team_orchestrator.disband_team(team_id).await.map_err(|e| {
            SwarmError::TeamDisbandFailed(format!("{:?}", e))
        })?;

        // Update state
        {
            let mut state = self.state.write().await;
            state.active_teams = state.active_teams.saturating_sub(1);
            state.last_updated = Utc::now();
        }

        // Emit event
        self.emit_event(SwarmEvent::TeamDisbanded { team_id }).await;

        tracing::info!("Disbanded team {}", team_id);

        Ok(())
    }

    /// Get agent by ID.
    #[must_use]
    pub fn get_agent(&self, agent_id: AgentId) -> Option<AgentHandle> {
        self.agent_lifecycle.get(agent_id)
    }

    /// Get team by ID.
    #[must_use]
    pub fn get_team(&self, team_id: TeamId) -> Option<TeamHandle> {
        self.team_orchestrator.get_team(team_id)
    }

    /// List all active agents.
    pub async fn list_agents(&self) -> Vec<AgentId> {
        self.agent_lifecycle.list_active().await
    }

    /// List all teams.
    pub fn list_teams(&self) -> Vec<TeamId> {
        self.team_orchestrator.list_teams()
    }

    /// Get current swarm state.
    pub async fn state(&self) -> SwarmState {
        self.state.read().await.clone()
    }

    /// Get swarm metrics.
    pub async fn metrics(&self) -> SwarmMetrics {
        self.metrics.read().await.clone()
    }

    /// Subscribe to swarm events.
    pub async fn subscribe_events(&self) -> tokio::sync::mpsc::UnboundedReceiver<SwarmEvent> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let mut subscribers = self.event_subscribers.write().await;
        subscribers.push(tx);
        rx
    }

    /// Emit an event to all subscribers.
    async fn emit_event(&self, event: SwarmEvent) {
        let subscribers = self.event_subscribers.read().await;
        for tx in subscribers.iter() {
            let _ = tx.send(event.clone());
        }
    }

    /// Shutdown the entire swarm gracefully.
    pub async fn shutdown(&self) {
        tracing::info!("Shutting down swarm...");

        // Disband all teams first
        for team_id in self.team_orchestrator.list_teams() {
            let _ = self.disband_team(team_id).await;
        }

        // Terminate all agents
        self.agent_lifecycle.shutdown_all().await;

        // Update state to reflect all agents terminated
        {
            let mut state = self.state.write().await;
            state.active_agents = 0;
            state.active_teams = 0;
            state.last_updated = Utc::now();
        }

        tracing::info!("Swarm shutdown complete");
    }

    /// Perform health check on the swarm.
    pub async fn health_check(&self) -> SwarmHealth {
        let mut healthy_agents = 0;
        let mut unhealthy_agents = 0;

        // Check each active agent
        for agent_id in self.list_agents().await {
            if let Some(agent) = self.get_agent(agent_id) {
                if let Ok(latency) = agent.ping().await {
                    if latency.as_secs() < self.config.heartbeat_timeout_seconds {
                        healthy_agents += 1;
                    } else {
                        unhealthy_agents += 1;
                    }
                } else {
                    unhealthy_agents += 1;
                }
            }
        }

        SwarmHealth {
            healthy_agents,
            unhealthy_agents,
            total_agents: healthy_agents + unhealthy_agents,
            timestamp: Utc::now(),
        }
    }
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
}

/// Swarm health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmHealth {
    /// Number of healthy agents.
    pub healthy_agents: usize,
    /// Number of unhealthy agents.
    pub unhealthy_agents: usize,
    /// Total agents checked.
    pub total_agents: usize,
    /// When the health check was performed.
    pub timestamp: DateTime<Utc>,
}

/// Swarm errors.
#[derive(thiserror::Error, Debug, Clone)]
pub enum SwarmError {
    /// Maximum agents exceeded.
    #[error("Maximum agents ({max}) exceeded")]
    MaxAgentsExceeded { max: usize },

    /// Maximum teams exceeded.
    #[error("Maximum teams ({max}) exceeded")]
    MaxTeamsExceeded { max: usize },

    /// Agent spawn failed.
    #[error("Agent spawn failed: {0}")]
    AgentSpawnFailed(String),

    /// Agent termination failed.
    #[error("Agent termination failed: {0}")]
    AgentTerminationFailed(String),

    /// Team creation failed.
    #[error("Team creation failed: {0}")]
    TeamCreationFailed(String),

    /// Team disband failed.
    #[error("Team disband failed: {0}")]
    TeamDisbandFailed(String),

    /// Agent not found.
    #[error("Agent {0} not found")]
    AgentNotFound(AgentId),

    /// Team not found.
    #[error("Team {0} not found")]
    TeamNotFound(TeamId),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Capability;

    #[tokio::test]
    async fn test_coordinator_creation() {
        let coordinator = Coordinator::new();

        assert_eq!(coordinator.config.max_agents, 100);
        assert_eq!(coordinator.config.max_teams, 20);
    }

    #[tokio::test]
    async fn test_coordinator_with_config() {
        let config = CoordinatorConfig::new()
            .with_max_agents(50)
            .with_max_teams(10);

        let coordinator = Coordinator::with_config(config);

        assert_eq!(coordinator.config.max_agents, 50);
        assert_eq!(coordinator.config.max_teams, 10);
    }

    #[tokio::test]
    async fn test_spawn_and_terminate_agent() {
        let coordinator = Coordinator::new();

        let config = AgentConfig::new("test-agent")
            .with_capability(Capability::FileRead);

        let agent = coordinator.spawn_agent(config).await.unwrap();
        let agent_id = agent.id();

        assert!(coordinator.get_agent(agent_id).is_some());

        let state = coordinator.state().await;
        assert_eq!(state.active_agents, 1);

        // Terminate
        coordinator.terminate_agent(agent_id).await.unwrap();

        assert!(coordinator.get_agent(agent_id).is_none());

        let state = coordinator.state().await;
        assert_eq!(state.active_agents, 0);
    }

    #[tokio::test]
    async fn test_create_and_disband_team() {
        let coordinator = Coordinator::new();

        let config = TeamConfig::new("test-team").with_max_agents(5);
        let team = coordinator.create_team(config).await.unwrap();
        let team_id = team.id;

        assert!(coordinator.get_team(team_id).is_some());

        let state = coordinator.state().await;
        assert_eq!(state.active_teams, 1);

        // Disband
        coordinator.disband_team(team_id).await.unwrap();

        assert!(coordinator.get_team(team_id).is_none());

        let state = coordinator.state().await;
        assert_eq!(state.active_teams, 0);
    }

    #[tokio::test]
    async fn test_max_agents_limit() {
        let config = CoordinatorConfig::new().with_max_agents(2);
        let coordinator = Coordinator::with_config(config);

        // Spawn two agents (should succeed)
        let _agent1 = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let _agent2 = coordinator
            .spawn_agent(AgentConfig::new("agent-2"))
            .await
            .unwrap();

        // Third agent should fail
        let result = coordinator.spawn_agent(AgentConfig::new("agent-3")).await;
        assert!(matches!(result, Err(SwarmError::MaxAgentsExceeded { max: 2 })));
    }

    #[tokio::test]
    async fn test_list_agents_and_teams() {
        let coordinator = Coordinator::new();

        // Spawn agents
        let agent1 = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let agent2 = coordinator
            .spawn_agent(AgentConfig::new("agent-2"))
            .await
            .unwrap();

        let agents = coordinator.list_agents().await;
        assert_eq!(agents.len(), 2);
        assert!(agents.contains(&agent1.id()));
        assert!(agents.contains(&agent2.id()));

        // Create teams
        let team1 = coordinator
            .create_team(TeamConfig::new("team-1"))
            .await
            .unwrap();
        let team2 = coordinator
            .create_team(TeamConfig::new("team-2"))
            .await
            .unwrap();

        let teams = coordinator.list_teams();
        assert_eq!(teams.len(), 2);
        assert!(teams.contains(&team1.id));
        assert!(teams.contains(&team2.id));
    }

    #[tokio::test]
    async fn test_swarm_events() {
        let coordinator = Coordinator::new();
        let mut events = coordinator.subscribe_events().await;

        // Spawn agent should emit event
        let agent = coordinator
            .spawn_agent(AgentConfig::new("test-agent"))
            .await
            .unwrap();

        // Check event received
        let event = events.recv().await.unwrap();
        match event {
            SwarmEvent::AgentSpawned { agent_id, .. } => {
                assert_eq!(agent_id, agent.id());
            }
            _ => panic!("Expected AgentSpawned event"),
        }
    }

    #[tokio::test]
    async fn test_swarm_metrics() {
        let coordinator = Coordinator::new();

        // Spawn some agents
        let _agent1 = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let _agent2 = coordinator
            .spawn_agent(AgentConfig::new("agent-2"))
            .await
            .unwrap();

        // Terminate one
        let agent3 = coordinator
            .spawn_agent(AgentConfig::new("agent-3"))
            .await
            .unwrap();
        coordinator.terminate_agent(agent3.id()).await.unwrap();

        let metrics = coordinator.metrics().await;
        assert_eq!(metrics.total_agents_spawned, 3);
        assert_eq!(metrics.total_agents_terminated, 1);
    }

    #[tokio::test]
    async fn test_health_check() {
        let coordinator = Coordinator::new();

        // Spawn agents
        let _agent1 = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let _agent2 = coordinator
            .spawn_agent(AgentConfig::new("agent-2"))
            .await
            .unwrap();

        let health = coordinator.health_check().await;
        assert_eq!(health.total_agents, 2);
        // Agents should be healthy (just spawned)
        assert_eq!(health.healthy_agents, 2);
        assert_eq!(health.unhealthy_agents, 0);
    }

    #[tokio::test]
    async fn test_swarm_shutdown() {
        let coordinator = Coordinator::new();

        // Spawn agents
        let _agent1 = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let _agent2 = coordinator
            .spawn_agent(AgentConfig::new("agent-2"))
            .await
            .unwrap();

        // Create team
        let _team = coordinator
            .create_team(TeamConfig::new("team-1"))
            .await
            .unwrap();

        // Shutdown
        coordinator.shutdown().await;

        let state = coordinator.state().await;
        assert_eq!(state.active_agents, 0);
        assert_eq!(state.active_teams, 0);
    }

    #[tokio::test]
    async fn test_swarm_state_updates() {
        let coordinator = Coordinator::new();

        let initial_state = coordinator.state().await;
        assert_eq!(initial_state.active_agents, 0);
        assert_eq!(initial_state.completed_work, 0);

        // Spawn agent updates state
        let _agent = coordinator
            .spawn_agent(AgentConfig::new("agent-1"))
            .await
            .unwrap();

        let state = coordinator.state().await;
        assert_eq!(state.active_agents, 1);
        assert!(state.last_updated >= initial_state.last_updated);
    }
}
