//! # Permissions Crate
//!
//! The R.A.D Codicological 2.x permission system provides security and access control
//! for tool execution.
//!
//! ## Permission Modes
//!
//! - [`PermissionMode::Default`] - Normal interactive approval flow
//! - [`PermissionMode::Plan`] - Planning mode (validate without executing)
//! - [`PermissionMode::BypassPermissions`] - Skip all permission checks
//! - [`PermissionMode::Auto`] - Automatically approve based on configuration
//!
//! ## Core Components
//!
//! - [`PermissionManager`] - Central coordinator for permission decisions
//! - [`PermissionRequest`] - Represents a pending permission request
//! - [`PermissionDecision`] - Outcome of a permission check (allow/deny)
//! - [`PermissionStore`] - Persistence for permission decisions
//! - [`PermissionHook`] - Extension point for custom permission logic
//!
//! ## Sandboxed Execution
//!
//! The [`Sandbox`] module provides isolated execution for dangerous tools like
//! [`BashTool`](tools::BashTool) and [`FileWriteTool`](tools::FileWriteTool).
//!
//! ## Usage
//!
//! ```rust
//! use permissions::{PermissionManager, PermissionConfig, PermissionMode, PermissionRequest};
//! use tools::{ToolInput, BashTool};
//!
//! # tokio_test::block_on(async {
//! let config = PermissionConfig::new().with_mode(PermissionMode::Default);
//! let manager = PermissionManager::new(config);
//! let tool = BashTool::new();
//! let input = ToolInput::new().with_arg("command", "echo hello");
//!
//! let request = PermissionRequest::new(&tool, input, "session-123");
//! let decision = manager.evaluate(request).await;
//! # });
//! ```

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, info, warn};

// Re-export types
pub mod decision;
pub mod hook;
pub mod mode;
pub mod request;
pub mod sandbox;
pub mod store;

pub use decision::{PermissionDecision, PermissionDecisionReason};
pub use hook::{PermissionHook, PermissionHookResult};
pub use mode::PermissionMode;
pub use request::PermissionRequest;
pub use sandbox::{Sandbox, SandboxConfig, SandboxError, SandboxResult};
pub use store::{PermissionRecord, PermissionStore, PersistenceError};

use tools::{Tool, ToolInput, ToolMetadata};

/// Errors that can occur in the permission system.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum PermissionError {
    /// Permission was denied.
    #[error("Permission denied: {message}")]
    Denied { message: String },

    /// The permission request was cancelled.
    #[error("Permission request cancelled")]
    Cancelled,

    /// Invalid permission configuration.
    #[error("Invalid permission configuration: {message}")]
    InvalidConfig { message: String },

    /// Persistence error.
    #[error("Persistence error: {message}")]
    Persistence { message: String },

    /// Sandbox error.
    #[error("Sandbox error: {0}")]
    Sandbox(String),

    /// The operation timed out.
    #[error("Permission request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Internal error.
    #[error("Internal permission error: {message}")]
    Internal { message: String },
}

/// Result type for permission operations.
pub type PermissionResult<T> = Result<T, PermissionError>;

impl From<store::PersistenceError> for PermissionError {
    fn from(err: store::PersistenceError) -> Self {
        match err {
            store::PersistenceError::NotFound => PermissionError::InvalidConfig {
                message: "Permission record not found".to_string(),
            },
            _ => PermissionError::Persistence {
                message: err.to_string(),
            },
        }
    }
}

/// A pending permission that is awaiting resolution.
#[derive(Debug, Clone)]
pub struct PendingPermission {
    /// Unique identifier for this permission request.
    pub id: String,
    /// The tool being requested.
    pub tool_name: String,
    /// The input to the tool.
    pub input: ToolInput,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// The session ID this request belongs to.
    pub session_id: String,
}

/// Configuration for the permission system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionConfig {
    /// The default permission mode.
    pub mode: PermissionMode,
    /// Tools that are automatically approved.
    pub auto_approve: Vec<String>,
    /// Tools that require explicit approval.
    pub require_approval: Vec<String>,
    /// Whether to persist permission decisions.
    pub persistence_enabled: bool,
    /// Path to the permission storage.
    pub storage_path: Option<PathBuf>,
    /// Sandbox configuration.
    pub sandbox: SandboxConfig,
    /// Timeout for permission requests in milliseconds.
    pub request_timeout_ms: u64,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            mode: PermissionMode::Default,
            auto_approve: vec![
                "FileReadTool".to_string(),
                "GlobTool".to_string(),
                "GrepTool".to_string(),
            ],
            require_approval: vec![
                "BashTool".to_string(),
                "FileWriteTool".to_string(),
                "FileEditTool".to_string(),
            ],
            persistence_enabled: true,
            storage_path: None,
            sandbox: SandboxConfig::default(),
            request_timeout_ms: 300_000, // 5 minutes
        }
    }
}

