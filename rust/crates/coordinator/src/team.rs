//! Team orchestration for multi-agent coordination.
//!
//! This module provides team management capabilities, allowing multiple agents
//! to work together on complex tasks with various coordination strategies.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::agent::{AgentConfig, AgentHandle, AgentId, AgentLifecycle, WorkAssignment};
use crate::message_bus::MessageBus;
use crate::result_aggregator::{AggregationStrategy, ResultAggregator};
use crate::swarm::SwarmEvent;
use crate::work_distribution::{DistributionStrategy, TaskDistributor, WorkUnit};

/// Unique identifier for a team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TeamId(pub Uuid);

impl TeamId {
    /// Generate a new unique team ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
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

/// Team configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// Team name.
    pub name: String,
    /// Team description.
    pub description: String,
    /// Maximum number of agents.
    pub max_agents: usize,
    /// Work distribution strategy.
    pub distribution_strategy: DistributionStrategy,
    /// Result aggregation strategy.
    pub aggregation_strategy: AggregationStrategy,
    /// Team goal/objective.
    pub goal: String,
    /// Shared context for all team members.
    pub shared_context: serde_json::Value,
    /// Agent template for auto-scaling.
    pub agent_template: Option<AgentConfig>,
    /// Auto-scale enabled.
    pub auto_scale: bool,
    /// Minimum agents.
    pub min_agents: usize,
}

impl TeamConfig {
    /// Create a new team configuration.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            max_agents: 10,
            distribution_strategy: DistributionStrategy::RoundRobin,
            aggregation_strategy: AggregationStrategy::Concatenate,
            goal: String::new(),
            shared_context: serde_json::Value::Object(serde_json::Map::new()),
            agent_template: None,
            auto_scale: false,
            min_agents: 1,
        }
    }

    /// Set the team goal.
    #[must_use]
    pub fn with_goal(mut self, goal: impl Into<String>) -> Self {
        self.goal = goal.into();
        self
    }

    /// Set max agents.
    #[must_use]
    pub fn with_max_agents(mut self, max: usize) -> Self {
        self.max_agents = max;
        self
    }

    /// Set distribution strategy.
    #[must_use]
    pub fn with_distribution_strategy(mut self, strategy: DistributionStrategy) -> Self {
        self.distribution_strategy = strategy;
        self
    }

    /// Set aggregation strategy.
    #[must_use]
    pub fn with_aggregation_strategy(mut self, strategy: AggregationStrategy) -> Self {
        self.aggregation_strategy = strategy;
        self
    }

    /// Set agent template for auto-scaling.
    #[must_use]
    pub fn with_agent_template(mut self, template: AgentConfig) -> Self {
        self.agent_template = Some(template);
        self
    }

    /// Enable auto-scaling.
    #[must_use]
    pub fn with_auto_scale(mut self, enabled: bool) -> Self {
        self.auto_scale = enabled;
        self
    }
}

/// Team state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamState {
    /// Team ID.
    pub id: TeamId,
    /// Current status.
    pub status: TeamStatus,
    /// Agent members.
    pub members: Vec<AgentId>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Started timestamp.
    pub started_at: Option<DateTime<Utc>>,
    /// Completed timestamp.
    pub completed_at: Option<DateTime<Utc>>,
    /// Total work units assigned.
    pub work_units_assigned: u64,
    /// Total work units completed.
    pub work_units_completed: u64,
    /// Failed work units.
    pub work_units_failed: u64,
    /// Current strategy in use.
    pub current_strategy: String,
}

/// Team status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamStatus {
    /// Team is forming.
    Forming,
    /// Team is ready to work.
    Ready,
    /// Team is actively working.
    Working,
    /// Team is pausing.
    Pausing,
    /// Team has completed work.
    Completed,
    /// Team has been disbanded.
    Disbanded,
    /// Team encountered an error.
    Error,
}

impl std::fmt::Display for TeamStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TeamStatus::Forming => write!(f, "forming"),
            TeamStatus::Ready => write!(f, "ready"),
            TeamStatus::Working => write!(f, "working"),
            TeamStatus::Pausing => write!(f, "pausing"),
            TeamStatus::Completed => write!(f, "completed"),
            TeamStatus::Disbanded => write!(f, "disbanded"),
            TeamStatus::Error => write!(f, "error"),
        }
    }
}

/// Work strategy for team coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkStrategy {
    /// Divide work among agents.
    DivideAndConquer,
    /// Agents work sequentially.
    Sequential,
    /// All agents work on same task (voting).
    Voting,
    /// Leader assigns work to workers.
    LeaderWorker,
}

