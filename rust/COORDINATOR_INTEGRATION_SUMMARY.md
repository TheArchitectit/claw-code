# Coordinator Integration Summary

## Overview
The coordinator integration for the QueryEngine has been analyzed and the supporting infrastructure is in place. Due to pre-existing syntax errors in the repository that prevent compilation, I was unable to fully apply the QueryEngine changes, but all the necessary integration points are documented below.

## What Already Exists

### 1. Coordinator Integration Module (`coordinator_integration.rs`)
Located at: `/mnt/ollama/git/claw-code/rust/crates/runtime/src/coordinator_integration.rs`

This module already provides (1117 lines):

**Core Types:**
- `AgentId` - Unique identifier for agents
- `TeamId` - Unique identifier for teams
- `Capability` - Agent capability descriptors
- `AgentConfig` - Configuration for agent creation
- `AgentHandle` - Handle to a running agent
- `TeamConfig` / `TeamHandle` - Team management
- `InterAgentMessage` - Message type for agent communication
- `MessageBus` - Pub/sub message routing
- `Coordinator` / `CoordinatorHandle` - Main coordinator types

**Integration Methods (on `CoordinatorHandle`):**
- `store_agent_definitions()` - Store and validate agent definitions
- `update_agent_definitions()` - Dynamic agent definition updates
- `validate_agent_definitions()` - Validation logic
- `route_tool_to_agent()` - Route tool calls to specific agents
- `send_message_to_agent()` - Direct messaging
- `send_message_to_team()` - Team messaging
- `broadcast_message()` - Broadcast to all agents
- `join_team()` / `leave_team()` - Team membership
- `coordinate_with_team()` - Coordinate on shared tasks
- `aggregate_team_results()` - Aggregate results from team members
- `receive_messages()` / `poll_messages()` - Message receiving
- `handle_send_message_tool()` - Tool routing
- `get_task_status()` / `update_task_status()` - Task tracking

## Changes Required in `query_engine.rs`

### 1. Add Coordinator Handle Field to QueryEngine

```rust
pub struct QueryEngine {
    // ... existing fields ...
    /// Coordinator handle for multi-agent support.
    coordinator_handle: Arc<Mutex<crate::coordinator_integration::CoordinatorHandle>>,
}
```

### 2. Initialize in QueryEngine::new()

```rust
pub fn new(config: QueryEngineConfig) -> Self {
    // ... existing initialization ...
    Self {
        // ... existing fields ...
        coordinator_handle: Arc::new(Mutex::new(
            crate::coordinator_integration::CoordinatorHandle::new_without_agent()
        )),
    }
}
```

### 3. Add Required Methods to QueryEngine

```rust
impl QueryEngine {
    /// Get the coordinator handle for multi-agent operations.
    pub fn coordinator_handle(&self) -> &Arc<Mutex<CoordinatorHandle>> {
        &self.coordinator_handle
    }

    /// Route a tool call to a specific agent.
    pub async fn route_tool_to_agent(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        agent_id: Option<String>,
    ) -> QueryResult<ToolOutput> {
        if let Some(target_agent) = agent_id {
            let handle = self.coordinator_handle.lock().await;
            handle.route_tool_to_agent(tool_name, params, Some(target_agent)).await
                .map_err(|e| QueryEngineError::Validation { message: e.to_string() })
        } else {
            // Execute locally - use existing tool execution path
            // ... local execution logic ...
        }
    }

    /// Send a message to another agent.
    pub async fn send_message_to_agent(
        &self,
        target_agent_id: AgentId,
        payload: serde_json::Value,
    ) -> Result<(), CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.send_message_to_agent(target_agent_id, payload, None).await
    }

    /// Send a message to the current team.
    pub async fn send_message_to_team(
        &self,
        payload: serde_json::Value,
    ) -> Result<(), CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.send_message_to_team(payload).await
    }

    /// Join a team.
    pub async fn join_team(
        &self,
        team_id: TeamId,
    ) -> Result<(), CoordinatorIntegrationError> {
        let mut handle = self.coordinator_handle.lock().await;
        handle.join_team(team_id).await
    }

    /// Leave the current team.
    pub async fn leave_team(&self) -> Result<(), CoordinatorIntegrationError> {
        let mut handle = self.coordinator_handle.lock().await;
        handle.leave_team().await
    }

    /// Get current team ID.
    pub async fn team_id(&self) -> Option<TeamId> {
        let handle = self.coordinator_handle.lock().await;
        handle.team_id().await
    }

    /// Coordinate with team on shared task.
    pub async fn coordinate_with_team(
        &self,
        task_description: &str,
    ) -> Result<(), CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.coordinate_with_team(task_description).await
    }

    /// Aggregate team results.
    pub async fn aggregate_team_results(
        &self,
    ) -> Result<Vec<InterAgentMessage>, CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.aggregate_team_results().await
    }

    /// Update agent definitions.
    pub async fn update_agent_definitions(
        &self,
        definitions: AgentDefinitions,
    ) -> Result<(), CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.update_agent_definitions(definitions).await;
        Ok(())
    }

    /// Receive messages from other agents.
    pub async fn receive_messages(&self) -> Vec<InterAgentMessage> {
        let handle = self.coordinator_handle.lock().await;
        handle.receive_messages().await
    }

    /// Poll for new messages.
    pub async fn poll_messages(
        &self,
    ) -> Result<Vec<InterAgentMessage>, CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.poll_messages().await
    }

    /// Handle SendMessageTool routing.
    pub async fn handle_send_message_tool(
        &self,
        params: &serde_json::Value,
    ) -> Result<ToolOutput, CoordinatorIntegrationError> {
        let handle = self.coordinator_handle.lock().await;
        handle.handle_send_message_tool(params).await
    }

    /// Get agent task status.
    pub async fn get_agent_task_status(
        &self,
        task_id: &str,
    ) -> Option<AgentTaskStatus> {
        let handle = self.coordinator_handle.lock().await;
        handle.get_task_status(task_id).await
    }

    /// Update agent task status.
    pub async fn update_agent_task_status(&self, task_id: &str, status: AgentTaskStatus) {
        let handle = self.coordinator_handle.lock().await;
        handle.update_task_status(task_id, status).await;
    }

    /// Get pending agent tasks.
    pub async fn get_pending_agent_tasks(&self) -> Vec<String> {
        let handle = self.coordinator_handle.lock().await;
        handle.get_pending_tasks().await
    }
}
```

### 4. Update QueryEngineBuilder

Add a method to set the coordinator handle:

```rust
impl QueryEngineBuilder {
    /// Set the coordinator handle.
    pub fn with_coordinator_handle(
        mut self,
        handle: CoordinatorHandle,
    ) -> Self {
        self.coordinator_handle = Some(handle);
        self
    }
}
```

## Integration Points with Coordinator Crate

The integration avoids circular dependencies by:
1. Using stub types in `coordinator_integration.rs` that mirror the coordinator crate types
2. Providing a `CoordinatorHandle` that encapsulates multi-agent functionality
3. Using the `ToolOutput` type from the runtime crate for tool results

## Testing Recommendations

Tests should verify:
1. Agent definition storage and validation
2. Tool routing to agents
3. Message passing between agents
4. Team join/leave operations
5. Task status tracking

## Notes

- The repository at HEAD (beac5f0) has pre-existing syntax errors in `query_engine.rs`
- The `coordinator_integration.rs` module is complete and ready to use
- Once the syntax errors are fixed, the QueryEngine integration can be applied
- The coordinator dependency was noted as optional in earlier Cargo.toml versions
