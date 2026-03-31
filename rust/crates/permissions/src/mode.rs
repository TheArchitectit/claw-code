//! Permission modes define how the permission system behaves.
//!
//! The four modes are:
//! - **Default**: Normal interactive approval flow
//! - **Plan**: Validate without executing (planning mode)
//! - **BypassPermissions**: Skip all permission checks
//! - **Auto**: Automatically approve based on configuration

use serde::{Deserialize, Serialize};
use std::fmt;

/// The permission mode determines how permission requests are handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    /// Normal interactive approval flow.
    ///
    /// Users are prompted to approve or deny potentially dangerous operations.
    /// Decisions can be persisted for future use.
    #[default]
    Default,

    /// Planning mode - validate without executing.
    ///
    /// In this mode, tools are validated but not actually executed.
    /// Useful for previewing what operations would be performed.
    Plan,

    /// Bypass all permission checks.
    ///
    /// **Warning**: This mode disables all permission checks and should
    /// only be used in trusted environments or for testing.
    BypassPermissions,

    /// Automatically approve based on configuration.
    ///
    /// Uses the auto-approve list to determine which tools can run
    /// without user interaction. Tools not in the list are denied.
    Auto,
}

impl PermissionMode {
    /// Get a human-readable description of this mode.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Default => "Interactive approval flow with persistence",
            Self::Plan => "Planning mode - validate without executing",
            Self::BypassPermissions => "Bypass all permission checks (use with caution)",
            Self::Auto => "Automatically approve based on configuration",
        }
    }

    /// Check if this mode allows interactive prompts.
    #[must_use]
    pub fn is_interactive(&self) -> bool {
        matches!(self, Self::Default)
    }

    /// Check if this mode executes tools.
    #[must_use]
    pub fn executes(&self) -> bool {
        !matches!(self, Self::Plan)
    }

    /// Check if this mode requires user approval.
    #[must_use]
    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::Default)
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "default"),
            Self::Plan => write!(f, "plan"),
            Self::BypassPermissions => write!(f, "bypass"),
            Self::Auto => write!(f, "auto"),
        }
    }
}

impl From<&str> for PermissionMode {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "plan" => Self::Plan,
            "bypass" | "bypasspermissions" => Self::BypassPermissions,
            "auto" => Self::Auto,
            _ => Self::Default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_default() {
        let mode = PermissionMode::default();
        assert_eq!(mode, PermissionMode::Default);
        assert!(mode.is_interactive());
        assert!(mode.executes());
        assert!(mode.requires_approval());
    }

    #[test]
    fn test_mode_plan() {
        let mode = PermissionMode::Plan;
        assert!(!mode.is_interactive());
        assert!(!mode.executes());
        assert!(!mode.requires_approval());
    }

    #[test]
    fn test_mode_bypass() {
        let mode = PermissionMode::BypassPermissions;
        assert!(!mode.is_interactive());
        assert!(mode.executes());
        assert!(!mode.requires_approval());
    }

    #[test]
    fn test_mode_auto() {
        let mode = PermissionMode::Auto;
        assert!(!mode.is_interactive());
        assert!(mode.executes());
        assert!(!mode.requires_approval());
    }

    #[test]
    fn test_mode_display() {
        assert_eq!(PermissionMode::Default.to_string(), "default");
        assert_eq!(PermissionMode::Plan.to_string(), "plan");
        assert_eq!(PermissionMode::BypassPermissions.to_string(), "bypass");
        assert_eq!(PermissionMode::Auto.to_string(), "auto");
    }

    #[test]
    fn test_mode_from_str() {
        assert_eq!(PermissionMode::from("default"), PermissionMode::Default);
        assert_eq!(PermissionMode::from("plan"), PermissionMode::Plan);
        assert_eq!(PermissionMode::from("bypass"), PermissionMode::BypassPermissions);
        assert_eq!(PermissionMode::from("bypassPermissions"), PermissionMode::BypassPermissions);
        assert_eq!(PermissionMode::from("auto"), PermissionMode::Auto);
        assert_eq!(PermissionMode::from("unknown"), PermissionMode::Default);
    }

    #[test]
    fn test_mode_description() {
        assert!(!PermissionMode::Default.description().is_empty());
        assert!(!PermissionMode::Plan.description().is_empty());
        assert!(!PermissionMode::BypassPermissions.description().is_empty());
        assert!(!PermissionMode::Auto.description().is_empty());
    }
}
