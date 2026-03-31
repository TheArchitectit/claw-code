//! Permission hooks for custom permission logic.
//!
//! Hooks allow extending the permission system with custom rules and logic.

use async_trait::async_trait;
use chrono::Timelike;
use std::fmt;

use crate::{PermissionConfig, PermissionDecision, PermissionRequest};

/// Result of a permission hook evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionHookResult {
    /// The hook allows this permission request.
    Allow(PermissionDecision),
    /// The hook denies this permission request.
    Deny(String),
    /// The hook has no opinion - continue to next hook.
    Continue,
}

impl PermissionHookResult {
    /// Create an allow result.
    #[must_use]
    pub fn allow() -> Self {
        Self::Allow(PermissionDecision::allow())
    }

    /// Create a deny result with a message.
    #[must_use]
    pub fn deny(message: impl Into<String>) -> Self {
        Self::Deny(message.into())
    }

    /// Create a continue result.
    #[must_use]
    pub fn continue_() -> Self {
        Self::Continue
    }

    /// Check if this result allows the request.
    #[must_use]
    pub fn is_allow(&self) -> bool {
        matches!(self, Self::Allow(_))
    }

    /// Check if this result denies the request.
    #[must_use]
    pub fn is_deny(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    /// Check if this result continues to the next hook.
    #[must_use]
    pub fn is_continue(&self) -> bool {
        matches!(self, Self::Continue)
    }
}

/// A hook that can modify or veto permission decisions.
#[async_trait]
pub trait PermissionHook: Send + Sync + fmt::Debug {
    /// Get the name of this hook.
    fn name(&self) -> &str;

    /// Evaluate a permission request.
    ///
    /// Returns:
    /// - `Allow(decision)` to immediately allow the request
    /// - `Deny(reason)` to immediately deny the request
    /// - `Continue` to let other hooks or the default logic decide
    async fn evaluate(
        &self,
        request: &PermissionRequest,
        config: &PermissionConfig,
    ) -> PermissionHookResult;
}

/// A hook that allows all requests from a specific session.
#[derive(Debug, Clone)]
pub struct SessionAllowHook {
    session_id: String,
    name: String,
}

impl SessionAllowHook {
    /// Create a hook that allows all requests from a specific session.
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        let session_id = session_id.into();
        Self {
            name: format!("SessionAllow({})", session_id),
            session_id,
        }
    }
}

#[async_trait]
impl PermissionHook for SessionAllowHook {
    fn name(&self) -> &str {
        &self.name
    }

    async fn evaluate(
        &self,
        request: &PermissionRequest,
        _config: &PermissionConfig,
    ) -> PermissionHookResult {
        if request.session_id == self.session_id {
            PermissionHookResult::allow()
        } else {
            PermissionHookResult::continue_()
        }
    }
}

/// A hook that denies all requests to specific tools.
#[derive(Debug, Clone)]
pub struct ToolDenyHook {
    tool_names: Vec<String>,
    reason: String,
}

