# QueryEngine Architecture

## Overview

The `QueryEngine` is the core orchestration component of the R.A.D Codicological Runtime. It manages the complete lifecycle of LLM conversations, including message handling, tool execution, streaming responses, and cost tracking.

## Key Design Decisions

### One QueryEngine Per Conversation
Each `QueryEngine` instance represents a single conversation session. Multiple calls to `submit_message()` create multiple turns within the same conversation, sharing state (message history, usage, cost).

### Thread-Safe State Management
All mutable state uses `Arc` with appropriate synchronization:
- `Arc<RwLock<Vec<Message>>>` for message history (read-heavy)
- `Arc<Mutex<T>>` for counters and accumulators (write-heavy)

### Per-Turn Execution Context
`QueryExecution` is created for each turn and owns the execution logic while sharing state with the parent `QueryEngine`.

### Streaming-First Design
The LLM client interface is streaming-based. The QueryEngine accumulates chunks into complete messages while supporting cancellation.

## Component Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Application Layer                        │
│         (CLI, API Server, or UI Integration)               │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      │ QueryEngine API
                      v
┌─────────────────────────────────────────────────────────────┐
│                        QueryEngine                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  Session State                        │   │
│  │  • session_id: Unique conversation identifier        │   │
│  │  • messages: Arc<RwLock<Vec<Message>>>               │   │
│  │  • total_usage: Arc<Mutex<Usage>>                  │   │
│  │  • total_cost: Arc<Mutex<f64>>                     │   │
│  │  • turn_count: Arc<Mutex<u32>>                     │   │
│  └──────────────────────────────────────────────────────┘   │
│                      │ submit_message()                      │
│                      v                                        │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                   QueryExecution                      │   │
│  │  • Per-turn execution context                        │   │
│  │  • State: Initial → Processing → Completed           │   │
│  │  • Manages conversation loop                         │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      v
┌─────────────────────────────────────────────────────────────┐
│                     LLM Client Layer                       │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│  │   Anthropic  │  │   OpenAI     │  │    Mock      │     │
│  │    Client    │  │   Client     │  │   Client     │     │
│  └──────────────┘  └──────────────┘  └──────────────┘     │
│                                                              │
│  Features:                                                   │
│  • Streaming response processing                            │
│  • Text/Thinking/ToolUse chunk handling                     │
│  • Cancellation support (checked every 10 chunks)         │
│  • Automatic model fallback                               │
│                                                              │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      v
┌─────────────────────────────────────────────────────────────┐
│                      Tool Registry                         │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                  Tool Resolution                       │  │
│  │  1. Lookup by primary name                             │  │
│  │  2. Lookup by alias                                    │  │
│  │  3. Execute with permission check                      │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 Permission System                      │  │
│  │  • can_use_tool callback                               │  │
│  │  • PermissionResult: Allow / Deny / Passthrough      │  │
│  │  • Denial tracking                                     │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                 Tool Execution                         │  │
│  │  • create_tool_use_context()                           │  │
│  │  • tool.execute(input, context, id, progress)        │  │
│  │  • Process tool results into ContentBlocks           │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## Data Flow

### 1. Conversation Initiation Flow

```
┌─────────┐     submit_message()      ┌─────────────────┐
│  User   │ ─────────────────────────> │  QueryEngine    │
│ Input   │                            │                 │
└─────────┘                            └────────┬────────┘
                                              │
                                              │ Create UserMessage
                                              │ Add to messages
                                              │ Increment turn_count
                                              │ Check max_turns
                                              │
                                              v
                                       ┌─────────────────┐
                                       │  QueryExecution │
                                       │  (per-turn)       │
                                       └────────┬────────┘
                                                │
                                                │ execute()
                                                v
                                       ┌─────────────────┐
                                       │ Conversation    │
                                       │ Loop            │
                                       └─────────────────┘
```

### 2. Conversation Loop Flow

```
┌─────────────────┐
│   Start Loop    │
└────────┬────────┘
         │
         v
┌─────────────────┐     EndTurn/StopSequence      ┌─────────────┐
│  Get LLM        │ ─────────────────────────────> │   Return    │
│  Response       │                                │   Result    │
│  (streaming)    │                                └─────────────┘
└────────┬────────┘
         │
         │ ToolUse
         v
┌─────────────────┐     Not Found                  ┌─────────────┐
│  Extract Tool   │ ─────────────────────────────> │   Error     │
│  Calls          │                                └─────────────┘
└────────┬────────┘
         │
         v
┌─────────────────┐     Denied                     ┌─────────────┐
│  Check          │ ─────────────────────────────> │   Record    │
│  Permissions    │                                │   Denial    │
└────────┬────────┘
         │
         │ Allowed
         v
┌─────────────────┐
│  Execute Tool   │
│  (async)        │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Add Tool       │
│  Result to      │
│  Messages       │
└────────┬────────┘
         │
         └─────────────────┐
                           │
                           v
                    ┌─────────────────┐
                    │   Check Budget  │ ──Exceeded──> Error
                    │   Check Turns   │ ──Exceeded──> Error
                    └────────┬────────┘
                             │
                             └──────────────┐
                                            │
                                            v
                                     (Continue Loop)
```

