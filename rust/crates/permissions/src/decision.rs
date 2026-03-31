//! Permission decision types and logic.
//!
//! A [`PermissionDecision`] represents the outcome of a permission evaluation,
//! indicating whether a tool execution should proceed.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The reason for a permission decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDecisionReason {
    /// User explicitly approved.
    UserApproved,
    /// User explicitly denied.
    UserDenied,
    /// Automatically approved by configuration.
    AutoApproved,
    /// Automatically denied by configuration.
    AutoDenied,
    /// Approved by a permission hook.
    HookAllowed,
    /// Denied by a permission hook.
    HookDenied,
    /// Approved from persisted decision.
    PersistedAllow,
    /// Denied from persisted decision.
    PersistedDeny,
    /// In planning mode - no execution.
    PlanMode,
    /// Permissions were bypassed.
    Bypassed,
    /// Tool requires interactive approval.
    RequiresApproval,
    /// Tool was cancelled.
    Cancelled,
    /// Custom reason.
    Custom(String),
}

impl PermissionDecisionReason {
    /// Get a human-readable description of this reason.
    #[must_use]
    pub fn description(&self) -> String {
        match self {
            Self::UserApproved => "User approved".to_string(),
            Self::UserDenied => "User denied".to_string(),
            Self::AutoApproved => "Auto-approved".to_string(),
            Self::AutoDenied => "Auto-denied".to_string(),
            Self::HookAllowed => "Allowed by hook".to_string(),
            Self::HookDenied => "Denied by hook".to_string(),
            Self::PersistedAllow => "Previously allowed".to_string(),
            Self::PersistedDeny => "Previously denied".to_string(),
            Self::PlanMode => "Planning mode".to_string(),
            Self::Bypassed => "Permission bypassed".to_string(),
            Self::RequiresApproval => "Requires approval".to_string(),
            Self::Cancelled => "Cancelled".to_string(),
            Self::Custom(s) => s.clone(),
        }
    }

    /// Check if this reason indicates success (permission granted).
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            Self::UserApproved
                | Self::AutoApproved
                | Self::HookAllowed
                | Self::PersistedAllow
                | Self::PlanMode
                | Self::Bypassed
        )
    }

    /// Check if this reason indicates failure (permission denied).
    #[must_use]
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            Self::UserDenied
                | Self::AutoDenied
                | Self::HookDenied
                | Self::PersistedDeny
                | Self::Cancelled
        )
    }

    /// Check if this reason indicates pending (needs more input).
    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::RequiresApproval)
    }
}

impl fmt::Display for PermissionDecisionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// The outcome of a permission evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDecision {
    /// Whether permission is granted.
    pub allowed: bool,
    /// The reason for the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<PermissionDecisionReason>,
    /// Whether interactive approval is required.
    #[serde(default)]
    pub requires_approval: bool,
    /// Optional message explaining the decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PermissionDecision {
    /// Create a new permission decision.
    #[must_use]
    pub fn new(
        allowed: bool,
        reason: PermissionDecisionReason,
        requires_approval: bool,
    ) -> Self {
        Self {
            allowed,
            reason: Some(reason),
            requires_approval,
            message: None,
        }
    }

    /// Create an allow decision.
    #[must_use]
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: Some(PermissionDecisionReason::UserApproved),
            requires_approval: false,
            message: None,
        }
    }

    /// Create an allow decision with a specific reason.
    #[must_use]
    pub fn allow_with_reason(reason: PermissionDecisionReason) -> Self {
        Self {
            allowed: true,
            reason: Some(reason),
            requires_approval: false,
            message: None,
        }
    }

    /// Create a deny decision.
    #[must_use]
    pub fn deny(message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(PermissionDecisionReason::UserDenied),
            requires_approval: false,
            message: Some(message.into()),
        }
    }

    /// Create a deny decision with a specific reason.
    #[must_use]
    pub fn deny_with_reason(reason: PermissionDecisionReason, message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(reason),
            requires_approval: false,
            message: Some(message.into()),
        }
    }

    /// Create a decision that requires interactive approval.
    #[must_use]
    pub fn requires_approval(message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            reason: Some(PermissionDecisionReason::RequiresApproval),
            requires_approval: true,
            message: Some(message.into()),
        }
    }

    /// Create a cancelled decision.
    #[must_use]
    pub fn cancelled() -> Self {
        Self {
            allowed: false,
            reason: Some(PermissionDecisionReason::Cancelled),
            requires_approval: false,
            message: Some("Permission request was cancelled".to_string()),
        }
    }

    /// Set a message on this decision.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Check if this decision allows execution.
    #[must_use]
    pub fn is_allowed(&self) -> bool {
        self.allowed && !self.requires_approval
    }

    /// Check if this decision denies execution.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        !self.allowed && !self.requires_approval
    }

    /// Check if this decision requires interactive approval.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.requires_approval
    }

    /// Get the message, if any.
    #[must_use]
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Get the reason, if any.
    #[must_use]
    pub fn reason(&self) -> Option<&PermissionDecisionReason> {
        self.reason.as_ref()
    }
}