impl PermissionConfig {
    /// Create a new default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the permission mode.
    #[must_use]
    pub fn with_mode(mut self, mode: PermissionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Add a tool to the auto-approve list.
    #[must_use]
    pub fn with_auto_approve(mut self, tool: impl Into<String>) -> Self {
        self.auto_approve.push(tool.into());
        self
    }

    /// Add a tool to the require-approval list.
    #[must_use]
    pub fn with_require_approval(mut self, tool: impl Into<String>) -> Self {
        self.require_approval.push(tool.into());
        self
    }

    /// Set whether persistence is enabled.
    #[must_use]
    pub fn with_persistence(mut self, enabled: bool) -> Self {
        self.persistence_enabled = enabled;
        self
    }

    /// Set the storage path.
    #[must_use]
    pub fn with_storage_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.storage_path = Some(path.into());
        self
    }

    /// Set the sandbox configuration.
    #[must_use]
    pub fn with_sandbox(mut self, config: SandboxConfig) -> Self {
        self.sandbox = config;
        self
    }

    /// Set the request timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.request_timeout_ms = timeout_ms;
        self
    }

    /// Check if a tool is in the auto-approve list.
    #[must_use]
    pub fn is_auto_approved(&self, tool_name: &str) -> bool {
        self.auto_approve.iter().any(|t| t == tool_name)
    }

    /// Check if a tool requires explicit approval.
    #[must_use]
    pub fn requires_approval(&self, tool_name: &str) -> bool {
        self.require_approval.iter().any(|t| t == tool_name)
    }
}

/// Context passed during permission evaluation.
#[derive(Debug, Clone)]
pub struct PermissionContext {
    /// The session ID.
    pub session_id: String,
    /// The working directory.
    pub cwd: PathBuf,
    /// Additional context data.
    pub data: HashMap<String, String>,
}

impl PermissionContext {
    /// Create a new permission context.
    #[must_use]
    pub fn new(session_id: impl Into<String>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            session_id: session_id.into(),
            cwd: cwd.into(),
            data: HashMap::new(),
        }
    }

    /// Add context data.
    #[must_use]
    pub fn with_data(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.data.insert(key.into(), value.into());
        self
    }
}

/// The central permission manager that coordinates all permission decisions.
pub struct PermissionManager {
    /// The current configuration.
    config: PermissionConfig,
    /// The permission store for persistence.
    store: Arc<dyn PermissionStore>,
    /// Registered permission hooks.
    hooks: Vec<Box<dyn PermissionHook>>,
    /// Pending permission requests.
    pending: Arc<DashMap<String, PendingPermission>>,
    /// Completed permission decisions (cached).
    cache: Arc<DashMap<String, PermissionDecision>>,
}

impl fmt::Debug for PermissionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PermissionManager")
            .field("config", &self.config)
            .field("hooks_count", &self.hooks.len())
            .field("pending_count", &self.pending.len())
            .field("cache_count", &self.cache.len())
            .finish()
    }
}

impl PermissionManager {
    /// Create a new permission manager with the given configuration.
    #[must_use]
    pub fn new(config: PermissionConfig) -> Self {
        let store = Arc::new(store::MemoryPermissionStore::new());
        Self::with_store(config, store)
    }

    /// Create a new permission manager with a specific store.
    #[must_use]
    pub fn with_store(config: PermissionConfig, store: Arc<dyn PermissionStore>) -> Self {
        Self {
            config,
            store,
            hooks: Vec::new(),
            pending: Arc::new(DashMap::new()),
            cache: Arc::new(DashMap::new()),
        }
    }

    /// Register a permission hook.
    pub fn register_hook<H>(&mut self, hook: H)
    where
        H: PermissionHook + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    /// Get the current configuration.
    #[must_use]
    pub fn config(&self) -> &PermissionConfig {
        &self.config
    }

    /// Update the configuration.
    pub fn set_config(&mut self, config: PermissionConfig) {
        self.config = config;
    }

