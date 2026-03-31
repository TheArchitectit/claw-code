//! Permission results and update types.

use serde::{Deserialize, Serialize};

use super::types::{PermissionBehavior, PermissionMode, PermissionRuleValue, PermissionUpdateDestination, ToolPermissionRulesBySource};

/// A permission result that may include passthrough.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PermissionResult {
    /// A standard permission decision.
    Decision(super::decisions::PermissionDecision),

    /// Pass through to another handler.
    Passthrough {
        /// The behavior (always Passthrough).
        behavior: PermissionBehavior,

        /// The message.
        message: String,

        /// The decision reason.
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_reason: Option<super::decisions::PermissionDecisionReason>,

        /// Suggestions for updates.
        #[serde(skip_serializing_if = "Option::is_none")]
        suggestions: Option<Vec<super::decisions::PermissionUpdate>>,

        /// Blocked path.
        #[serde(skip_serializing_if = "Option::is_none")]
        blocked_path: Option<String>,

        /// Pending classifier check.
        #[serde(skip_serializing_if = "Option::is_none")]
        pending_classifier_check: Option<super::decisions::PendingClassifierCheck>,
    },
}

impl PermissionResult {
    /// Create an allow result.
    #[must_use]
    pub fn allow() -> Self {
        use super::decisions::{PermissionAllowDecision, PermissionDecision};
        Self::Decision(PermissionDecision::Allow(PermissionAllowDecision {
            behavior: PermissionBehavior::Allow,
            updated_input: None,
            user_modified: None,
            decision_reason: None,
            tool_use_id: None,
            accept_feedback: None,
            content_blocks: None,
        }))
    }

    /// Create a deny result.
    #[must_use]
    pub fn deny(message: impl Into<String>) -> Self {
        use super::decisions::{PermissionDecision, PermissionDenyDecision, PermissionDecisionReason};
        Self::Decision(PermissionDecision::Deny(PermissionDenyDecision {
            behavior: PermissionBehavior::Deny,
            message: message.into(),
            decision_reason: PermissionDecisionReason::Other {
                reason: "Explicit denial".to_string(),
            },
            tool_use_id: None,
        }))
    }

    /// Create an ask result.
    #[must_use]
    pub fn ask(message: impl Into<String>) -> Self {
        use super::decisions::{PermissionAskDecision, PermissionDecision};
        Self::Decision(PermissionDecision::Ask(PermissionAskDecision {
            behavior: PermissionBehavior::Ask,
            message: message.into(),
            updated_input: None,
            decision_reason: None,
            suggestions: None,
            blocked_path: None,
            metadata: None,
            is_bash_security_check_for_misparsing: None,
            pending_classifier_check: None,
            content_blocks: None,
        }))
    }

    /// Check if this result is an allow decision.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Decision(d) if d.is_allowed())
    }

    /// Check if this result is a deny decision.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Decision(d) if d.is_denied())
    }

    /// Check if this result is an ask decision.
    #[must_use]
    pub fn is_ask(&self) -> bool {
        matches!(self, Self::Decision(d) if d.is_ask())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_result_helpers() {
        let allow = PermissionResult::allow();
        assert!(allow.is_allowed());
        assert!(!allow.is_denied());
        assert!(!allow.is_ask());

        let deny = PermissionResult::deny("test");
        assert!(!deny.is_allowed());
        assert!(deny.is_denied());
        assert!(!deny.is_ask());

        let ask = PermissionResult::ask("test");
        assert!(!ask.is_allowed());
        assert!(!ask.is_denied());
        assert!(ask.is_ask());
    }

    #[test]
    fn test_tool_permission_rules_by_source() {
        use super::super::types::PermissionRuleValue;

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
}
