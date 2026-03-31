//! Permission decisions and decision-related types.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::types::{ConfidenceLevel, PermissionBehavior, PermissionMode, PermissionRule, PermissionRuleValue, PermissionUpdateDestination, SandboxOverrideReason};

/// The reason for a permission decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionDecisionReason {
    /// Decision was based on a rule.
    Rule {
        /// The rule that triggered the decision.
        rule: PermissionRule,
    },

    /// Decision was based on the current mode.
    Mode {
        /// The mode that triggered the decision.
        mode: PermissionMode,
    },

    /// Decision was based on subcommand results.
    SubcommandResults {
        /// Reasons for each subcommand.
        #[serde(with = "crate::utils::serde_hashmap")]
        reasons: HashMap<String, super::results::PermissionResult>,
    },

    /// Decision was made by the permission prompt tool.
    PermissionPromptTool {
        /// The name of the permission prompt tool.
        permission_prompt_tool_name: String,

        /// The result from the tool.
        tool_result: serde_json::Value,
    },

    /// Decision was based on a hook.
    Hook {
        /// The name of the hook.
        hook_name: String,

        /// The source of the hook.
        #[serde(skip_serializing_if = "Option::is_none")]
        hook_source: Option<String>,

        /// The reason from the hook.
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },

    /// Decision was based on an async agent check.
    AsyncAgent {
        /// The reason from the agent.
        reason: String,
    },

    /// Decision was overridden by sandbox.
    SandboxOverride {
        /// The specific override reason.
        reason: SandboxOverrideReason,
    },

    /// Decision was based on a classifier.
    Classifier {
        /// The classifier used.
        classifier: String,

        /// The reason from the classifier.
        reason: String,
    },

    /// Decision was based on working directory checks.
    WorkingDir {
        /// The reason for the working directory decision.
        reason: String,
    },

    /// Decision was based on a safety check.
    SafetyCheck {
        /// The reason for the safety check.
        reason: String,

        /// Whether the classifier can approve this.
        classifier_approvable: bool,
    },

    /// Other reason.
    Other {
        /// The reason.
        reason: String,
    },
}

/// Metadata attached to permission decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionMetadata {
    /// Command metadata if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<PermissionCommandMetadata>,
}

/// Minimal command shape for permission metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionCommandMetadata {
    /// The command name.
    pub name: String,

    /// The command description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A decision when permission is allowed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionAllowDecision {
    /// The behavior (always Allow).
    pub behavior: PermissionBehavior,

    /// Updated input if modified during permission check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,

    /// Whether the user modified the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_modified: Option<bool>,

    /// The reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<PermissionDecisionReason>,

    /// The tool use ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Feedback content blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_feedback: Option<String>,

    /// Additional content blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<serde_json::Value>>,
}

/// Metadata for a pending classifier check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingClassifierCheck {
    /// The command being checked.
    pub command: String,

    /// The current working directory.
    pub cwd: String,

    /// Descriptions for the check.
    pub descriptions: Vec<String>,
}

/// A decision when user should be prompted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionAskDecision {
    /// The behavior (always Ask).
    pub behavior: PermissionBehavior,

    /// The message to display.
    pub message: String,

    /// Updated input if modified.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,

    /// The reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_reason: Option<PermissionDecisionReason>,

    /// Suggestions for permission updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestions: Option<Vec<PermissionUpdate>>,

    /// Path that was blocked.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_path: Option<String>,

    /// Metadata for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PermissionMetadata>,

    /// Whether this was a security check for misparsing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_bash_security_check_for_misparsing: Option<bool>,

    /// Pending classifier check if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_classifier_check: Option<PendingClassifierCheck>,

    /// Content blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_blocks: Option<Vec<serde_json::Value>>,
}

/// A decision when permission is denied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDenyDecision {
    /// The behavior (always Deny).
    pub behavior: PermissionBehavior,

    /// The message explaining the denial.
    pub message: String,

    /// The reason for the decision.
    pub decision_reason: PermissionDecisionReason,

    /// The tool use ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,
}

/// A permission decision - allow, ask, or deny.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PermissionDecision {
    /// Permission granted.
    Allow(PermissionAllowDecision),

    /// Permission denied.
    Deny(PermissionDenyDecision),

    /// Ask user for permission.
    Ask(PermissionAskDecision),
}

impl PermissionDecision {
    /// Get the behavior type for this decision.
    #[must_use]
    pub fn behavior(&self) -> PermissionBehavior {
        match self {
            Self::Allow(_) => PermissionBehavior::Allow,
            Self::Deny(_) => PermissionBehavior::Deny,
            Self::Ask(_) => PermissionBehavior::Ask,
        }
    }

    /// Check if this decision allows the operation.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow(_))
    }

    /// Check if this decision denies the operation.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Deny(_))
    }

    /// Check if this decision asks for user input.
    #[must_use]
    pub fn is_ask(&self) -> bool {
        matches!(self, Self::Ask(_))
    }
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

