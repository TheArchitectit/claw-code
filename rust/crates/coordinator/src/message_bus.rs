//! Inter-agent messaging system.
//!
//! This module provides a pub/sub message bus for communication between agents,
//! supporting direct messaging, team channels, and broadcast patterns.

use std::collections::{HashMap, HashSet};

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::agent::AgentId;
use crate::team::TeamId;

/// Unique identifier for a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MessageId(pub Uuid);

impl MessageId {
    /// Generate a new message ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for MessageId {
    fn default() -> Self {
        Self::new()
    }
}

/// Inter-agent message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterAgentMessage {
    /// Message ID.
    pub id: MessageId,
    /// Message type.
    pub message_type: MessageType,
    /// Sender agent ID.
    pub sender: AgentId,
    /// Target agent ID (None for broadcast).
    pub target: Option<AgentId>,
    /// Team context (if team message).
    pub team_id: Option<TeamId>,
    /// Message payload.
    pub payload: serde_json::Value,
    /// Message priority.
    pub priority: MessagePriority,
    /// Timestamp.
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl InterAgentMessage {
    /// Create a new direct message.
    #[must_use]
    pub fn direct(sender: AgentId, target: AgentId, payload: serde_json::Value) -> Self {
        Self {
            id: MessageId::new(),
            message_type: MessageType::Direct,
            sender,
            target: Some(target),
            team_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a team message.
    #[must_use]
    pub fn team(sender: AgentId, team_id: TeamId, payload: serde_json::Value) -> Self {
        Self {
            id: MessageId::new(),
            message_type: MessageType::Team,
            sender,
            target: None,
            team_id: Some(team_id),
            payload,
            priority: MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a broadcast message.
    #[must_use]
    pub fn broadcast(sender: AgentId, payload: serde_json::Value) -> Self {
        Self {
            id: MessageId::new(),
            message_type: MessageType::Broadcast,
            sender,
            target: None,
            team_id: None,
            payload,
            priority: MessagePriority::Normal,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Create a team disbanding notification.
    #[must_use]
    pub fn team_disbanding(team_id: TeamId) -> Self {
        Self {
            id: MessageId::new(),
            message_type: MessageType::TeamDisbanding,
            sender: AgentId::new(), // System agent
            target: None,
            team_id: Some(team_id),
            payload: serde_json::json!({"team_id": team_id.to_string()}),
            priority: MessagePriority::High,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set priority.
    #[must_use]
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }
}

/// Message type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessageType {
    /// Direct message to specific agent.
    Direct,
    /// Message to all team members.
    Team,
    /// Broadcast to all agents.
    Broadcast,
    /// Work assignment.
    WorkAssignment,
    /// Work result.
    WorkResult,
    /// Status update.
    StatusUpdate,
    /// Request for help.
    HelpRequest,
    /// Coordination signal.
    Coordination,
    /// Team disbanding notification.
    TeamDisbanding,
}

/// Message priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePriority {
    /// Low priority.
    Low = 1,
    /// Normal priority.
    Normal = 5,
    /// High priority.
    High = 10,
    /// Critical priority.
    Critical = 20,
}

/// Message receipt confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageReceipt {
    /// Message ID being acknowledged.
    pub message_id: MessageId,
    /// Agent that received the message.
    pub receiver: AgentId,
    /// When the message was received.
    pub received_at: chrono::DateTime<chrono::Utc>,
    /// Whether processing is complete.
    pub processed: bool,
}

/// Agent mailbox.
struct AgentMailbox {
    /// Agent ID.
    agent_id: AgentId,
    /// Message sender channel.
    sender: mpsc::UnboundedSender<InterAgentMessage>,
    /// Subscribed team channels.
    team_subscriptions: HashSet<TeamId>,
}

/// Message bus for inter-agent communication.
pub struct MessageBus {
    /// Agent mailboxes.
    mailboxes: DashMap<AgentId, AgentMailbox>,
    /// Team channels (team_id -> set of agent_ids).
    team_channels: DashMap<TeamId, HashSet<AgentId>>,
    /// Global broadcast subscribers.
    broadcast_subscribers: DashMap<AgentId, ()>,
}

impl MessageBus {
    /// Create a new message bus.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mailboxes: DashMap::new(),
            team_channels: DashMap::new(),
            broadcast_subscribers: DashMap::new(),
        }
    }

    /// Register an agent with the message bus.
    pub fn register_agent(&self, agent_id: AgentId) -> mpsc::UnboundedReceiver<InterAgentMessage> {
        let (tx, rx) = mpsc::unbounded_channel();

        let mailbox = AgentMailbox {
            agent_id,
            sender: tx,
            team_subscriptions: HashSet::new(),
        };

        self.mailboxes.insert(agent_id, mailbox);
        self.broadcast_subscribers.insert(agent_id, ());

        rx
    }

    /// Unregister an agent.
    pub fn unregister_agent(&self, agent_id: AgentId) {
        // Remove from all team channels
        for entry in self.team_channels.iter() {
            let mut agents = entry.value().clone();
            agents.remove(&agent_id);
            self.team_channels.insert(*entry.key(), agents);
        }

        self.mailboxes.remove(&agent_id);
        self.broadcast_subscribers.remove(&agent_id);
    }

    /// Create a team channel.
    pub async fn create_team_channel(&self, team_id: TeamId) {
        self.team_channels.insert(team_id, HashSet::new());
    }

    /// Remove a team channel.
    pub async fn remove_team_channel(&self, team_id: TeamId) {
        // Notify all subscribed agents
        if let Some((_, agents)) = self.team_channels.remove(&team_id) {
            let disband_msg = InterAgentMessage::team_disbanding(team_id);
            for agent_id in agents {
                self.send_to_agent(agent_id, disband_msg.clone()).await;
            }
        }
    }

    /// Subscribe an agent to a team channel.
    pub async fn subscribe_to_team(&self, team_id: TeamId, agent_id: AgentId) {
        // Add to team channel
        if let Some(mut entry) = self.team_channels.get_mut(&team_id) {
            entry.insert(agent_id);
        } else {
            let mut agents = HashSet::new();
            agents.insert(agent_id);
            self.team_channels.insert(team_id, agents);
        }

        // Update agent mailbox
        if let Some(mut entry) = self.mailboxes.get_mut(&agent_id) {
            entry.team_subscriptions.insert(team_id);
        }
    }

    /// Unsubscribe an agent from a team channel.
    pub async fn unsubscribe_from_team(&self, team_id: TeamId, agent_id: AgentId) {
        // Remove from team channel
        if let Some(mut entry) = self.team_channels.get_mut(&team_id) {
            entry.remove(&agent_id);
        }

        // Update agent mailbox
        if let Some(mut entry) = self.mailboxes.get_mut(&agent_id) {
            entry.team_subscriptions.remove(&team_id);
        }
    }

    /// Send a message to a specific agent.
    pub async fn send_to_agent(
        &self,
        agent_id: AgentId,
        message: InterAgentMessage,
    ) -> Result<(), MessageBusError> {
        if let Some(entry) = self.mailboxes.get(&agent_id) {
            entry
                .sender
                .send(message)
                .map_err(|_| MessageBusError::AgentDisconnected(agent_id))?;
            Ok(())
        } else {
            Err(MessageBusError::AgentNotFound(agent_id))
        }
    }

    /// Send a message to all members of a team.
    pub async fn send_to_team(
        &self,
        team_id: TeamId,
        message: InterAgentMessage,
    ) -> Result<usize, MessageBusError> {
        if let Some(entry) = self.team_channels.get(&team_id) {
            let agents: Vec<_> = entry.iter().copied().collect();
            let count = agents.len();

            for agent_id in agents {
                if let Err(e) = self.send_to_agent(agent_id, message.clone()).await {
                    tracing::warn!("Failed to send to agent {}: {}", agent_id, e);
                }
            }

            Ok(count)
        } else {
            Err(MessageBusError::TeamNotFound(team_id))
        }
    }

    /// Broadcast a message to all agents.
    pub async fn broadcast(&self, message: InterAgentMessage) -> usize {
        let agents: Vec<_> = self.broadcast_subscribers.iter().map(|e| *e.key()).collect();

        for agent_id in agents {
            if let Err(e) = self.send_to_agent(agent_id, message.clone()).await {
                tracing::warn!("Failed to broadcast to agent {}: {}", agent_id, e);
            }
        }

        self.broadcast_subscribers.len()
    }

    /// Get count of registered agents.
    pub fn agent_count(&self) -> usize {
        self.mailboxes.len()
    }

    /// Get count of team channels.
    pub fn team_count(&self) -> usize {
        self.team_channels.len()
    }

    /// Check if an agent is registered.
    pub fn is_registered(&self, agent_id: AgentId) -> bool {
        self.mailboxes.contains_key(&agent_id)
    }

    /// Get team members.
    pub fn get_team_members(&self, team_id: TeamId) -> Vec<AgentId> {
        self.team_channels
            .get(&team_id)
            .map(|entry| entry.iter().copied().collect())
            .unwrap_or_default()
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Message bus errors.
#[derive(thiserror::Error, Debug, Clone)]
pub enum MessageBusError {
    /// Agent not found.
    #[error("Agent {0} not found")]
    AgentNotFound(AgentId),

    /// Agent disconnected.
    #[error("Agent {0} disconnected")]
    AgentDisconnected(AgentId),

    /// Team not found.
    #[error("Team {0} not found")]
    TeamNotFound(TeamId),

    /// Message delivery failed.
    #[error("Message delivery failed: {0}")]
    DeliveryFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_bus_register_unregister() {
        let bus = MessageBus::new();
        let agent_id = AgentId::new();

        let _rx = bus.register_agent(agent_id);
        assert!(bus.is_registered(agent_id));
        assert_eq!(bus.agent_count(), 1);

        bus.unregister_agent(agent_id);
        assert!(!bus.is_registered(agent_id));
        assert_eq!(bus.agent_count(), 0);
    }

    #[tokio::test]
    async fn test_direct_message() {
        let bus = MessageBus::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();

        let mut rx1 = bus.register_agent(agent1);
        let mut rx2 = bus.register_agent(agent2);

        // Send message from agent1 to agent2
        let msg = InterAgentMessage::direct(
            agent1,
            agent2,
            serde_json::json!({"content": "Hello!"}),
        );
        bus.send_to_agent(agent2, msg).await.unwrap();

        // Agent2 should receive the message
        let received = rx2.recv().await.unwrap();
        assert_eq!(received.sender, agent1);
        assert_eq!(received.target, Some(agent2));

        // Agent1 should not receive anything
        assert!(rx1.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_team_messaging() {
        let bus = MessageBus::new();
        let team_id = TeamId::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();
        let agent3 = AgentId::new();

        let mut rx1 = bus.register_agent(agent1);
        let mut rx2 = bus.register_agent(agent2);
        let _rx3 = bus.register_agent(agent3);

        // Create team and subscribe agents
        bus.create_team_channel(team_id).await;
        bus.subscribe_to_team(team_id, agent1).await;
        bus.subscribe_to_team(team_id, agent2).await;

        // Send team message
        let msg = InterAgentMessage::team(agent1, team_id, serde_json::json!({"data": "team data"}));
        let count = bus.send_to_team(team_id, msg).await.unwrap();
        assert_eq!(count, 2);

        // Both team members should receive
        let received1 = rx1.recv().await.unwrap();
        assert_eq!(received1.team_id, Some(team_id));

        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received2.team_id, Some(team_id));
    }

    #[tokio::test]
    async fn test_broadcast() {
        let bus = MessageBus::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();

        let mut rx1 = bus.register_agent(agent1);
        let mut rx2 = bus.register_agent(agent2);

        // Send broadcast
        let msg = InterAgentMessage::broadcast(agent1, serde_json::json!({"announcement": "Hello all!"}));
        let count = bus.broadcast(msg).await;
        assert_eq!(count, 2);

        // Both should receive
        let received1 = rx1.recv().await.unwrap();
        assert_eq!(received1.message_type, MessageType::Broadcast);

        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received2.message_type, MessageType::Broadcast);
    }

    #[tokio::test]
    async fn test_team_unsubscribe() {
        let bus = MessageBus::new();
        let team_id = TeamId::new();
        let agent1 = AgentId::new();

        let mut rx = bus.register_agent(agent1);

        bus.create_team_channel(team_id).await;
        bus.subscribe_to_team(team_id, agent1).await;

        // Unsubscribe
        bus.unsubscribe_from_team(team_id, agent1).await;

        // Send team message
        let msg = InterAgentMessage::team(agent1, team_id, serde_json::json!({}));
        bus.send_to_team(team_id, msg).await.unwrap();

        // Agent should not receive (unsubscribed)
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_remove_team_channel() {
        let bus = MessageBus::new();
        let team_id = TeamId::new();
        let agent1 = AgentId::new();

        let mut rx = bus.register_agent(agent1);

        bus.create_team_channel(team_id).await;
        bus.subscribe_to_team(team_id, agent1).await;

        // Remove team channel
        bus.remove_team_channel(team_id).await;

        // Agent should receive disband notification
        let received = rx.recv().await.unwrap();
        assert_eq!(received.message_type, MessageType::TeamDisbanding);
    }

    #[tokio::test]
    async fn test_message_priority() {
        let bus = MessageBus::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();

        let _rx1 = bus.register_agent(agent1);
        let mut rx2 = bus.register_agent(agent2);

        // Send high priority message
        let msg = InterAgentMessage::direct(agent1, agent2, serde_json::json!({}))
            .with_priority(MessagePriority::Critical);

        bus.send_to_agent(agent2, msg).await.unwrap();

        let received = rx2.recv().await.unwrap();
        assert_eq!(received.priority, MessagePriority::Critical);
    }

    #[tokio::test]
    async fn test_get_team_members() {
        let bus = MessageBus::new();
        let team_id = TeamId::new();
        let agent1 = AgentId::new();
        let agent2 = AgentId::new();

        let _rx1 = bus.register_agent(agent1);
        let _rx2 = bus.register_agent(agent2);

        bus.create_team_channel(team_id).await;
        bus.subscribe_to_team(team_id, agent1).await;
        bus.subscribe_to_team(team_id, agent2).await;

        let members = bus.get_team_members(team_id);
        assert_eq!(members.len(), 2);
        assert!(members.contains(&agent1));
        assert!(members.contains(&agent2));
    }
}
