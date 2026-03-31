//! Configuration and builder for the QueryEngine.

use std::sync::Arc;

use crate::context::{AgentDefinitions, McpConnection, ToolUseContext};
use crate::llm_client::{LlmClient, MockLlmClient};
use crate::messages::Message;
use crate::permissions::PermissionResult;
use crate::registry::ToolRegistry;
use crate::tool::Tool;
use crate::types::{MessageId, ToolUseId};

/// Default model to use when none specified.
pub const DEFAULT_MODEL: &str = "claude-3-5-sonnet";

/// Default fallback model when primary fails.
pub const DEFAULT_FALLBACK_MODEL: &str = "claude-3-haiku";

/// Maximum number of fallback attempts.
pub const MAX_FALLBACK_ATTEMPTS: u32 = 2;

/// Delay before fallback attempt (milliseconds).
pub const FALLBACK_DELAY_MS: u64 = 500;

/// Model pricing constants (per million tokens in USD).
pub const CLAUDE_3_5_SONNET_PRICE: f64 = 3.0; // $3 per million input tokens
pub const CLAUDE_3_5_SONNET_OUTPUT_PRICE: f64 = 15.0; // $15 per million output tokens
pub const CLAUDE_3_HAIKU_PRICE: f64 = 0.25;
pub const CLAUDE_3_HAIKU_OUTPUT_PRICE: f64 = 1.25;
pub const CLAUDE_3_OPUS_PRICE: f64 = 15.0;
pub const CLAUDE_3_OPUS_OUTPUT_PRICE: f64 = 75.0;

/// Model fallback priority for automatic model selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelPriority {
    /// Fast, cost-effective model for simple tasks.
    Fast,
    /// Balanced model for general use.
    Balanced,
    /// High-capability model for complex reasoning.
    Complex,
}

impl Default for ModelPriority {
    fn default() -> Self {
        Self::Balanced
    }
}

/// Configuration for a single model in the fallback chain.
#[derive(Debug, Clone)]
pub struct FallbackModelConfig {
    /// The model identifier.
    pub model: String,
    /// The priority of this model for automatic selection.
    pub priority: ModelPriority,
    /// Whether this model supports large context windows.
    pub supports_large_context: bool,
    /// Whether this model supports complex reasoning.
    pub supports_complex_reasoning: bool,
}

impl FallbackModelConfig {
    /// Create a new fallback model config.
    #[must_use]
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            priority: ModelPriority::Balanced,
            supports_large_context: false,
            supports_complex_reasoning: false,
        }
    }

    /// Set the priority.
    #[must_use]
    pub fn with_priority(mut self, priority: ModelPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set large context support.
    #[must_use]
    pub fn with_large_context(mut self, supports: bool) -> Self {
        self.supports_large_context = supports;
        self
    }

    /// Set complex reasoning support.
    #[must_use]
    pub fn with_complex_reasoning(mut self, supports: bool) -> Self {
        self.supports_complex_reasoning = supports;
        self
    }
}

/// Statistics for model fallback tracking.
#[derive(Debug, Clone, Default)]
pub struct FallbackStatistics {
    /// Number of times fallback was triggered.
    pub fallback_count: u32,
    /// Number of successful fallbacks.
    pub successful_fallbacks: u32,
    /// Number of failed fallbacks (all models exhausted).
    pub failed_fallbacks: u32,
    /// Total cost incurred from fallback attempts.
    pub total_fallback_cost_usd: f64,
    /// Models tried in order during last fallback.
    pub models_tried: Vec<String>,
    /// Current model in use (may differ from primary).
    pub current_model: Option<String>,
}

impl FallbackStatistics {
    /// Record a fallback attempt.
    pub fn record_attempt(&mut self, from_model: &str, to_model: &str) {
        self.fallback_count += 1;
        self.models_tried.push(format!("{} -> {}", from_model, to_model));
        self.current_model = Some(to_model.to_string());
    }

    /// Record a successful fallback.
    pub fn record_success(&mut self, model: &str, cost: f64) {
        self.successful_fallbacks += 1;
        self.total_fallback_cost_usd += cost;
        self.current_model = Some(model.to_string());
    }

