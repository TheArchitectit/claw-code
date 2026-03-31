# R.A.D Codicological 2.x - Project Guidelines

## Code Organization

### Modular Coding Requirement

**All source files MUST be under 500 lines of code.**

This rule ensures:
- **Readability**: Files are easy to scan and understand
- **Maintainability**: Changes are localized and reviewable
- **Compilation speed**: Smaller units compile faster in parallel
- **Testability**: Small modules are easier to unit test

#### Enforcement

- Maximum line count: 500 lines per `.rs` file (excluding tests)
- When a file approaches this limit, split it into submodules
- Use `mod.rs` or directory-based module structures

#### Refactoring Strategy

When a file exceeds 400 lines, plan to split it:

```rust
// Instead of query_engine.rs (3000+ lines)
query_engine/
  mod.rs          # Public API exports (~50 lines)
  engine.rs       # Core QueryEngine struct (~300 lines)
  config.rs       # QueryEngineConfig (~200 lines)
  execution.rs    # QueryExecution (~300 lines)
  streaming.rs    # Stream handling (~250 lines)
  types.rs        # Type definitions (~150 lines)
```

#### Module Structure Template

```rust
// In query_engine/mod.rs
pub mod config;
pub mod engine;
pub mod execution;
pub mod streaming;
pub mod types;

pub use config::QueryEngineConfig;
pub use engine::QueryEngine;
pub use execution::QueryExecution;
pub use types::*;
```

## Dependencies

- Use workspace dependencies from root `Cargo.toml`
- Add new dependencies to workspace root first, then reference in crates

## Testing

- Unit tests should be in the same file or adjacent `tests/` directory
- Integration tests in `tests/` folder at crate root

## Documentation

- All public APIs must have doc comments
- Use `///` for item documentation
- Use `//!` for module-level documentation