/// An explanation of why a permission decision was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionExplanation {
    /// The risk level assessed.
    pub risk_level: RiskLevel,

    /// The explanation text.
    pub explanation: String,

    /// Reasoning behind the decision.
    pub reasoning: String,

    /// Description of the risk.
    pub risk: String,
}

/// A result from the classifier about whether an operation is safe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassifierResult {
    /// Whether the classifier matched a known safe pattern.
    pub matches: bool,

    /// Description of what matched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_description: Option<String>,

    /// Confidence level of the classification.
    pub confidence: ConfidenceLevel,

    /// Human-readable reason for the classification.
    pub reason: String,
}

/// An operation to update permission configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PermissionUpdate {
    /// Add permission rules.
    AddRules {
        /// Where to persist the update.
        destination: PermissionUpdateDestination,

        /// The rules to add.
        rules: Vec<PermissionRuleValue>,

        /// The behavior for the rules.
        behavior: PermissionBehavior,
    },

    /// Replace permission rules.
    ReplaceRules {
        /// Where to persist the update.
        destination: PermissionUpdateDestination,

        /// The rules to replace with.
        rules: Vec<PermissionRuleValue>,

        /// The behavior for the rules.
        behavior: PermissionBehavior,
    },

    /// Remove permission rules.
    RemoveRules {
        /// Where to remove from.
        destination: PermissionUpdateDestination,

        /// The rules to remove.
        rules: Vec<PermissionRuleValue>,

        /// The behavior of the rules to remove.
        behavior: PermissionBehavior,
    },

    /// Set the permission mode.
    SetMode {
        /// Where to persist the update.
        destination: PermissionUpdateDestination,

        /// The mode to set.
        mode: PermissionMode,
    },

    /// Add directories to the working set.
    AddDirectories {
        /// Where to persist the update.
        destination: PermissionUpdateDestination,

        /// The directories to add.
        directories: Vec<String>,
    },

    /// Remove directories from the working set.
    RemoveDirectories {
        /// Where to remove from.
        destination: PermissionUpdateDestination,

        /// The directories to remove.
        directories: Vec<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::types::{PermissionRuleSource, RiskLevel};

    #[test]
    fn test_permission_decision_helpers() {
        let allow = PermissionDecision::Allow(PermissionAllowDecision {
            behavior: PermissionBehavior::Allow,
            updated_input: None,
            user_modified: None,
            decision_reason: None,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        });

        let deny = PermissionDecision::Deny(PermissionDenyDecision {
            behavior: PermissionBehavior::Deny,
            message: "Access denied".to_string(),
            decision_reason: PermissionDecisionReason::Other { reason: "test".to_string() },
            tool_use_id: None,
        });

        let ask = PermissionDecision::Ask(PermissionAskDecision {
            behavior: PermissionBehavior::Ask,
            message: "Please confirm".to_string(),
            updated_input: None,
            decision_reason: None,
            suggestions: None,
            blocked_path: None,
            metadata: None,
            is_bash_security_check_for_misparsing: None,
            pending_classifier_check: None,
            content_blocks: None,
        });

        assert!(allow.is_allowed());
        assert!(!allow.is_denied());
        assert!(!allow.is_ask());

        assert!(!deny.is_allowed());
        assert!(deny.is_denied());
        assert!(!deny.is_ask());

        assert!(!ask.is_allowed());
        assert!(!ask.is_denied());
        assert!(ask.is_ask());
    }

    #[test]
    fn test_permission_metadata() {
        let metadata = PermissionMetadata { command: None };
        assert!(metadata.command.is_none());
    }

    #[test]
    fn test_classifier_result() {
        let result = ClassifierResult {
            matches: true,
            matched_description: Some("pattern matched".to_string()),
            confidence: ConfidenceLevel::High,
            reason: "test reason".to_string(),
        };

        assert!(result.matches);
        assert_eq!(result.confidence, ConfidenceLevel::High);
    }

    #[test]
    fn test_permission_command_metadata() {
        let metadata = PermissionCommandMetadata {
            name: "Bash".to_string(),
            description: Some("Execute bash command".to_string()),
        };

        assert_eq!(metadata.name, "Bash");
    }

    #[test]
    fn test_permission_update_variants() {
        let add_rules = PermissionUpdate::AddRules {
            destination: PermissionUpdateDestination::UserSettings,
            rules: vec![PermissionRuleValue {
                tool_name: "Bash".to_string(),
                rule_content: Some("echo *".to_string()),
            }],
            behavior: PermissionBehavior::Allow,
        };

        let set_mode = PermissionUpdate::SetMode {
            destination: PermissionUpdateDestination::Session,
            mode: PermissionMode::Default,
        };

        let _ = add_rules;
        let _ = set_mode;
    }
}
