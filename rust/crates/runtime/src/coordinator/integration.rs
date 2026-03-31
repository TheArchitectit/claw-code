//! Coordinator handle for QueryEngine integration.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::context::AgentDefinitions;
use crate::tool::ToolOutput;
use crate::types::{QueryEngineError, QueryResult, ToolError, ToolUseId};

use super::handles::{AgentHandle, TeamHandle};
use super::message_bus::Coordinator;
use super::types::*;
use super::config::{AgentConfig, TeamConfig};

/// Coordinator handle for QueryEngine integration.
///
/// This provides a simplified interface for the QueryEngine to interact
/// with the multi-agent coordinator system.
#[derive(Clone)]
pub struct CoordinatorHandle {
    /// The underlying coordinator (crate-visible for submodules).
    pub(crate) coordinator: Arc<Coordinator>,
    /// Agent definitions registry (crate-visible for submodules).
    pub(crate) agent_definitions: Arc<RwLock<AgentDefinitions>>,
    /// Message receiver for this agent/entity (crate-visible for submodules).
    pub(crate) message_rx: Arc<Mutex<mpsc::UnboundedReceiver<InterAgentMessage>>>,
    /// This entity's agent ID (if registered) (crate-visible for submodules).
    pub(crate) own_agent_id: Arc<RwLock<Option<AgentId>>>,
    /// Active agent handles for delegated tasks (crate-visible for submodules).
    pub(crate) delegated_agents: Arc<Mutex<HashMap<String, AgentHandle>>>,
    /// Current team context (if any) (crate-visible for submodules).
    pub(crate) current_team_id: Arc<RwLock<Option<TeamId>>>,
}