    /// Check if a tool requires permission based on configuration.
    #[must_use]
    pub fn requires_permission(&self, tool_name: &str) -> bool {
        // In bypass mode, nothing requires permission
        if self.config.mode == PermissionMode::BypassPermissions {
            return false;
        }

        // Check if explicitly in require_approval list
        if self.config.requires_approval(tool_name) {
            return true;
        }

        // Auto-approved tools don't require permission
        if self.config.is_auto_approved(tool_name) {
            return false;
        }

        // Default: require permission for destructive operations
        matches!(
            tool_name,
            "BashTool" | "FileWriteTool" | "FileEditTool" | "AgentTool" | "McpTool"
        )
    }

    /// Evaluate a permission request and return a decision.
    ///
    /// This method:
    /// 1. Checks the cache for a previous decision
    /// 2. Runs permission hooks
    /// 3. Applies permission mode logic
    /// 4. Returns a decision
    pub async fn evaluate(
        &self,
        request: PermissionRequest,
    ) -> PermissionResult<PermissionDecision> {
        let tool_name = request.tool_name.clone();
        let request_id = request.id.clone();

        info!(
            "Evaluating permission for {} (mode: {:?})",
            tool_name, self.config.mode
        );

        // Check cache first
        if let Some(decision) = self.cache.get(&request_id) {
            debug!("Returning cached decision for {}", request_id);
            return Ok(decision.clone());
        }

        // Add to pending
        let pending = PendingPermission {
            id: request_id.clone(),
            tool_name: tool_name.clone(),
            input: request.input.clone(),
            created_at: Utc::now(),
            session_id: request.session_id.clone(),
        };
        self.pending.insert(request_id.clone(), pending);

        // Run hooks first
        for hook in &self.hooks {
            debug!("Running permission hook: {}", hook.name());
            match hook.evaluate(&request, &self.config).await {
                PermissionHookResult::Allow(decision) => {
                    info!("Hook {} allowed permission for {}", hook.name(), tool_name);
                    self.cache_request(&request_id, decision.clone()).await?;
                    self.pending.remove(&request_id);
                    return Ok(decision);
                }
                PermissionHookResult::Deny(reason) => {
                    warn!("Hook {} denied permission for {}: {}", hook.name(), tool_name, reason);
                    let decision = PermissionDecision::deny(reason);
                    self.cache_request(&request_id, decision.clone()).await?;
                    self.pending.remove(&request_id);
                    return Ok(decision);
                }
                PermissionHookResult::Continue => {
                    // Continue to next hook or default logic
                }
            }
        }

        // Apply mode-specific logic
        let decision = match self.config.mode {
            PermissionMode::BypassPermissions => {
                debug!("Bypassing permissions for {}", tool_name);
                PermissionDecision::allow()
            }
            PermissionMode::Auto => {
                if self.config.is_auto_approved(&tool_name) {
                    debug!("Auto-approving {}", tool_name);
                    PermissionDecision::allow()
                } else {
                    debug!("Auto-denying {}", tool_name);
                    PermissionDecision::deny(format!(
                        "{} is not in the auto-approve list",
                        tool_name
                    ))
                }
            }
            PermissionMode::Plan => {
                // In plan mode, we validate but don't actually execute
                debug!("Plan mode: validating {}", tool_name);
                PermissionDecision::allow_with_reason(PermissionDecisionReason::PlanMode)
            }
            PermissionMode::Default => {
                // Check store for persisted decision
                if let Ok(Some(record)) = self.store.get(&tool_name, &request.session_id).await {
                    debug!("Using persisted decision for {}", tool_name);
                    if record.allowed {
                        PermissionDecision::allow()
                    } else {
                        PermissionDecision::deny("Previously denied".to_string())
                    }
                } else {
                    // Default: require interactive approval
                    debug!("Interactive approval required for {}", tool_name);
                    PermissionDecision::requires_approval(format!(
                        "Please approve execution of {} tool",
                        tool_name
                    ))
                }
            }
        };

        self.cache_request(&request_id, decision.clone()).await?;

        // Only remove from pending if no approval required
        if !decision.requires_approval {
            self.pending.remove(&request_id);
        }

        Ok(decision)
    }

    /// Approve a pending permission request.
    pub async fn approve(
        &self,
        request_id: &str,
        permanent: bool,
    ) -> PermissionResult<PermissionDecision> {
        let (tool_name, session_id) = {
            let pending = self
                .pending
                .get(request_id)
                .ok_or_else(|| PermissionError::InvalidConfig {
                    message: format!("Unknown permission request: {}", request_id),
                })?;
            (pending.tool_name.clone(), pending.session_id.clone())
        }; // pending Ref is dropped here

        let decision = PermissionDecision::allow();

        // Persist if requested
        if permanent && self.config.persistence_enabled {
            let record = PermissionRecord {
                tool_name,
                session_id,
                allowed: true,
                created_at: Utc::now(),
                expires_at: None,
            };
            self.store.set(record).await?;
        }

        self.cache.insert(request_id.to_string(), decision.clone());
        self.pending.remove(request_id);

        info!("Approved permission request {}", request_id);
        Ok(decision)
    }

