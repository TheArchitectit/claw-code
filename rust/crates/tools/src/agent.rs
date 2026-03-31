//! AgentTool - Spawn and manage sub-agents.
//!
//! This tool enables the multi-agent coordinator system by allowing
//! agents to spawn sub-agents with color coding and task descriptions.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Color coding for agents to distinguish them visually.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentColor {
    #[serde(rename = "blue")]
    Blue,
    #[serde(rename = "pink")]
    Pink,
    #[serde(rename = "green")]
    Green,
    #[serde(rename = "yellow")]
    Yellow,
    #[serde(rename = "red")]
    Red,
    #[serde(rename = "purple")]
    Purple,
    #[serde(rename = "orange")]
    Orange,
    #[serde(rename = "cyan")]
    Cyan,
}

impl AgentColor {
    /// Get the color as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentColor::Blue => "blue",
            AgentColor::Pink => "pink",
            AgentColor::Green => "green",
            AgentColor::Yellow => "yellow",
            AgentColor::Red => "red",
            AgentColor::Purple => "purple",
            AgentColor::Orange => "orange",
            AgentColor::Cyan => "cyan",
        }
    }
}

impl std::str::FromStr for AgentColor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "blue" => Ok(AgentColor::Blue),
            "pink" => Ok(AgentColor::Pink),
            "green" => Ok(AgentColor::Green),
            "yellow" => Ok(AgentColor::Yellow),
            "red" => Ok(AgentColor::Red),
            "purple" => Ok(AgentColor::Purple),
            "orange" => Ok(AgentColor::Orange),
            "cyan" => Ok(AgentColor::Cyan),
            _ => Err(format!("Invalid color: {s}")),
        }
    }
}

/// Status of an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "stopped")]
    Stopped,
}

impl AgentStatus {
    /// Get the status as a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Idle => "idle",
            AgentStatus::Running => "running",
            AgentStatus::Completed => "completed",
            AgentStatus::Failed => "failed",
            AgentStatus::Stopped => "stopped",
        }
    }
}

/// Information about a spawned agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInfo {
    pub agent_id: String,
    pub name: String,
    pub color: String,
    pub description: String,
    pub status: AgentStatus,
    pub parent_agent: Option<String>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl AgentInfo {
    /// Create a new agent info.
    pub fn new(
        name: impl Into<String>,
        color: AgentColor,
        description: impl Into<String>,
        parent_agent: Option<String>,
    ) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let random_suffix = rand::random::<u16>();
        let agent_id = format!("agent-{:x}-{:x}", now, random_suffix);

        Self {
            agent_id,
            name: name.into(),
            color: color.as_str().to_string(),
            description: description.into(),
            status: AgentStatus::Idle,
            parent_agent,
            created_at: now,
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
        }
    }

    /// Set the status and update timestamps accordingly.
    pub fn set_status(&mut self, status: AgentStatus) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.status = status.clone();
        match status {
            AgentStatus::Running => {
                self.started_at = Some(now);
            }
            AgentStatus::Completed | AgentStatus::Failed | AgentStatus::Stopped => {
                self.completed_at = Some(now);
            }
            _ => {}
        }
    }
}

/// The agent store for managing agent lifecycle.
#[derive(Debug, Clone)]
pub struct AgentStore {
    agents: Arc<Mutex<HashMap<String, AgentInfo>>>,
}

