# QueryEngine Phase 6 Implementation

## Overview

The QueryEngine is the core orchestration component for LLM interactions in the R.A.D Codicological 2.x runtime. It manages the complete lifecycle of a conversation, from message submission through response streaming, tool execution, and cost tracking.

### Architecture

The QueryEngine is structured as a modular system with three primary components:

```
┌─────────────────────────────────────────────────────────────────┐
│                     QueryEngine                                │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Engine     │  │  Execution  │  │    Config    │          │
│  │  (engine.rs) │  │(execution.rs)│  │  (config.rs) │          │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘          │
│         │                 │                  │                  │
│         └─────────────────┴──────────────────┘                  │
│                           │                                    │
│                   ┌───────▼────────┐                           │
│                   │ Anthropic Client│                           │
│                   │  (client.rs)   │                           │
│                   └────────────────┘                           │
└─────────────────────────────────────────────────────────────────┘
```

**Key Design Principles:**
- One `QueryEngine` per conversation session
- State persists across conversation turns
- Each `submit_message()` creates a new `QueryExecution` context
- Parallel tool execution for concurrency-safe tools
- Streaming responses with fallback support

---

## Core Features Implemented

### 1. Anthropic API Client Integration

The Anthropic client provides full integration with Anthropic's Messages API:

- **Streaming responses** via Server-Sent Events (SSE)
- **Non-streaming completions** for synchronous use cases
- **Tool use support** with streaming tool input accumulation
- **Automatic message format conversion** from runtime types to Anthropic API format
- **Rate limiting handling** with retry-after support

**Stream Processing:**
- `content_block_start` - Detects tool use blocks
- `content_block_delta` - Accumulates text and partial JSON
- `content_block_stop` - Finalizes tool use blocks
- `message_stop` - Captures usage statistics

### 2. Cost Tracking and Budget Enforcement

Per-model token pricing with automatic cost calculation:

| Model | Input (per 1M tokens) | Output (per 1M tokens) |
|-------|----------------------|----------------------|
| Claude 3.5 Sonnet | $3.00 | $15.00 |
| Claude 3 Haiku | $0.25 | $1.25 |
| Claude 3 Opus | $15.00 | $75.00 |

**Features:**
- Automatic cost calculation after each API call
- Session-level cost accumulation
- Configurable maximum budget enforcement
- Budget checking before each turn

### 3. Tool Execution Orchestration

The QueryEngine manages the complete tool execution lifecycle:

**Permission Checking:**
- Configurable `can_use_tool` callback for permission decisions
- Records permission denials for audit trail
- Supports both decision-based and passthrough permission models

**Parallel Tool Execution:**
Tools are categorized by concurrency safety:
- **Concurrency-safe tools**: Executed in parallel using `futures::join_all`
- **Non-concurrency-safe tools**: Executed sequentially to prevent conflicts

**Tool-Call Loop:**
1. User submits message
2. LLM decides to use tools (stop_reason = `ToolUse`)
3. Tools execute (parallel where safe)
4. Results returned to LLM
5. LLM generates final response

### 4. Model Fallback Handling

Intelligent fallback chain for resilience:

**Fallback Chain:**
1. Primary model (default: `claude-3-5-sonnet`)
2. Fallback to `claude-3-haiku` (fast, cost-effective)
3. Fallback to `claude-3-opus` (high capability, large context)

**Error-Based Model Selection:**
- **Rate limits** → Prefer Haiku (fast/cheap)
- **Context length exceeded** → Prefer Opus (large context)
- **Model unavailable** → Try any available model
- **Timeout** → Prefer faster models

**Statistics Tracking:**
- Fallback count and success rate
- Cost incurred from fallback attempts
- Models tried during fallback

---

## Key Components

### QueryEngine

The main orchestrator for conversation sessions.

**Location:** `crates/runtime/src/query_engine/engine.rs`

```rust
pub struct QueryEngine {
    pub(super) config: QueryEngineConfig,
    pub(super) messages: Arc<RwLock<Vec<Message>>>,
    pub(super) permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    pub(super) total_usage: Arc<Mutex<Usage>>,
    pub(super) total_cost: Arc<Mutex<f64>>,
    pub(super) turn_count: Arc<Mutex<u32>>,
    pub(super) session_id: SessionId,
}
```

**Key Methods:**