/// Round-robin work distribution.
pub struct RoundRobinStrategy {
    agents: Vec<AgentId>,
    current_index: std::sync::atomic::AtomicUsize,
}

impl RoundRobinStrategy {
    /// Create a new round-robin strategy.
    #[must_use]
    pub fn new(agents: Vec<AgentId>) -> Self {
        Self {
            agents,
            current_index: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// Get the next agent.
    pub fn next_agent(&self) -> Option<AgentId> {
        if self.agents.is_empty() {
            return None;
        }

        let index = self
            .current_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            % self.agents.len();

        Some(self.agents[index])
    }
}

/// Team handle for managing a team.
pub struct TeamHandle {
    /// Team ID.
    pub id: TeamId,
    /// Team configuration.
    pub config: TeamConfig,
    /// Current state.
    state: Arc<RwLock<TeamState>>,
    /// Agent lifecycle manager.
    lifecycle: Arc<AgentLifecycle>,
    /// Message bus for inter-agent communication.
    message_bus: Arc<MessageBus>,
    /// Result aggregator.
    result_aggregator: Arc<ResultAggregator>,
    /// Work queue.
    work_queue: Arc<RwLock<VecDeque<WorkUnit>>>,
}

impl Clone for TeamHandle {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            config: self.config.clone(),
            state: Arc::clone(&self.state),
            lifecycle: Arc::clone(&self.lifecycle),
            message_bus: Arc::clone(&self.message_bus),
            result_aggregator: Arc::clone(&self.result_aggregator),
            work_queue: Arc::clone(&self.work_queue),
        }
    }
}

impl TeamHandle {
    /// Add an agent to the team.
    pub async fn add_agent(&self, agent_id: AgentId) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        if state.members.len() >= self.config.max_agents {
            return Err(TeamError::TeamFull {
                max_size: self.config.max_agents,
            });
        }

        if state.members.contains(&agent_id) {
            return Err(TeamError::AgentAlreadyMember(agent_id));
        }

        state.members.push(agent_id);

        // Subscribe agent to team channel
        self.message_bus
            .subscribe_to_team(self.id, agent_id)
            .await;