impl ToolDenyHook {
    /// Create a hook that denies specific tools.
    #[must_use]
    pub fn new(tool_names: Vec<String>, reason: impl Into<String>) -> Self {
        Self {
            tool_names,
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl PermissionHook for ToolDenyHook {
    fn name(&self) -> &str {
        "ToolDeny"
    }

    async fn evaluate(
        &self,
        request: &PermissionRequest,
        _config: &PermissionConfig,
    ) -> PermissionHookResult {
        if self.tool_names.iter().any(|t| t == &request.tool_name) {
            PermissionHookResult::deny(&self.reason)
        } else {
            PermissionHookResult::continue_()
        }
    }
}

/// A hook that allows read-only tools without prompting.
#[derive(Debug, Clone)]
pub struct ReadOnlyAllowHook;

impl ReadOnlyAllowHook {
    /// Create a new read-only allow hook.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadOnlyAllowHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PermissionHook for ReadOnlyAllowHook {
    fn name(&self) -> &str {
        "ReadOnlyAllow"
    }

    async fn evaluate(
        &self,
        request: &PermissionRequest,
        _config: &PermissionConfig,
    ) -> PermissionHookResult {
        let read_only_tools = [
            "FileReadTool",
            "GlobTool",
            "GrepTool",
            "LspTool",
            "WebFetchTool",
        ];

        if read_only_tools.contains(&request.tool_name.as_str()) {
            PermissionHookResult::allow()
        } else {
            PermissionHookResult::continue_()
        }
    }
}

/// A hook that implements time-based restrictions.
#[derive(Debug, Clone)]
pub struct TimeRestrictionHook {
    allowed_hours: (u8, u8), // (start, end) in 24h format
}

impl TimeRestrictionHook {
    /// Create a time restriction hook.
    ///
    /// Only allows operations during the specified hours (24h format).
    #[must_use]
    pub fn new(start_hour: u8, end_hour: u8) -> Self {
        Self {
            allowed_hours: (start_hour, end_hour),
        }
    }
}

#[async_trait]
impl PermissionHook for TimeRestrictionHook {
    fn name(&self) -> &str {
        "TimeRestriction"
    }

    async fn evaluate(
        &self,
        _request: &PermissionRequest,
        _config: &PermissionConfig,
    ) -> PermissionHookResult {
        use chrono::Local;

        let now = Local::now();
        let hour = now.hour() as u8;

        let (start, end) = self.allowed_hours;

        if hour >= start && hour < end {
            PermissionHookResult::continue_()
        } else {
            PermissionHookResult::deny(format!(
                "Operations not allowed outside of {}:00 - {}:00",
                start, end
            ))
        }
    }
}

/// A composite hook that chains multiple hooks together.
#[derive(Debug)]
pub struct CompositeHook {
    name: String,
    hooks: Vec<Box<dyn PermissionHook>>,
}

impl CompositeHook {
    /// Create a new composite hook.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            hooks: Vec::new(),
        }
    }

    /// Add a hook to the chain.
    pub fn add<H>(&mut self, hook: H)
    where
        H: PermissionHook + 'static,
    {
        self.hooks.push(Box::new(hook));
    }

    /// Create a composite hook with the given hooks.
    #[must_use]
    pub fn with_hooks(name: impl Into<String>, hooks: Vec<Box<dyn PermissionHook>>) -> Self {
        Self {
            name: name.into(),
            hooks,
        }
    }
}

#[async_trait]
impl PermissionHook for CompositeHook {
    fn name(&self) -> &str {
        &self.name
    }

    async fn evaluate(
        &self,
        request: &PermissionRequest,
        config: &PermissionConfig,
    ) -> PermissionHookResult {
        for hook in &self.hooks {
            match hook.evaluate(request, config).await {
                PermissionHookResult::Continue => continue,
                result => return result,
            }
        }
        PermissionHookResult::continue_()
    }
}

/// A hook that allows based on a custom predicate function.
pub struct PredicateHook {
    name: String,
    predicate: Box<dyn Fn(&PermissionRequest) -> bool + Send + Sync>,
}

impl PredicateHook {
    /// Create a new predicate hook.
    pub fn new<F>(name: impl Into<String>, predicate: F) -> Self
    where
        F: Fn(&PermissionRequest) -> bool + Send + Sync + 'static,
    {
        Self {
            name: name.into(),
            predicate: Box::new(predicate),
        }
    }
}

impl fmt::Debug for PredicateHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PredicateHook")
            .field("name", &self.name)
            .finish()
    }
}

#[async_trait]
impl PermissionHook for PredicateHook {
    fn name(&self) -> &str {
        &self.name
    }