### 3. Streaming Response Processing

```
┌─────────────────┐
│  LLM API        │
│  Stream         │
└────────┬────────┘
         │
         │ LlmStreamChunk
         v
┌──────────────────────────────────────────────────┐
│              Chunk Processing                      │
│                                                    │
│  • Text { text } ─────────> Accumulate text      │
│  • Thinking { thinking } ──> Accumulate thinking │
│  • ToolUseStart { id, name } ──> Finalize pending │
│  • ToolUseDelta { partial_json } ──> Accumulate │
│  • ToolUseComplete { id, input } ──> Add block   │
│  • Usage { usage } ─────────> Update stats       │
│  • Stop { reason } ─────────> Set stop_reason    │
│  • Error { message } ──────> Return error       │
│                                                    │
│  Every 10 chunks: Check cancellation             │
└──────────────────────────────────────────────────┘
         │
         v
┌─────────────────┐
│  Finalize       │
│  Content Blocks │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Create         │
│  AssistantMessage│
└─────────────────┘
```

## Usage Examples

### Simple Conversation

```rust
use runtime::{QueryEngineBuilder, ToolRegistry};

async fn simple_conversation() -> Result<(), Box<dyn std::error::Error>> {
    let engine = QueryEngineBuilder::new("/tmp", ToolRegistry::new())
        .with_user_model("claude-3-5-sonnet")
        .build();

    let result = engine.run("Hello, how are you?", None).await?;
    println!("Response: {}", result.response);
    Ok(())
}
```

### Conversation with Tools

```rust
use runtime::{QueryEngineBuilder, ToolRegistryBuilder, PermissionResult};
use std::sync::Arc;

async fn conversation_with_tools() -> Result<(), Box<dyn std::error::Error>> {
    let registry = ToolRegistryBuilder::new()
        .register(ReadFileTool)
        .register(WriteFileTool)
        .build();

    let engine = QueryEngineBuilder::new("/workspace", registry)
        .with_permission_checker(Arc::new(|tool, input, context, id| {
            Box::pin(async move {
                // Custom permission logic
                if tool.name() == "WriteFile" {
                    // Check if path is allowed
                    PermissionResult::allow()
                } else {
                    PermissionResult::allow()
                }
            })
        }))
        .with_max_budget(5.0)
        .build();

    let result = engine.run_with_tools("Read config.json and update the port to 8080", None).await?;
    println!("Result: {}", result.response);
    println!("Cost: ${:.4}", result.cost_usd);
    Ok(())
}
```

### Multi-Turn Conversation

```rust
use runtime::{QueryEngineBuilder, ToolRegistry};

async fn multi_turn() -> Result<(), Box<dyn std::error::Error>> {
    let engine = QueryEngineBuilder::new("/tmp", ToolRegistry::new()).build();

    // Turn 1
    let result1 = engine.run("What is 2 + 2?", None).await?;
    println!("Turn 1: {}", result1.response);

    // Turn 2 - engine maintains history
    let result2 = engine.run("Multiply that by 10", None).await?;
    println!("Turn 2: {}", result2.response);

    // Check accumulated state
    let cost = engine.total_cost().await;
    let messages = engine.get_messages().await;
    println!("Total cost: ${:.4}, Total messages: {}", cost, messages.len());

    Ok(())
}
```

### Error Handling

```rust
use runtime::{QueryEngineBuilder, ToolRegistry, QueryEngineError};

async fn error_handling() {
    let engine = QueryEngineBuilder::new("/tmp", ToolRegistry::new())
        .with_max_turns(3)
        .build();

    match engine.run("Hello", None).await {
        Ok(result) => println!("Success: {}", result.response),
        Err(QueryEngineError::MaxTurnsExceeded { max_turns }) => {
            eprintln!("Conversation exceeded {} turns", max_turns);
        }
        Err(QueryEngineError::MaxBudgetExceeded { max_budget }) => {
            eprintln!("Session exceeded ${} budget", max_budget);
        }
        Err(QueryEngineError::Tool(tool_error)) => {
            eprintln!("Tool error: {}", tool_error);
        }
        Err(e) => eprintln!("Other error: {}", e),
    }
}
```

