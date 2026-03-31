//! Coordinator - Multi-agent swarm orchestration for R.A.D Codicological
//!
//! This crate provides the infrastructure for managing multiple AI agents
//! working together as a team. It handles:
//!
//! - Agent lifecycle (spawn, monitor, terminate)
//! - Team creation and orchestration
//! - Inter-agent messaging
//! - Work distribution
//! - Result aggregation
//! - Reconnection handling
//!
//! # Example
//!
//! ```rust,no_run
//! use coordinator::{Coordinator, AgentConfig};
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create a coordinator
//!     let coordinator = Coordinator::new();
//!
//!     // Configure an agent
//!     let agent_config = AgentConfig::new("agent-1")
//!         .with_model("claude-3-5-sonnet");
//!
//!     // Spawn the agent
//!     let agent = coordinator.spawn_agent(agent_config).await.unwrap();
//! }
//! ```

#![warn(clippy::all)]
#![warn(clippy::pedantic)]

pub mod agent;
pub mod message_bus;
pub mod result_aggregator;
pub mod swarm;
pub mod team;
pub mod work_distribution;

pub use agent::{
    AgentConfig, AgentHandle, AgentId, AgentLifecycle, AgentState, AgentStatus,
    Capability, ReconnectionPolicy, WorkAssignment,
};

pub use message_bus::{
    InterAgentMessage, MessageBus, MessagePriority, MessageReceipt, MessageType,
};

pub use result_aggregator::{
    AggregationStrategy, ResultAggregator, ResultEntry, VoteResult,
};

pub use swarm::{
    Coordinator, CoordinatorConfig, SwarmEvent, SwarmMetrics, SwarmState,
};

pub use team::{
    RoundRobinStrategy, TeamConfig, TeamHandle, TeamId, TeamOrchestrator, TeamState, TeamStatus, WorkStrategy,
};

pub use work_distribution::{
    DistributionStrategy, PartitionStrategy, PriorityWorkQueue, TaskDistributor, TaskPartition,
    WorkUnit,
};

use std::sync::Arc;
use tokio::sync::RwLock;

/// Version of the coordinator crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared coordinator reference.
pub type SharedCoordinator = Arc<RwLock<Coordinator>>;
