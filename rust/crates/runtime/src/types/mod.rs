//! Core types and error definitions for the runtime crate.
//!
//! This module provides the foundational types used throughout the runtime
//! including error handling patterns, identifiers, and common data structures.

pub mod errors;
pub mod ids;
pub mod results;
pub mod schema;

// Re-export all ID types
pub use ids::{CheckpointId, MessageId, SessionId, ToolUseId};

// Re-export all error types
pub use errors::{ErrorCategory, LlmApiError, QueryEngineError, ToolError};

// Re-export all result and data types
pub use results::{ConversationAction, Cost, ModelInfo, ModelProvider, QueryResult, ToolResult, Usage};

// Re-export schema types
pub use schema::JsonSchema;
