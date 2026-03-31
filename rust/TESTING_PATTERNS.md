# Testing Patterns for R.A.D Codicological 2.x

This document describes the testing patterns, infrastructure, and best practices for the R.A.D Codicological 2.x Rust codebase.

## Table of Contents

1. [Test Organization](#test-organization)
2. [Test Utilities](#test-utilities)
3. [Writing Tests for Tools](#writing-tests-for-tools)
4. [Writing Tests for Commands](#writing-tests-for-commands)
5. [Writing Tests for QueryEngine](#writing-tests-for-queryengine)
6. [Async Testing Patterns](#async-testing-patterns)
7. [Mock Implementations](#mock-implementations)
8. [Test Coverage Recommendations](#test-coverage-recommendations)

---

## Test Organization

### Directory Structure

```
rust/
├── Cargo.toml                    # Workspace configuration
├── tests/
│   └── integration_tests.rs      # Cross-crate integration tests
└── crates/
    ├── commands/
    │   └── src/
    │       └── tests.rs          # Module-level unit tests
    ├── tools/
    │   ├── src/
    │   │   ├── file_read.rs      # Inline unit tests at EOF
    │   │   └── ...               # Each tool has inline tests
    │   └── tests/
    │       └── integration_tests.rs  # Cross-tool integration tests
    ├── runtime/
    │   └── src/
    │       ├── llm_client/
    │       │   └── mock.rs       # Mock LLM client + tests
    │       ├── messages/
    │       │   └── tests.rs       # Message type tests
    │       └── query_engine_execution_loop_tests.rs  # QueryEngine tests
    └── test-utils/
        └── src/
            └── lib.rs            # Shared testing utilities
```

### Test Types

| Location | Purpose | Scope |
|----------|---------|-------|
| `#[cfg(test)]` module at EOF | Unit tests | Single module |
| `crates/<name>/tests/*.rs` | Integration tests | Single crate |
| `rust/tests/*.rs` | Cross-crate integration | Multiple crates |

### Running Tests

```bash
# All tests
cargo test

# Tests for specific crate
cargo test -p tools
cargo test -p runtime

# Specific test
cargo test test_file_read_text

# Integration tests only
cargo test --test integration_tests

# With output
cargo test -- --nocapture
```

---

## Test Utilities

The `test-utils` crate provides shared testing infrastructure.

### TempDirFixture

Temporary directory management with automatic cleanup.

```rust
use test_utils::TempDirFixture;

#[tokio::test]
async fn test_with_temp_dir() {
    let temp = TempDirFixture::new().await;

    // Create files
    let file_path = temp.create_file("test.txt", "content").await.unwrap();

    // Create nested directories
    let nested = temp.create_file("a/b/c/nested.txt", "nested").await.unwrap();

    // Directory auto-deletes when temp goes out of scope
}
```

### Fixture Helpers

```rust
use test_utils::fixtures;

// Create a Rust project structure
let files = fixtures::rust_project(&temp).await.unwrap();
// Creates: Cargo.toml, src/main.rs, src/lib.rs

// Create mixed file tree
let paths = fixtures::mixed_file_tree(&temp).await.unwrap();
// Creates: README.md, src/*.rs, tests/*.rs, docs/*.md, config.json, .gitignore

// Create large files for testing
let path = fixtures::large_text_file(&temp, "large.txt", 1000).await.unwrap();
let path = fixtures::binary_file(&temp, "binary.dat", 1024).await.unwrap();
```

### Mock Tools

#### Simple MockTool

```rust
use test_utils::MockTool;
use tools::{ToolInput, ToolOutput};

let tool = MockTool::new("test_tool")
    .with_description("A test tool")
    .read_only(true)
    .concurrency_safe(true)
    .with_handler(|input| {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        ToolOutput::new().with_field("greeting", format!("Hello, {name}!"))
    });

let input = ToolInput::new().with_arg("name", "World");
let output = tool.execute(input).await;
```

#### TrackedMockTool (Invocation Tracking)

```rust
use test_utils::TrackedMockTool;

let tool = TrackedMockTool::new("tracked_tool")
    .with_handler(|_| ToolOutput::new().with_field("result", "ok"));

// Execute multiple times
for i in 0..3 {
    tool.execute(ToolInput::new().with_arg("index", i)).await;
}

// Verify invocations
tool.assert_invoked_times(3).await;
let invocations = tool.get_invocations().await;
```

### Assertion Helpers

```rust
use test_utils::assertions::*;

// Assert output fields
assert_output_field(&output, "field_name", "expected_value");
assert_has_field(&output, "field_name");
assert_success(&output);
assert_failure(&output);
assert_has_error(&output);

// String assertions
assert_contains(haystack, "needle");
assert_not_contains(haystack, "needle");
```

### Prelude (Import All)

```rust
use test_utils::prelude::*;
// Imports: TempDirFixture, MockTool, TrackedMockTool, assertions::*, fixtures::*,
//          SessionId, ToolRegistry, ToolUseContext, ToolUseId, ToolInput, ToolOutput, ToolResult
```

---

## Writing Tests for Tools

### Basic Tool Test Structure

```rust
// In src/my_tool.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_basic_execution() {
        let tool = MyTool::new();
        let input = ToolInput::new()
            .with_arg("param1", "value1")
            .with_arg("param2", "value2");

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("result"), Some(&json!("expected")));
    }

    #[tokio::test]
    async fn test_tool_validation_failure() {
        let tool = MyTool::new();
        let input = ToolInput::new(); // Missing required params

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }
}
```

### File-Based Tool Testing

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_read_text() {
        let tool = FileReadTool::new();

        // Create a temp file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();

        let path = file.path().to_string_lossy().to_string();
        let input = ToolInput::new().with_arg("file_path", &path);

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("text")
        );
    }
}
```

### Using test-utils for Complex Scenarios

```rust
use test_utils::prelude::*;

#[tokio::test]
async fn test_tool_with_fixtures() {
    let temp = TempDirFixture::new().await;

    // Create structured project
    let _ = fixtures::rust_project(&temp).await.unwrap();

    // Test tool execution
    let tool = GlobTool::new();
    let input = ToolInput::new()
        .with_arg("pattern", "**/*.rs")
        .with_arg("path", temp.path().to_str().unwrap());

    let output = tool.execute(input).await;

    assert_has_field(&output, "filenames");
    assert_success(&output);
}
```

### Error Handling Tests

```rust
#[tokio::test]
async fn test_tool_error_handling() {
    let tool = FileReadTool::new();

    // Non-existent file
    let input = ToolInput::new()
        .with_arg("file_path", "/nonexistent/path/file.txt");

    let output = tool.execute(input).await;

    // Check error type field
    assert_eq!(
        output.data.get("type").and_then(|v| v.as_str()),
        Some("not_found")
    );
    assert!(output.data.contains_key("error"));
}
```

---

## Writing Tests for Commands

### Command Registry Tests

```rust
#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_registry_register_and_find() {
        let mut registry = CommandRegistry::new();
        registry.register(HelpCommand::simple());

        assert!(registry.has("help"));
        assert!(registry.has("?")); // alias

        let cmd = registry.find("help");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().metadata().name, "help");
    }

    #[tokio::test]
    async fn test_registry_execute_found() {
        let mut registry = CommandRegistry::new();
        registry.register(HelpCommand::simple());

        let ctx = CommandContext::new("/tmp");
        let result = registry.execute("help", &ctx).await;

        assert!(result.is_ok());
    }
}
```

### Command Execution Tests

```rust
#[tokio::test]
async fn test_command_execution() {
    let cmd = StatusCommand::new();
    let ctx = CommandContext::new("/tmp");

    let result = cmd.execute(&ctx).await;
    assert!(result.is_ok());

    match result.unwrap() {
        CommandOutput::Json(data) => {
            assert!(data.get("session").is_some());
        }
        _ => panic!("Expected JSON output"),
    }
}
```

### Command Context with Flags

```rust
#[tokio::test]
async fn test_command_with_flags() {
    let cmd = StatusCommand::new();
    let mut ctx = CommandContext::new("/tmp");

    // Add flags
    ctx.flags.insert("tools".to_string(), "true".to_string());
    ctx.flags.insert("env".to_string(), "true".to_string());

    let result = cmd.execute(&ctx).await;
    assert!(result.is_ok());
}
```

---

## Writing Tests for QueryEngine

### Using MockLlmClient

The `MockLlmClient` provides pre-programmed LLM responses for testing QueryEngine.

```rust
use runtime::llm_client::mock::{MockLlmClient, MockResponse};
use runtime::llm_client::types::{LlmRequest, LlmResponse};
use runtime::messages::ContentBlock;
use runtime::types::Usage;

#[tokio::test]
async fn test_simple_conversation() {
    // Create mock LLM client
    let client = MockLlmClient::simple_text_response("Hello! I can help you.");

    // Build QueryEngine with mock client
    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(Arc::new(client))
        .with_max_turns(10)
        .build();

    // Execute query
    let mut execution = engine
        .submit_message("Hi there", None)
        .await
        .expect("Failed to submit message");

    let messages = execution.execute().await.expect("Execution failed");

    // Verify completion
    assert!(execution.is_complete());
    let result = execution.get_result().expect("Should have result");

    match result {
        QueryResultMessage::Success { stop_reason, num_turns, .. } => {
            assert_eq!(stop_reason.as_deref(), Some("end_turn"));
            assert_eq!(*num_turns, 1);
        }
        _ => panic!("Expected Success result"),
    }
}
```

### Testing Tool Call Scenarios

```rust
#[tokio::test]
async fn test_multi_turn_with_tools() {
    let registry = create_registry();

    // First response: tool call
    let tool_response = LlmResponse {
        content: vec![ContentBlock::ToolUse {
            id: ToolUseId::generate(),
            name: "file_read".to_string(),
            input: json!({"file_path": "/tmp/test.txt"}),
        }],
        stop_reason: StopReason::ToolUse,
        usage: Usage::default(),
        model: "mock".to_string(),
    };

    // Second response: final text
    let final_response = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Done!".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: Usage::default(),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![
        tool_response,
        final_response,
    ]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_turns(10)
        .build();

    let mut execution = engine
        .submit_message("Read the file", None)
        .await
        .expect("Failed to submit");

    let _ = execution.execute().await.expect("Execution failed");

    // Verify multi-turn
    let result = execution.get_result().expect("Should have result");
    match result {
        QueryResultMessage::Success { num_turns, .. } => {
            assert!(*num_turns >= 2, "Should have at least 2 turns");
        }
        _ => panic!("Expected Success result"),
    }
}
```

### Testing Budget Enforcement

```rust
#[tokio::test]
async fn test_budget_enforcement() {
    let registry = create_registry();

    let response_with_usage = LlmResponse {
        content: vec![ContentBlock::Text {
            text: "Response".to_string(),
            citation: None,
        }],
        stop_reason: StopReason::EndTurn,
        usage: Usage::new(1000, 500, 0, 0),
        model: "mock".to_string(),
    };

    let llm_client = Arc::new(MockLlmClient::new(vec![response_with_usage]));

    let engine = QueryEngineBuilder::new("/tmp", registry)
        .with_llm_client(llm_client)
        .with_max_budget(0.001)
        .with_max_turns(100)
        .build();

    // Pre-set high cost to trigger budget exceeded
    {
        let mut cost = engine.total_cost.lock().await;
        *cost = 0.002;
    }

    let mut execution = engine
        .submit_message("Test", None)
        .await
        .expect("Failed to submit");

    let result = execution.execute().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        QueryEngineError::MaxBudgetExceeded { max_budget } => {
            assert!((max_budget - 0.001).abs() < 0.0001);
        }
        other => panic!("Expected MaxBudgetExceeded, got {:?}", other),
    }
}
```

### Verifying Request Recording

```rust
#[tokio::test]
async fn test_request_recording() {
    let client = MockLlmClient::simple_text_response("Response");
    let request = LlmRequest::new("test-model");

    let _ = client.complete(request.clone()).await.unwrap();

    // Verify recorded requests
    let recorded = client.get_recorded_requests();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].model, "test-model");
    assert_eq!(client.request_count(), 1);

    // Clear for next test
    client.clear_recorded_requests();
}
```

---

## Async Testing Patterns

### Basic Async Test

```rust
#[tokio::test]
async fn test_async_operation() {
    let result = some_async_function().await;
    assert!(result.is_ok());
}
```

### Using tokio-test for Timeouts

```rust
use tokio_test::timeout;