impl AgentStore {
    /// Create a new empty agent store.
    pub fn new() -> Self {
        Self {
            agents: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all agents.
    pub fn clear(&self) {
        let mut agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        agents.clear();
    }

    /// Create a new agent.
    pub fn create(
        &self,
        name: impl Into<String>,
        color: AgentColor,
        description: impl Into<String>,
        parent_agent: Option<String>,
    ) -> AgentInfo {
        let agent = AgentInfo::new(name, color, description, parent_agent);
        let mut agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        agents.insert(agent.agent_id.clone(), agent.clone());
        agent
    }

    /// Get an agent by ID.
    pub fn get(&self, agent_id: &str) -> Option<AgentInfo> {
        let agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        agents.get(agent_id).cloned()
    }

    /// Update an agent.
    pub fn update<F>(&self, agent_id: &str, f: F) -> Option<AgentInfo>
    where
        F: FnOnce(&mut AgentInfo),
    {
        let mut agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(agent) = agents.get_mut(agent_id) {
            f(agent);
            Some(agent.clone())
        } else {
            None
        }
    }

    /// List all agents, optionally filtered by parent.
    pub fn list(&self, parent_agent: Option<&str>) -> Vec<AgentInfo> {
        let agents = self.agents.lock().unwrap_or_else(|e| e.into_inner());
        agents
            .values()
            .filter(|a| {
                if let Some(parent) = parent_agent {
                    a.parent_agent.as_ref().map(|p| p == parent).unwrap_or(false)
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    /// Stop an agent.
    pub fn stop(&self, agent_id: &str) -> Option<AgentInfo> {
        self.update(agent_id, |agent| {
            agent.set_status(AgentStatus::Stopped);
        })
    }

    /// Set agent result.
    pub fn set_result(&self, agent_id: &str, result: impl Into<String>) -> Option<AgentInfo> {
        self.update(agent_id, |agent| {
            agent.result = Some(result.into());
            agent.set_status(AgentStatus::Completed);
        })
    }

    /// Set agent error.
    pub fn set_error(&self, agent_id: &str, error: impl Into<String>) -> Option<AgentInfo> {
        self.update(agent_id, |agent| {
            agent.error = Some(error.into());
            agent.set_status(AgentStatus::Failed);
        })
    }
}

impl Default for AgentStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global agent store instance
use std::sync::OnceLock;

static GLOBAL_AGENT_STORE: OnceLock<AgentStore> = OnceLock::new();

/// Get the global agent store instance.
pub fn get_agent_store() -> AgentStore {
    GLOBAL_AGENT_STORE.get_or_init(AgentStore::new).clone()
}

/// Reset the global agent store (for testing).
pub fn reset_agent_store() {
    if let Some(store) = GLOBAL_AGENT_STORE.get() {
        store.clear();
    }
}

/// Input schema for the AgentTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentInput {
    /// The action to perform: "create", "run", "stop", "status", or "list".
    pub action: String,
    /// The agent name (required for create/run).
    #[serde(default)]
    pub name: Option<String>,
    /// The color for the agent (required for create/run).
    #[serde(default)]
    pub color: Option<String>,
    /// The task description (required for create/run).
    #[serde(default)]
    pub description: Option<String>,
    /// The agent ID (required for stop/status).
    #[serde(default)]
    pub agent_id: Option<String>,
    /// Optional parent agent ID.
    #[serde(default)]
    pub parent_agent: Option<String>,
}

/// The AgentTool spawns and manages sub-agents.
#[derive(Debug, Clone, Default)]
pub struct AgentTool;

impl AgentTool {
    /// Create a new AgentTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "AgentTool",
                "Spawn and manage sub-agents with color coding and task descriptions",
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

        let valid_actions = ["create", "run", "stop", "status", "list"];
        if !valid_actions.contains(&action) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Invalid action: {action}. Valid actions are: create, run, stop, status, list"
                ),
                error_code: Some(2),
            });
        }

        // Validate required fields for create/run
        if action == "create" || action == "run" {
            if input.get("name").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "name is required for create/run actions".to_string(),
                    error_code: Some(3),
                });
            }
            if input.get("color").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "color is required for create/run actions".to_string(),
                    error_code: Some(4),
                });
            }
            if let Some(color) = input.get("color").and_then(|v| v.as_str()) {
                if color.parse::<AgentColor>().is_err() {
                    return Err(ToolError::ValidationFailed {
                        message: format!(
                            "Invalid color: {color}. Valid colors are: blue, pink, green, yellow, red, purple, orange, cyan"
                        ),
                        error_code: Some(5),
                    });
                }
            }
        }

        // Validate required fields for stop/status
        if action == "stop" || action == "status" {
            if input.get("agent_id").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "agent_id is required for stop/status actions".to_string(),
                    error_code: Some(6),
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

        let store = get_agent_store();

        match action {
            "create" => {
                let name = input
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unnamed Agent");
                let color_str = input
                    .get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or("blue");
                let color = color_str.parse::<AgentColor>().unwrap_or(AgentColor::Blue);
                let description = input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let parent_agent = input
                    .get("parent_agent")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let agent = store.create(name, color, description, parent_agent);

                ToolOutput::new()
                    .with_field("agent_id", &agent.agent_id)
                    .with_field("name", &agent.name)
                    .with_field("color", &agent.color)
                    .with_field("status", agent.status.as_str())
                    .with_field("type", "agent_created")
            }
            "run" => {
                let name = input
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unnamed Agent");
                let color_str = input
                    .get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or("blue");
                let color = color_str.parse::<AgentColor>().unwrap_or(AgentColor::Blue);
                let description = input
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let parent_agent = input
                    .get("parent_agent")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let agent = store.create(name, color, description, parent_agent);
                let agent_id = agent.agent_id.clone();

                // Mark agent as running
                store.update(&agent_id, |a| {
                    a.set_status(AgentStatus::Running);
                });

                // In a real implementation, this would spawn the agent process
                // For now, we simulate success

                ToolOutput::new()
                    .with_field("agent_id", &agent_id)
                    .with_field("name", &agent.name)
                    .with_field("color", &agent.color)
                    .with_field("status", "running")
                    .with_field("type", "agent_started")
            }
            "stop" => {
                let agent_id = match input.get("agent_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_id")
                            .with_field("type", "validation_error");
                    }
                };

                match store.stop(agent_id) {
                    Some(agent) => ToolOutput::new()
                        .with_field("agent_id", &agent.agent_id)
                        .with_field("name", &agent.name)
                        .with_field("status", agent.status.as_str())
                        .with_field("type", "agent_stopped"),
                    None => ToolOutput::new()
                        .with_field("error", format!("Agent not found: {agent_id}"))
                        .with_field("type", "not_found"),
                }
            }
            "status" => {
                let agent_id = match input.get("agent_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: agent_id")
                            .with_field("type", "validation_error");
                    }
                };

                match store.get(agent_id) {
                    Some(agent) => ToolOutput::new()
                        .with_field("agent_id", &agent.agent_id)
                        .with_field("name", &agent.name)
                        .with_field("color", &agent.color)
                        .with_field("status", agent.status.as_str())
                        .with_field("description", &agent.description)
                        .with_field("created_at", agent.created_at)
                        .with_field("started_at", agent.started_at.unwrap_or(0))
                        .with_field("completed_at", agent.completed_at.unwrap_or(0))
                        .with_field("type", "agent_status"),
                    None => ToolOutput::new()
                        .with_field("error", format!("Agent not found: {agent_id}"))
                        .with_field("type", "not_found"),
                }
            }
            "list" => {
                let parent_agent = input
                    .get("parent_agent")
                    .and_then(|v| v.as_str());
                let agents = store.list(parent_agent);
                let total = agents.len();

                ToolOutput::new()
                    .with_field("agents", agents)
                    .with_field("total", total)
                    .with_field("type", "agent_list")
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
        reset_agent_store();
        guard
    }

    #[tokio::test]
    async fn test_agent_tool_validation() {
        let tool = AgentTool::new();

        // Valid - create action
        let input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Test Agent")
            .with_arg("color", "blue");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - run action
        let input = ToolInput::new()
            .with_arg("action", "run")
            .with_arg("name", "Test Agent")
            .with_arg("color", "green");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - list action
        let input = ToolInput::new().with_arg("action", "list");
        assert!(tool.validate(&input).await.is_ok());

        // Missing action
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Invalid action
        let input = ToolInput::new().with_arg("action", "invalid");
        assert!(tool.validate(&input).await.is_err());

        // Missing name for create
        let input = ToolInput::new().with_arg("action", "create").with_arg("color", "blue");
        assert!(tool.validate(&input).await.is_err());

        // Missing color for create
        let input = ToolInput::new().with_arg("action", "create").with_arg("name", "Test");
        assert!(tool.validate(&input).await.is_err());

        // Invalid color
        let input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Test")
            .with_arg("color", "invalid_color");
        assert!(tool.validate(&input).await.is_err());

        // Missing agent_id for stop
        let input = ToolInput::new().with_arg("action", "stop");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_agent_tool_create() {
        let _guard = setup();
        let tool = AgentTool::new();

        let input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Test Agent")
            .with_arg("color", "pink")
            .with_arg("description", "A test agent");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_created")
        );
        assert!(output.data.contains_key("agent_id"));
        assert_eq!(
            output.data.get("name").and_then(|v| v.as_str()),
            Some("Test Agent")
        );
        assert_eq!(
            output.data.get("color").and_then(|v| v.as_str()),
            Some("pink")
        );
        assert_eq!(
            output.data.get("status").and_then(|v| v.as_str()),
            Some("idle")
        );
    }

    #[tokio::test]
    async fn test_agent_tool_run() {
        let _guard = setup();
        let tool = AgentTool::new();

        let input = ToolInput::new()
            .with_arg("action", "run")
            .with_arg("name", "Worker Agent")
            .with_arg("color", "green")
            .with_arg("description", "Processing task");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_started")
        );
        assert!(output.data.contains_key("agent_id"));
        assert_eq!(
            output.data.get("status").and_then(|v| v.as_str()),
            Some("running")
        );
    }

    #[tokio::test]
    async fn test_agent_tool_status() {
        let _guard = setup();
        let tool = AgentTool::new();

        // First create an agent
        let create_input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Status Test Agent")
            .with_arg("color", "yellow");

        let create_output = tool.execute(create_input).await;
        let agent_id = create_output
            .data
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Get status
        let status_input = ToolInput::new().with_arg("action", "status").with_arg("agent_id", &agent_id);
        let status_output = tool.execute(status_input).await;

        assert_eq!(
            status_output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_status")
        );
        assert_eq!(
            status_output.data.get("name").and_then(|v| v.as_str()),
            Some("Status Test Agent")
        );
    }

    #[tokio::test]
    async fn test_agent_tool_stop() {
        let _guard = setup();
        let tool = AgentTool::new();

        // Create and run an agent
        let run_input = ToolInput::new()
            .with_arg("action", "run")
            .with_arg("name", "Stop Test Agent")
            .with_arg("color", "red");

        let run_output = tool.execute(run_input).await;
        let agent_id = run_output
            .data
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Stop the agent
        let stop_input = ToolInput::new()
            .with_arg("action", "stop")
            .with_arg("agent_id", &agent_id);
        let stop_output = tool.execute(stop_input).await;

        assert_eq!(
            stop_output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_stopped")
        );
        assert_eq!(
            stop_output.data.get("status").and_then(|v| v.as_str()),
            Some("stopped")
        );
    }

    #[tokio::test]
    async fn test_agent_tool_list() {
        let _guard = setup();
        let tool = AgentTool::new();

        // Create multiple agents
        for i in 0..3 {
            let input = ToolInput::new()
                .with_arg("action", "create")
                .with_arg("name", format!("List Agent {}", i))
                .with_arg("color", "purple");
            tool.execute(input).await;
        }

        // List all agents
        let list_input = ToolInput::new().with_arg("action", "list");
        let list_output = tool.execute(list_input).await;

        assert_eq!(
            list_output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_list")
        );
        let total = list_output
            .data
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert!(total >= 3, "Expected at least 3 agents");
    }

    #[tokio::test]
    async fn test_agent_tool_not_found() {
        let tool = AgentTool::new();

        let input = ToolInput::new()
            .with_arg("action", "status")
            .with_arg("agent_id", "nonexistent-agent-99999");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("not_found")
        );
        assert!(output.data.get("error").is_some());
    }

    #[tokio::test]
    async fn test_agent_tool_parent_agent() {
        let _guard = setup();
        let tool = AgentTool::new();

        // Create parent agent
        let parent_input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Parent Agent")
            .with_arg("color", "cyan");
        let parent_output = tool.execute(parent_input).await;
        let parent_id = parent_output
            .data
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();

        // Create child agent
        let child_input = ToolInput::new()
            .with_arg("action", "create")
            .with_arg("name", "Child Agent")
            .with_arg("color", "orange")
            .with_arg("parent_agent", &parent_id);
        let child_output = tool.execute(child_input).await;

        assert_eq!(
            child_output.data.get("type").and_then(|v| v.as_str()),
            Some("agent_created")
        );

        // List agents with parent filter
        let list_input = ToolInput::new()
            .with_arg("action", "list")
            .with_arg("parent_agent", &parent_id);
        let list_output = tool.execute(list_input).await;

        let total = list_output
            .data
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        assert_eq!(total, 1, "Expected 1 child agent");
    }

    #[test]
    fn test_agent_color_parse() {
        assert!("blue".parse::<AgentColor>().is_ok());
        assert!("pink".parse::<AgentColor>().is_ok());
        assert!("green".parse::<AgentColor>().is_ok());
        assert!("yellow".parse::<AgentColor>().is_ok());
        assert!("red".parse::<AgentColor>().is_ok());
        assert!("purple".parse::<AgentColor>().is_ok());
        assert!("orange".parse::<AgentColor>().is_ok());
        assert!("cyan".parse::<AgentColor>().is_ok());
        assert!("invalid".parse::<AgentColor>().is_err());
    }

    #[test]
    fn test_agent_store() {
        let _guard = setup();
        let store = get_agent_store();

        let agent = store.create("Test", AgentColor::Blue, "Description", None);
        assert!(!agent.agent_id.is_empty());
        assert_eq!(agent.name, "Test");
        assert_eq!(agent.color, "blue");

        let retrieved = store.get(&agent.agent_id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test");
    }
}