    /// Record a failed fallback (all models exhausted).
    pub fn record_failure(&mut self) {
        self.failed_fallbacks += 1;
    }

    /// Reset statistics.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Configuration for the QueryEngine.
#[derive(Clone)]
pub struct QueryEngineConfig {
    /// The current working directory.
    pub cwd: String,

    /// The tool registry.
    pub tool_registry: ToolRegistry,

    /// MCP client connections.
    pub mcp_clients: Vec<McpConnection>,

    /// Agent definitions.
    pub agent_definitions: AgentDefinitions,

    /// Callback for checking tool permissions.
    pub can_use_tool: Arc<
        dyn for<'a> Fn(
                &'a (dyn Tool + Send + Sync),
                &'a serde_json::Value,
                &'a ToolUseContext,
                ToolUseId,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = PermissionResult> + Send + 'a>,
            > + Send
            + Sync,
    >,

    /// Callback for getting app state.
    pub get_app_state: Arc<dyn Fn() -> serde_json::Value + Send + Sync>,

    /// Callback for setting app state.
    pub set_app_state: Arc<dyn Fn(serde_json::Value) + Send + Sync>,

    /// Initial messages.
    pub initial_messages: Vec<Message>,

    /// Custom system prompt.
    pub custom_system_prompt: Option<String>,

    /// Additional system prompt to append.
    pub append_system_prompt: Option<String>,

    /// User-specified model.
    pub user_specified_model: Option<String>,

    /// Fallback model.
    pub fallback_model: Option<String>,

    /// Enable/disable fallback to alternative models on failure.
    pub fallback_enabled: bool,

    /// Custom fallback chain (ordered list of models to try).
    pub fallback_chain: Vec<String>,

    /// Max fallback attempts.
    pub max_fallback_attempts: u32,

    /// Maximum number of turns.
    pub max_turns: Option<u32>,

    /// Maximum budget in USD.
    pub max_budget_usd: Option<f64>,

    /// Whether to include partial messages.
    pub include_partial_messages: bool,

    /// Whether to replay user messages.
    pub replay_user_messages: bool,

    /// Abort controller for cancellation.
    pub abort_controller: Arc<tokio::sync::Notify>,

    /// Handler for URL elicitations.
    pub handle_elicitation: Option<Arc<dyn Fn(String, serde_json::Value) + Send + Sync>>,

    /// LLM client for API calls.
    pub llm_client: Option<Arc<dyn LlmClient + Send + Sync>>,
}

impl QueryEngineConfig {
    /// Create a new query engine config.
    #[must_use]
    pub fn new(cwd: impl Into<String>, tool_registry: ToolRegistry) -> Self {
        Self {
            cwd: cwd.into(),
            tool_registry,
            mcp_clients: Vec::new(),
            agent_definitions: AgentDefinitions::default(),
            can_use_tool: Arc::new(|_, _, _, _| Box::pin(async { PermissionResult::allow() })),
            get_app_state: Arc::new(|| serde_json::Value::Object(serde_json::Map::new())),
            set_app_state: Arc::new(|_| {}),
            initial_messages: Vec::new(),
            custom_system_prompt: None,
            append_system_prompt: None,
            user_specified_model: None,
            fallback_model: None,
            fallback_enabled: true,
            fallback_chain: Vec::new(),
            max_fallback_attempts: MAX_FALLBACK_ATTEMPTS,
            max_turns: Some(100),
            max_budget_usd: None,
            include_partial_messages: false,
            replay_user_messages: false,
            abort_controller: Arc::new(tokio::sync::Notify::new()),
            handle_elicitation: None,
            llm_client: None,
        }
    }

    /// Calculate the cost for a given model and token usage.
    ///
    /// # Arguments
    /// * `model` - The model name (e.g., "claude-3-5-sonnet")
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    ///
    /// # Returns
    /// The cost in USD
    #[must_use]
    pub fn calculate_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        let model_lower = model.to_lowercase();

