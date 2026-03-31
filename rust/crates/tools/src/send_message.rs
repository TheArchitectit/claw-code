//! SendMessageTool - Inter-agent messaging system.
//!
//! This tool enables agents to communicate with each other through
//! a message queue system. Messages can be sent to specific agents
//! by name and persisted for later retrieval.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Priority levels for messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePriority {
    #[serde(rename = "low")]
    Low,
    #[serde(rename = "normal")]
    Normal,
    #[serde(rename = "high")]
    High,
    #[serde(rename = "urgent")]
    Urgent,
}

impl MessagePriority {
    /// Get the priority as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            MessagePriority::Low => "low",
            MessagePriority::Normal => "normal",
            MessagePriority::High => "high",
            MessagePriority::Urgent => "urgent",
        }
    }
}

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

impl std::str::FromStr for MessagePriority {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "low" => Ok(MessagePriority::Low),
            "normal" => Ok(MessagePriority::Normal),
            "high" => Ok(MessagePriority::High),
            "urgent" => Ok(MessagePriority::Urgent),
            _ => Err(format!("Invalid priority: {s}")),
        }
    }
}

/// A message in the inter-agent messaging system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message_id: String,
    pub from: String,
    pub to: String,
    pub content: String,
    pub priority: String,
    pub timestamp: u64,
    pub read: bool,
    pub message_type: String,
}

impl Message {
    /// Create a new message.
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        content: impl Into<String>,
        priority: MessagePriority,
        message_type: impl Into<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let random_suffix = rand::random::<u16>();
        let message_id = format!("msg-{:x}-{:x}", now, random_suffix);

        Self {
            message_id,
            from: from.into(),
            to: to.into(),
            content: content.into(),
            priority: priority.as_str().to_string(),
            timestamp: now,
            read: false,
            message_type: message_type.into(),
        }
    }
}

/// Summary of a message for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSummary {
    pub message_id: String,
    pub from: String,
    pub priority: String,
    pub timestamp: u64,
    pub read: bool,
    pub preview: String,
}

impl From<&Message> for MessageSummary {
    fn from(msg: &Message) -> Self {
        let preview = if msg.content.len() > 100 {
            format!("{}...", &msg.content[..100])
        } else {
            msg.content.clone()
        };

        Self {
            message_id: msg.message_id.clone(),
            from: msg.from.clone(),
            priority: msg.priority.clone(),
            timestamp: msg.timestamp,
            read: msg.read,
            preview,
        }
    }
}

/// The message store for managing inter-agent communication.
#[derive(Debug, Clone)]
pub struct MessageStore {
    // Agent name -> queue of messages
    messages: Arc<Mutex<HashMap<String, VecDeque<Message>>>>,
}

