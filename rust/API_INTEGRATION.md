# R.A.D Codicological 2.x - API and Integration Patterns

This document provides comprehensive documentation of the API surfaces, integration patterns, and architectural boundaries between crates in the R.A.D Codicological Rust implementation.

## Table of Contents

1. [Crate Dependency Architecture](#crate-dependency-architecture)
2. [Public API Surface by Crate](#public-api-surface-by-crate)
3. [Cross-Crate Integration Patterns](#cross-crate-integration-patterns)
4. [Thread Safety Architecture](#thread-safety-architecture)
5. [Error Propagation Patterns](#error-propagation-patterns)
6. [Configuration Flow](#configuration-flow)
7. [External Service Integrations](#external-service-integrations)

---

## Crate Dependency Architecture

### Dependency Graph

```
                    ┌─────────────────┐
                    │  rusty-claude-cli│
                    │    (binary)      │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                             │
    ┌─────────▼──────────┐        ┌─────────▼──────────┐
    │   compat-harness   │        │      runtime       │
    │   (TypeScript bridge)│       │   (core engine)    │
    └─────────┬──────────┘        └─────────┬──────────┘
              │                             │
    ┌─────────▼───────────────────────────────▼──────────┐
    │                    tools                          │
    │  (Tool trait + implementations: bash, files, etc) │
    └─────────┬───────────────────────────┬─────────────┘
              │                           │
    ┌─────────▼──────────┐      ┌─────────▼──────────┐
    │   permissions      │      │   coordinator    │
    │ (security/approval)│      │ (multi-agent swarm)│
    └────────────────────┘      └─────────┬──────────┘
                                        │
                              ┌─────────▼──────────┐
                              │      state        │
                              │ (persistence layer)│
                              └─────────┬──────────┘
                                        │
                              ┌─────────▼──────────┐
                              │     services        │
                              │(OAuth, analytics,   │
                              │  telemetry)         │
                              └─────────────────────┘
```

### Dependency Direction

- **Downstream crates** depend on **upstream crates**
- **runtime** → depends on **tools**, **state**
- **permissions** → depends on **tools**
- **coordinator** → depends on **tools** (avoided circular with runtime)
- **compat-harness** → depends on **commands**, **runtime**, **tools**

### Circular Dependency Avoidance

The `coordinator` crate deliberately does NOT depend on `runtime` to avoid circular dependencies:

```toml
# crates/coordinator/Cargo.toml
[dependencies]
# Internal dependencies - removed runtime to avoid circular dependency
tools = { path = "../tools" }
# runtime = { path = "../runtime" }  # INTENTIONALLY OMITTED
```

Runtime provides coordinator integration through a stub module:
```rust
// crates/runtime/src/coordinator/mod.rs
// Provides CoordinatorHandle, AgentConfig, TeamConfig without depending on coordinator crate internals
```

---

## Public API Surface by Crate

### 1. `tools` Crate

**Core Trait:**
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;
    async fn validate(&self, input: &ToolInput) -> ToolResult<()>;
    async fn execute(&self, input: ToolInput) -> ToolOutput;
    fn name(&self) -> &str;
}
```

**Public Types:**
- `ToolInput` - Structured input with builder pattern (`with_arg`)
- `ToolOutput` - Structured output with builder pattern (`with_field`)
- `ToolMetadata` - Tool description, read-only status, concurrency safety
- `ToolError` - Error variants: ValidationFailed, ExecutionFailed, PermissionDenied, NotFound, Timeout, Internal, Cancelled
- `ToolResult<T>` - Result alias for ToolError

**Tool Implementations:**
- `BashTool` - Shell command execution
- `FileReadTool`, `FileWriteTool`, `FileEditTool` - File operations
- `GlobTool`, `GrepTool` - File discovery
- `WebFetchTool`, `WebSearchTool` - Web operations
- `McpTool` - Model Context Protocol integration
- `LspTool` - Language Server Protocol integration
- `SendMessageTool` - Inter-agent messaging
- `TaskCreateTool`, `TaskUpdateTool`, `TaskGetTool`, `TaskListTool` - Task management
- `TodoWriteTool` - Todo management

**Registry Types:**
- `ToolRegistry` - Static manifest of available tools
- `ToolManifestEntry` - Entry in the manifest
- `ToolSource` - Base vs Conditional tool sources

### 2. `runtime` Crate

**Core Types:**
```rust
// Re-exports from tools (common use)
pub use context::ToolUseContext;
pub use registry::{ToolRegistry, ToolRegistryBuilder};
pub use tool::{Tool, ToolOutput, ToolResult};
pub use types::{SessionId, ToolUseId};

// Error handling utilities
pub use error_handling::{
    calculate_rate_limit_delay, http_status_to_error, with_panic_catch, with_retry,
    ConversationCheckpoint, RetryConfig,
};
```

**Submodules:**
- `query_engine` - Main QueryEngine with execution loop
- `registry` - Dynamic tool registry with `DashMap` storage
- `llm_client` - LLM client trait and implementations
- `anthropic` - Anthropic API client
- `stream_handler` - Streaming response accumulation
- `messages` - Conversation message types
- `context` - ToolUseContext for tool execution
- `permissions` - Permission checking integration
- `types` - Core type definitions (ids, errors, results)
- `coordinator` - Coordinator integration stubs
- `checkpoint` - Conversation checkpointing
- `error_handling` - Retry logic and error categorization

**QueryEngine API:**
```rust
pub struct QueryEngine {
    // Config: max_turns, max_budget_usd, model, etc.
}

impl QueryEngine {
    pub async fn submit_message(&self, content: &str, options: Option<SubmitMessageOptions>) -> Result<QueryExecution>;
    pub async fn run(&self, content: &str, options: Option<SubmitMessageOptions>) -> Result<ConversationResult>;
    pub async fn run_with_tools(&self, content: &str, options: Option<SubmitMessageOptions>) -> Result<ConversationResult>;
    pub async fn reset(&self);
    pub async fn get_messages(&self) -> Vec<Message>;
    pub async fn total_usage(&self) -> Usage;
    pub fn session_id(&self) -> SessionId;
}
```

### 3. `state` Crate

**Core Type - StateManager:**
```rust
#[derive(Debug, Clone)]
pub struct StateManager {
    inner: Arc<RwLock<StateManagerInner>>,
}

impl StateManager {
    pub async fn new(project_dir: impl Into<PathBuf>) -> StateResult<Self>;
    pub async fn sessions(&self) -> SessionManager;
    pub async fn tasks(&self) -> TaskStorage;
    pub async fn settings(&self) -> SettingsManager;
    pub async fn memdir(&self) -> MemDirManager;
    pub async fn worktrees(&self) -> WorktreeManager;
    pub async fn plugins(&self) -> PluginStateManager;
    pub async fn save_all(&self) -> StateResult<()>;
    pub async fn load_all(&self) -> StateResult<()>;
}
```

**Public Types:**
- `StateError`, `StateResult<T>` - Error handling
- `SessionManager`, `SessionInfo`, `SessionStorageConfig` - Session management
- `TaskStorage`, `Task`, `TaskStatus`, `TaskPersistence` - Task persistence
- `SettingsManager`, `Settings`, `SettingSource` - Configuration
- `MemDirManager`, `MemDirConfig`, `MemoryEntry`, `MemoryType` - Memory directory
- `WorktreeManager`, `GitWorktreeState` - Git worktree tracking
- `PluginStateManager`, `PluginState` - Plugin state

### 4. `permissions` Crate

**Core Type - PermissionManager:**
```rust
pub struct PermissionManager {
    config: PermissionConfig,
    store: Arc<dyn PermissionStore>,
    hooks: Vec<Box<dyn PermissionHook>>,
    pending: Arc<DashMap<String, PendingPermission>>,
    cache: Arc<DashMap<String, PermissionDecision>>,
}

impl PermissionManager {
    pub fn new(config: PermissionConfig) -> Self;
    pub fn with_store(config: PermissionConfig, store: Arc<dyn PermissionStore>) -> Self;
    pub fn register_hook<H: PermissionHook + 'static>(&mut self, hook: H);
    pub async fn evaluate(&self, request: PermissionRequest) -> PermissionResult<PermissionDecision>;
    pub async fn approve(&self, request_id: &str, permanent: bool) -> PermissionResult<PermissionDecision>;
    pub async fn deny(&self, request_id: &str, reason: impl Into<String>, permanent: bool) -> PermissionResult<PermissionDecision>;
    pub async fn execute_with_permission<T: Tool>(&self, tool: &T, input: ToolInput, context: PermissionContext) -> PermissionResult<ToolOutput>;
}
```

**Public Types:**
- `PermissionConfig` - Mode, auto-approve lists, sandbox config, timeouts
- `PermissionMode` - Default, Plan, BypassPermissions, Auto
- `PermissionRequest` - Request for permission evaluation
- `PermissionDecision` - Allow/deny with reason
- `PermissionContext` - Session ID, CWD, data
- `PermissionError` - Denied, Cancelled, InvalidConfig, Persistence, Sandbox, Timeout, Internal
- `PermissionHook` trait - Extension point for custom logic
- `Sandbox`, `SandboxConfig`, `SandboxError` - Sandboxed execution
- `PermissibleTool` trait - Blanket impl for all Tool types

### 5. `coordinator` Crate

**Core Type - Coordinator:**
```rust
pub struct Coordinator {
    pub config: CoordinatorConfig,
    pub agent_lifecycle: Arc<AgentLifecycle>,
    pub team_orchestrator: Arc<TeamOrchestrator>,
    pub message_bus: Arc<MessageBus>,
    // ... internal fields
}

impl Coordinator {
    pub fn new() -> Self;
    pub fn with_config(config: CoordinatorConfig) -> Self;
    pub async fn spawn_agent(&self, config: AgentConfig) -> Result<AgentHandle, SwarmError>;
    pub async fn terminate_agent(&self, agent_id: AgentId) -> Result<(), SwarmError>;
    pub async fn create_team(&self, config: TeamConfig) -> Result<TeamHandle, SwarmError>;
    pub async fn disband_team(&self, team_id: TeamId) -> Result<(), SwarmError>;
    pub async fn submit_work(&self, work: WorkUnit) -> Result<String, SwarmError>;
    pub async fn broadcast_message(&self, message: InterAgentMessage) -> Result<(), SwarmError>;
    pub async fn send_message_to_agent(&self, to: AgentId, message: InterAgentMessage) -> Result<(), SwarmError>;
}
```

**Public Types:**
- `AgentConfig`, `AgentHandle`, `AgentId`, `AgentState`, `AgentStatus`
- `TeamConfig`, `TeamHandle`, `TeamId`, `TeamState`, `TeamStatus`
- `InterAgentMessage`, `MessageBus`, `MessagePriority`, `MessageType`
- `WorkUnit`, `TaskDistributor`, `DistributionStrategy`
- `ResultAggregator`, `AggregationStrategy`, `ResultEntry`
- `SwarmEvent`, `SwarmMetrics`, `SwarmState`, `SwarmError`
- `Capability`, `WorkAssignment`, `ReconnectionPolicy`
- `SharedCoordinator` - `Arc<RwLock<Coordinator>>`

### 6. `services` Crate

**Public Types:**
```rust
// OAuth
pub use oauth::{OAuthManager, OAuthProvider, OAuthToken, OAuthConfig};
pub use oauth::{SecureStorage, StorageBackend, KeyringStorage, EncryptedFileStorage, EnvironmentStorage};

// Analytics
pub use analytics::{AnalyticsClient, AnalyticsEvent, AnalyticsConfig, EventBuilder, SessionMetrics};
pub use analytics::{AnalyticsSink, ConsoleSink, MemorySink, HttpSink};
pub use analytics::{init_global_analytics, global_analytics};

// Telemetry
pub use telemetry::{Telemetry, TelemetryClient, TelemetryConfig, TelemetrySpan, SpanBuilder, TelemetryValue};
pub use telemetry::{init_global_telemetry, global_telemetry, shutdown_global_telemetry};
```

**Convenience Function:**
```rust
pub async fn init_all(service_name: impl Into<String>) -> Result<(AnalyticsClient, Arc<OAuthManager>, Telemetry)>;
```

### 7. `compat-harness` Crate

**Purpose:** Bridge between TypeScript upstream and Rust implementation

**Public Types:**
```rust
pub struct UpstreamPaths { repo_root: PathBuf }
pub struct ExtractedManifest {
    pub commands: CommandRegistry,
    pub tools: ToolRegistry,
    pub bootstrap: BootstrapPlan,
}

impl UpstreamPaths {
    pub fn from_repo_root(repo_root: impl Into<PathBuf>) -> Self;
    pub fn from_workspace_dir(workspace_dir: impl AsRef<Path>) -> Self;
    pub fn commands_path(&self) -> PathBuf;  // -> src/commands.ts
    pub fn tools_path(&self) -> PathBuf;     // -> src/tools.ts
    pub fn cli_path(&self) -> PathBuf;     // -> src/entrypoints/cli.tsx
}

pub fn extract_manifest(paths: &UpstreamPaths) -> std::io::Result<ExtractedManifest>;
pub fn extract_commands(source: &str) -> CommandRegistry;
pub fn extract_tools(source: &str) -> ToolRegistry;
pub fn extract_bootstrap_plan(source: &str) -> BootstrapPlan;
```

### 8. `commands` Crate

**Public Types:**
```rust
pub struct CommandManifestEntry { pub name: String, pub source: CommandSource }
pub struct CommandRegistry { entries: Vec<CommandManifestEntry> }
pub enum CommandSource { Builtin, InternalOnly, FeatureGated }

impl CommandRegistry {
    pub fn new(entries: Vec<CommandManifestEntry>) -> Self;
    pub fn entries(&self) -> &[CommandManifestEntry];
}
```

---

## Cross-Crate Integration Patterns

### Pattern 1: Trait-Based Tool Integration

Tools implement the `Tool` trait from `tools` crate, usable across all downstream crates:

```rust
// In tools crate
trait Tool: Send + Sync {
    async fn execute(&self, input: ToolInput) -> ToolOutput;
}

// In runtime crate - tool gets wrapped with additional capabilities
trait RuntimeTool: Tool {
    async fn execute_with_context(&self, input: Value, context: &ToolUseContext, ...) -> ToolResult<ToolOutput>;
}
```

### Pattern 2: Registry Pattern with DashMap

Thread-safe concurrent registries using `DashMap`:

```rust
// crates/runtime/src/registry/registry.rs
pub struct ToolRegistry {
    tools: Arc<DashMap<String, SharedTool>>,      // Primary lookup
    aliases: Arc<DashMap<String, String>>,        // alias -> primary name
}

// Usage:
let registry = ToolRegistry::from_tools(tools);
if let Some(tool) = registry.get("BashTool") {
    let output = tool.execute(input).await;
}
```

### Pattern 3: Arc<RwLock<T>> for Shared Mutable State

Standard pattern for shared mutable state across async boundaries:

```rust
// crates/state/src/lib.rs
pub struct StateManager {
    inner: Arc<RwLock<StateManagerInner>>,
}

// crates/coordinator/src/swarm.rs
pub struct Coordinator {
    state: Arc<RwLock<SwarmState>>,
    metrics: Arc<RwLock<SwarmMetrics>>,
    event_subscribers: Arc<RwLock<Vec<UnboundedSender<SwarmEvent>>>>,
}

// crates/runtime/src/context/mod.rs
pub struct ToolUseContext {
    messages: Arc<RwLock<Vec<Message>>>,
    app_state: Arc<RwLock<serde_json::Value>>,
    tool_decisions: Arc<RwLock<HashMap<String, ToolDecision>>>,
    in_progress_tool_use_ids: Arc<Mutex<HashSet<ToolUseId>>>,
}
```

### Pattern 4: async_trait for Async Traits

All async traits use `async_trait` macro:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn stream(&self, request: LlmRequest) -> QueryResult<LlmStream>;
    async fn complete(&self, request: LlmRequest) -> QueryResult<LlmResponse>;
}

#[async_trait]
pub trait Tool: Send + Sync {
    async fn validate(&self, input: &ToolInput) -> ToolResult<()>;
    async fn execute(&self, input: ToolInput) -> ToolOutput;
}

#[async_trait]
pub trait PermissionStore: Send + Sync {
    async fn get(&self, tool_name: &str, session_id: &str) -> Result<Option<PermissionRecord>, PersistenceError>;
    async fn set(&self, record: PermissionRecord) -> Result<(), PersistenceError>;
}
```

### Pattern 5: Type Aliases for Common Patterns

```rust
// crates/runtime/src/tool/r#trait.rs
pub type BoxedTool = Box<dyn Tool>;
pub type SharedTool = Arc<dyn Tool>;

// crates/runtime/src/llm_client/client.rs
pub type BoxedLlmClient = Box<dyn LlmClient>;
pub type SharedLlmClient = Arc<dyn LlmClient>;

// crates/coordinator/src/lib.rs
pub type SharedCoordinator = Arc<RwLock<Coordinator>>;

// crates/permissions/src/lib.rs
pub type PermissionResult<T> = Result<T, PermissionError>;

// crates/state/src/lib.rs
pub type StateResult<T> = Result<T, StateError>;

// crates/services/src/lib.rs
pub type Result<T> = anyhow::Result<T>;
```

### Pattern 6: Cross-Crate Error Mapping

Errors are mapped at crate boundaries:

```rust
// crates/permissions/src/lib.rs
impl From<store::PersistenceError> for PermissionError {
    fn from(err: store::PersistenceError) -> Self {
        match err {
            store::PersistenceError::NotFound => PermissionError::InvalidConfig { ... },
            _ => PermissionError::Persistence { message: err.to_string() },
        }
    }
}

// crates/runtime/src/types/errors.rs
impl QueryEngineError {
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::LlmApi(api_err) => api_err.category(),
            Self::Tool(tool_err) => /* categorize tool errors */,
            _ => ErrorCategory::Permanent,
        }
    }
}
```

---

## Thread Safety Architecture

### Core Thread Safety Requirements

All types that cross async/task boundaries must be `Send + Sync`:

```rust
// Tool trait requires Send + Sync
#[async_trait]
pub trait Tool: Send + Sync { ... }

// LLM client requires Send + Sync
#[async_trait]
pub trait LlmClient: Send + Sync { ... }

// Permission hooks require Send + Sync
#[async_trait]
pub trait PermissionHook: Send + Sync { ... }
```

### Concurrency Primitives Used

| Primitive | Use Case | Crate |
|-----------|----------|-------|
| `Arc<RwLock<T>>` | Shared mutable state with many readers | state, coordinator, runtime/context |
| `Arc<Mutex<T>>` | Shared mutable state with exclusive access | runtime/context, permissions |
| `Arc<DashMap<K, V>>` | Concurrent hash map (lock-free reads) | runtime/registry, permissions |
| `tokio::sync::mpsc` | Message passing between agents | coordinator/message_bus |
| `OnceLock<T>` | Lazy static initialization | tools/mcp (global store) |
| `parking_lot` | Faster synchronization primitives | runtime (optional optimization) |

### Example: Thread-Safe State Management

```rust
// crates/state/src/lib.rs
pub struct StateManager {
    inner: Arc<RwLock<StateManagerInner>>,
}

impl StateManager {
    pub async fn sessions(&self) -> SessionManager {
        let inner = self.inner.read().await;  // Non-blocking read
        inner.session_manager.clone()
    }

    pub async fn save_all(&self) -> StateResult<()> {
        let inner = self.inner.read().await;
        inner.task_storage.save_all().await?;  // Multiple reads concurrent
        inner.session_manager.save_current().await?;
        Ok(())
    }
}
```

---

## Error Propagation Patterns

### Error Hierarchy

```
ErrorCategory (transient vs permanent)
    │
    ├── LlmApiError (runtime)
    │       ├── RateLimit (transient)
    │       ├── Authentication (permanent)
    │       ├── ServerError (transient)
    │       ├── Timeout (transient)
    │       └── ...
    │
    ├── ToolError (runtime/tools)
    │       ├── Cancelled
    │       ├── InvalidInput
    │       ├── ExecutionFailed
    │       ├── NotFound
    │       ├── PermissionDenied
    │       ├── Timeout (transient)
    │       ├── Panic (transient)
    │       └── Internal
    │
    ├── QueryEngineError (runtime)
    │       ├── Tool(ToolError)
    │       ├── LlmApi(LlmApiError)
    │       ├── Aborted
    │       ├── MaxTurnsExceeded
    │       ├── MaxBudgetExceeded
    │       └── ...
    │
    ├── PermissionError (permissions)
    │       ├── Denied
    │       ├── Cancelled
    │       ├── Timeout
    │       └── ...
    │
    └── StateError (state)
            ├── Io
            ├── Serialization
            └── NotFound
```

### Retry Logic Pattern

```rust
// crates/runtime/src/types/errors.rs
impl LlmApiError {
    pub fn category(&self) -> ErrorCategory {
        match self {
            // Transient - should retry
            Self::RateLimit { .. }
            | Self::ServerError { .. }
            | Self::Timeout { .. }
            | Self::Network { .. }
            | Self::ModelUnavailable { .. }
            | Self::ContextLengthExceeded { .. } => ErrorCategory::Transient,

            // Permanent - fail fast
            Self::Authentication { .. }
            | Self::ParseError { .. }
            | Self::Other { .. } => ErrorCategory::Permanent,
        }
    }

    pub fn is_fallback_trigger(&self) -> bool {
        matches!(self,
            Self::RateLimit { .. }
            | Self::ModelUnavailable { .. }
            | Self::Timeout { .. }
            | Self::ContextLengthExceeded { .. }
        )
    }
}
```

### Error Handling in QueryEngine

```rust
// crates/runtime/src/query_engine/engine.rs
match self.execute_turn().await {
    Ok(()) => continue,
    Err(QueryEngineError::LlmApi(api_err)) if api_err.is_fallback_trigger() => {
        // Try fallback model
        self.try_fallback_model(api_err).await?;
    }
    Err(QueryEngineError::MaxTurnsExceeded { max_turns }) => {
        return Err(QueryEngineError::MaxTurnsExceeded { max_turns });
    }
    Err(e) if e.is_retryable() && retries < max_retries => {
        retries += 1;
        sleep(backoff).await;
        continue;
    }
    Err(e) => return Err(e),
}
```

---

## Configuration Flow

### Configuration Hierarchy

```
1. Default values (in code)
2. Configuration files (settings.json)
3. Environment variables
4. CLI arguments
5. Runtime overrides (per-session)
```

### Config Types by Crate

```rust
// crates/runtime/src/query_engine/config.rs
pub struct QueryEngineConfig {
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub model: String,
    pub fallback_models: Vec<FallbackModelConfig>,
    pub timeout_seconds: u64,
}

// crates/permissions/src/lib.rs
pub struct PermissionConfig {
    pub mode: PermissionMode,
    pub auto_approve: Vec<String>,
    pub require_approval: Vec<String>,
    pub persistence_enabled: bool,
    pub sandbox: SandboxConfig,
    pub request_timeout_ms: u64,
}

// crates/coordinator/src/swarm.rs
pub struct CoordinatorConfig {
    pub max_agents: usize,
    pub max_teams: usize,
    pub default_team_size: usize,
    pub auto_scaling: bool,
    pub heartbeat_timeout_seconds: u64,
}

// crates/state/src/settings.rs
pub struct Settings {
    pub auto_approval_enabled: bool,
    pub default_model: String,
    pub theme: String,
    pub telemetry_enabled: bool,
}
```

### Builder Pattern

```rust
// crates/runtime/src/query_engine/config.rs
impl QueryEngineBuilder {
    pub fn new(cwd: impl Into<PathBuf>, registry: ToolRegistry) -> Self;
    pub fn with_max_turns(mut self, max: u32) -> Self;
    pub fn with_max_budget(mut self, budget: f64) -> Self;
    pub fn with_model(mut self, model: impl Into<String>) -> Self;
    pub fn with_llm_client(mut self, client: SharedLlmClient) -> Self;
    pub fn with_fallback_models(mut self, models: Vec<FallbackModelConfig>) -> Self;
    pub fn build(self) -> QueryEngine;
}
```

---

## External Service Integrations

### 1. Anthropic API Client

**Location:** `crates/runtime/src/anthropic/`

```rust
pub struct AnthropicClient {
    api_key: String,
    model: String,
    base_url: String,
    http_client: reqwest::Client,
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn stream(&self, request: LlmRequest) -> QueryResult<LlmStream>;
    async fn complete(&self, request: LlmRequest) -> QueryResult<LlmResponse>;
}
```

**Features:**
- Streaming responses via Server-Sent Events (SSE)
- Request/response tracing
- Rate limit handling with exponential backoff
- Model fallback on `ContextLengthExceeded`

### 2. MCP (Model Context Protocol) Integration

**Location:** `crates/tools/src/mcp.rs`

```rust
// JSON-RPC 2.0 request/response
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<serde_json::Value>,
    pub error: Option<McpError>,
}

// Global store for MCP servers
static GLOBAL_MCP_STORE: OnceLock<McpStore> = OnceLock::new();

pub struct McpStore {
    servers: Arc<Mutex<HashMap<String, McpServerConfig>>>,
}
```

**Integration Points:**
- Runtime stores MCP connections in `ToolUseContext.mcp_clients: Arc<RwLock<Vec<McpConnection>>>`
- MCP tool discovers and invokes external MCP servers
- Resources are cached in `ToolUseContext.mcp_resources`

### 3. LSP (Language Server Protocol) Integration

**Location:** `crates/tools/src/lsp.rs`

```rust
pub struct LspTool {
    // LSP client implementation
}

#[async_trait]
impl Tool for LspTool {
    async fn execute(&self, input: ToolInput) -> ToolOutput {
        // Communicate with LSP servers for:
        // - Symbol lookup
        // - Code completion
        // - Diagnostics
        // - Refactoring
    }
}
```

### 4. OAuth & Secure Storage

**Location:** `crates/services/src/oauth.rs`

```rust
pub struct OAuthManager {
    storage: Arc<dyn SecureStorage>,
}

pub trait SecureStorage: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<String>>;
    async fn set(&self, key: &str, value: &str) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
}

// Implementations:
pub struct KeyringStorage;      // OS keyring
pub struct EncryptedFileStorage; // Encrypted file on disk
pub struct EnvironmentStorage;   // Environment variables
```

### 5. OpenTelemetry Integration

**Location:** `crates/services/src/telemetry.rs`

```rust
pub struct Telemetry {
    tracer: Tracer,
    meter: Meter,
}

pub struct TelemetryConfig {
    pub service_name: String,
    pub otlp_endpoint: String,
    pub enabled: bool,
}

pub fn init_global_telemetry(config: TelemetryConfig) -> Result<Telemetry>;
pub fn global_telemetry() -> Option<Telemetry>;
pub fn shutdown_global_telemetry();
```

### 6. Web Services (Fetch/Search)

**Location:** `crates/tools/src/web_fetch.rs`, `crates/tools/src/web_search.rs`

```rust
pub struct WebFetchTool;
pub struct WebSearchTool;

// Uses reqwest for HTTP
// Supports: HTML parsing (scraper), URL encoding, form submission
```

---

## Summary of Key Integration Points

| Integration | Source Crate | Target Crate | Pattern |
|-------------|--------------|--------------|---------|
| Tool Execution | runtime | tools | Trait + Registry |
| Permission Check | runtime | permissions | PermissionManager.evaluate() |
| State Persistence | runtime | state | StateManager via Arc<RwLock> |
| Multi-agent Coordination | runtime | coordinator | CoordinatorHandle (stub to avoid circular dep) |
| LLM API Calls | runtime | anthropic | LlmClient trait |
| MCP Invocation | tools | MCP servers | JSON-RPC over HTTP |
| LSP Operations | tools | LSP servers | JSON-RPC over stdio/TCP |
| OAuth Tokens | services | services | SecureStorage trait |
| Analytics | services | services | Global singleton |
| Telemetry | services | services | OpenTelemetry OTLP |

---

## File Locations

- **API Documentation:** `/mnt/ollama/git/claw-code/rust/API_INTEGRATION.md`
- **Crate Sources:** `/mnt/ollama/git/claw-code/rust/crates/*/src/`
- **Error Definitions:** `crates/runtime/src/types/errors.rs`, `crates/permissions/src/lib.rs`
- **Trait Definitions:** `crates/tools/src/tool.rs`, `crates/runtime/src/tool/r#trait.rs`
- **Registry Implementation:** `crates/runtime/src/registry/`
- **Configuration:** `crates/runtime/src/query_engine/config.rs`
