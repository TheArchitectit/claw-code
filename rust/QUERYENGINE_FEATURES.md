# QueryEngine Features

## Overview

The QueryEngine is the core conversation orchestration component. It manages the complete lifecycle of LLM interactions including message handling, tool execution loops, and streaming responses.

## Core Capabilities

### 1. Conversation Lifecycle Management

The QueryEngine manages conversation state across multiple turns:

- **Message History**: Persistent conversation context
- **Turn Counting**: Track and limit conversation depth
- **Session Management**: Unique session IDs for conversation isolation
- **State Reset**: Clear conversation and start fresh

### 2. LLM Integration

Abstract interface supporting multiple LLM backends:

- **Streaming**: Real-time SSE response handling
- **Non-streaming**: Complete response requests
- **Model Fallback**: Automatic fallback on rate limits/errors
- **Cost Tracking**: Per-model pricing calculation

### 3. Tool Execution

Complete tool-call loop implementation:

- **Tool Registry**: Dynamic tool discovery
- **Permission System**: Pre-execution permission checks
- **Parallel Execution**: Execute multiple tools concurrently
- **Result Aggregation**: Collect and process tool outputs

### 4. Budget and Limit Enforcement

Built-in safeguards for resource management:

- **Max Turns**: Limit conversation depth
- **Budget Limits**: Cost-based conversation termination
- **Rate Limiting**: Retry with exponential backoff
- **Timeout Handling**: Execution time limits

### 5. Message Processing

Full message pipeline:

- **Normalization**: Convert internal to external formats
- **Content Blocks**: Support text, tool_use, tool_result
- **Citation Tracking**: Source attribution
- **Synthetic Messages**: System-generated context

## Architecture

### Thread Safety

All state protected by async-aware primitives:

```rust
pub struct QueryEngine {
    config: QueryEngineConfig,
    messages: Arc<RwLock<Vec<Message>>>,
    total_usage: Arc<Mutex<Usage>>,
    total_cost: Arc<Mutex<f64>>,
    turn_count: Arc<Mutex<u32>>,
    session_id: SessionId,
}
```

### Configuration

```rust
pub struct QueryEngineConfig {
    pub max_turns: Option<u32>,
    pub max_budget_usd: Option<f64>,
    pub tool_registry: Arc<ToolRegistry>,
    pub llm_client: Arc<dyn LlmClient>,
    pub working_dir: PathBuf,
}
```

## Usage Examples

### Basic Usage

```rust
let registry = ToolRegistryBuilder::new().build();
let engine = QueryEngineBuilder::new("/tmp", registry).build();

let result = engine.run("Hello!", None).await?;
println!("{}", result.response);
```

### With Budget Limits

```rust
let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_max_turns(50)
    .with_max_budget(10.0)
    .build();
```

### Streaming

```rust
let execution = engine.submit_message("Hello", None).await?;
let mut stream = execution.into_stream();

while let Some(chunk) = stream.next().await {
    match chunk {
        LlmStreamChunk::Text(text) => print!("{}", text),
        LlmStreamChunk::ToolUse(tool) => println!("Tool: {}", tool.name),
        _ => {}
    }
}
```

### With Fallback Models

```rust
let fallback_config = FallbackModelConfig::new(
    vec!["model-1", "model-2", "model-3"]
);

let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_fallback_models(fallback_config)
    .build();
```

## Cost Tracking

### Model Pricing (per million tokens)

| Model | Input | Output |
|-------|-------|--------|
| model-1 | $15.00 | $75.00 |
| model-2 | $3.00 | $15.00 |
| model-3 | $0.25 | $1.25 |

### Budget Enforcement

```rust
if engine.check_budget().await {
    // Budget exceeded - handle appropriately
}
```

## Error Handling

| Error | Description |
|-------|-------------|
| MaxTurnsExceeded | Conversation exceeded turn limit |
| MaxBudgetExceeded | Conversation exceeded budget |
| ToolExecutionError | Tool failed during execution |
| LlmError | LLM API error |
| StreamingError | Stream processing error |

## Testing

### Mock LLM Client

```rust
let llm_client = Arc::new(MockLlmClient::simple_text_response("Hello!"));
let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_llm_client(llm_client)
    .build();
```

## Protocol Compatibility

- **MCP**: Tool and resource provider integration
- **LSP**: IDE integration capabilities
- **LLM APIs**: Standard API support

## Legal Notice

This is a clean-room reverse engineering implementation. All code is original work developed from public API specifications.

For educational and research purposes.
