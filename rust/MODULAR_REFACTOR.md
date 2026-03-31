# Modular Architecture Documentation

## Overview

This document describes the modular architecture of the R.A.D. Codicological runtime, organized to maximize maintainability and readability.

## Philosophy

Following clean architecture principles, no source file exceeds 500 lines. When a file approaches this limit, it is split into a directory-based module structure.

## Before/After Structure

### Original Structure (11 files)

| File | Lines | Issue |
|------|-------|-------|
| messages.rs | 2,151 | Too large, multiple concerns |
| query_engine.rs | 1,746 | Monolithic, hard to navigate |
| types.rs | 943 | Mixed type categories |
| permissions.rs | 1,028 | Complex permission logic |
| context.rs | 1,180 | Multiple context types |
| registry.rs | 1,000 | Registry + builder + adapters |
| tool.rs | 759 | Trait + types + output |

### New Structure (67 files)

| Directory | Files | Lines (max) |
|-----------|-------|-------------|
| query_engine/ | 7 | 534 |
| messages/ | 6 | 718 (tests) |
| llm_client/ | 5 | 386 |
| tool/ | 6 | 411 |
| types/ | 5 | 364 |
| streaming/ | 4 | 211 |

## Benefits

1. **Compilation Speed**: Smaller units compile faster in parallel
2. **Readability**: Files are easy to scan and understand
3. **Maintainability**: Changes are localized
4. **Testability**: Small modules are easier to unit test
5. **Code Review**: Reviewers can focus on specific concerns

## Module Organization

### query_engine/

```
query_engine/
├── mod.rs           # Public exports
├── engine.rs        # Core QueryEngine struct
├── execution.rs     # QueryExecution
├── config.rs        # Configuration
├── streaming.rs     # Streaming operations
├── types.rs         # QueryEngine types
└── utils.rs         # Utilities
```

### messages/

```
messages/
├── mod.rs           # Public exports
├── types.rs         # Core message types
├── assistant.rs     # Assistant messages
├── user.rs          # User messages
├── content.rs       # Content blocks
├── tools.rs         # Tool-related messages
└── normalization.rs # Message normalization
```

### llm_client/

```
llm_client/
├── mod.rs           # Public exports
├── client.rs        # LlmClient trait
├── mock.rs          # Mock implementations
├── stream.rs        # Streaming types
└── types.rs         # Request/Response types
```

### tool/

```
tool/
├── mod.rs           # Tool trait
├── types.rs         # Tool-related types
├── output.rs        # ToolOutput
└── registry_adapter.rs # Registry integration
```

## Public API Exports

Each module uses a re-export strategy for clean public APIs:

```rust
// In query_engine/mod.rs
pub mod config;
pub mod engine;
pub mod execution;

pub use config::QueryEngineConfig;
pub use engine::QueryEngine;
pub use execution::QueryExecution;
```

## Guidelines for Future Refactoring

When a file approaches 400 lines:

1. Identify distinct concerns
2. Create directory with mod.rs
3. Move related code to submodules
4. Re-export public APIs at module root
5. Update imports in dependent code

## Statistics

- **Total files**: 67
- **Directory modules**: 13
- **Average lines per file**: ~150
- **Max lines**: 534 (execution.rs)

## Legal Notice

This is a clean-room reverse engineering implementation. All code is original work developed from public protocol specifications.

For educational and research purposes.