        tracing::info!("Agent {} added to team {}", agent_id, self.id);
        Ok(())
    }

    /// Remove an agent from the team.
    pub async fn remove_agent(&self, agent_id: AgentId) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        if let Some(pos) = state.members.iter().position(|&id| id == agent_id) {
            state.members.remove(pos);

            // Unsubscribe agent from team channel
            self.message_bus
                .unsubscribe_from_team(self.id, agent_id)
                .await;

            tracing::info!("Agent {} removed from team {}", agent_id, self.id);
            Ok(())
        } else {
            Err(TeamError::AgentNotMember(agent_id))
        }
    }

    /// Get team state.
    pub async fn state(&self) -> TeamState {
        self.state.read().await.clone()
    }

    /// Start the team working.
    pub async fn start(&self) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        match state.status {
            TeamStatus::Forming | TeamStatus::Ready => {
                state.status = TeamStatus::Working;
                state.started_at = Some(Utc::now());
                tracing::info!("Team {} started working", self.id);
                Ok(())
            }
            _ => Err(TeamError::InvalidState {
                current: state.status,
                operation: "start".to_string(),
            }),
        }
    }

    /// Pause the team.
    pub async fn pause(&self) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        if state.status == TeamStatus::Working {
            state.status = TeamStatus::Pausing;
            Ok(())
        } else {
            Err(TeamError::InvalidState {
                current: state.status,
                operation: "pause".to_string(),
            })
        }
    }

    /// Resume the team.
    pub async fn resume(&self) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        if state.status == TeamStatus::Pausing {
            state.status = TeamStatus::Working;
            Ok(())
        } else {
            Err(TeamError::InvalidState {
                current: state.status,
                operation: "resume".to_string(),
            })
        }
    }

    /// Assign work to the team.
    pub async fn assign_work(&self, work: WorkUnit) -> Result<(), TeamError> {
        let state = self.state.read().await;

        if state.status != TeamStatus::Working {
            return Err(TeamError::InvalidState {
                current: state.status,
                operation: "assign_work".to_string(),
            });
        }

        let mut queue = self.work_queue.write().await;
        queue.push_back(work);

        drop(state);
        drop(queue);

        // Distribute work according to strategy
        self.distribute_work().await?;

        Ok(())
    }

    /// Distribute work to available agents.
    async fn distribute_work(&self) -> Result<(), TeamError> {
        let strategy = &self.config.distribution_strategy;
        let state = self.state.read().await;
        let agents = state.members.clone();
        drop(state);

        let distributor = TaskDistributor::new(strategy.clone());
        let mut queue = self.work_queue.write().await;

        while let Some(work) = queue.pop_front() {
            if let Some(agent_id) = distributor.select_agent(&agents, &work) {
                if let Some(agent) = self.lifecycle.get(agent_id) {
                    let assignment = WorkAssignment {
                        description: work.description.clone(),
                        context: work.context.clone(),
                        priority: work.priority,
                        deadline: work.deadline,
                    };

                    if let Err(e) = agent.assign_work(work.id.clone(), assignment).await {
                        tracing::warn!("Failed to assign work to agent {}: {}", agent_id, e);
                        // Put work back in queue
                        queue.push_front(work);
                        break;
                    }

                    let mut state = self.state.write().await;
                    state.work_units_assigned += 1;
                }
            }
        }

        Ok(())
    }

    /// Get the count of pending work units.
    pub async fn pending_work_count(&self) -> usize {
        self.work_queue.read().await.len()
    }

    /// Report work completion.
    pub async fn report_completion(
        &self,
        agent_id: AgentId,
        work_id: String,
        result: serde_json::Value,
    ) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        state.work_units_completed += 1;

        // Aggregate result
        self.result_aggregator
            .add_result(agent_id, work_id, result)
            .await;

        // Check if all work is done
        if state.work_units_completed >= state.work_units_assigned && self.pending_work_count().await == 0 {
            state.status = TeamStatus::Completed;
            state.completed_at = Some(Utc::now());
        }

        Ok(())
    }

    /// Get aggregated results.
    pub async fn get_aggregated_results(&self) -> Option<serde_json::Value> {
        self.result_aggregator.get_aggregated_result().await
    }

    /// Disband the team.
    pub async fn disband(&self) -> Result<(), TeamError> {
        let mut state = self.state.write().await;

        // Notify all members
        for agent_id in &state.members {
            self.message_bus
                .send_to_agent(
                    *agent_id,
                    crate::message_bus::InterAgentMessage::team_disbanding(self.id),
                )
                .await;
        }

        state.status = TeamStatus::Disbanded;
        state.members.clear();

        tracing::info!("Team {} disbanded", self.id);
        Ok(())
    }

    /// Get team member count.
    pub async fn member_count(&self) -> usize {
        self.state.read().await.members.len()
    }
}

/// Team orchestrator for managing multiple teams.
pub struct TeamOrchestrator {
    /// Active teams.
    teams: DashMap<TeamId, TeamHandle>,
    /// Agent lifecycle manager.
    lifecycle: Arc<AgentLifecycle>,
    /// Global message bus.
    message_bus: Arc<MessageBus>,
}

impl TeamOrchestrator {
    /// Create a new team orchestrator.
    #[must_use]
    pub fn new(lifecycle: Arc<AgentLifecycle>, message_bus: Arc<MessageBus>) -> Self {
        Self {
            teams: DashMap::new(),
            lifecycle,
            message_bus,
        }
    }

    /// Create a new team.
    pub async fn create_team(&self, config: TeamConfig) -> Result<TeamHandle, TeamError> {
        let id = TeamId::new();

        let state = TeamState {
            id,
            status: TeamStatus::Forming,
            members: Vec::new(),
            created_at: Utc::now(),
            started_at: None,
            completed_at: None,
            work_units_assigned: 0,
            work_units_completed: 0,
            work_units_failed: 0,
            current_strategy: config.distribution_strategy.to_string(),
        };

        let result_aggregator = ResultAggregator::new(config.aggregation_strategy.clone());

        let handle = TeamHandle {
            id,
            config: config.clone(),
            state: Arc::new(RwLock::new(state)),
            lifecycle: Arc::clone(&self.lifecycle),
            message_bus: Arc::clone(&self.message_bus),
            result_aggregator: Arc::new(result_aggregator),
            work_queue: Arc::new(RwLock::new(VecDeque::new())),
        };

        self.teams.insert(id, handle);

        // Create team channel in message bus
        self.message_bus.create_team_channel(id).await;

        tracing::info!("Team {} created: {}", id, config.name);
        Ok(self.teams.get(&id).unwrap().clone())
    }

