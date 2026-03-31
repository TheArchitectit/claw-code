//! Coordinator integration for multi-agent support in QueryEngine.
//!
//! This module provides integration between the QueryEngine and the coordinator
//! crate for multi-agent orchestration. Currently provides stub implementations
//! to avoid circular dependencies between runtime and coordinator crates.

pub mod builder;
pub mod config;
pub mod handles;
pub mod integration;
pub mod message_bus;
pub mod routing;
pub mod types;

pub use builder::CoordinatorHandleBuilder;
pub use config::{AgentConfig, TeamConfig};
pub use handles::{AgentHandle, TeamHandle};
pub use integration::CoordinatorHandle;
pub use message_bus::{Coordinator, MessageBus};
pub use types::*;

// Backward compatibility type alias
pub type CoordinatorIntegration = CoordinatorHandle;
