//! The Tool trait and related types.
//!
//! This module defines the core `Tool` trait that all tools must implement,
//! along with supporting types for tool input/output schemas, progress
//! reporting, and tool results.

pub mod builder;
pub mod helpers;
pub mod output;
pub mod r#trait;
pub mod validation;

// Re-export all public types for backward compatibility
pub use builder::ToolBuilder;
pub use helpers::{find_tool_by_name, tool_matches_name};
pub use output::{McpMeta, ToolOutput};
pub use r#trait::{BoxedTool, SharedTool, Tool};
pub use validation::{InterruptBehavior, MaxResultSize, McpInfo, SearchOrReadInfo, ValidationResult};

// Re-export ToolResult from types for public use
pub use crate::types::ToolResult;

/// A collection of tools.
pub type Tools = Vec<BoxedTool>;