    async fn evaluate(
        &self,
        request: &PermissionRequest,
        _config: &PermissionConfig,
    ) -> PermissionHookResult {
        if (self.predicate)(request) {
            PermissionHookResult::allow()
        } else {
            PermissionHookResult::continue_()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::{BashTool, FileReadTool, ToolInput};

    #[test]
    fn test_hook_result_allow() {
        let result = PermissionHookResult::allow();
        assert!(result.is_allow());
        assert!(!result.is_deny());
        assert!(!result.is_continue());
    }

    #[test]
    fn test_hook_result_deny() {
        let result = PermissionHookResult::deny("test");
        assert!(!result.is_allow());
        assert!(result.is_deny());
        assert!(!result.is_continue());
    }

    #[test]
    fn test_hook_result_continue() {
        let result = PermissionHookResult::continue_();
        assert!(!result.is_allow());
        assert!(!result.is_deny());
        assert!(result.is_continue());
    }

    #[tokio::test]
    async fn test_session_allow_hook() {
        let hook = SessionAllowHook::new("allowed-session");
        let config = PermissionConfig::default();

        let tool = BashTool::new();
        let input = ToolInput::new();
        let request = PermissionRequest::new(&tool, input, "allowed-session");

        let result = hook.evaluate(&request, &config).await;
        assert!(result.is_allow());

        let request2 = PermissionRequest::new(&tool, ToolInput::new(), "other-session");
        let result2 = hook.evaluate(&request2, &config).await;
        assert!(result2.is_continue());
    }

    #[tokio::test]
    async fn test_tool_deny_hook() {
        let hook = ToolDenyHook::new(vec!["BashTool".to_string()], "Bash is disabled");
        let config = PermissionConfig::default();

        let bash = BashTool::new();
        let input = ToolInput::new();
        let request = PermissionRequest::new(&bash, input, "session-1");

        let result = hook.evaluate(&request, &config).await;
        assert!(result.is_deny());

        let file_read = FileReadTool::new();
        let request2 = PermissionRequest::new(&file_read, ToolInput::new(), "session-1");
        let result2 = hook.evaluate(&request2, &config).await;
        assert!(result2.is_continue());
    }

    #[tokio::test]
    async fn test_read_only_allow_hook() {
        let hook = ReadOnlyAllowHook::new();
        let config = PermissionConfig::default();

        let file_read = FileReadTool::new();
        let request = PermissionRequest::new(&file_read, ToolInput::new(), "session-1");

        let result = hook.evaluate(&request, &config).await;
        assert!(result.is_allow());

        let bash = BashTool::new();
        let request2 = PermissionRequest::new(&bash, ToolInput::new(), "session-1");
        let result2 = hook.evaluate(&request2, &config).await;
        assert!(result2.is_continue());
    }

    #[tokio::test]
    async fn test_composite_hook() {
        let mut hook = CompositeHook::new("TestComposite");
        hook.add(ReadOnlyAllowHook::new());
        hook.add(ToolDenyHook::new(vec!["BashTool".to_string()], "denied"));

        let config = PermissionConfig::default();

        let file_read = FileReadTool::new();
        let request = PermissionRequest::new(&file_read, ToolInput::new(), "session-1");

        let result = hook.evaluate(&request, &config).await;
        assert!(result.is_allow()); // ReadOnlyAllow allows it

        let bash = BashTool::new();
        let request2 = PermissionRequest::new(&bash, ToolInput::new(), "session-1");
        let result2 = hook.evaluate(&request2, &config).await;
        assert!(result2.is_deny()); // ToolDeny denies it
    }

    #[tokio::test]
    async fn test_predicate_hook() {
        let hook = PredicateHook::new("TestPredicate", |req| req.tool_name == "FileReadTool");
        let config = PermissionConfig::default();

        let file_read = FileReadTool::new();
        let request = PermissionRequest::new(&file_read, ToolInput::new(), "session-1");

        let result = hook.evaluate(&request, &config).await;
        assert!(result.is_allow());

        let bash = BashTool::new();
        let request2 = PermissionRequest::new(&bash, ToolInput::new(), "session-1");
        let result2 = hook.evaluate(&request2, &config).await;
        assert!(result2.is_continue());
    }
}
