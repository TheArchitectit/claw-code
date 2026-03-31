# R.A.D Codicological 2.x - Testing Guide

## Running Tests

Due to shared global state in the task management system, tests must be run single-threaded:

```bash
# Run all tests
cargo test --workspace -- --test-threads=1

# Run specific crate tests
cargo test -p tools -- --test-threads=1
cargo test -p runtime -- --test-threads=1

# For CI/CD, use this configuration
```

## Test Configuration

### Cargo Alias (Recommended)

Add to `.cargo/config.toml`:

```toml
[alias]
test = "test -- --test-threads=1"
```

Then run:
```bash
cargo test --workspace
```

## Current Test Status

| Crate | Tests | Status |
|-------|-------|--------|
| runtime | 10+ | Passing |
| tools | 70 | Passing |
| commands | 8+ | Passing |
| test-utils | 10 | Passing |
| rusty-claude-cli | 10 | Passing |

## Known Issues

1. **Parallel Test Deadlock**: The task management tests use a global TEST_MUTEX to serialize access to the global TaskStore. Running tests in parallel causes deadlock.

2. **Shared Global State**: The `get_task_store()` function returns a singleton instance. Tests must be serialized or use the mutex.

## Solution

The long-term fix is to refactor tools to accept a store parameter:

```rust
// Instead of:
let store = get_task_store();

// Use:
fn execute_with_store(&self, input: ToolInput, store: &TaskStore) -> ToolOutput
```

This would allow tests to use isolated store instances.