    /// Deny a pending permission request.
    pub async fn deny(
        &self,
        request_id: &str,
        reason: impl Into<String>,
        permanent: bool,
    ) -> PermissionResult<PermissionDecision> {
        let (tool_name, session_id) = {
            let pending = self
                .pending
                .get(request_id)
                .ok_or_else(|| PermissionError::InvalidConfig {
                    message: format!("Unknown permission request: {}", request_id),
                })?;
            (pending.tool_name.clone(), pending.session_id.clone())
        }; // pending Ref is dropped here

        let reason = reason.into();
        let decision = PermissionDecision::deny(reason.clone());

        // Persist if requested
        if permanent && self.config.persistence_enabled {
            let record = PermissionRecord {
                tool_name,
                session_id,
                allowed: false,
                created_at: Utc::now(),
                expires_at: None,
            };
            self.store.set(record).await?;
        }

        self.cache.insert(request_id.to_string(), decision.clone());
        self.pending.remove(request_id);

        warn!("Denied permission request {}: {}", request_id, reason);
        Ok(decision)
    }

    /// Cancel a pending permission request.
    pub fn cancel(&self, request_id: &str) -> PermissionResult<()> {
        self.pending
            .remove(request_id)
            .ok_or_else(|| PermissionError::InvalidConfig {
                message: format!("Unknown permission request: {}", request_id),
            })?;

        info!("Cancelled permission request {}", request_id);
        Ok(())
    }