impl Default for PermissionDecision {
    fn default() -> Self {
        Self::requires_approval("Permission evaluation pending")
    }
}

impl fmt::Display for PermissionDecision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.allowed {
            write!(f, "Allowed")?;
        } else if self.requires_approval {
            write!(f, "Requires approval")?;
        } else {
            write!(f, "Denied")?;
        }

        if let Some(reason) = &self.reason {
            write!(f, " ({})", reason)?;
        }

        if let Some(message) = &self.message {
            write!(f, ": {}", message)?;
        }

        Ok(())
    }
}

/// Builder for permission decisions.
pub struct PermissionDecisionBuilder {
    allowed: bool,
    reason: Option<PermissionDecisionReason>,
    requires_approval: bool,
    message: Option<String>,
}

impl PermissionDecisionBuilder {
    /// Create a new builder for an allow decision.
    #[must_use]
    pub fn allow() -> Self {
        Self {
            allowed: true,
            reason: None,
            requires_approval: false,
            message: None,
        }
    }

    /// Create a new builder for a deny decision.
    #[must_use]
    pub fn deny() -> Self {
        Self {
            allowed: false,
            reason: None,
            requires_approval: false,
            message: None,
        }
    }

    /// Create a new builder for a pending decision.
    #[must_use]
    pub fn pending() -> Self {
        Self {
            allowed: false,
            reason: Some(PermissionDecisionReason::RequiresApproval),
            requires_approval: true,
            message: None,
        }
    }

    /// Set the reason.
    #[must_use]
    pub fn with_reason(mut self, reason: PermissionDecisionReason) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Set the message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Build the decision.
    #[must_use]
    pub fn build(self) -> PermissionDecision {
        PermissionDecision {
            allowed: self.allowed,
            reason: self.reason,
            requires_approval: self.requires_approval,
            message: self.message,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_allow() {
        let decision = PermissionDecision::allow();
        assert!(decision.is_allowed());
        assert!(!decision.is_denied());
        assert!(!decision.is_pending());
    }

    #[test]
    fn test_decision_deny() {
        let decision = PermissionDecision::deny("Test reason");
        assert!(!decision.is_allowed());
        assert!(decision.is_denied());
        assert!(!decision.is_pending());
        assert_eq!(decision.message(), Some("Test reason"));
    }

    #[test]
    fn test_decision_requires_approval() {
        let decision = PermissionDecision::requires_approval("Please approve");
        assert!(!decision.is_allowed());
        assert!(!decision.is_denied());
        assert!(decision.is_pending());
        assert_eq!(decision.message(), Some("Please approve"));
    }

    #[test]
    fn test_decision_cancelled() {
        let decision = PermissionDecision::cancelled();
        assert!(!decision.is_allowed());
        assert!(decision.is_denied());
        assert!(!decision.is_pending());
    }

    #[test]
    fn test_decision_with_reason() {
        let decision = PermissionDecision::allow_with_reason(PermissionDecisionReason::AutoApproved);
        assert!(decision.is_allowed());
        assert_eq!(decision.reason(), Some(&PermissionDecisionReason::AutoApproved));
    }

    #[test]
    fn test_decision_builder() {
        let decision = PermissionDecisionBuilder::allow()
            .with_reason(PermissionDecisionReason::HookAllowed)
            .with_message("Hook approved")
            .build();

        assert!(decision.is_allowed());
        assert_eq!(decision.message(), Some("Hook approved"));
    }

    #[test]
    fn test_decision_builder_deny() {
        let decision = PermissionDecisionBuilder::deny()
            .with_reason(PermissionDecisionReason::HookDenied)
            .with_message("Hook rejected")
            .build();

        assert!(decision.is_denied());
        assert_eq!(decision.message(), Some("Hook rejected"));
    }

    #[test]
    fn test_decision_reason_is_success() {
        assert!(PermissionDecisionReason::UserApproved.is_success());
        assert!(PermissionDecisionReason::AutoApproved.is_success());
        assert!(PermissionDecisionReason::HookAllowed.is_success());
        assert!(!PermissionDecisionReason::UserDenied.is_success());
        assert!(!PermissionDecisionReason::HookDenied.is_success());
    }

    #[test]
    fn test_decision_reason_is_failure() {
        assert!(PermissionDecisionReason::UserDenied.is_failure());
        assert!(PermissionDecisionReason::HookDenied.is_failure());
        assert!(!PermissionDecisionReason::UserApproved.is_failure());
    }

    #[test]
    fn test_decision_display() {
        let decision = PermissionDecision::allow();
        let display = format!("{}", decision);
        assert!(display.contains("Allowed"));
    }

    #[test]
    fn test_decision_reason_display() {
        let reason = PermissionDecisionReason::UserApproved;
        assert_eq!(format!("{}", reason), "User approved");
    }

    #[test]
    fn test_decision_reason_custom() {
        let reason = PermissionDecisionReason::Custom("custom reason".to_string());
        assert_eq!(reason.description(), "custom reason");
    }

    #[test]
    fn test_decision_default() {
        let decision: PermissionDecision = Default::default();
        assert!(decision.is_pending());
    }
}
