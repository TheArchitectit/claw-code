//! Error types for the state crate

use std::io;
use thiserror::Error;

/// Result type for state operations
pub type StateResult<T> = Result<T, StateError>;

/// Error type for state operations
#[derive(Debug, Error)]
pub enum StateError {
    /// IO error during state operation
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Task not found
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Session not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Invalid settings
    #[error("Invalid settings: {0}")]
    InvalidSettings(String),

    /// Worktree not found
    #[error("Worktree not found: {0}")]
    WorktreeNotFound(String),

    /// Plugin not found
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),

    /// Memory directory error
    #[error("Memory directory error: {0}")]
    MemDir(String),

    /// Lock error (concurrent access)
    #[error("Lock error: {0}")]
    Lock(String),

    /// Validation error
    #[error("Validation error: {0}")]
    Validation(String),
}

impl From<serde_json::Error> for StateError {
    fn from(err: serde_json::Error) -> Self {
        StateError::Serialization(err.to_string())
    }
}

impl<T> From<std::sync::PoisonError<T>> for StateError {
    fn from(_: std::sync::PoisonError<T>) -> Self {
        StateError::Lock("Poisoned lock".to_string())
    }
}
