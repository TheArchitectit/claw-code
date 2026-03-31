//! Base permission types with no inter-dependencies.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Permission modes that control tool execution behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Default mode - tools may prompt for permission.
    Default,

    /// Accept edits without prompting.
    AcceptEdits,

    /// Bypass all permission checks.
    BypassPermissions,

    /// Don't ask for permission, fail dangerous operations.
    DontAsk,

    /// Plan mode - requires explicit approval for each step.
    Plan,

    /// Auto mode - uses classifier to decide.
    Auto,

    /// Bubble mode - delegates to parent context.
    Bubble,
}

impl PermissionMode {
    /// Check if this mode allows automatic tool execution without prompts.
    #[must_use]
    pub fn is_auto_approve(&self) -> bool {
        matches!(self, Self::BypassPermissions | Self::AcceptEdits)
    }

    /// Check if this mode requires explicit user confirmation.
    #[must_use]
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, Self::Plan | Self::Default)
    }

    /// Check if this mode skips permission prompts entirely.
    #[must_use]
    pub fn skips_prompts(&self) -> bool {
        matches!(self, Self::DontAsk | Self::BypassPermissions)
    }
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

/// The behavior for a permission decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionBehavior {
    /// Allow the operation.
    Allow,

    /// Deny the operation.
    Deny,

    /// Ask the user for permission.
    Ask,
}

/// The source of a permission rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRuleSource {
    /// Rule from user settings.
    UserSettings,

    /// Rule from project settings.
    ProjectSettings,

    /// Rule from local settings.
    LocalSettings,

    /// Rule from flag settings.
    FlagSettings,

    /// Rule from policy settings.
    PolicySettings,

    /// Rule from CLI argument.
    CliArg,

    /// Rule from a command.
    Command,

    /// Rule from the current session.
    Session,
}

impl PermissionRuleSource {
    /// Get the priority of this source (lower is higher priority).
    #[must_use]
    pub fn priority(&self) -> u8 {
        match self {
            Self::CliArg => 0,
            Self::Session => 1,
            Self::Command => 2,
            Self::LocalSettings => 3,
            Self::ProjectSettings => 4,
            Self::UserSettings => 5,
            Self::FlagSettings => 6,
            Self::PolicySettings => 7,
        }
    }
}

/// A permission rule value specifying what tool/content it applies to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRuleValue {
    /// The tool name this rule applies to.
    pub tool_name: String,

    /// Optional specific content pattern for the rule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

/// A complete permission rule with source, behavior, and value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Where this rule originated from.
    pub source: PermissionRuleSource,

    /// The behavior this rule specifies.
    pub rule_behavior: PermissionBehavior,

    /// The value this rule matches against.
    pub rule_value: PermissionRuleValue,
}

/// A set of permission rules grouped by their source.
pub type ToolPermissionRulesBySource = HashMap<String, Vec<PermissionRuleValue>>;

/// Where a permission update should be persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionUpdateDestination {
    /// Update user settings.
    UserSettings,

    /// Update project settings.
    ProjectSettings,

    /// Update local settings.
    LocalSettings,

    /// Update session state.
    Session,

    /// Update CLI arguments.
    CliArg,
}

/// Reasons for sandbox overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SandboxOverrideReason {
    /// Command was excluded from sandbox.
    ExcludedCommand,

    /// Sandbox was disabled.
    DangerouslyDisableSandbox,
}

/// Confidence levels for classifier results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfidenceLevel {
    /// High confidence classification.
    High,

    /// Medium confidence classification.
    Medium,

    /// Low confidence classification.
    Low,
}

/// Risk level for permission explanations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RiskLevel {
    /// Low risk operation.
    Low,

    /// Medium risk operation.
    Medium,

    /// High risk operation.
    High,
}

/// An additional working directory included in permission scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdditionalWorkingDirectory {
    /// The path to the working directory.
    pub path: String,

    /// The source of this directory entry.
    pub source: PermissionRuleSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_mode_checks() {
        assert!(PermissionMode::BypassPermissions.is_auto_approve());
        assert!(PermissionMode::AcceptEdits.is_auto_approve());
        assert!(!PermissionMode::Default.is_auto_approve());

        assert!(PermissionMode::Plan.requires_confirmation());
        assert!(PermissionMode::Default.requires_confirmation());
        assert!(!PermissionMode::BypassPermissions.requires_confirmation());

        assert!(PermissionMode::DontAsk.skips_prompts());
        assert!(PermissionMode::BypassPermissions.skips_prompts());
        assert!(!PermissionMode::Default.skips_prompts());
    }

    #[test]
    fn test_permission_mode_equality() {
        assert_eq!(PermissionMode::Default, PermissionMode::Default);
        assert_ne!(PermissionMode::Default, PermissionMode::BypassPermissions);
    }

    #[test]
    fn test_permission_rule_source_priority() {
        let sources = vec![
            PermissionRuleSource::UserSettings,
            PermissionRuleSource::ProjectSettings,
            PermissionRuleSource::LocalSettings,
            PermissionRuleSource::FlagSettings,
            PermissionRuleSource::PolicySettings,
        ];

        let priorities: Vec<u8> = sources.iter().map(|s| s.priority()).collect();
        assert!(!priorities.is_empty());
    }

    #[test]
    fn test_tool_permission_rules_by_source() {
        let mut rules = ToolPermissionRulesBySource::default();
        assert!(rules.is_empty());

        let rule_value = PermissionRuleValue {
            tool_name: "Bash".to_string(),
            rule_content: Some("echo *".to_string()),
        };

        rules.insert("userSettings".to_string(), vec![rule_value]);
        assert!(!rules.is_empty());
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn test_permission_behavior_variants() {
        let behaviors = vec![
            PermissionBehavior::Allow,
            PermissionBehavior::Deny,
            PermissionBehavior::Ask,
        ];

        for behavior in behaviors {
            let _ = format!("{:?}", behavior);
        }
    }

    #[test]
    fn test_sandbox_override_reason() {
        let reasons = vec![
            SandboxOverrideReason::ExcludedCommand,
            SandboxOverrideReason::DangerouslyDisableSandbox,
        ];

        assert_eq!(reasons.len(), 2);
    }

    #[test]
    fn test_additional_working_directory() {
        let dir = AdditionalWorkingDirectory {
            path: "/tmp".to_string(),
            source: PermissionRuleSource::LocalSettings,
        };

        assert_eq!(dir.path, "/tmp");
    }
}