#[tokio::test]
async fn test_with_timeout() {
    let result = timeout(
        std::time::Duration::from_secs(5),
        some_slow_operation()
    ).await;

    assert!(result.is_ok());
}
```

### Sequential Test Execution

For tests that modify shared state, use a mutex to ensure sequential execution:

```rust
use std::sync::Mutex;

// Global mutex for test serialization
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn setup() -> std::sync::MutexGuard<'static, ()> {
    // Reset shared state
    reset_global_state();
    TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
}

#[tokio::test]
async fn test_task_creation() {
    let _guard = setup(); // Other tests wait here

    // Test code that modifies global state
}
```

### Example: Task Store Testing

```rust
// In crates/tools/src/task_store.rs

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        reset_task_store();
        TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner())
    }

    #[tokio::test]
    async fn test_task_creation_and_retrieval() {
        let _guard = setup();

        let tool = TaskCreateTool::new();
        let output = tool.execute(
            ToolInput::new()
                .with_arg("subject", "Test Task")
                .with_arg("description", "Test Description")
        ).await;

        let task_id = output.data.get("task_id")
            .and_then(|v| v.as_str())
            .expect("Should have task_id");

        // Retrieve the task
        let get_tool = TaskGetTool::new();
        let get_output = get_tool.execute(
            ToolInput::new().with_arg("task_id", task_id)
        ).await;

        assert_eq!(
            get_output.data.get("subject").and_then(|v| v.as_str()),
            Some("Test Task")
        );
    }
}
```

---

## Mock Implementations

### MockLlmClient

The `MockLlmClient` implements the `LlmClient` trait for testing.

```rust
use runtime::llm_client::mock::{MockLlmClient, MockResponse};