| Method | Description |
|--------|-------------|
| `new(config)` | Create a new QueryEngine with configuration |
| `submit_message(prompt, options)` | Start a new conversation turn, returns a `QueryExecution` |
| `run(message, options)` | High-level method that runs a complete turn and returns the final response |
| `run_with_tools(message, options)` | Run with simulated tool-call loop for testing |
| `reset()` | Clear conversation state and start fresh |
| `total_usage()` | Get accumulated token usage for the session |
| `total_cost()` | Get accumulated cost for the session |
| `check_budget()` | Check if current cost exceeds max budget |

### QueryExecution

Per-query execution context that manages a single conversation turn.

**Location:** `crates/runtime/src/query_engine/execution.rs`

```rust
pub struct QueryExecution {
    config: QueryEngineConfig,
    messages: Arc<RwLock<Vec<Message>>>,
    permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    total_usage: Arc<Mutex<Usage>>,
    total_cost: Arc<Mutex<f64>>,
    start_time: Instant,
    session_id: SessionId,
    user_message: UserMessage,
    state: QueryExecutionState,
}
```

**Key Methods:**

| Method | Description |
|--------|-------------|
| `execute()` | Run the conversation loop and return all messages |
| `execute_tool(name, input, tool_use_id)` | Execute a single tool with permission checking |
| `into_stream()` | Convert execution into a `MessageStream` for async consumption |
| `is_complete()` | Check if execution has finished |
| `get_result()` | Get final result if complete |

**Conversation Loop:**
The `run_conversation_loop()` method implements the core interaction pattern:
1. Check max turns and budget constraints
2. Get LLM response (with fallback support)
3. Update usage and cost tracking
4. Handle stop reason (EndTurn, ToolUse, Error)
5. If ToolUse, execute tools and continue loop

### QueryEngineConfig

Configuration and builder for QueryEngine instances.

**Location:** `crates/runtime/src/query_engine/config.rs`

```rust
pub struct QueryEngineConfig {
    pub cwd: String,
    pub tool_registry: ToolRegistry,
    pub mcp_clients: Vec<McpConnection>,
    pub agent_definitions: AgentDefinitions,
    pub can_use_tool: Arc<...>,  // Permission callback
    pub get_app_state: Arc<dyn Fn() -> serde_json::Value>,
    pub set_app_state: Arc<dyn Fn(serde_json::Value)>,
    pub initial_messages: Vec<Message>,
    pub custom_system_prompt: Option<String>,
    pub user_specified_model: Option<String>,
    pub fallback_model: Option<String>,
    pub fallback_enabled: bool,
    pub fallback_chain: Vec<String>,
    pub max_fallback_attempts: u32,
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub abort_controller: Arc<tokio::sync::Notify>,
    pub llm_client: Option<Arc<dyn LlmClient + Send + Sync>>,
}
```

**Configuration Methods:**

| Method | Description |
|--------|-------------|
| `new(cwd, tool_registry)` | Create configuration with defaults |
| `with_max_turns(n)` | Set maximum conversation turns |
| `with_max_budget(usd)` | Set maximum budget in USD |
| `with_fallback_model(model)` | Set fallback model |
| `with_fallback_enabled(bool)` | Enable/disable fallback |
| `with_fallback_chain(chain)` | Set custom fallback chain |
| `with_custom_system_prompt(prompt)` | Set system prompt |
| `calculate_cost(model, input, output)` | Calculate cost for token usage |
| `would_exceed_budget(current)` | Check if budget would be exceeded |
| `select_fallback_model(error, attempted)` | Select next model based on error |

---

## Usage Examples

### Basic Usage

```rust
use runtime::query_engine::{QueryEngine, QueryEngineConfig};
use runtime::registry::ToolRegistry;

// Create configuration
let tool_registry = ToolRegistry::default();
let config = QueryEngineConfig::new("/workspace", tool_registry)
    .with_max_turns(50)
    .with_max_budget(5.0);  // $5.00 budget

// Create QueryEngine
let engine = QueryEngine::new(config);

// Run a conversation turn
let result = engine.run("Hello, Claude!", None).await?;
println!("Response: {}", result.response);
println!("Cost: ${:.4}", result.cost_usd);
```

### Using QueryExecution for Streaming

```rust
// Submit message to start a turn
let execution = engine.submit_message("List my tasks", None).await?;

// Convert to stream for async message consumption
let mut stream = execution.into_stream();

while let Some(message) = stream.next().await {
    match message {
        NormalizedMessage::Assistant(msg) => {
            println!("Assistant: {:?}", msg.content);
        }
        NormalizedMessage::Result(result) => {
            println!("Turn complete: {:?}", result);
        }
        _ => {}
    }
}
```

### With Custom Fallback Chain