impl CoordinatorHandle {
    /// Create a new coordinator handle with the given coordinator.
    #[must_use]
    pub fn new(coordinator: Arc<Coordinator>) -> Self {
        // Register with message bus to receive messages
        let agent_id = AgentId::new();
        let message_rx = coordinator.message_bus.register_agent(agent_id);

        Self {
            coordinator,
            agent_definitions: Arc::new(RwLock::new(AgentDefinitions::default())),
            message_rx: Arc::new(Mutex::new(message_rx)),
            own_agent_id: Arc::new(RwLock::new(Some(agent_id))),
            delegated_agents: Arc::new(Mutex::new(HashMap::new())),
            current_team_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a new coordinator handle without an agent ID (for non-agent contexts).
    #[must_use]
    pub fn new_without_agent() -> Self {
        // Create a minimal coordinator
        let coordinator = Arc::new(Coordinator::new());
        let (_tx, rx) = mpsc::unbounded_channel();

        Self {
            coordinator,
            agent_definitions: Arc::new(RwLock::new(AgentDefinitions::default())),
            message_rx: Arc::new(Mutex::new(rx)),
            own_agent_id: Arc::new(RwLock::new(None)),
            delegated_agents: Arc::new(Mutex::new(HashMap::new())),
            current_team_id: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the coordinator reference.
    #[must_use]
    pub fn coordinator(&self) -> Arc<Coordinator> {
        Arc::clone(&self.coordinator)
    }

    /// Get the message bus.
    #[must_use]
    pub fn message_bus(&self) -> Arc<super::message_bus::MessageBus> {
        Arc::clone(&self.coordinator.message_bus)
    }

    /// Store agent definitions from the coordinator.
    ///
    /// This replaces the current agent definitions with new ones
    /// and validates each definition.
    pub async fn store_agent_definitions(&self, definitions: AgentDefinitions) {
        let mut guard = self.agent_definitions.write().await;

        // Validate definitions
        for agent in &definitions.all_agents {
            if agent.name.is_empty() {
                warn!("Agent definition has empty name");
            }
            if agent.agent_type.is_empty() {
                warn!("Agent '{}' has empty type", agent.name);
            }
        }

        *guard = definitions;
        info!("Stored {} agent definitions", guard.all_agents.len());
    }

    /// Get current agent definitions.
    pub async fn get_agent_definitions(&self) -> AgentDefinitions {
        self.agent_definitions.read().await.clone()
    }

    /// Update agent definitions dynamically.
    ///
    /// Supports adding, updating, or removing agent definitions at runtime.
    pub async fn update_agent_definitions(&self, definitions: AgentDefinitions) {
        let mut guard = self.agent_definitions.write().await;
        guard.active_agents = definitions.active_agents;
        guard.all_agents = definitions.all_agents;
        debug!("Updated agent definitions");
    }

    /// Validate agent definitions.
    ///
    /// Returns a list of validation errors (empty if all valid).
    pub async fn validate_agent_definitions(&self) -> Vec<String> {
        let guard = self.agent_definitions.read().await;
        let mut errors = Vec::new();

        for agent in &guard.all_agents {
            if agent.name.is_empty() {
                errors.push("Agent has empty name".to_string());
            }
            if agent.agent_type.is_empty() {
                errors.push(format!("Agent '{}' has empty type", agent.name));
            }
            if agent.description.is_empty() {
                warn!("Agent '{}' has no description", agent.name);
            }
        }

        errors
    }

    /// Route a tool call to a specific agent.
    ///
    /// If `agent_id` is provided, routes to that specific agent.
    /// Otherwise, executes via the main QueryEngine (returns None).
    ///
    /// # Arguments
    /// * `tool_name` - The name of the tool to execute
    /// * `params` - Tool parameters
    /// * `agent_id` - Optional target agent ID
    ///
    /// # Returns
    /// * `Ok(Some(output))` - If routed to an agent and executed
    /// * `Ok(None)` - If should be handled by main QueryEngine
    /// * `Err(e)` - If routing or execution failed
    pub async fn route_tool_to_agent(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        agent_id: Option<String>,
    ) -> QueryResult<Option<ToolOutput>> {
        if let Some(agent_id_str) = agent_id {
            // Try to parse the agent ID
            let agent_uuid = match uuid::Uuid::parse_str(&agent_id_str) {
                Ok(u) => AgentId(u),
                Err(e) => {
                    return Err(QueryEngineError::Tool(ToolError::invalid_input(
                        format!("Invalid agent ID: {}", e),
                    )));
                }
            };

            // Look up the agent handle
            let agent = self
                .coordinator
                .get_agent(agent_uuid)
                .ok_or_else(|| QueryEngineError::Tool(ToolError::not_found(&agent_id_str)))?;

            // Check agent capabilities
            let state = agent.state().await;
            if !state.is_active() {
                return Err(QueryEngineError::Tool(ToolError::invalid_input(
                    format!("Agent {} is not active (status: {:?})", agent_id_str, state.status),
                )));
            }

            // Create work assignment for the agent
            let work = WorkAssignment {
                description: format!("Execute tool: {}", tool_name),
                context: serde_json::json!({
                    "tool_name": tool_name,
                    "params": params,
                    "type": "tool_execution"
                }),
                priority: 5,
                deadline: None,
            };

            // Assign work to the agent
            agent
                .assign_work(format!("tool-{}-{}", tool_name, ToolUseId::generate()), work)
                .await
                .map_err(|e| {
                    QueryEngineError::Tool(ToolError::execution_failed(format!(
                        "Failed to assign work to agent: {:?}",
                        e
                    )))
                })?;

            // Return success - actual tool execution result would come via message
            let output = ToolOutput::new(serde_json::json!({
                "status": "assigned",
                "agent_id": agent_id_str,
                "tool": tool_name
            }));

            return Ok(Some(output));
        }

        // No specific agent requested - route to main QueryEngine
        Ok(None)
    }

    /// Send a message to another agent.
    ///
    /// # Arguments
    /// * `target_agent_id` - The recipient agent ID
    /// * `payload` - Message payload
    /// * `priority` - Message priority
    pub async fn send_message_to_agent(
        &self,
        target_agent_id: &str,
        payload: serde_json::Value,
        priority: MessagePriority,
    ) -> Result<(), CoordinatorIntegrationError> {
        let own_id = self.own_agent_id.read().await;
        let sender_id = own_id.ok_or(CoordinatorIntegrationError::NotRegistered)?;

        let target_uuid = uuid::Uuid::parse_str(target_agent_id)
            .map_err(|e| CoordinatorIntegrationError::InvalidId(format!("Invalid target ID: {}", e)))?;
        let target_id = AgentId(target_uuid);

        let message = InterAgentMessage::direct(sender_id, target_id, payload)
            .with_priority(priority);

        self.coordinator
            .message_bus
            .send_to_agent(target_id, message)
            .await
            .map_err(|e| CoordinatorIntegrationError::MessageBusError(e.to_string()))?;

        Ok(())
    }

    /// Send a message to all members of a team.
    ///
    /// # Arguments
    /// * `team_id` - The team ID
    /// * `payload` - Message payload
    /// * `priority` - Message priority
    pub async fn send_message_to_team(
        &self,
        team_id: &str,
        payload: serde_json::Value,
        priority: MessagePriority,
    ) -> Result<usize, CoordinatorIntegrationError> {
        let own_id = self.own_agent_id.read().await;
        let sender_id = own_id.ok_or(CoordinatorIntegrationError::NotRegistered)?;

        let team_uuid = uuid::Uuid::parse_str(team_id)
            .map_err(|e| CoordinatorIntegrationError::InvalidId(format!("Invalid team ID: {}", e)))?;
        let team_id = TeamId(team_uuid);

        let message = InterAgentMessage::team(sender_id, team_id, payload)
            .with_priority(priority);

        let count = self.coordinator
            .message_bus
            .send_to_team(team_id, message)
            .await
            .map_err(|e| CoordinatorIntegrationError::MessageBusError(e.to_string()))?;

        Ok(count)
    }

    /// Broadcast a message to all agents.
    ///
    /// # Arguments
    /// * `payload` - Message payload
    /// * `priority` - Message priority
    pub async fn broadcast_message(
        &self,
        payload: serde_json::Value,
        priority: MessagePriority,
    ) -> Result<usize, CoordinatorIntegrationError> {
        let own_id = self.own_agent_id.read().await;
        let sender_id = own_id.ok_or(CoordinatorIntegrationError::NotRegistered)?;

        let message = InterAgentMessage::broadcast(sender_id, payload)
            .with_priority(priority);

        let count = self.coordinator.message_bus.broadcast(message).await;
        Ok(count)
    }

    /// Receive messages from other agents.
    ///
    /// Returns pending messages received since last call.
    pub async fn receive_messages(&self) -> Vec<InterAgentMessage> {
        let mut rx = self.message_rx.lock().await;
        let mut messages = Vec::new();

        // Drain all pending messages
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }

        messages
    }

    /// Set the current team context.
    ///
    /// This affects how messages are routed and how work is distributed.
    pub async fn set_team_context(&self, team_id: Option<TeamId>) {
        let mut guard = self.current_team_id.write().await;
        *guard = team_id;
    }

    /// Get the current team context.
    pub async fn get_team_context(&self) -> Option<TeamId> {
        *self.current_team_id.read().await
    }

    /// Join a team.
    ///
    /// Registers this agent as a member of the specified team.
    pub async fn join_team(&self, team_id: &str) -> Result<(), CoordinatorIntegrationError> {
        let team_uuid = uuid::Uuid::parse_str(team_id)
            .map_err(|e| CoordinatorIntegrationError::InvalidId(format!("Invalid team ID: {}", e)))?;
        let team_id = TeamId(team_uuid);

        let own_id = self.own_agent_id.read().await;
        let agent_id = own_id.ok_or(CoordinatorIntegrationError::NotRegistered)?;

        // Subscribe to team channel
        self.coordinator
            .message_bus
            .subscribe_to_team(team_id, agent_id)
            .await;

        // Update team context
        self.set_team_context(Some(team_id)).await;

        info!("Agent {} joined team {}", agent_id, team_id);
        Ok(())
    }

    /// Leave the current team.
    pub async fn leave_team(&self) -> Result<(), CoordinatorIntegrationError> {
        let team_id_opt = self.get_team_context().await;
        let team_id = team_id_opt.ok_or(CoordinatorIntegrationError::NoTeamContext)?;

        let own_id = self.own_agent_id.read().await;
        let agent_id = own_id.ok_or(CoordinatorIntegrationError::NotRegistered)?;

        // Unsubscribe from team channel
        self.coordinator
            .message_bus
            .unsubscribe_from_team(team_id, agent_id)
            .await;

        // Clear team context
        self.set_team_context(None).await;

        info!("Agent {} left team {}", agent_id, team_id);
        Ok(())
    }

    /// Get this entity's agent ID (if registered).
    pub async fn get_own_agent_id(&self) -> Option<AgentId> {
        *self.own_agent_id.read().await
    }
}

impl Default for CoordinatorHandle {
    fn default() -> Self {
        Self::new_without_agent()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AgentDefinitions;
    use crate::coordinator::CoordinatorHandleBuilder;

    #[tokio::test]
    async fn test_coordinator_handle_creation() {
        let handle = CoordinatorHandle::new_without_agent();
        assert!(handle.get_own_agent_id().await.is_none());
    }

    #[tokio::test]
    async fn test_store_and_get_agent_definitions() {
        let handle = CoordinatorHandle::new_without_agent();

        let definitions = AgentDefinitions {
            all_agents: vec![],
            active_agents: vec![],
        };

        handle.store_agent_definitions(definitions.clone()).await;
        let retrieved = handle.get_agent_definitions().await;

        assert!(retrieved.all_agents.is_empty());
    }

    #[tokio::test]
    async fn test_validate_agent_definitions() {
        let handle = CoordinatorHandle::new_without_agent();

        let definitions = AgentDefinitions {
            all_agents: vec![crate::context::AgentDefinition {
                name: "".to_string(),
                agent_type: "".to_string(),
                description: "".to_string(),
                capabilities: vec![],
                config: serde_json::json!({}),
            }],
            active_agents: vec![],
        };

        handle.store_agent_definitions(definitions).await;
        let errors = handle.validate_agent_definitions().await;

        assert!(!errors.is_empty());
        assert!(errors[0].contains("empty name"));
    }

    #[tokio::test]
    async fn test_receive_messages_empty() {
        let handle = CoordinatorHandle::new_without_agent();
        let messages = handle.receive_messages().await;
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_team_context() {
        let handle = CoordinatorHandle::new_without_agent();

        // Initially no team
        assert!(handle.get_team_context().await.is_none());

        // Set team context
        let team_id = TeamId::new();
        handle.set_team_context(Some(team_id)).await;

        // Should have team
        assert_eq!(handle.get_team_context().await, Some(team_id));
    }

    #[tokio::test]
    async fn test_delegated_agents_list() {
        let handle = CoordinatorHandle::new_without_agent();
        let agents = handle.list_delegated_agents().await;
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn test_builder_without_coordinator() {
        let handle = CoordinatorHandleBuilder::new().build();
        assert!(handle.get_own_agent_id().await.is_none());
    }

    #[tokio::test]
    async fn test_builder_with_coordinator() {
        let coordinator = Arc::new(Coordinator::new());
        let handle = CoordinatorHandleBuilder::new()
            .with_coordinator(coordinator)
            .build();
        assert!(handle.get_own_agent_id().await.is_some());
    }
}
