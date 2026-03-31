# Runtime Architecture Documentation

## Overview

The Runtime crate (`crates/runtime`) is the core execution engine of R.A.D Codicological 2.x, providing the LLM orchestration layer, tool execution framework, and conversation management. It implements the QueryEngine which manages the complete conversation lifecycle from message submission through tool execution to response streaming.

## Architecture

### Core Components

```
runtime/
├── src/
│   ├── lib.rs                    # Crate root with re-exports
│   ├── query_engine/             # QueryEngine and conversation management
│   │   ├── mod.rs               # Public API exports
│   │   ├── engine.rs            # Core QueryEngine struct
│   │   ├── execution.rs         # QueryExecution for active queries
│   │   ├── config.rs            # QueryEngineConfig and builder
│   │   ├── streaming.rs         # Streaming response handling
│   │   ├── types.rs             # QueryEngine-specific types
│   │   └── utils.rs             # Utility functions
│   ├── llm_client/              # LLM API abstraction
│   │   ├── mod.rs              # Module exports
│   │   ├── client.rs           # LlmClient trait
│   │   ├── mock.rs             # Mock client for testing
│   │   ├── stream.rs           # Streaming types
│   │   └── types.rs            # Request/Response types
│   ├── messages/                # Message types and normalization
│   │   ├── mod.rs              # Public exports
│   │   ├── types.rs            # Core message types
│   │   ├── assistant.rs        # Assistant message handling
│   │   ├── user.rs             # User message handling
│   │   ├── content.rs          # Content blocks
│   │   └── normalization.rs    # Message normalization
│   ├── tool/                    # Tool trait and implementations
│   │   ├── mod.rs              # Tool trait definition
│   │   ├── types.rs            # Tool-related types
│   │   ├── output.rs           # ToolOutput handling
│   │   └── registry_adapter.rs # ToolRegistry integration
│   ├── registry.rs              # ToolRegistry for tool management
│   ├── permissions.rs           # Permission system integration
│   ├── context.rs               # ToolUseContext for tool execution
│   ├── types.rs                 # Common types (SessionId, ToolUseId, etc.)
│   ├── coordinator.rs           # Multi-agent coordinator integration
│   ├── stream_handler.rs        # Stream handling utilities
│   ├── checkpoint.rs            # Conversation checkpointing
│   ├── error_handling.rs        # Error handling and retry logic
│   └── utils.rs                 # General utilities
└── CLAUDE.md                    # Project guidelines
```

## QueryEngine

The `QueryEngine` is the primary interface for LLM interactions. It manages:

- **Session State**: Conversation history, usage tracking, cost tracking
- **Message Flow**: User message submission, assistant response generation
- **Tool Execution**: Tool-call loop management
- **Streaming**: Real-time response streaming
- **Budget Enforcement**: Cost limits and turn limits

### Thread Safety

All QueryEngine state is protected by async-aware synchronization primitives:

```rust
pub struct QueryEngine {
    pub config: QueryEngineConfig,
    pub messages: Arc<RwLock<Vec<Message>>>,          // Read-heavy, use RwLock
    pub permission_denials: Arc<Mutex<Vec<PermissionDenial>>>,
    pub total_usage: Arc<Mutex<Usage>>,
    pub total_cost: Arc<Mutex<f64>>,
    pub turn_count: Arc<Mutex<u32>>,
    pub session_id: SessionId,
}
```

- `RwLock` for messages (many reads, few writes)
- `Mutex` for counters and simple state
- `Arc` for shared ownership across tasks

### Key Methods

| Method | Purpose |
|--------|---------|
| `submit_message()` | Start a new conversation turn |
| `run()` | High-level method: submit, execute, return result |
| `run_with_tools()` | Run with simulated tool-call loop |
| `reset()` | Clear conversation state |
| `total_usage()` | Get accumulated token usage |
| `total_cost()` | Get accumulated cost in USD |

## QueryExecution

