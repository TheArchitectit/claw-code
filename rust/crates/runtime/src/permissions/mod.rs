//! Permission system types for tool execution control.
//!
//! This module defines the permission model including modes, rules, and
//! decision types that control when and how tools can be executed.

pub mod context;
pub mod decisions;
pub mod results;
pub mod types;

// Re-exports for backward compatibility
pub use context::ToolPermissionContext;
pub use decisions::{
    ClassifierResult, PermissionAllowDecision, PermissionAskDecision,
    PermissionDecision, PermissionDecisionReason, PermissionDenyDecision, PermissionExplanation,
    PermissionMetadata, PermissionCommandMetadata, PermissionUpdate,
    PendingClassifierCheck, RiskLevel,
};
pub use results::PermissionResult;
pub use types::{
    AdditionalWorkingDirectory, ConfidenceLevel, PermissionBehavior, PermissionMode,
    PermissionRule, PermissionRuleSource, PermissionRuleValue, PermissionUpdateDestination,
    SandboxOverrideReason, ToolPermissionRulesBySource,
};