// Simple text response
let client = MockLlmClient::simple_text_response("Hello!");

// With tool call
let client = MockLlmClient::with_tool_call(
    "file_read",
    json!({"file_path": "/tmp/test.txt"})
);

// With multiple responses (cycles through them)
let client = MockLlmClient::with_responses(vec![
    response1,
    response2,
    response3,
]);

// With MockResponse configurations
let client = MockLlmClient::with_mock_responses(vec![
    MockResponse::text("First"),
    MockResponse::tool_call("tool_name", json!({})),
    MockResponse::with_thinking("Answer", "Thinking..."),
    MockResponse::error("Something went wrong"),
]);

// Error response
let client = MockLlmClient::with_error_response("Error message");
```

### MockResponse Builder

```rust
use runtime::llm_client::mock::MockResponse;
use runtime::types::Usage;

let response = MockResponse::text("Hello")
    .with_usage(Usage::new(100, 50, 10, 5))
    .with_delay(100); // Simulate 100ms latency

let response = MockResponse::tool_call("tool_name", json!({"arg": "value"}))
    .with_usage(Usage::new(50, 30, 0, 0));

let response = MockResponse::with_thinking(
    "Final answer",
    "Thinking process here"
);
```

### MockTool and TrackedMockTool

See [Test Utilities](#test-utilities) section above.

---

## Test Coverage Recommendations

### Tool Tests Checklist

For each tool, test:

- [ ] **Basic execution** with valid inputs
- [ ] **Validation failure** with missing/invalid inputs
- [ ] **Error handling** (file not found, permission denied, etc.)
- [ ] **Edge cases** (empty files, large files, special characters)
- [ ] **File operations** (create, read, update, delete where applicable)

### Command Tests Checklist

For each command, test:

- [ ] **Metadata correctness** (name, description, aliases)
- [ ] **Command matching** (name and all aliases)
- [ ] **Execution success** in normal conditions
- [ ] **Execution failure** (e.g., non-git directory for git commands)
- [ ] **Context with flags** and arguments

### QueryEngine Tests Checklist

- [ ] **Natural completion** (EndTurn stop reason)
- [ ] **Tool use scenarios** (ToolUse stop reason, tool execution)
- [ ] **Max turns enforcement**
- [ ] **Budget enforcement**
- [ ] **Usage accumulation** across turns
- [ ] **Permission denial handling**
- [ ] **Stop sequence handling**
- [ ] **Max tokens handling**
- [ ] **Error scenarios**

### Integration Tests Checklist

- [ ] **Cross-tool workflows** (file write → read, glob → grep)
- [ ] **Task lifecycle** (create → update → output → retrieve)
- [ ] **End-to-end file operations**
- [ ] **Error propagation** across components

---

## Best Practices

### 1. Use test-utils Crate

Always use the shared test utilities rather than creating your own temp directories or mock tools.

### 2. Clean Up Shared State

Use mutexes or setup functions to ensure tests that modify global state run sequentially.

### 3. Test Both Success and Failure

Every tool/command should have tests for both happy path and error conditions.

### 4. Use Descriptive Test Names

```rust
// Good
#[tokio::test]
async fn test_file_edit_replace_all_replaces_all_occurrences() { }