`QueryExecution` represents an active query being processed. It is created by `QueryEngine::submit_message()` and manages:

1. **Conversation Loop**: The tool-call loop with LLM
2. **Tool Execution**: Calling tools and processing results
3. **Streaming**: Real-time message streaming via channels
4. **State Management**: Tracking execution state (Initial → Processing → Completed)

### Execution Flow

```
submit_message()
    ↓
QueryExecution::new()
    ↓
execute()
    ↓
run_conversation_loop()
    ↓
  loop:
    - Check max turns
    - Check budget
    - Get LLM response
    - Update usage/cost
    - Handle tool calls
    - Return on stop_reason
```

## LLM Client Abstraction

The `llm_client` module provides a trait-based abstraction over LLM APIs:

### LlmClient Trait

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, request: LlmRequest) -> Result<LlmResponse, LlmError>;
    async fn complete_stream(&self, request: LlmRequest) -> Result<LlmStream, LlmError>;
}
```

### Implementations

- **MockLlmClient**: For testing, returns predefined responses
- **AnthropicClient**: Production client for Claude API (in `anthropic/` module)

## Message System

### Message Types

The runtime uses a dual-message representation:

1. **Internal Messages** (`Message` enum): Full message state with metadata
2. **Normalized Messages** (`NormalizedMessage`): API-friendly representation

### Message Normalization

The `normalize()` method converts internal messages to normalized form for:
- API responses
- Conversation history
- Persistence

```rust
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
}

pub enum NormalizedMessage {
    User(NormalizedUserMessage),
    Assistant(NormalizedAssistantMessage),
    Result(QueryResultMessage),
}
```

## Tool Integration

### Tool Trait

Tools implement the `Tool` trait:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn input_schema(&self) -> JsonSchema;
    async fn execute(&self, input: Value, context: &ToolUseContext, ...) -> ToolResult<ToolOutput>;
    async fn check_permissions(&self, input: &Value, context: &ToolUseContext) -> PermissionResult;
}
```

### Tool Registry

`ToolRegistry` manages available tools:

```rust
pub struct ToolRegistry {
    tools: DashMap<String, BoxedTool>,
}
```

Features:
- Thread-safe tool storage with `DashMap`
- Runtime tool lookup by name
- Builder pattern for construction

## Streaming Architecture

### QueryExecutionOps

`QueryExecutionOps` provides operations for streaming query execution:

```rust
pub struct QueryExecutionOps {
    config: QueryEngineConfig,
    messages: Arc<RwLock<Vec<Message>>>,
    state: QueryExecutionState,
    // ... channels for streaming
}
```

### LlmStream

Streaming LLM responses via `LlmStream`:

```rust
pub struct LlmStream {
    chunks: Receiver<LlmStreamChunk>,
}
```

## Configuration

### QueryEngineConfig

```rust
pub struct QueryEngineConfig {
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub tool_registry: Arc<ToolRegistry>,
    pub llm_client: Arc<dyn LlmClient>,
    pub initial_messages: Vec<Message>,
    pub working_dir: PathBuf,
}
```

### Builder Pattern

```rust
let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_max_turns(50)
    .with_max_budget(10.0)
    .with_llm_client(client)
    .build();
```

## Cost Tracking

### Model Pricing

Built-in pricing for Claude models:

```rust
pub const CLAUDE_3_OPUS_PRICE: f64 = 15.0;        // per million input tokens
pub const CLAUDE_3_OPUS_OUTPUT_PRICE: f64 = 75.0;   // per million output tokens
pub const CLAUDE_3_5_SONNET_PRICE: f64 = 3.0;
pub const CLAUDE_3_5_SONNET_OUTPUT_PRICE: f64 = 15.0;
pub const CLAUDE_3_HAIKU_PRICE: f64 = 0.25;
pub const CLAUDE_3_HAIKU_OUTPUT_PRICE: f64 = 1.25;
```

### Budget Enforcement

