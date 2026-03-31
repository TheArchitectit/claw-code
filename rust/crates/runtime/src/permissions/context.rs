//! Permission context for tool execution.

use serde::{Deserialize, Serialize};

use super::types::{AdditionalWorkingDirectory, PermissionMode, PermissionRuleSource, PermissionRuleValue, ToolPermissionRulesBySource};

/// The context needed for permission checking in tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPermissionContext {
    /// The current permission mode.
    pub mode: PermissionMode,

    /// Additional working directories included in scope.
    #[serde(with = "crate::utils::serde_hashmap")]
    pub additional_working_directories: std::collections::HashMap<String, AdditionalWorkingDirectory>,

    /// Rules that always allow certain operations.
    pub always_allow_rules: ToolPermissionRulesBySource,

    /// Rules that always deny certain operations.
    pub always_deny_rules: ToolPermissionRulesBySource,

    /// Rules that always ask for certain operations.
    pub always_ask_rules: ToolPermissionRulesBySource,

    /// Whether bypass permissions mode is available.
    pub is_bypass_permissions_mode_available: bool,

    /// Rules that were stripped as dangerous.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stripped_dangerous_rules: Option<ToolPermissionRulesBySource>,

    /// When true, permission prompts should be avoided (e.g., background agents).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub should_avoid_permission_prompts: Option<bool>,

    /// When true, automated checks should be awaited before showing dialog.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub await_automated_checks_before_dialog: Option<bool>,

    /// The permission mode before entering plan mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre_plan_mode: Option<PermissionMode>,
}

impl ToolPermissionContext {
    /// Create a new empty permission context with default mode.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the permission mode.
    pub fn with_mode(mut self, mode: PermissionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Add an additional working directory.
    pub fn with_working_directory(
        mut self,
        path: impl Into<String>,
        source: PermissionRuleSource,
    ) -> Self {
        let path = path.into();
        self.additional_working_directories.insert(
            path.clone(),
            AdditionalWorkingDirectory { path, source },
        );
        self
    }

    /// Add an always-allow rule.
    pub fn with_allow_rule(
        mut self,
        source: impl Into<String>,
        rule: PermissionRuleValue,
    ) -> Self {
        self.always_allow_rules
            .entry(source.into())
            .or_default()
            .push(rule);
        self
    }

    /// Check if a tool is covered by an always-allow rule.
    #[must_use]
    pub fn has_always_allow(&self, tool_name: &str, content: Option<&str>) -> bool {
        self.check_rules(&self.always_allow_rules, tool_name, content)
    }

    /// Check if a tool is covered by an always-deny rule.
    #[must_use]
    pub fn has_always_deny(&self, tool_name: &str, content: Option<&str>) -> bool {
        self.check_rules(&self.always_deny_rules, tool_name, content)
    }

    fn check_rules(
        &self,
        rules: &ToolPermissionRulesBySource,
        tool_name: &str,
        content: Option<&str>,
    ) -> bool {
        for rule_values in rules.values() {
            for rule_value in rule_values {
                if rule_value.tool_name == tool_name {
                    if let Some(ref rule_content) = rule_value.rule_content {
                        if let Some(content) = content {
                            if content.contains(rule_content) {
                                return true;
                            }
                        }
                    } else {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_context_rules() {
        let ctx = ToolPermissionContext::new()
            .with_allow_rule(
                "userSettings",
                PermissionRuleValue {
                    tool_name: "Bash".to_string(),
                    rule_content: None,
                },
            );

        assert!(ctx.has_always_allow("Bash", None));
        assert!(!ctx.has_always_allow("Read", None));
        assert!(!ctx.has_always_deny("Bash", None));
    }
}
