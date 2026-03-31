//! Supporting types for the tool use context.

/// File reading limits.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileReadingLimits {
    /// Maximum tokens to read.
    pub max_tokens: Option<usize>,

    /// Maximum bytes to read.
    pub max_size_bytes: Option<usize>,

    /// Maximum lines to read.
    pub max_lines: Option<usize>,
}

/// Glob limits.
#[derive(Debug, Clone, Copy, Default)]
pub struct GlobLimits {
    /// Maximum number of results.
    pub max_results: Option<usize>,
}

/// A tool decision record.
#[derive(Debug, Clone)]
pub struct ToolDecision {
    /// The source of the decision.
    pub source: String,

    /// The decision outcome.
    pub decision: ToolDecisionOutcome,

    /// The timestamp of the decision.
    pub timestamp: u64,
}

/// Tool decision outcomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolDecisionOutcome {
    /// The tool was accepted.
    Accept,

    /// The tool was rejected.
    Reject,
}

/// Query chain tracking for subagents.
#[derive(Debug, Clone)]
pub struct QueryChainTracking {
    /// The chain ID.
    pub chain_id: String,

    /// The depth in the chain.
    pub depth: u32,
}

/// Denial tracking state.
#[derive(Debug, Clone, Default)]
pub struct DenialTrackingState {
    /// The number of consecutive denials.
    pub consecutive_denials: u32,

    /// The last tool that was denied.
    pub last_denied_tool: Option<String>,

    /// The timestamp of the last denial.
    pub last_denial_timestamp: Option<u64>,
}

/// Thinking configuration.
#[derive(Debug, Clone)]
pub enum ThinkingConfig {
    /// Thinking is disabled.
    Disabled,

    /// Adaptive thinking based on context.
    Adaptive,

    /// Enabled thinking with a specific budget.
    Enabled { budget_tokens: u32 },
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self::Disabled
    }
}

/// Agent definitions container.
#[derive(Debug, Clone, Default)]
pub struct AgentDefinitions {
    /// Active agent definitions.
    pub active_agents: Vec<AgentDefinition>,

    /// All available agent definitions.
    pub all_agents: Vec<AgentDefinition>,
}

/// An agent definition.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    /// The agent name.
    pub name: String,

    /// The agent type.
    pub agent_type: String,

    /// The agent description.
    pub description: String,

    /// The prompt template.
    pub prompt_template: String,

    /// Allowed tools for this agent.
    pub allowed_tools: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_reading_limits() {
        let limits = FileReadingLimits::default();
        assert_eq!(limits.max_size_bytes, None);

        let custom = FileReadingLimits {
            max_size_bytes: Some(5000),
            max_tokens: Some(100),
            max_lines: Some(50),
        };
        assert_eq!(custom.max_size_bytes, Some(5000));
        assert_eq!(custom.max_tokens, Some(100));
    }

    #[test]
    fn test_glob_limits() {
        let limits = GlobLimits::default();
        assert_eq!(limits.max_results, None);

        let custom = GlobLimits {
            max_results: Some(100),
        };
        assert_eq!(custom.max_results, Some(100));
    }

    #[test]
    fn test_tool_decision_outcome() {
        let accept = ToolDecisionOutcome::Accept;
        let reject = ToolDecisionOutcome::Reject;

        assert_eq!(accept, ToolDecisionOutcome::Accept);
        assert_eq!(reject, ToolDecisionOutcome::Reject);
        assert_ne!(accept, reject);
    }

    #[test]
    fn test_thinking_config_default() {
        let config: ThinkingConfig = Default::default();
        matches!(config, ThinkingConfig::Disabled);
    }

    #[test]
    fn test_thinking_config_variants() {
        let disabled: ThinkingConfig = Default::default();
        assert!(matches!(disabled, ThinkingConfig::Disabled));

        let enabled = ThinkingConfig::Enabled { budget_tokens: 1000 };
        if let ThinkingConfig::Enabled { budget_tokens } = enabled {
            assert_eq!(budget_tokens, 1000);
        }
    }

    #[test]
    fn test_agent_definition_creation() {
        let agent = AgentDefinition {
            name: "test-agent".to_string(),
            agent_type: "default".to_string(),
            description: "A test agent".to_string(),
            prompt_template: "test template".to_string(),
            allowed_tools: vec!["bash".to_string()],
        };

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.description, "A test agent");
    }

    #[test]
    fn test_agent_definitions_collection() {
        let agent = AgentDefinition {
            name: "agent1".to_string(),
            agent_type: "default".to_string(),
            description: "Test agent".to_string(),
            prompt_template: "template".to_string(),
            allowed_tools: vec![],
        };

        let definitions = AgentDefinitions {
            active_agents: vec![agent.clone()],
            all_agents: vec![agent],
        };

        assert!(!definitions.active_agents.is_empty());
        assert_eq!(definitions.all_agents.len(), 1);
    }

    #[test]
    fn test_denial_tracking_state() {
        let state = DenialTrackingState {
            consecutive_denials: 0,
            last_denied_tool: None,
            last_denial_timestamp: None,
        };
        assert_eq!(state.consecutive_denials, 0);
        assert!(state.last_denied_tool.is_none());

        let updated = DenialTrackingState {
            consecutive_denials: 1,
            last_denied_tool: Some("Bash".to_string()),
            last_denial_timestamp: Some(12345678),
        };
        assert_eq!(updated.consecutive_denials, 1);
        assert_eq!(updated.last_denied_tool, Some("Bash".to_string()));
    }

    #[test]
    fn test_tool_decision() {
        let decision = ToolDecision {
            source: "test".to_string(),
            decision: ToolDecisionOutcome::Accept,
            timestamp: 12345678,
        };

        assert_eq!(decision.decision, ToolDecisionOutcome::Accept);
        assert_eq!(decision.source, "test");
    }

    #[test]
    fn test_query_chain_tracking() {
        let tracking = QueryChainTracking {
            chain_id: "chain1".to_string(),
            depth: 0,
        };
        assert_eq!(tracking.chain_id, "chain1");
        assert_eq!(tracking.depth, 0);
    }
}