```rust
pub async fn check_budget(&self) -> bool {
    let current_cost = *self.total_cost.lock().await;
    self.config.would_exceed_budget(current_cost)
}
```

## Error Handling

### Error Types

```rust
pub enum QueryEngineError {
    MaxTurnsExceeded { max_turns: u32 },
    MaxBudgetExceeded { max_budget: f64 },
    ToolExecutionError { tool_name: String, error: String },
    LlmError { source: LlmError },
    StreamingError { message: String },
}
```

### Retry Logic

The `error_handling` module provides:
- Exponential backoff for rate limits
- Panic catching for tool execution
- Conversation checkpointing for recovery

## Coordinator Integration

The runtime integrates with the Coordinator for multi-agent scenarios:

```rust
pub struct CoordinatorIntegration {
    agent_manager: Arc<AgentManager>,
    state_manager: Arc<StateManager>,
    event_broker: Arc<EventBroker>,
    task_distributor: Arc<TaskDistributor>,
}
```

Features:
- Agent lifecycle management
- Inter-agent message routing
- State synchronization
- Task distribution

## Bootstrap System

The runtime includes a bootstrap system for initialization phases:

```rust
pub enum BootstrapPhase {
    CliEntry,
    FastPathVersion,
    StartupProfiler,
    SystemPromptFastPath,
    ChromeMcpFastPath,
    // ... more phases
    MainRuntime,
}
```

## Testing

### Mock Implementations

- `MockLlmClient`: Returns predefined responses
- `StubTool`: Minimal tool implementation for testing

### Test Patterns

```rust
#[tokio::test]
async fn test_query_engine_creation() {
    let registry = create_registry();
    let engine = QueryEngineBuilder::new("/tmp", registry).build();
    assert!(!engine.session_id().to_string().is_empty());
}
```

## Thread Safety Summary

| Type | Primitive | Rationale |
|------|-----------|-----------|
| Messages | `Arc<RwLock<Vec<Message>>>` | Read-heavy, need concurrent reads |
| Counters | `Arc<Mutex<T>>` | Simple state, infrequent access |
| Tool Registry | `Arc<ToolRegistry>` + `DashMap` | Concurrent read/write, high throughput |
| LLM Client | `Arc<dyn LlmClient>` | Shared across executions |

## Performance Considerations

1. **RwLock vs Mutex**: Use `RwLock` for messages because reads are frequent, writes are rare
2. **DashMap**: Tool registry uses `DashMap` for concurrent access without locking
3. **Arc**: All shared state uses `Arc` to avoid cloning large structures
4. **Streaming**: Channel-based streaming to avoid buffering entire responses

## Integration Points

### With Tools Crate

- Tool trait definition lives in runtime (shared)
- Tool implementations live in tools crate
- ToolRegistry connects both

### With Commands Crate

- Commands use QueryEngine for LLM interactions
- Commands modify state through QueryEngine methods

### With State Crate

- Session persistence
- Checkpoint management
- State recovery

## Future Work

### Pending Refactors (per CLAUDE.md <500 line rule)

- [ ] Split `messages.rs` (2151 lines) into `messages/` directory
- [ ] Split `permissions.rs` (1028 lines) into `permissions/` directory
- [ ] Split `context.rs` (1180 lines) into `context/` directory
- [ ] Split `types.rs` (943 lines) into `types/` directory
- [ ] Split `registry.rs` (1000 lines) into `registry/` directory

### Planned Features

- [ ] Full Anthropic API integration with streaming
- [ ] Advanced model fallback handling
- [ ] Conversation checkpoint persistence
- [ ] Enhanced error recovery

## See Also

- [Tools Documentation](TOOLS_DOCUMENTATION.md)
- [Commands Documentation](COMMANDS_DOCUMENTATION.md)
- [QueryEngine Features](QUERYENGINE_FEATURES.md)
- [API Integration](API_INTEGRATION.md)
- [Testing Patterns](TESTING_PATTERNS.md)
