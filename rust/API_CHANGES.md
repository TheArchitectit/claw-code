# API Changes and New Types Documentation

This document describes the public API changes, new types, and migration guide for the runtime crate.

## Table of Contents

1. [New Public APIs](#new-public-apis)
2. [Type Changes](#type-changes)
3. [Re-exports](#re-exports)
4. [Migration from Old Imports](#migration-from-old-imports)
5. [Thread Safety Improvements](#thread-safety-improvements)

---

## New Public APIs

### LLM Client

A new LLM client implementation supporting standard APIs.

**Location**: `crates/runtime/src/llm_client/mod.rs`

```rust
// Re-exports from llm_client module
pub use client::{BoxedLlmClient, LlmClient, SharedLlmClient};
pub use mock::{MockLlmClient, MockResponse};
pub use stream::{LlmStream, LlmStreamChunk, StopReason};
pub use types::{LlmRequest, LlmResponse};
```

**Key Types**:

| Type | Description |
|------|-------------|
| `LlmClient` | Trait for LLM client implementations |
| `MockLlmClient` | Mock implementation for testing |
| `LlmRequest` | Request structure for LLM calls |
| `LlmResponse` | Response structure from LLM calls |

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

---

### Tool Execution Methods

New methods for executing tools during conversation loops.

**Location**: `crates/runtime/src/query_engine/execution.rs`

| Method | Signature | Description |
|--------|-----------|-------------|
| `execute_tool` | `pub async fn execute_tool(&self, tool_name: &str, input: Value, tool_use_id: ToolUseId) -> QueryResult<ToolOutput>` | Execute a single tool |
| `execute_tools_parallel` | `async fn execute_tools_parallel(&self, tool_calls: Vec<...>, output_messages: &mut Vec<NormalizedMessage>) -> QueryResult<()>` | Execute tools with parallel support |

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

**Impact**: `ToolOutput` is now `Sync`, enabling safe sharing across threads.

---

## Re-exports

### Crate Root Re-exports

**Location**: `crates/runtime/src/lib.rs`

These types are available directly from `runtime`:

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

---

## Migration from Old Imports

### Import Path Changes

| Old Path | New Path | Notes |
|----------|----------|-------|
| `crate::types::*` | `crate::types::{ids, errors, results, schema}` | Split into submodules |
| `crate::messages::*` | `crate::messages::{content, normalized, tools, types}` | Split into submodules |

**Note**: All types are still re-exported at the `types` module level, so existing imports continue to work.

---

## Thread Safety Improvements

### ToolOutput Sync Fix

**Problem**: `ToolOutput` could not be shared between threads because `Box<dyn FnOnce>` is not `Sync`.

**Solution**: Wrapped the `context_modifier` in `Arc<Mutex<...>>`:

```rust
pub context_modifier: Option<Arc<Mutex<Option<Box<dyn FnOnce(&mut ToolUseContext) + Send>>>>>,
```

This makes `ToolOutput` both `Send` and `Sync`, enabling parallel tool execution.

### Error Handling Send Bounds

Error types have been updated with proper `Send` bounds:

```rust
// ToolError implements Clone + Send + Sync
#[derive(Error, Debug, Clone)]
pub enum ToolError { ... }

// LlmApiError implements Clone + Send + Sync
#[derive(Error, Debug, Clone)]
pub enum LlmApiError { ... }
```

---

## Legal Notice

This is a clean-room reverse engineering implementation. All code is original work developed from public protocol specifications.

For educational and research purposes.