    /// Get a team by ID.
    #[must_use]
    pub fn get_team(&self, id: TeamId) -> Option<TeamHandle> {
        self.teams.get(&id).map(|t| t.clone())
    }

    /// List all active teams.
    pub fn list_teams(&self) -> Vec<TeamId> {
        self.teams.iter().map(|t| *t.key()).collect()
    }

    /// Disband a team.
    pub async fn disband_team(&self, id: TeamId) -> Result<(), TeamError> {
        if let Some((_, team)) = self.teams.remove(&id) {
            team.disband().await?;
            self.message_bus.remove_team_channel(id).await;
        }
        Ok(())
    }

    /// Get count of active teams.
    pub fn team_count(&self) -> usize {
        self.teams.len()
    }

    /// Find teams by status.
    pub fn find_by_status(&self, status: TeamStatus) -> Vec<TeamId> {
        self.teams
            .iter()
            .filter(|t| {
                let rt = tokio::runtime::Handle::try_current();
                if let Ok(rt) = rt {
                    let state = t.value().state.clone();
                    let state = rt.block_on(async move { state.read().await.clone() });
                    state.status == status
                } else {
                    false
                }
            })
            .map(|t| *t.key())
            .collect()
    }

    /// Auto-scale a team based on work load.
    pub async fn auto_scale_team(&self, team_id: TeamId) -> Result<(), TeamError> {
        let team = self
            .get_team(team_id)
            .ok_or(TeamError::TeamNotFound(team_id))?;

        let config = &team.config;

        if !config.auto_scale {
            return Ok(());
        }

        let state = team.state().await;
        let current_members = state.members.len();
        let pending_work = team.pending_work_count().await;

        // Scale up if there's pending work and room for more agents
        if pending_work > 0 && current_members < config.max_agents {
            if let Some(template) = &config.agent_template {
                let new_agent = self.lifecycle.spawn(template.clone()).await.map_err(|e| {
                    TeamError::AgentSpawnFailed(format!("{:?}", e))
                })?;

                team.add_agent(new_agent.id()).await?;
                tracing::info!("Auto-scaled team {} with agent {}", team_id, new_agent.id());
            }
        }

        // Scale down if too many idle agents (simplified)
        if current_members > config.min_agents && pending_work == 0 {
            // Remove an idle agent
            if let Some(agent_id) = state.members.last() {
                team.remove_agent(*agent_id).await?;
                tracing::info!("Scaled down team {} by removing agent {}", team_id, agent_id);
            }
        }

        Ok(())
    }
}

/// Team errors.
#[derive(Error, Debug, Clone)]
pub enum TeamError {
    /// Team is full.
    #[error("Team is full (max size: {max_size})")]
    TeamFull {
        max_size: usize,
    },

    /// Agent is already a member.
    #[error("Agent {0} is already a team member")]
    AgentAlreadyMember(AgentId),

    /// Agent is not a member.
    #[error("Agent {0} is not a team member")]
    AgentNotMember(AgentId),

    /// Invalid team state for operation.
    #[error("Invalid team state '{current}' for operation '{operation}'")]
    InvalidState {
        current: TeamStatus,
        operation: String,
    },

    /// Team not found.
    #[error("Team {0} not found")]
    TeamNotFound(TeamId),

    /// Failed to spawn agent.
    #[error("Failed to spawn agent: {0}")]
    AgentSpawnFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::InMemoryStateStore;
    use crate::message_bus::MessageBus;
    use crate::result_aggregator::AggregationStrategy;
    use crate::work_distribution::DistributionStrategy;

    fn create_orchestrator() -> TeamOrchestrator {
        let store = Arc::new(InMemoryStateStore::new());
        let lifecycle = Arc::new(AgentLifecycle::new(store));
        let message_bus = Arc::new(MessageBus::new());
        TeamOrchestrator::new(lifecycle, message_bus)
    }

    #[tokio::test]
    async fn test_team_creation() {
        let orchestrator = create_orchestrator();

        let config = TeamConfig::new("test-team")
            .with_goal("Test the team system")
            .with_max_agents(5);

        let team = orchestrator.create_team(config).await.unwrap();

        assert_eq!(team.config.name, "test-team");
        assert_eq!(team.config.goal, "Test the team system");

        let state = team.state().await;
        assert_eq!(state.status, TeamStatus::Forming);
        assert!(state.members.is_empty());
    }