// Bad
#[tokio::test]
async fn test_edit() { }
```

### 5. Keep Tests Deterministic

Avoid tests that depend on timing or external state. Use MockLlmClient instead of real API calls.

### 6. Use Type-Safe Assertions

```rust
// Good
assert_eq!(output.data.get("count").and_then(|v| v.as_u64()), Some(5));

// Avoid
assert!(output.data.get("count").unwrap().as_u64().unwrap() == 5);
```

### 7. Document Test Intent

Add comments explaining what scenario the test covers, especially for complex integration tests.

---

## Troubleshooting

### Common Issues

**Tests failing due to shared state:**
```rust
// Use mutex to serialize tests
static TEST_MUTEX: Mutex<()> = Mutex::new(());
let _guard = TEST_MUTEX.lock().unwrap();
```

**Temp files not cleaned up:**
```rust
// Use TempDirFixture instead of manual tempfile
let temp = TempDirFixture::new().await;
// Auto-cleans when temp goes out of scope
```

**Async test panics:**
```rust
// Ensure tokio runtime is available
#[tokio::test]
async fn test_name() {
    // test code
}
```

**Missing test dependencies:**
```toml
# Add to Cargo.toml [dev-dependencies]
tokio-test = { workspace = true }
test-utils = { path = "../test-utils" }
```

---

## Additional Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Tokio Testing](https://tokio.rs/tokio/topics/testing)
- [Mockall for Mocking](https://docs.rs/mockall/latest/mockall/)
- [Tempfile Crate](https://docs.rs/tempfile/latest/tempfile/)