    /// Get all pending permission requests.
    #[must_use]
    pub fn pending_requests(&self) -> Vec<PendingPermission> {
        self.pending
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Cache a permission decision.
    async fn cache_request(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> PermissionResult<()> {
        self.cache.insert(request_id.to_string(), decision);
        Ok(())
    }

    /// Clear the permission cache.
    pub fn clear_cache(&self) {
        self.cache.clear();
        debug!("Cleared permission cache");
    }

    /// Execute a tool with permission checking and sandboxing if needed.
    ///
    /// This is a convenience method that:
    /// 1. Evaluates the permission request
    /// 2. If approved, runs the tool in a sandbox if configured
    /// 3. Returns the result
    pub async fn execute_with_permission<T>(
        &self,
        tool: &T,
        input: ToolInput,
        context: PermissionContext,
    ) -> PermissionResult<tools::ToolOutput>
    where
        T: Tool,
    {
        let request = PermissionRequest::new(tool, input.clone(), &context.session_id);
        let decision = self.evaluate(request).await?;

        match decision {
            PermissionDecision {
                allowed: true,
                reason: _,
                requires_approval: false,
                message: _,
            } => {
                // Permission granted - execute with sandbox if needed
                let tool_name = tool.metadata().name.clone();

                if self.config.sandbox.enabled && self.requires_sandbox(&tool_name) {
                    debug!("Running {} in sandbox", tool_name);
                    let sandbox = Sandbox::new(self.config.sandbox.clone());
                    let result = sandbox
                        .execute(tool, input)
                        .await
                        .map_err(|e| PermissionError::Sandbox(e.to_string()))?;
                    Ok(result)
                } else {
                    Ok(tool.execute(input).await)
                }
            }
            PermissionDecision {
                allowed: false,
                reason: _,
                requires_approval: _,
                message: Some(msg),
            } => Err(PermissionError::Denied { message: msg }),
            _ => Err(PermissionError::Denied {
                message: "Permission denied".to_string(),
            }),
        }
    }

    /// Check if a tool should run in a sandbox.
    fn requires_sandbox(&self, tool_name: &str) -> bool {
        matches!(
            tool_name,
            "BashTool" | "FileWriteTool" | "FileEditTool" | "AgentTool"
        )
    }
}

/// Extension trait for tools to integrate with the permission system.
#[async_trait]
pub trait PermissibleTool: Tool {
    /// Execute the tool with permission checking.
    ///
    /// This method should be called instead of `execute` when permission
    /// control is desired.
    async fn execute_with_permission(
        &self,
        input: ToolInput,
        manager: &PermissionManager,
        context: PermissionContext,
    ) -> PermissionResult<tools::ToolOutput>
    where
        Self: Sized,
    {
        manager.execute_with_permission(self, input, context).await
    }

    /// Check if this tool requires permission.
    fn requires_permission(&self, config: &PermissionConfig) -> bool {
        let tool_name = self.metadata().name.clone();
        if config.mode == PermissionMode::BypassPermissions {
            return false;
        }
        config.requires_approval(&tool_name) || !config.is_auto_approved(&tool_name)
    }
}

// Blanket implementation for all Tool types
#[async_trait]
impl<T: Tool + ?Sized> PermissibleTool for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::{BashTool, FileReadTool};

    #[test]
    fn test_permission_config_default() {
        let config = PermissionConfig::default();
        assert_eq!(config.mode, PermissionMode::Default);
        assert!(config.is_auto_approved("FileReadTool"));
        assert!(!config.is_auto_approved("BashTool"));
        assert!(config.requires_approval("BashTool"));
        assert!(config.persistence_enabled);
    }

    #[test]
    fn test_permission_config_builder() {
        let config = PermissionConfig::new()
            .with_mode(PermissionMode::Auto)
            .with_auto_approve("CustomTool")
            .with_persistence(false);

        assert_eq!(config.mode, PermissionMode::Auto);
        assert!(config.is_auto_approved("CustomTool"));
        assert!(!config.persistence_enabled);
    }

    #[test]
    fn test_permission_context() {
        let ctx = PermissionContext::new("session-123", "/tmp").with_data("key", "value");

        assert_eq!(ctx.session_id, "session-123");
        assert_eq!(ctx.cwd, PathBuf::from("/tmp"));
        assert_eq!(ctx.data.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_permission_manager_requires_permission() {
        let config = PermissionConfig::default();
        let manager = PermissionManager::new(config);

        assert!(manager.requires_permission("BashTool"));
        assert!(!manager.requires_permission("FileReadTool"));
    }

    #[test]
    fn test_permission_manager_bypass_mode() {
        let config = PermissionConfig::new().with_mode(PermissionMode::BypassPermissions);
        let manager = PermissionManager::new(config);

        assert!(!manager.requires_permission("BashTool"));
        assert!(!manager.requires_permission("FileWriteTool"));
    }

    #[tokio::test]
    async fn test_permission_manager_evaluate_auto_mode() {
        let config = PermissionConfig::new().with_mode(PermissionMode::Auto);
        let manager = PermissionManager::new(config);

        let tool = FileReadTool::new();
        let input = ToolInput::new().with_arg("file_path", "/test");
        let request = PermissionRequest::new(&tool, input, "session-1");

        let decision = manager.evaluate(request).await.unwrap();
        assert!(decision.allowed);
    }

    #[tokio::test]
    async fn test_permission_manager_evaluate_bypass_mode() {
        let config = PermissionConfig::new().with_mode(PermissionMode::BypassPermissions);
        let manager = PermissionManager::new(config);

        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo test");
        let request = PermissionRequest::new(&tool, input, "session-1");

        let decision = manager.evaluate(request).await.unwrap();
        assert!(decision.allowed);
        assert!(!decision.requires_approval);
    }

    #[tokio::test]
    async fn test_permission_manager_approve() {
        let config = PermissionConfig::new().with_mode(PermissionMode::Default);
        let manager = PermissionManager::new(config);

        // Create a pending request by evaluating it
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo test");
        let request = PermissionRequest::new(&tool, input.clone(), "session-1");
        let request_id = request.id.clone();

        // Evaluate - should require approval
        let decision = manager.evaluate(request).await.unwrap();
        assert!(decision.requires_approval);

        // Approve it
        let decision = manager.approve(&request_id, false).await.unwrap();
        assert!(decision.allowed);
        assert!(!decision.requires_approval);
    }

    #[tokio::test]
    async fn test_permission_manager_deny() {
        let config = PermissionConfig::new().with_mode(PermissionMode::Default);
        let manager = PermissionManager::new(config);

        // Create a pending request
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo test");
        let request = PermissionRequest::new(&tool, input, "session-1");
        let request_id = request.id.clone();

        // Evaluate - should require approval
        let _ = manager.evaluate(request).await.unwrap();

        // Deny it
        let decision = manager.deny(&request_id, "Not allowed", false).await.unwrap();
        assert!(!decision.allowed);
    }

    #[test]
    fn test_permission_error_display() {
        let err = PermissionError::Denied {
            message: "test".to_string(),
        };
        assert!(err.to_string().contains("test"));
    }
}
