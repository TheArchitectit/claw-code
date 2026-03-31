# API Changes and New Types Documentation

This document describes the public API changes, new types, and migration guide for the rustv2 runtime crate.

## Table of Contents

1. [New Public APIs](#new-public-apis)
2. [Type Changes](#type-changes)
3. [Re-exports](#re-exports)
4. [Migration from Old Imports](#migration-from-old-imports)
5. [Thread Safety Improvements](#thread-safety-improvements)

---

## New Public APIs

### AnthropicClient

A new LLM client implementation that connects to the Anthropic API (Claude).

**Location**: `crates/runtime/src/anthropic/mod.rs`

```rust
// Re-exports from anthropic module
pub use config::{AnthropicClient, AnthropicConfig};
pub use error::AnthropicError;
```

**Key Types**:

| Type | Description |
|------|-------------|
| `AnthropicClient` | Concrete implementation of `LlmClient` trait for Anthropic API |
| `AnthropicConfig` | Configuration for the Anthropic client (API key, model, timeout) |
| `AnthropicError` | Error types specific to Anthropic API operations |

**Usage Example**:

```rust
use rad_runtime::anthropic::{AnthropicClient, AnthropicConfig};
use rad_runtime::llm_client::{LlmClient, LlmRequest};

// Create client from environment (ANTHROPIC_API_KEY)
let client = AnthropicClient::from_env()?;

// Or create with explicit config
let config = AnthropicConfig::new("your-api-key")
    .with_model("claude-3-5-sonnet")
    .with_timeout(Duration::from_secs(120));
let client = AnthropicClient::new(config)?;

// Use for streaming
let request = LlmRequest::new("claude-3-5-sonnet").with_messages(messages);
let stream = client.stream(request).await?;

// Or for complete responses
let response = client.complete(request).await?;
```

---

### Cost Tracking Methods on QueryEngine

New methods for tracking usage costs and enforcing budgets.

**Location**: `crates/runtime/src/query_engine/engine.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `total_usage` | `async fn total_usage(&self) -> Usage` | Get total token usage for the session |
| `total_cost` | `async fn total_cost(&self) -> f64` | Get total cost in USD for the session |
| `add_usage_cost` | `async fn add_usage_cost(&self, usage: Usage, model: &str)` | Add cost from usage for a specific model |
| `check_budget` | `async fn check_budget(&self) -> bool` | Check if current cost exceeds max budget |
| `calculate_cost` | `fn calculate_cost(&self, usage: Usage, model: &str) -> f64` | Calculate cost for given usage and model |
| `get_model_pricing` | `fn get_model_pricing(&self, model: &str) -> (f64, f64)` | Get (input, output) price per million tokens |
| `get_pricing_constants` | `fn get_pricing_constants() -> serde_json::Value` | Get all model pricing constants |

**Usage Example**:

```rust
use rad_runtime::query_engine::{QueryEngine, QueryEngineBuilder};

let engine = QueryEngineBuilder::new("/tmp", registry)
    .with_max_budget(10.0)  // $10 USD budget
    .build();

// After conversation
let usage = engine.total_usage().await;
let cost = engine.total_cost().await;
println!("Used {} tokens, cost ${:.4}", usage.total_tokens(), cost);

// Check budget before expensive operation
if engine.check_budget().await {
    println!("Budget exceeded!");
}
```

---

### Tool Execution Methods

New methods for executing tools during conversation loops.

**Location**: `crates/runtime/src/query_engine/execution.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `execute_tool` | `pub async fn execute_tool(&self, tool_name: &str, input: Value, tool_use_id: ToolUseId) -> QueryResult<ToolOutput>` | Execute a single tool |
| `execute_tools_parallel` | `async fn execute_tools_parallel(&self, tool_calls: Vec<...>, output_messages: &mut Vec<NormalizedMessage>) -> QueryResult<()>` | Execute tools with parallel support |
| `add_tool_result_to_conversation` | `async fn add_tool_result_to_conversation(&self, tool_use_id: ToolUseId, tool_result: QueryResult<ToolOutput>, output_messages: &mut Vec<NormalizedMessage>)` | Add tool result to conversation history |

**Location**: `crates/runtime/src/query_engine/engine.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `run_with_tools` | `pub async fn run_with_tools(&self, message: impl Into<String>, options: Option<SubmitMessageOptions>) -> QueryResult<ConversationResult>` | Run conversation with simulated tool calls |

---

## Type Changes

### ToolOutput Thread Safety Fix

**Location**: `crates/runtime/src/tool/output.rs`

The `ToolOutput` struct has been updated for thread safety. The `context_modifier` field now uses `Arc<Mutex<...>>` instead of a raw `Option<Box<dyn FnOnce...>>`.

**Before**:
```rust
pub struct ToolOutput {
    pub data: serde_json::Value,
    pub new_messages: Vec<Message>,
    pub context_modifier: Option<Box<dyn FnOnce(&mut ToolUseContext) + Send>>,
    pub mcp_meta: Option<McpMeta>,
}
```

**After**:
```rust
pub struct ToolOutput {
    pub data: serde_json::Value,
    pub new_messages: Vec<Message>,
    /// Wrapped in Arc<Mutex<...>> to make ToolOutput Sync + Send
    pub context_modifier: Option<Arc<Mutex<Option<Box<dyn FnOnce(&mut ToolUseContext) + Send>>>>>,
    pub mcp_meta: Option<McpMeta>,
}
```

**Impact**: `ToolOutput` is now `Sync`, enabling safe sharing across threads. The `Clone` implementation now clears the `context_modifier` (since `FnOnce` cannot be cloned).

---

### New Types in types/ Module

**Location**: `crates/runtime/src/types/mod.rs`

The `types` module has been reorganized into submodules:

| Module | Types |
|--------|-------|
| `types::ids` | `MessageId`, `SessionId`, `ToolUseId`, `CheckpointId` |
| `types::errors` | `ToolError`, `LlmApiError`, `QueryEngineError`, `ErrorCategory` |
| `types::results` | `QueryResult<T>`, `ToolResult<T>`, `Usage`, `Cost`, `ModelInfo`, `ModelProvider`, `ConversationAction` |
| `types::schema` | `JsonSchema` |

**Re-exports at `types` module level**:

```rust
pub use ids::{CheckpointId, MessageId, SessionId, ToolUseId};
pub use errors::{ErrorCategory, LlmApiError, QueryEngineError, ToolError};
pub use results::{ConversationAction, Cost, ModelInfo, ModelProvider, QueryResult, ToolResult, Usage};
pub use schema::JsonSchema;
```

---

### New Message Types

**Location**: `crates/runtime/src/messages/mod.rs`

The messages module has been reorganized into submodules:

| Module | Types |
|--------|-------|
| `messages::content` | `ContentBlock`, `ImageSource`, `ToolResultContent` |
| `messages::normalized` | `NormalizedMessage`, `NormalizedUserMessage`, `NormalizedAssistantMessage`, `ContentDelta`, `MessageDelta`, `StreamEvent`, `PermissionDenial`, `QueryResult` |
| `messages::tools` | `ProgressData`, `ProgressMessage`, `ToolUseResult`, `ToolUseSummaryMessage` |
| `messages::types` | `UserMessage`, `AssistantMessage`, `SystemMessage`, `AttachmentMessage`, `TombstoneMessage`, etc. |

**Main `Message` enum**:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    User(UserMessage),
    Assistant(AssistantMessage),
    Progress(ProgressMessage),
    Attachment(AttachmentMessage),
    System(SystemMessage),
    Tombstone(TombstoneMessage),
    ToolUseSummary(ToolUseSummaryMessage),
}
```

**Helper methods on `Message`**:
- `id()` - Get the message ID
- `timestamp()` - Get the timestamp
- `session_id()` - Get the session ID
- `parent_tool_use_id()` - Get parent tool use ID if applicable
- `is_user()`, `is_assistant()`, `is_system()`, `is_progress()`, `is_tool_result()` - Type checks
- `to_json()`, `from_json()` - Serialization
- `normalize()` - Convert to `NormalizedMessage`

---

## Re-exports

### Crate Root Re-exports

**Location**: `crates/runtime/src/lib.rs`

These types are available directly from `rad_runtime`:

```rust
// Core types
pub use context::ToolUseContext;
pub use registry::{ToolRegistry, ToolRegistryBuilder};
pub use tool::{Tool, ToolOutput, ToolResult};
pub use types::{SessionId, ToolUseId};

// Error handling utilities
pub use error_handling::{
    calculate_rate_limit_delay,
    http_status_to_error,
    with_panic_catch,
    with_retry,
    ConversationCheckpoint,
    RetryConfig,
};
```

### Query Engine Re-exports

**Location**: `crates/runtime/src/query_engine/mod.rs`

```rust
pub use config::{
    FallbackModelConfig, FallbackStatistics, ModelPriority, QueryEngineConfig,
    QueryEngineBuilder, DEFAULT_FALLBACK_MODEL, DEFAULT_MODEL, FALLBACK_DELAY_MS,
    MAX_FALLBACK_ATTEMPTS,
};
pub use engine::QueryEngine;
pub use execution::QueryExecution;
pub use streaming::QueryExecutionOps;
pub use types::{ConversationResult, MessageStream, SubmitMessageOptions};
pub use utils::{process_tool_results, format_tool_output, truncate_text};
```

### Tool Module Re-exports

**Location**: `crates/runtime/src/tool/mod.rs`

```rust
pub use builder::ToolBuilder;
pub use helpers::{find_tool_by_name, tool_matches_name};
pub use output::{McpMeta, ToolOutput};
pub use r#trait::{BoxedTool, SharedTool, Tool};
pub use validation::{InterruptBehavior, MaxResultSize, McpInfo, SearchOrReadInfo, ValidationResult};
pub use crate::types::ToolResult;
```

---

## Migration from Old Imports

### Import Path Changes

| Old Path | New Path | Notes |
|----------|----------|-------|
| `crate::types::*` | `crate::types::{ids, errors, results, schema}` | Split into submodules |
| `crate::messages::*` | `crate::messages::{content, normalized, tools, types}` | Split into submodules |
| `crate::ToolResult` | `crate::types::ToolResult` | Now re-exported from types::results |
| `crate::QueryResult` | `crate::types::QueryResult` | Now re-exported from types::results |
| `crate::types::ToolError` | `crate::types::errors::ToolError` | Moved to errors submodule |
| `crate::types::QueryEngineError` | `crate::types::errors::QueryEngineError` | Moved to errors submodule |
| `crate::types::MessageId` | `crate::types::ids::MessageId` | Moved to ids submodule |
| `crate::types::SessionId` | `crate::types::ids::SessionId` | Moved to ids submodule |
| `crate::types::ToolUseId` | `crate::types::ids::ToolUseId` | Moved to ids submodule |

**Note**: All types are still re-exported at the `types` module level, so existing imports like `use crate::types::MessageId` continue to work.

### Feature Flag for Anthropic

The `AnthropicClient` is behind the `anthropic` feature flag:

```toml
[dependencies]
rad_runtime = { version = "0.2", features = ["anthropic"] }
```

---

## Thread Safety Improvements

### ToolOutput Sync Fix

**Problem**: `ToolOutput` could not be shared between threads because `Box<dyn FnOnce>` is not `Sync`.

**Solution**: Wrapped the `context_modifier` in `Arc<Mutex<...>>`:

```rust
pub context_modifier: Option<Arc<Mutex<Option<Box<dyn FnOnce(&mut ToolUseContext) + Send>>>>>,
```

This makes `ToolOutput` both `Send` and `Sync`, enabling parallel tool execution.

### ToolLookupResult Debug Implementation

All result types in the codebase now implement `std::fmt::Debug` for better error reporting and logging.

### Error Handling Send Bounds

Error types have been updated with proper `Send` bounds:

```rust
// ToolError implements Clone + Send + Sync
#[derive(Error, Debug, Clone)]
pub enum ToolError { ... }

// LlmApiError implements Clone + Send + Sync
#[derive(Error, Debug, Clone)]
pub enum LlmApiError { ... }

// QueryEngineError implements Send
#[derive(Error, Debug)]
pub enum QueryEngineError { ... }
```

The `ToolError` and `LlmApiError` types implement `Clone` for retry scenarios. `QueryEngineError` does not implement `Clone` (due to internal errors), but can be converted to a string representation for logging.

---

## Summary of Breaking Changes

1. **ToolOutput context_modifier**: Now wrapped in `Arc<Mutex<...>>`. Use `with_context_modifier()` method to set it.

2. **Module reorganization**: Types and messages are now in submodules, though re-exports maintain backward compatibility at the module level.

3. **Anthropic feature flag**: The `anthropic` module requires the `anthropic` feature to be enabled.

4. **QueryEngine cost tracking**: New fields in `QueryEngineConfig` for budget enforcement:
   - `max_budget_usd: Option<f64>`
   - Pricing constants for cost calculation

5. **New ID types**: `CheckpointId` added for checkpoint operations.

---

## Feature Flags

| Flag | Description |
|------|-------------|
| `anthropic` | Enables the `AnthropicClient` and related types |
| `default` | No features enabled by default |

---

*Last updated: 2026-03-31*