        // Determine pricing based on model
        let (input_price, output_price) = if model_lower.contains("opus") {
            (CLAUDE_3_OPUS_PRICE, CLAUDE_3_OPUS_OUTPUT_PRICE)
        } else if model_lower.contains("haiku") {
            (CLAUDE_3_HAIKU_PRICE, CLAUDE_3_HAIKU_OUTPUT_PRICE)
        } else {
            // Default to Sonnet pricing for any sonnet model or unknown model
            (CLAUDE_3_5_SONNET_PRICE, CLAUDE_3_5_SONNET_OUTPUT_PRICE)
        };

        // Calculate cost per million tokens
        let input_cost = (input_tokens as f64 / 1_000_000.0) * input_price;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * output_price;

        input_cost + output_cost
    }

    /// Get the input token price for a model (per million tokens).
    #[must_use]
    pub fn get_input_price(&self, model: &str) -> f64 {
        let model_lower = model.to_lowercase();
        if model_lower.contains("opus") {
            CLAUDE_3_OPUS_PRICE
        } else if model_lower.contains("haiku") {
            CLAUDE_3_HAIKU_PRICE
        } else {
            CLAUDE_3_5_SONNET_PRICE
        }
    }

    /// Get the output token price for a model (per million tokens).
    #[must_use]
    pub fn get_output_price(&self, model: &str) -> f64 {
        let model_lower = model.to_lowercase();
        if model_lower.contains("opus") {
            CLAUDE_3_OPUS_OUTPUT_PRICE
        } else if model_lower.contains("haiku") {
            CLAUDE_3_HAIKU_OUTPUT_PRICE
        } else {
            CLAUDE_3_5_SONNET_OUTPUT_PRICE
        }
    }

    /// Check if a given cost would exceed the max budget.
    ///
    /// # Arguments
    /// * `current_cost` - The current accumulated cost
    ///
    /// # Returns
    /// `true` if the budget would be exceeded, `false` otherwise
    #[must_use]
    pub fn would_exceed_budget(&self, current_cost: f64) -> bool {
        self.max_budget_usd
            .map(|max_budget| current_cost > max_budget)
            .unwrap_or(false)
    }

    /// Build the default fallback chain.
    #[must_use]
    pub fn default_fallback_chain() -> Vec<FallbackModelConfig> {
        vec![
            FallbackModelConfig::new(DEFAULT_MODEL).with_priority(ModelPriority::Balanced),
            FallbackModelConfig::new("claude-3-haiku")
                .with_priority(ModelPriority::Fast)
                .with_large_context(false),
            FallbackModelConfig::new("claude-3-opus")
                .with_priority(ModelPriority::Complex)
                .with_large_context(true)
                .with_complex_reasoning(true),
        ]
    }

    /// Get the effective fallback chain based on configuration.
    #[must_use]
    pub fn get_fallback_chain(&self) -> Vec<String> {
        if !self.fallback_chain.is_empty() {
            self.fallback_chain.clone()
        } else if let Some(ref fallback) = self.fallback_model {
            vec![self.user_specified_model.clone().unwrap_or_else(|| DEFAULT_MODEL.to_string()), fallback.clone()]
        } else {
            Self::default_fallback_chain()
                .into_iter()
                .map(|c| c.model)
                .collect()
        }
    }

    /// Select the best fallback model based on error type and context.
    #[must_use]
    pub fn select_fallback_model(
        &self,
        error: &crate::types::LlmApiError,
        attempted_models: &[String],
    ) -> Option<String> {
        let chain = self.get_fallback_chain();

        // Filter out already attempted models
        let available: Vec<_> = chain
            .into_iter()
            .filter(|m| !attempted_models.contains(m))
            .collect();

        if available.is_empty() {
            return None;
        }

        match error {
            // For rate limits, prefer fast/cheap models
            crate::types::LlmApiError::RateLimit { .. } => {
                available.iter().find(|m| m.contains("haiku")).cloned()
                    .or_else(|| available.first().cloned())
            }
            // For context length issues, prefer models with large context support
            crate::types::LlmApiError::ContextLengthExceeded { .. } => {
                available.iter().find(|m| m.contains("opus")).cloned()
                    .or_else(|| available.first().cloned())
            }
            // For model unavailable, try any other model
            crate::types::LlmApiError::ModelUnavailable { .. } => {
                available.first().cloned()
            }
            // For timeout, prefer faster models
            crate::types::LlmApiError::Timeout { .. } => {
                available.iter().find(|m| m.contains("haiku")).cloned()
                    .or_else(|| available.first().cloned())
            }
            _ => available.first().cloned(),
        }
    }