impl MessageStore {
    /// Create a new empty message store.
    pub fn new() -> Self {
        Self {
            messages: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all messages.
    pub fn clear(&self) {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        messages.clear();
    }

    /// Send a message to an agent.
    pub fn send(&self, message: Message) {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        let queue = messages.entry(message.to.clone()).or_default();

        // Insert based on priority (higher priority first)
        let priority = match message.priority.as_str() {
            "urgent" => 3,
            "high" => 2,
            "normal" => 1,
            _ => 0,
        };

        // Find insertion position based on priority
        let insert_pos = queue.iter().position(|m| {
            let m_priority = match m.priority.as_str() {
                "urgent" => 3,
                "high" => 2,
                "normal" => 1,
                _ => 0,
            };
            m_priority < priority
        });

        if let Some(pos) = insert_pos {
            queue.insert(pos, message);
        } else {
            queue.push_back(message);
        }
    }

    /// Get the next message for an agent (FIFO with priority).
    pub fn receive(&self, agent_name: &str) -> Option<Message> {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = messages.get_mut(agent_name) {
            queue.pop_front().map(|mut msg| {
                msg.read = true;
                msg
            })
        } else {
            None
        }
    }

    /// Peek at messages for an agent without removing them.
    pub fn peek(&self, agent_name: &str, limit: usize) -> Vec<Message> {
        let messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = messages.get(agent_name) {
            queue.iter().take(limit).cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Get message count for an agent.
    pub fn count(&self, agent_name: &str) -> usize {
        let messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        messages
            .get(agent_name)
            .map(|q| q.len())
            .unwrap_or(0)
    }

    /// Get all messages for an agent.
    pub fn list(&self, agent_name: &str) -> Vec<MessageSummary> {
        let messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = messages.get(agent_name) {
            queue.iter().map(MessageSummary::from).collect()
        } else {
            Vec::new()
        }
    }

    /// Mark a message as read.
    pub fn mark_read(&self, agent_name: &str, message_id: &str) -> bool {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = messages.get_mut(agent_name) {
            if let Some(msg) = queue.iter_mut().find(|m| m.message_id == message_id) {
                msg.read = true;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Delete a specific message.
    pub fn delete(&self, agent_name: &str, message_id: &str) -> bool {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(queue) = messages.get_mut(agent_name) {
            let initial_len = queue.len();
            queue.retain(|m| m.message_id != message_id);
            queue.len() < initial_len
        } else {
            false
        }
    }

    /// Clear all messages for an agent.
    pub fn clear_agent_messages(&self, agent_name: &str) {
        let mut messages = self.messages.lock().unwrap_or_else(|e| e.into_inner());
        messages.remove(agent_name);
    }
}

impl Default for MessageStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global message store instance
use std::sync::OnceLock;

static GLOBAL_MESSAGE_STORE: OnceLock<MessageStore> = OnceLock::new();

/// Get the global message store instance.
pub fn get_message_store() -> MessageStore {
    GLOBAL_MESSAGE_STORE.get_or_init(MessageStore::new).clone()
}

/// Reset the global message store (for testing).
pub fn reset_message_store() {
    if let Some(store) = GLOBAL_MESSAGE_STORE.get() {
        store.clear();
    }
}

/// Input schema for the SendMessageTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SendMessageInput {
    /// The action to perform: "send", "receive", "peek", "list", or "mark_read".
    pub action: String,
    /// The sender agent name (for send action).
    #[serde(default)]
    pub from: Option<String>,
    /// The recipient agent name.
    #[serde(default)]
    pub to: Option<String>,
    /// The message content (for send action).
    #[serde(default)]
    pub content: Option<String>,
    /// The message priority: "low", "normal", "high", "urgent" (default: "normal").
    #[serde(default)]
    pub priority: Option<String>,
    /// The message type (default: "text").
    #[serde(default)]
    pub message_type: Option<String>,
    /// The agent name (for receive/peek/list actions).
    #[serde(default)]
    pub agent_name: Option<String>,
    /// The message ID (for mark_read action).
    #[serde(default)]
    pub message_id: Option<String>,
    /// Maximum number of messages to peek (default: 10).
    #[serde(default)]
    pub limit: Option<usize>,
}

/// The SendMessageTool enables inter-agent messaging.
#[derive(Debug, Clone, Default)]
pub struct SendMessageTool;

impl SendMessageTool {
    /// Create a new SendMessageTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for SendMessageTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "SendMessageTool",
                "Send and receive messages between agents",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let action = input
            .require("action")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "action must be a string".to_string(),
                error_code: Some(1),
            })?;

        let valid_actions = ["send", "receive", "peek", "list", "mark_read"];
        if !valid_actions.contains(&action) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Invalid action: {action}. Valid actions are: send, receive, peek, list, mark_read"
                ),
                error_code: Some(2),
            });
        }