    #[tokio::test]
    async fn test_add_remove_agents() {
        let orchestrator = create_orchestrator();
        let lifecycle = orchestrator.lifecycle.clone();

        let config = TeamConfig::new("test-team").with_max_agents(3);
        let team = orchestrator.create_team(config).await.unwrap();

        // Spawn and add agents
        let agent1 = lifecycle
            .spawn(crate::agent::AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let agent2 = lifecycle
            .spawn(crate::agent::AgentConfig::new("agent-2"))
            .await
            .unwrap();

        team.add_agent(agent1.id()).await.unwrap();
        team.add_agent(agent2.id()).await.unwrap();

        let state = team.state().await;
        assert_eq!(state.members.len(), 2);

        // Remove one agent
        team.remove_agent(agent1.id()).await.unwrap();

        let state = team.state().await;
        assert_eq!(state.members.len(), 1);

        // Cleanup
        lifecycle.terminate(agent1.id()).await.ok();
        lifecycle.terminate(agent2.id()).await.ok();
    }

    #[tokio::test]
    async fn test_team_full_error() {
        let orchestrator = create_orchestrator();
        let lifecycle = orchestrator.lifecycle.clone();

        let config = TeamConfig::new("test-team").with_max_agents(1);
        let team = orchestrator.create_team(config).await.unwrap();

        let agent1 = lifecycle
            .spawn(crate::agent::AgentConfig::new("agent-1"))
            .await
            .unwrap();
        let agent2 = lifecycle
            .spawn(crate::agent::AgentConfig::new("agent-2"))
            .await
            .unwrap();

        team.add_agent(agent1.id()).await.unwrap();

        // Should fail - team is full
        let result = team.add_agent(agent2.id()).await;
        assert!(matches!(result, Err(TeamError::TeamFull { .. })));

        // Cleanup
        lifecycle.terminate(agent1.id()).await.ok();
        lifecycle.terminate(agent2.id()).await.ok();
    }

    #[tokio::test]
    async fn test_team_workflow() {
        let orchestrator = create_orchestrator();
        let lifecycle = orchestrator.lifecycle.clone();

        let config = TeamConfig::new("test-team")
            .with_distribution_strategy(DistributionStrategy::RoundRobin)
            .with_aggregation_strategy(AggregationStrategy::Concatenate);

        let team = orchestrator.create_team(config).await.unwrap();

        // Add an agent
        let agent = lifecycle
            .spawn(crate::agent::AgentConfig::new("worker"))
            .await
            .unwrap();
        team.add_agent(agent.id()).await.unwrap();

        // Start the team
        team.start().await.unwrap();

        let state = team.state().await;
        assert_eq!(state.status, TeamStatus::Working);

        // Pause and resume
        team.pause().await.unwrap();
        let state = team.state().await;
        assert_eq!(state.status, TeamStatus::Pausing);

        team.resume().await.unwrap();
        let state = team.state().await;
        assert_eq!(state.status, TeamStatus::Working);

        // Cleanup
        lifecycle.terminate(agent.id()).await.ok();
    }

    #[tokio::test]
    async fn test_round_robin_strategy() {
        let agents = vec![AgentId::new(), AgentId::new(), AgentId::new()];
        let strategy = RoundRobinStrategy::new(agents.clone());

        // Should cycle through agents
        let first = strategy.next_agent().unwrap();
        let second = strategy.next_agent().unwrap();
        let third = strategy.next_agent().unwrap();
        let fourth = strategy.next_agent().unwrap(); // Should wrap to first

        assert_eq!(first, agents[0]);
        assert_eq!(second, agents[1]);
        assert_eq!(third, agents[2]);
        assert_eq!(fourth, agents[0]); // Wraps around
    }

    #[tokio::test]
    async fn test_orchestrator_list_teams() {
        let orchestrator = create_orchestrator();

        let config1 = TeamConfig::new("team-1");
        let config2 = TeamConfig::new("team-2");

        let team1 = orchestrator.create_team(config1).await.unwrap();
        let team2 = orchestrator.create_team(config2).await.unwrap();

        let teams = orchestrator.list_teams();
        assert_eq!(teams.len(), 2);
        assert!(teams.contains(&team1.id));
        assert!(teams.contains(&team2.id));
    }

    #[tokio::test]
    async fn test_disband_team() {
        let orchestrator = create_orchestrator();

        let config = TeamConfig::new("test-team");
        let team = orchestrator.create_team(config).await.unwrap();
        let team_id = team.id;

        orchestrator.disband_team(team_id).await.unwrap();

        // Team should be disbanded
        let state = team.state().await;
        assert_eq!(state.status, TeamStatus::Disbanded);
    }
}
