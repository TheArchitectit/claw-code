//! Tool use context for tool execution.
//!
//! This module defines the `ToolUseContext` struct which provides tools
//! with access to conversation state, configuration, and callbacks.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::messages::{Message, ProgressData, SystemMessage};
use crate::permissions::ToolPermissionContext;
use crate::types::{SessionId, ToolUseId};
use crate::tool::ToolOutput;

pub mod cache;
pub mod mcp;
pub mod notifications;
pub mod types;

pub use cache::{FileState, FileSource, FileStateCache};
pub use mcp::{McpConnection, McpConnectionState, McpResource};
pub use notifications::{Notification, NotificationLevel, OsNotificationOptions, ToolJsxOptions};
pub use types::{
    AgentDefinition, AgentDefinitions, DenialTrackingState, FileReadingLimits, GlobLimits,
    QueryChainTracking, ThinkingConfig, ToolDecision, ToolDecisionOutcome,
};

/// Compute a content hash for file content.
fn compute_content_hash(content: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Context provided to tools during execution.
///
/// This struct contains all the state and callbacks that tools need
/// to interact with the conversation and system.
#[derive(Clone)]
pub struct ToolUseContext {
    /// The session ID.
    pub session_id: SessionId,

    /// The current working directory.
    pub cwd: PathBuf,

    /// The conversation messages.
    pub messages: Arc<RwLock<Vec<Message>>>,

    /// The application state (generic JSON for flexibility).
    pub app_state: Arc<RwLock<serde_json::Value>>,

    /// Callback to get the latest app state.
    pub get_app_state: Arc<dyn Fn() -> serde_json::Value + Send + Sync>,

    /// Callback to update the app state.
    pub set_app_state: Arc<dyn Fn(serde_json::Value) + Send + Sync>,

    /// The tool permission context.
    pub permission_context: ToolPermissionContext,

    /// Callback for setting tool JSX (UI components).
    pub set_tool_jsx: Arc<dyn Fn(Option<ToolJsxOptions>) + Send + Sync>,

    /// Callback for adding notifications.
    pub add_notification: Arc<dyn Fn(Notification) + Send + Sync>,

    /// Callback for appending system messages.
    pub append_system_message: Arc<dyn Fn(SystemMessage) + Send + Sync>,

    /// Callback for sending OS-level notifications.
    pub send_os_notification: Arc<dyn Fn(OsNotificationOptions) + Send + Sync>,

    /// In-progress tool use IDs.
    pub in_progress_tool_use_ids: Arc<Mutex<std::collections::HashSet<ToolUseId>>>,

    /// Callback for setting in-progress tool use IDs.
    pub set_in_progress_tool_use_ids: Arc<dyn Fn(std::collections::HashSet<ToolUseId>) + Send + Sync>,

    /// Callback for setting response length.
    pub set_response_length: Arc<dyn Fn(usize) + Send + Sync>,

    /// File reading limits.
    pub file_reading_limits: FileReadingLimits,

    /// Glob limits.
    pub glob_limits: GlobLimits,

    /// Tool decisions for this session.
    pub tool_decisions: Arc<RwLock<HashMap<String, ToolDecision>>>,

    /// Query tracking for subagent chains.
    pub query_tracking: Option<QueryChainTracking>,

    /// The current tool use ID if applicable.
    pub tool_use_id: Option<ToolUseId>,

    /// Critical system reminder (experimental).
    pub critical_system_reminder: Option<String>,

    /// Whether to preserve tool use results in transcripts.
    pub preserve_tool_use_results: bool,

    /// Local denial tracking for async subagents.
    pub local_denial_tracking: Arc<Mutex<DenialTrackingState>>,

    /// The rendered system prompt bytes.
    pub rendered_system_prompt: Option<Vec<u8>>,

    /// MCP client connections.
    pub mcp_clients: Arc<RwLock<Vec<McpConnection>>>,

    /// MCP resources.
    pub mcp_resources: Arc<RwLock<HashMap<String, Vec<McpResource>>>>,

    /// The main loop model.
    pub main_loop_model: String,

    /// Thinking configuration.
    pub thinking_config: ThinkingConfig,

    /// Whether this is a non-interactive session.
    pub is_non_interactive_session: bool,

    /// Agent definitions.
    pub agent_definitions: Arc<RwLock<AgentDefinitions>>,

    /// Maximum budget in USD.
    pub max_budget_usd: Option<f64>,

    /// Custom system prompt.
    pub custom_system_prompt: Option<String>,

    /// Additional system prompt to append.
    pub append_system_prompt: Option<String>,

    /// File state cache for tracking read files.
    pub file_state_cache: Arc<RwLock<FileStateCache>>,

    /// Nested memory attachment triggers.
    pub nested_memory_attachment_triggers: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Loaded nested memory paths (for deduplication).
    pub loaded_nested_memory_paths: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Dynamic skill directory triggers.
    pub dynamic_skill_dir_triggers: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Discovered skill names for telemetry.
    pub discovered_skill_names: Arc<Mutex<std::collections::HashSet<String>>>,

    /// Progress callback for the current tool.
    pub on_progress: Arc<dyn Fn(ProgressData) + Send + Sync>,

    /// Cumulative tool outputs tracked during session.
    pub cumulative_tool_outputs: Arc<RwLock<HashMap<String, Vec<ToolOutput>>>>,
}

impl ToolUseContext {
    /// Create a new tool use context with the given session ID and working directory.
    #[must_use]
    pub fn new(session_id: SessionId, cwd: impl Into<PathBuf>) -> Self {
        let cwd = cwd.into();

        Self {
            session_id,
            cwd,
            messages: Arc::new(RwLock::new(Vec::new())),
            app_state: Arc::new(RwLock::new(serde_json::Value::Object(serde_json::Map::new()))),
            get_app_state: Arc::new(move || serde_json::Value::Object(serde_json::Map::new())),
            set_app_state: Arc::new(|_| {}),
            permission_context: ToolPermissionContext::new(),
            set_tool_jsx: Arc::new(|_| {}),
            add_notification: Arc::new(|_| {}),
            append_system_message: Arc::new(|_| {}),
            send_os_notification: Arc::new(|_| {}),
            in_progress_tool_use_ids: Arc::new(Mutex::new(std::collections::HashSet::new())),
            set_in_progress_tool_use_ids: Arc::new(|_| {}),
            set_response_length: Arc::new(|_| {}),
            file_reading_limits: FileReadingLimits::default(),
            glob_limits: GlobLimits::default(),
            tool_decisions: Arc::new(RwLock::new(HashMap::new())),
            query_tracking: None,
            tool_use_id: None,
            critical_system_reminder: None,
            preserve_tool_use_results: false,
            local_denial_tracking: Arc::new(Mutex::new(DenialTrackingState::default())),
            rendered_system_prompt: None,
            mcp_clients: Arc::new(RwLock::new(Vec::new())),
            mcp_resources: Arc::new(RwLock::new(HashMap::new())),
            main_loop_model: String::new(),
            thinking_config: ThinkingConfig::default(),
            is_non_interactive_session: true,
            agent_definitions: Arc::new(RwLock::new(AgentDefinitions::default())),
            max_budget_usd: None,
            custom_system_prompt: None,
            append_system_prompt: None,
            file_state_cache: Arc::new(RwLock::new(FileStateCache::default())),
            nested_memory_attachment_triggers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            loaded_nested_memory_paths: Arc::new(Mutex::new(std::collections::HashSet::new())),
            dynamic_skill_dir_triggers: Arc::new(Mutex::new(std::collections::HashSet::new())),
            discovered_skill_names: Arc::new(Mutex::new(std::collections::HashSet::new())),
            on_progress: Arc::new(|_| {}),
            cumulative_tool_outputs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the permission context.
    pub fn with_permission_context(mut self, ctx: ToolPermissionContext) -> Self {
        self.permission_context = ctx;
        self
    }

    /// Set the messages.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = Arc::new(RwLock::new(messages));
        self
    }

    /// Set the main loop model.
    pub fn with_main_loop_model(mut self, model: impl Into<String>) -> Self {
        self.main_loop_model = model.into();
        self
    }

    /// Set whether this is a non-interactive session.
    pub fn with_non_interactive(mut self, non_interactive: bool) -> Self {
        self.is_non_interactive_session = non_interactive;
        self
    }

    /// Set the rendered system prompt.
    pub fn with_rendered_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.rendered_system_prompt = Some(prompt.into().into_bytes());
        self
    }

    /// Add an in-progress tool use ID.
    pub async fn add_in_progress_tool(&self, id: ToolUseId) {
        let mut guard: tokio::sync::MutexGuard<'_, std::collections::HashSet<ToolUseId>> =
            self.in_progress_tool_use_ids.lock().await;
        guard.insert(id);
    }

    /// Remove an in-progress tool use ID.
    pub async fn remove_in_progress_tool(&self, id: &ToolUseId) {
        let mut guard: tokio::sync::MutexGuard<'_, std::collections::HashSet<ToolUseId>> =
            self.in_progress_tool_use_ids.lock().await;
        guard.remove(id);
    }

    /// Report progress for the current tool.
    pub fn report_progress(&self, data: ProgressData) {
        (self.on_progress)(data);
    }

    /// Record a tool decision.
    pub async fn record_decision(&self, tool_name: String, decision: ToolDecisionOutcome) {
        let mut guard: tokio::sync::RwLockWriteGuard<'_, HashMap<String, ToolDecision>> =
            self.tool_decisions.write().await;
        guard.insert(
            tool_name,
            ToolDecision {
                source: "user".to_string(),
                decision,
                timestamp: chrono::Utc::now().timestamp_millis() as u64,
            },
        );
    }

    /// Get the current messages.
    pub async fn get_messages(&self) -> Vec<Message> {
        let _guard: tokio::sync::RwLockReadGuard<'_, Vec<Message>> = self.messages.read().await;
        _guard.clone()
    }

    /// Add a message to the conversation.
    pub async fn add_message(&self, message: Message) {
        let mut guard = self.messages.write().await;
        guard.push(message);
    }

    /// Record a file read operation.
    ///
    /// Reads the file, computes its content hash, and updates the file state cache.
    /// Returns the content hash on success, or an error if the file cannot be read.
    pub async fn record_file_read(&self, path: &std::path::Path) -> Result<String, std::io::Error> {
        // Read the file content
        let content = tokio::fs::read(path).await?;

        // Compute the hash
        let hash = compute_content_hash(&content);

        // Update the cache
        let mut cache = self.file_state_cache.write().await;
        cache.add_file(path, &hash);

        Ok(hash)
    }

    /// Record a glob operation.
    ///
    /// Tracks the pattern and its results in the file state cache.
    pub async fn record_glob_results(&self, pattern: &str, results: &[PathBuf]) {
        let mut cache = self.file_state_cache.write().await;
        for path in results {
            // For glob results, we don't have content yet - just mark as discovered
            cache.add_glob_result(path.clone(), pattern.to_string());
        }
    }

    /// Update agent definitions.
    ///
    /// Replaces the current agent definitions with new ones.
    pub async fn update_agent_definitions(&self, agents: AgentDefinitions) {
        let mut guard = self.agent_definitions.write().await;
        *guard = agents;
    }

    /// Track cumulative tool outputs.
    ///
    /// Records a tool output for the given tool name, accumulating outputs over the session.
    pub async fn record_tool_output(&self, tool_name: &str, output: ToolOutput) {
        let mut guard = self.cumulative_tool_outputs.write().await;
        guard
            .entry(tool_name.to_string())
            .or_default()
            .push(output);
    }

    /// Get current file state cache.
    ///
    /// Returns a clone of the current file state cache.
    pub async fn get_file_cache(&self) -> FileStateCache {
        self.file_state_cache.read().await.clone()
    }

    /// Get MCP resources for a server.
    ///
    /// Returns the resources for the given MCP server name.
    pub async fn get_mcp_resources(&self, server_name: &str) -> Option<Vec<McpResource>> {
        let guard = self.mcp_resources.read().await;
        guard.get(server_name).cloned()
    }

    /// Update MCP resources for a server.
    ///
    /// Sets the resources for the given MCP server name.
    pub async fn update_mcp_resources(&self, server_name: impl Into<String>, resources: Vec<McpResource>) {
        let mut guard = self.mcp_resources.write().await;
        guard.insert(server_name.into(), resources);
    }

    /// Update MCP connection state.
    ///
    /// Updates the state of an MCP connection by name.
    pub async fn update_mcp_connection_state(&self, name: &str, state: McpConnectionState) {
        let mut guard = self.mcp_clients.write().await;
        if let Some(conn) = guard.iter_mut().find(|c| c.name == name) {
            conn.state = state;
        }
    }

    /// Get active MCP connections.
    ///
    /// Returns a list of all MCP connections with their current states.
    pub async fn get_mcp_connections(&self) -> Vec<McpConnection> {
        self.mcp_clients.read().await.clone()
    }

    /// Add an MCP connection.
    ///
    /// Adds a new MCP connection to the context.
    pub async fn add_mcp_connection(&self, connection: McpConnection) {
        let mut guard = self.mcp_clients.write().await;
        guard.push(connection);
    }

    /// Update query tracking.
    ///
    /// Sets the query chain tracking for subagent chains.
    pub fn update_query_tracking(&mut self, tracking: QueryChainTracking) {
        self.query_tracking = Some(tracking);
    }

    /// Get cumulative tool outputs for a specific tool.
    ///
    /// Returns all recorded outputs for the given tool name.
    pub async fn get_tool_outputs(&self, tool_name: &str) -> Vec<ToolOutput> {
        let guard = self.cumulative_tool_outputs.read().await;
        guard.get(tool_name).cloned().unwrap_or_default()
    }
}