### Streaming with Cancellation

```rust
use runtime::{QueryEngineBuilder, ToolRegistry};
use tokio::time::{timeout, Duration};

async fn streaming_with_timeout() -> Result<(), Box<dyn std::error::Error>> {
    let engine = QueryEngineBuilder::new("/tmp", ToolRegistry::new()).build();
    let execution = engine.submit_message("Write a long story...", None).await?;

    let mut stream = execution.into_stream();
    let mut messages = Vec::new();

    // Process with 30-second timeout
    if let Err(_) = timeout(Duration::from_secs(30), async {
        while let Some(msg) = stream.next().await {
            messages.push(msg);
        }
    }).await {
        println!("Timed out after 30 seconds");
        println!("Received {} messages so far", messages.len());
    }

    Ok(())
}
```

## Core Components

### QueryEngine

The main orchestrator that manages conversation state across multiple turns.

**Key Responsibilities:**
- Session management (unique `SessionId` per conversation)
- Message history persistence
- Usage and cost accumulation
- Turn counting and limits
- Budget enforcement

**Thread Safety:**
All state is protected by synchronization primitives:
- `messages: Arc<RwLock<Vec<Message>>>` - Shared read, exclusive write
- `total_usage: Arc<Mutex<RuntimeUsage>>` - Exclusive access for updates
- `total_cost: Arc<Mutex<f64>>` - Exclusive access for updates
- `turn_count: Arc<Mutex<u32>>` - Exclusive access for updates

### QueryExecution

Per-turn execution context created by `submit_message()`.

**Lifecycle States:**
1. `Initial` - Ready to start execution
2. `Processing` - Currently in conversation loop
3. `Completed(QueryResultMessage)` - Successfully finished
4. `Failed(QueryEngineError)` - Error occurred

**Key Methods:**
- `execute()` - Run the conversation loop
- `into_stream()` - Convert to async message stream
- `is_complete()` - Check if execution finished
- `get_result()` - Get final result if complete

### QueryEngineConfig

Configuration for QueryEngine behavior.

**Key Options:**

| Field | Type | Description |
|-------|------|-------------|
| `cwd` | `String` | Working directory for tool execution |
| `tool_registry` | `ToolRegistry` | Available tools |
| `max_turns` | `Option<u32>` | Conversation turn limit |
| `max_budget_usd` | `Option<f64>` | Budget limit in USD |
| `user_specified_model` | `Option<String>` | Primary LLM model |
| `fallback_enabled` | `bool` | Enable model fallback |
| `fallback_chain` | `Vec<String>` | Ordered fallback models |
| `can_use_tool` | `Arc<dyn Fn(...) -> Future<PermissionResult>>` | Permission callback |
| `llm_client` | `Option<Arc<dyn LlmClient>>` | LLM API client |

### ConversationResult

The result of a completed conversation turn.

```rust
pub struct ConversationResult {
    pub response: String,              // Final assistant text
    pub messages: Vec<NormalizedMessage>, // Full conversation history
    pub duration_ms: u64,            // Total duration
    pub usage: RuntimeUsage,         // Token usage statistics
    pub cost_usd: f64,               // Estimated cost
    pub session_id: SessionId,       // Session identifier
}
```

## Error Handling Strategy

The QueryEngine uses a hierarchical error system:

### Error Types

1. **QueryEngineError** - Top-level errors
   - `Tool(ToolError)` - Tool execution failures
   - `Aborted` - User cancellation
   - `MaxTurnsExceeded { max_turns }` - Turn limit
   - `MaxBudgetExceeded { max_budget }` - Budget limit
   - `Api { message }` - LLM API errors
   - `Validation { message }` - Input validation

2. **ToolError** - Tool-specific errors
   - `Cancelled`
   - `InvalidInput { message }`
   - `ExecutionFailed { message }`
   - `NotFound { name }`
   - `PermissionDenied { message }`
   - `Timeout { duration_ms }`
   - `Panic { message }`
   - `Internal { message }`

### Error Recovery

- **MaxTurnsExceeded**: Conversation terminates gracefully
- **MaxBudgetExceeded**: Conversation terminates gracefully
- **ToolError**: Recorded in conversation, LLM can retry
- **ApiError**: May trigger model fallback
- **Cancelled**: Return partial results if available