    /// Set the initial messages.
    pub fn with_initial_messages(mut self, messages: Vec<Message>) -> Self {
        self.initial_messages = messages;
        self
    }

    /// Set the max turns limit.
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = Some(max_turns);
        self
    }

    /// Set the max budget.
    pub fn with_max_budget(mut self, max_budget: f64) -> Self {
        self.max_budget_usd = Some(max_budget);
        self
    }

    /// Set the fallback model.
    pub fn with_fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }

    /// Enable or disable fallback.
    pub fn with_fallback_enabled(mut self, enabled: bool) -> Self {
        self.fallback_enabled = enabled;
        self
    }

    /// Set the fallback chain.
    pub fn with_fallback_chain(mut self, chain: Vec<String>) -> Self {
        self.fallback_chain = chain;
        self
    }

    /// Set the max fallback attempts.
    pub fn with_max_fallback_attempts(mut self, attempts: u32) -> Self {
        self.max_fallback_attempts = attempts;
        self
    }

    /// Set the custom system prompt.
    pub fn with_custom_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.custom_system_prompt = Some(prompt.into());
        self
    }
}

/// Builder for creating QueryEngine instances with specific configurations.
pub struct QueryEngineBuilder {
    pub(crate) config: QueryEngineConfig,
}

impl QueryEngineBuilder {
    /// Create a new builder with required parameters.
    #[must_use]
    pub fn new(cwd: impl Into<String>, tool_registry: ToolRegistry) -> Self {
        Self {
            config: QueryEngineConfig::new(cwd, tool_registry),
        }
    }

    /// Set the MCP clients.
    pub fn with_mcp_clients(mut self, clients: Vec<McpConnection>) -> Self {
        self.config.mcp_clients = clients;
        self
    }

    /// Set the agent definitions.
    pub fn with_agent_definitions(mut self, definitions: AgentDefinitions) -> Self {
        self.config.agent_definitions = definitions;
        self
    }

    /// Set the initial messages.
    pub fn with_initial_messages(mut self, messages: Vec<Message>) -> Self {
        self.config.initial_messages = messages;
        self
    }

    /// Set the max turns.
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.config.max_turns = Some(max_turns);
        self
    }

    /// Set the max budget.
    pub fn with_max_budget(mut self, budget: f64) -> Self {
        self.config.max_budget_usd = Some(budget);
        self
    }

    /// Set the custom system prompt.
    pub fn with_custom_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.custom_system_prompt = Some(prompt.into());
        self
    }

    /// Set the append system prompt.
    pub fn with_append_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.config.append_system_prompt = Some(prompt.into());
        self
    }

    /// Set the user-specified model.
    pub fn with_user_model(mut self, model: impl Into<String>) -> Self {
        self.config.user_specified_model = Some(model.into());
        self
    }

    /// Set the permission checker callback.
    pub fn with_permission_checker(
        mut self,
        checker: Arc<
            dyn for<'a> Fn(
                    &'a (dyn Tool + Send + Sync),
                    &'a serde_json::Value,
                    &'a ToolUseContext,
                    ToolUseId,
                ) -> std::pin::Pin<
                    Box<dyn std::future::Future<Output = PermissionResult> + Send + 'a>,
                > + Send
                + Sync,
        >,
    ) -> Self {
        self.config.can_use_tool = checker;
        self
    }

    /// Set the LLM client.
    pub fn with_llm_client(mut self, client: Arc<dyn LlmClient + Send + Sync>) -> Self {
        self.config.llm_client = Some(client);
        self
    }

    /// Build the QueryEngine.
    #[must_use]
    pub fn build(self) -> super::QueryEngine {
        super::QueryEngine::new(self.config)
    }
}
