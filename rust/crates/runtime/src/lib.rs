pub mod checkpoint;
pub mod context;
pub mod coordinator;
pub mod error_handling;
pub mod llm_client;
pub mod messages;
pub mod permissions;
pub mod query_engine;
pub mod registry;
pub mod stream_handler;
pub mod tool;
pub mod types;
pub mod utils;

#[cfg(feature = "anthropic")]
pub mod anthropic;

// Re-export commonly used types at crate root for convenience
pub use context::ToolUseContext;
pub use registry::{ToolRegistry, ToolRegistryBuilder};
pub use tool::{Tool, ToolOutput, ToolResult};
pub use types::{SessionId, ToolUseId};

// Re-export error handling utilities
pub use error_handling::{
    calculate_rate_limit_delay, http_status_to_error, with_panic_catch, with_retry,
    ConversationCheckpoint, RetryConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapPhase {
    CliEntry,
    FastPathVersion,
    StartupProfiler,
    SystemPromptFastPath,
    ChromeMcpFastPath,
    DaemonWorkerFastPath,
    BridgeFastPath,
    DaemonFastPath,
    BackgroundSessionFastPath,
    TemplateFastPath,
    EnvironmentRunnerFastPath,
    MainRuntime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapPlan {
    phases: Vec<BootstrapPhase>,
}

impl BootstrapPlan {
    #[must_use]
    pub fn claude_code_default() -> Self {
        Self::from_phases(vec![
            BootstrapPhase::CliEntry,
            BootstrapPhase::FastPathVersion,
            BootstrapPhase::StartupProfiler,
            BootstrapPhase::SystemPromptFastPath,
            BootstrapPhase::ChromeMcpFastPath,
            BootstrapPhase::DaemonWorkerFastPath,
            BootstrapPhase::BridgeFastPath,
            BootstrapPhase::DaemonFastPath,
            BootstrapPhase::BackgroundSessionFastPath,
            BootstrapPhase::TemplateFastPath,
            BootstrapPhase::EnvironmentRunnerFastPath,
            BootstrapPhase::MainRuntime,
        ])
    }

    #[must_use]
    pub fn from_phases(phases: Vec<BootstrapPhase>) -> Self {
        let mut deduped = Vec::new();
        for phase in phases {
            if !deduped.contains(&phase) {
                deduped.push(phase);
            }
        }
        Self { phases: deduped }
    }

    #[must_use]
    pub fn phases(&self) -> &[BootstrapPhase] {
        &self.phases
    }
}