## Tool Execution Flow

```
┌─────────────────┐
│  LLM requests   │
│  tool use       │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  extract_tool   │
│  _calls()       │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  registry.get() │ ──Not Found──> ToolError::NotFound
└────────┬────────┘
         │
         v
┌─────────────────┐
│  create_tool_   │
│  use_context()  │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  can_use_tool() │ ──Denied──> Record Denial + Error
└────────┬────────┘
         │ Allowed
         v
┌─────────────────┐
│  tool.execute() │ ──Error──> Record Error + Continue
└────────┬────────┘
         │
         v
┌─────────────────┐
│  process_tool   │
│  _results()     │
└────────┬────────┘
         │
         v
┌─────────────────┐
│  Add to         │
│  conversation   │
└─────────────────┘
```

## Model Fallback Strategy

When the primary model fails:

1. Check if `fallback_enabled` is true
2. Wait `FALLBACK_DELAY_MS` (500ms)
3. Try next model in `fallback_chain`
4. Repeat up to `max_fallback_attempts` (default 2)
5. If all fail, return last error

Fallback chain defaults:
- Primary: `user_specified_model` or `DEFAULT_MODEL` ("claude-3-5-sonnet")
- First fallback: `fallback_model` or `DEFAULT_FALLBACK_MODEL` ("claude-3-haiku")

## Cost Calculation

Pricing is based on model type (per million tokens):

| Model | Input Price | Output Price |
|-------|-------------|--------------|
| Claude 3.5 Sonnet | $3.00 | $15.00 |
| Claude 3 Haiku | $0.25 | $1.25 |
| Default/Other | $3.00 | $15.00 |

Formula:
```
cost = (input_tokens / 1M * input_price) +
       (output_tokens / 1M * output_price) +
       (cache_read_tokens / 1M * input_price) +
       (cache_creation_tokens / 1M * input_price)
```

## Integration Points

### With ToolRegistry

```rust
// Tool lookup
tool_registry.get("ToolName") -> Option<SharedTool>

// Tool execution
tool.execute(input, context, tool_use_id, on_progress) -> ToolResult<ToolOutput>
```

### With LlmClient

```rust
// Request building
let request = LlmRequest::new(model)
    .with_system(prompt)
    .with_messages(messages)
    .with_tools(tools);

// Streaming
let stream = llm_client.stream(request).await?;
while let Some(chunk) = stream.next().await {
    // Process chunk
}
```

### With Permissions

```rust
// Permission check
let result = (config.can_use_tool)(tool, input, context, tool_use_id).await;

match result {
    PermissionResult::Decision(decision) if decision.is_allowed() => {
        // Execute tool
    }
    PermissionResult::Decision(_) => {
        // Record denial and return error
    }
    PermissionResult::Passthrough { .. } => {
        // Continue with execution
    }
}
```

## Testing Patterns

### Mock LLM Client

```rust
let llm_client = Arc::new(MockLlmClient::simple_text_response("Hello!"));
let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_llm_client(llm_client)
    .build();
```

### Stub Tool

```rust
struct StubTool;

#[async_trait]
impl Tool for StubTool {
    fn name(&self) -> &str { "stub" }

    async fn execute(&self, _input: Value, _ctx: &ToolUseContext, _id: ToolUseId, _progress: Option<...>) -> ToolResult<ToolOutput> {
        Ok(ToolOutput::new("result"))
    }
    // ... other methods
}
```

### Stream Testing

```rust
let execution = engine.submit_message("Hello", None).await?;
let mut stream = execution.into_stream();
let mut messages = Vec::new();

while let Some(msg) = stream.next().await {
    messages.push(msg);
}

assert!(!messages.is_empty());
```

## Performance Considerations

1. **Message Storage**: Use `Arc<RwLock<...>>` for shared access
   - Read-heavy pattern: many reads, few writes
   - Clone-on-write for message snapshots

2. **Streaming**: Process chunks as they arrive
   - No buffering of complete response
   - Cancellation checked every 10 chunks

3. **Tool Results**: Automatic truncation at 100KB
   - Prevents context window overflow
   - Configurable per-tool limits supported

4. **Cost Tracking**: Atomic updates to shared state
   - Minimal lock contention
   - Usage accumulated per-turn

## Future Enhancements

- **Parallel Tool Execution**: Execute independent tools concurrently
- **Tool Result Caching**: Cache deterministic tool results
- **Adaptive Fallback**: Learn optimal fallback chains
- **Conversation Compression**: Summarize old context
- **Multi-Model Ensemble**: Query multiple models for consensus