        // Validate required fields for send
        if action == "send" {
            if input.get("from").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "from is required for send action".to_string(),
                    error_code: Some(3),
                });
            }
            if input.get("to").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "to is required for send action".to_string(),
                    error_code: Some(4),
                });
            }
            if input.get("content").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "content is required for send action".to_string(),
                    error_code: Some(5),
                });
            }

            // Validate priority if provided
            if let Some(priority) = input.get("priority").and_then(|v| v.as_str()) {
                if priority.parse::<MessagePriority>().is_err() {
                    return Err(ToolError::ValidationFailed {
                        message: format!(
                            "Invalid priority: {priority}. Valid priorities are: low, normal, high, urgent"
                        ),
                        error_code: Some(6),
                    });
                }
            }
        }

        // Validate required fields for receive/peek/list
        if action == "receive" || action == "peek" || action == "list" {
            if input.get("agent_name").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "agent_name is required for receive/peek/list actions".to_string(),
                    error_code: Some(7),
                });
            }
        }

        // Validate required fields for mark_read
        if action == "mark_read" {
            if input.get("agent_name").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "agent_name is required for mark_read action".to_string(),
                    error_code: Some(8),
                });
            }
            if input.get("message_id").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "message_id is required for mark_read action".to_string(),
                    error_code: Some(9),
                });
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: action")
                    .with_field("type", "validation_error");
            }
        };

        let store = get_message_store();

        match action {
            "send" => {
                let from = input
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let to = input
                    .get("to")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let content = input
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let priority_str = input
                    .get("priority")
                    .and_then(|v| v.as_str())
                    .unwrap_or("normal");
                let priority = priority_str
                    .parse::<MessagePriority>()
                    .unwrap_or(MessagePriority::Normal);
                let message_type = input
                    .get("message_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("text");

                if to.is_empty() {
                    return ToolOutput::new()
                        .with_field("error", "Recipient (to) cannot be empty")
                        .with_field("type", "validation_error");
                }

                let message = Message::new(from, to, content, priority, message_type);
                let message_id = message.message_id.clone();

                store.send(message);

                ToolOutput::new()
                    .with_field("message_id", &message_id)
                    .with_field("to", to)
                    .with_field("priority", priority_str)
                    .with_field("type", "message_sent")
            }
            "receive" => {
                let agent_name = match input.get("agent_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_name")
                            .with_field("type", "validation_error");
                    }
                };

                match store.receive(agent_name) {
                    Some(message) => ToolOutput::new()
                        .with_field("message_id", &message.message_id)
                        .with_field("from", &message.from)
                        .with_field("content", &message.content)
                        .with_field("priority", &message.priority)
                        .with_field("timestamp", message.timestamp)
                        .with_field("message_type", &message.message_type)
                        .with_field("type", "message_received"),
                    None => ToolOutput::new()
                        .with_field("agent_name", agent_name)
                        .with_field("has_message", false)
                        .with_field("type", "no_messages"),
                }
            }
            "peek" => {
                let agent_name = match input.get("agent_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_name")
                            .with_field("type", "validation_error");
                    }
                };
                let limit = input
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(10);

                let messages = store.peek(agent_name, limit);
                let count = messages.len();

                ToolOutput::new()
                    .with_field("agent_name", agent_name)
                    .with_field("messages", messages)
                    .with_field("count", count)
                    .with_field("type", "messages_peeked")
            }
            "list" => {
                let agent_name = match input.get("agent_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_name")
                            .with_field("type", "validation_error");
                    }
                };

                let total = store.count(agent_name);
                let messages = store.list(agent_name);

                ToolOutput::new()
                    .with_field("agent_name", agent_name)
                    .with_field("messages", messages)
                    .with_field("total", total)
                    .with_field("type", "message_list")
            }
            "mark_read" => {
                let agent_name = match input.get("agent_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_name")
                            .with_field("type", "validation_error");
                    }
                };
                let message_id = match input.get("message_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: message_id")
                            .with_field("type", "validation_error");
                    }
                };

                let success = store.mark_read(agent_name, message_id);

                ToolOutput::new()
                    .with_field("agent_name", agent_name)
                    .with_field("message_id", message_id)
                    .with_field("success", success)
                    .with_field("type", if success { "marked_read" } else { "not_found" })
            }
            _ => ToolOutput::new()
                .with_field("error", format!("Invalid action: {action}"))
                .with_field("type", "validation_error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Global mutex to ensure tests run serially
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_message_store();
        guard
    }

    #[tokio::test]
    async fn test_send_message_validation() {
        let tool = SendMessageTool::new();

        // Valid - send action
        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "agent1")
            .with_arg("to", "agent2")
            .with_arg("content", "Hello!");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - receive action
        let input = ToolInput::new()
            .with_arg("action", "receive")
            .with_arg("agent_name", "agent2");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - list action
        let input = ToolInput::new()
            .with_arg("action", "list")
            .with_arg("agent_name", "agent1");
        assert!(tool.validate(&input).await.is_ok());

        // Missing action
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Invalid action
        let input = ToolInput::new().with_arg("action", "invalid");
        assert!(tool.validate(&input).await.is_err());

        // Missing from for send
        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("to", "agent2")
            .with_arg("content", "Hello");
        assert!(tool.validate(&input).await.is_err());

        // Missing to for send
        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "agent1")
            .with_arg("content", "Hello");
        assert!(tool.validate(&input).await.is_err());

        // Missing content for send
        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "agent1")
            .with_arg("to", "agent2");
        assert!(tool.validate(&input).await.is_err());

        // Invalid priority
        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "agent1")
            .with_arg("to", "agent2")
            .with_arg("content", "Hello")
            .with_arg("priority", "invalid");
        assert!(tool.validate(&input).await.is_err());

        // Missing agent_name for receive
        let input = ToolInput::new().with_arg("action", "receive");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_send_message() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        let input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "sender-agent")
            .with_arg("to", "receiver-agent")
            .with_arg("content", "Test message content")
            .with_arg("priority", "high")
            .with_arg("message_type", "task");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("message_sent")
        );
        assert!(output.data.contains_key("message_id"));
        assert_eq!(
            output.data.get("to").and_then(|v| v.as_str()),
            Some("receiver-agent")
        );
        assert_eq!(
            output.data.get("priority").and_then(|v| v.as_str()),
            Some("high")
        );
    }

    #[tokio::test]
    async fn test_receive_message() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        // First send a message
        let send_input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "sender")
            .with_arg("to", "receiver")
            .with_arg("content", "Hello, receiver!");
        tool.execute(send_input).await;

        // Then receive it
        let receive_input = ToolInput::new()
            .with_arg("action", "receive")
            .with_arg("agent_name", "receiver");
        let receive_output = tool.execute(receive_input).await;

        assert_eq!(
            receive_output.data.get("type").and_then(|v| v.as_str()),
            Some("message_received")
        );
        assert_eq!(
            receive_output
                .data
                .get("from")
                .and_then(|v| v.as_str()),
            Some("sender")
        );
        assert_eq!(
            receive_output
                .data
                .get("content")
                .and_then(|v| v.as_str()),
            Some("Hello, receiver!")
        );
        assert_eq!(
            receive_output
                .data
                .get("message_type")
                .and_then(|v| v.as_str()),
            Some("text")
        );
    }

    #[tokio::test]
    async fn test_receive_no_messages() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        let input = ToolInput::new()
            .with_arg("action", "receive")
            .with_arg("agent_name", "empty-agent");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("no_messages")
        );
        assert_eq!(
            output.data.get("has_message").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[tokio::test]
    async fn test_peek_messages() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        // Send multiple messages
        for i in 0..3 {
            let send_input = ToolInput::new()
                .with_arg("action", "send")
                .with_arg("from", "sender")
                .with_arg("to", "peek-agent")
                .with_arg("content", format!("Message {}", i));
            tool.execute(send_input).await;
        }

        // Peek at messages without removing them
        let peek_input = ToolInput::new()
            .with_arg("action", "peek")
            .with_arg("agent_name", "peek-agent")
            .with_arg("limit", 2u64);
        let peek_output = tool.execute(peek_input).await;

        assert_eq!(
            peek_output.data.get("type").and_then(|v| v.as_str()),
            Some("messages_peeked")
        );
        assert_eq!(
            peek_output.data.get("count").and_then(|v| v.as_u64()),
            Some(2)
        );

        // Verify messages are still there by receiving
        let receive_input = ToolInput::new()
            .with_arg("action", "receive")
            .with_arg("agent_name", "peek-agent");
        let receive_output = tool.execute(receive_input).await;
        assert_eq!(
            receive_output.data.get("type").and_then(|v| v.as_str()),
            Some("message_received")
        );
    }

    #[tokio::test]
    async fn test_list_messages() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        // Send messages
        for i in 0..2 {
            let send_input = ToolInput::new()
                .with_arg("action", "send")
                .with_arg("from", "sender")
                .with_arg("to", "list-agent")
                .with_arg("content", format!("Message content {}", i));
            tool.execute(send_input).await;
        }

        // List messages
        let list_input = ToolInput::new()
            .with_arg("action", "list")
            .with_arg("agent_name", "list-agent");
        let list_output = tool.execute(list_input).await;

        assert_eq!(
            list_output.data.get("type").and_then(|v| v.as_str()),
            Some("message_list")
        );
        assert_eq!(
            list_output.data.get("total").and_then(|v| v.as_u64()),
            Some(2)
        );
    }

    #[tokio::test]
    async fn test_mark_read() {
        let _guard = setup();
        let tool = SendMessageTool::new();

        // Send a message
        let send_input = ToolInput::new()
            .with_arg("action", "send")
            .with_arg("from", "sender")
            .with_arg("to", "mark-read-agent")
            .with_arg("content", "Please mark me as read");
        let send_output = tool.execute(send_input).await;
        let message_id = send_output
            .data
            .get("message_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Mark as read
        let mark_input = ToolInput::new()
            .with_arg("action", "mark_read")
            .with_arg("agent_name", "mark-read-agent")
            .with_arg("message_id", &message_id);
        let mark_output = tool.execute(mark_input).await;

        assert_eq!(
            mark_output.data.get("type").and_then(|v| v.as_str()),
            Some("marked_read")
        );
        assert_eq!(
            mark_output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_priority_ordering() {
        let _guard = setup();
        let store = get_message_store();

        // Send messages with different priorities
        let low = Message::new("a", "test", "low", MessagePriority::Low, "text");
        let normal = Message::new("a", "test", "normal", MessagePriority::Normal, "text");
        let high = Message::new("a", "test", "high", MessagePriority::High, "text");
        let urgent = Message::new("a", "test", "urgent", MessagePriority::Urgent, "text");

        // Send in reverse priority order
        store.send(low.clone());
        store.send(normal.clone());
        store.send(high.clone());
        store.send(urgent.clone());

        // Receive should return urgent first
        let first = store.receive("test").unwrap();
        assert_eq!(first.priority, "urgent");

        // Then high
        let second = store.receive("test").unwrap();
        assert_eq!(second.priority, "high");

        // Then normal
        let third = store.receive("test").unwrap();
        assert_eq!(third.priority, "normal");

        // Then low
        let fourth = store.receive("test").unwrap();
        assert_eq!(fourth.priority, "low");
    }

    #[test]
    fn test_message_priority_parse() {
        assert!("low".parse::<MessagePriority>().is_ok());
        assert!("normal".parse::<MessagePriority>().is_ok());
        assert!("high".parse::<MessagePriority>().is_ok());
        assert!("urgent".parse::<MessagePriority>().is_ok());
        assert!("invalid".parse::<MessagePriority>().is_err());
    }

    #[test]
    fn test_message_store() {
        let _guard = setup();
        let store = get_message_store();

        let message = Message::new("agent1", "agent2", "Hello", MessagePriority::Normal, "text");
        let message_id = message.message_id.clone();

        store.send(message);

        assert_eq!(store.count("agent2"), 1);

        let received = store.receive("agent2");
        assert!(received.is_some());
        assert_eq!(received.unwrap().message_id, message_id);

        assert_eq!(store.count("agent2"), 0);
    }

    #[test]
    fn test_message_summary() {
        let message = Message::new("sender", "receiver", "This is a test message", MessagePriority::High, "task");
        let summary = MessageSummary::from(&message);

        assert_eq!(summary.from, "sender");
        assert_eq!(summary.priority, "high");
        assert_eq!(summary.read, false);
        assert_eq!(summary.preview, "This is a test message");
    }

    #[test]
    fn test_message_summary_long_content() {
        let long_content = "a".repeat(200);
        let message = Message::new("sender", "receiver", &long_content, MessagePriority::Normal, "text");
        let summary = MessageSummary::from(&message);

        assert_eq!(summary.preview.len(), 103); // 100 + "..."
        assert!(summary.preview.ends_with("..."));
    }
}
