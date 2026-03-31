//! Routing and delegation methods for coordinator handle.

use serde_json::Value;
use tracing::info;

use crate::tool::ToolOutput;
use crate::types::ToolUseId;

use super::handles::{AgentHandle, TeamHandle};
use super::types::*;
use super::config::{AgentConfig, TeamConfig};
use super::integration::CoordinatorHandle;

impl CoordinatorHandle {
    /// Spawn a new agent for delegation.
    ///
    /// Creates a new agent with the given configuration and tracks it.
    pub async fn spawn_delegated_agent(
        &self,
        name: &str,
        capabilities: Vec<Capability>,
    ) -> Result<AgentHandle, CoordinatorIntegrationError> {
        let config = AgentConfig::new(name)
            .with_capabilities(capabilities);

        let handle = self
            .coordinator
            .spawn_agent(config)
            .await
            .map_err(|e| CoordinatorIntegrationError::AgentSpawnFailed(e.to_string()))?;

        // Track the delegated agent
        let mut delegated = self.delegated_agents.lock().await;
        delegated.insert(name.to_string(), handle.clone());

        info!("Spawned delegated agent: {} ({})", name, handle.id());
        Ok(handle)
    }

    /// Terminate a delegated agent.
    pub async fn terminate_delegated_agent(
        &self,
        name: &str,
    ) -> Result<(), CoordinatorIntegrationError> {
        let mut delegated = self.delegated_agents.lock().await;

        if let Some(handle) = delegated.remove(name) {
            self.coordinator
                .terminate_agent(handle.id())
                .await
                .map_err(|e| CoordinatorIntegrationError::AgentTerminationFailed(e.to_string()))?;
            info!("Terminated delegated agent: {}", name);
        }

        Ok(())
    }

    /// Get the status of a delegated agent.
    pub async fn get_delegated_agent_status(
        &self,
        name: &str,
    ) -> Option<AgentState> {
        let delegated = self.delegated_agents.lock().await;
        delegated.get(name).map(|_h| {
            // Return a placeholder that indicates the agent exists
            AgentState::default()
        })
    }

    /// Get list of delegated agent names.
    pub async fn list_delegated_agents(&self) -> Vec<String> {
        let delegated = self.delegated_agents.lock().await;
        delegated.keys().cloned().collect()
    }

    /// Execute work via a delegated agent.
    ///
    /// Assigns work to a specific delegated agent.
    pub async fn delegate_work(
        &self,
        agent_name: &str,
        work_description: &str,
        work_context: Value,
    ) -> Result<(), CoordinatorIntegrationError> {
        let delegated = self.delegated_agents.lock().await;

        let handle = delegated
            .get(agent_name)
            .ok_or_else(|| CoordinatorIntegrationError::AgentNotFound(agent_name.to_string()))?;

        let work = WorkAssignment {
            description: work_description.to_string(),
            context: work_context,
            priority: 5,
            deadline: None,
        };

        let work_id = format!("work-{}", ToolUseId::generate());
        handle
            .assign_work(work_id, work)
            .await
            .map_err(|e| CoordinatorIntegrationError::WorkAssignmentFailed(e.to_string()))?;

        Ok(())
    }

    /// Create a team.
    ///
    /// Creates a new team with the given configuration.
    pub async fn create_team(
        &self,
        name: &str,
        max_agents: usize,
    ) -> Result<TeamHandle, CoordinatorIntegrationError> {
        let config = TeamConfig::new(name).with_max_agents(max_agents);

        let handle = self
            .coordinator
            .create_team(config)
            .await
            .map_err(|e| CoordinatorIntegrationError::TeamCreationFailed(e.to_string()))?;

        info!("Created team: {} ({})", name, handle.id);
        Ok(handle)
    }

    /// Get team members.
    pub async fn get_team_members(&self, team_id: TeamId) -> Vec<AgentId> {
        self.coordinator.message_bus.get_team_members(team_id)
    }

    /// Aggregate results from team members.
    ///
    /// Collects and aggregates work results from all team members.
    pub async fn aggregate_team_results(&self) -> Result<Value, CoordinatorIntegrationError> {
        // In a real implementation, this would collect results from the result aggregator
        // For now, return a placeholder
        Ok(serde_json::json!({
            "status": "pending",
            "note": "Result aggregation would collect from team members"
        }))
    }

    /// Handle SendMessageTool routing.
    ///
    /// Routes SendMessageTool calls to the appropriate agent(s).
    pub async fn route_send_message(
        &self,
        to: &str,
        message: &str,
        message_type: &str,
    ) -> Result<ToolOutput, CoordinatorIntegrationError> {
        let payload = serde_json::json!({
            "message": message,
            "type": message_type,
        });

        let recipients = match to {
            "*" => {
                // Broadcast to all
                let count = self.broadcast_message(payload, MessagePriority::Normal).await?;
                format!("Broadcast to {} agents", count)
            }
            team if team.starts_with("team:") => {
                // Team message
                let team_id = &team[5..];
                let count = self.send_message_to_team(team_id, payload, MessagePriority::Normal).await?;
                format!("Sent to team {} ({} members)", team_id, count)
            }
            agent_id => {
                // Direct message
                self.send_message_to_agent(agent_id, payload, MessagePriority::Normal).await?;
                format!("Sent to agent {}", agent_id)
            }
        };

        let output = ToolOutput::new(serde_json::json!({
            "status": "sent",
            "recipients": recipients,
        }));

        Ok(output)
    }
}