```rust
let config = QueryEngineConfig::new("/workspace", tool_registry)
    .with_user_model("claude-3-opus")
    .with_fallback_chain(vec![
        "claude-3-opus".to_string(),
        "claude-3-5-sonnet".to_string(),
        "claude-3-haiku".to_string(),
    ])
    .with_max_fallback_attempts(3);

let engine = QueryEngine::new(config);
```

### With Anthropic Client

```rust
use runtime::anthropic::{AnthropicClient, AnthropicConfig};
use runtime::llm_client::LlmClient;

// Create Anthropic client
let anthropic_config = AnthropicConfig::new(api_key)
    .with_default_model("claude-3-5-sonnet".to_string());
let client = AnthropicClient::new(anthropic_config)?;

// Configure QueryEngine with LLM client
let config = QueryEngineConfig::new("/workspace", tool_registry)
    .with_llm_client(Arc::new(client));

let engine = QueryEngine::new(config);
```

---

## Configuration Options

### Budget and Limits

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `max_turns` | `Option<u32>` | `Some(100)` | Maximum conversation turns per session |
| `max_budget_usd` | `Option<f64>` | `None` | Maximum budget in USD for the session |
| `max_fallback_attempts` | `u32` | `2` | Maximum number of fallback attempts |

### Model Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `user_specified_model` | `Option<String>` | `None` | Primary model to use |
| `fallback_model` | `Option<String>` | `None` | Single fallback model |
| `fallback_enabled` | `bool` | `true` | Enable automatic fallback |
| `fallback_chain` | `Vec<String>` | `[]` | Ordered list of fallback models |

### Tool and Permission Configuration

| Option | Type | Description |
|--------|------|-------------|
| `tool_registry` | `ToolRegistry` | Registry of available tools |
| `can_use_tool` | `Arc<dyn Fn(...) -> PermissionResult>` | Permission checking callback |
| `mcp_clients` | `Vec<McpConnection>` | MCP client connections |

### Session Configuration

| Option | Type | Description |
|--------|------|-------------|
| `cwd` | `String` | Current working directory |
| `initial_messages` | `Vec<Message>` | Initial conversation messages |
| `custom_system_prompt` | `Option<String>` | Custom system prompt |
| `append_system_prompt` | `Option<String>` | Additional system prompt to append |
| `abort_controller` | `Arc<tokio::sync::Notify>` | Cancellation signal |
| `llm_client` | `Option<Arc<dyn LlmClient>>` | LLM client for API calls |

---

## Error Handling

The QueryEngine uses `QueryEngineError` for error reporting:

```rust
pub enum QueryEngineError {
    MaxTurnsExceeded { max_turns: u32 },
    MaxBudgetExceeded { max_budget: f64 },
    MaxRetriesExceeded { max_retries: u32, last_error: String },
    LlmApi(LlmApiError),
    Tool(ToolError),
    Api { message: String },
}
```

**LlmApiError variants for fallback triggering:**
- `RateLimit { retry_after_seconds: u64 }`
- `ContextLengthExceeded { message: String }`
- `ModelUnavailable { model: String }`
- `Timeout { seconds: u64 }`

---

## Streaming Architecture

The streaming implementation in `streaming.rs` provides:

- **SSE processing** from Anthropic API
- **Content block accumulation** (text, thinking, tool_use)
- **Cancellation support** via `abort_controller`
- **Fallback integration** with exponential backoff

**Stream Chunk Types:**
- `Text { text: String }` - Text content delta
- `Thinking { thinking: String }` - Thinking content
- `ToolUseStart { id, name }` - Tool use beginning
- `ToolUseDelta { id, partial_json }` - Tool input partial
- `ToolUseComplete { id, input }` - Tool use finalization
- `Usage { usage: Usage }` - Token usage stats
- `Stop { reason: StopReason }` - Stream completion
- `Error { message: String }` - Error during streaming

---

## Files Reference

| File | Purpose | Lines |
|------|---------|-------|
| `crates/runtime/src/query_engine/engine.rs` | Main QueryEngine struct and methods | ~454 |
| `crates/runtime/src/query_engine/execution.rs` | QueryExecution and conversation loop | ~525 |
| `crates/runtime/src/query_engine/config.rs` | Configuration and builder | ~535 |
| `crates/runtime/src/query_engine/streaming.rs` | Streaming and fallback logic | ~403 |
| `crates/runtime/src/query_engine/types.rs` | Type definitions | ~150 |
| `crates/runtime/src/query_engine/mod.rs` | Module exports | ~50 |
| `crates/runtime/src/anthropic/client.rs` | Anthropic API client | ~540 |
