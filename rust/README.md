# R.A.D Codicological 2.x

A production-ready Rust implementation of Claude Code — a drop-in replacement for the original TypeScript CLI.

## Overview

R.A.D Codicological 2.x is a comprehensive port of Anthropic's Claude Code CLI from TypeScript/Bun to Rust. It provides the same functionality with Rust's performance, type safety, and reliability.

**Status:** Phase 6 complete — QueryEngine implementation with ~40 tools, ~95 commands, full LLM integration.

## Architecture

```
rust/
├── crates/
│   ├── rusty-claude-cli/     # CLI entry point (TUI + command dispatch)
│   ├── runtime/               # Core runtime: QueryEngine, LLM client, streaming
│   │   ├── query_engine/      # Conversation lifecycle, tool-call loops
│   │   ├── llm_client/        # Anthropic API abstraction
│   │   ├── messages/           # Message types and normalization
│   │   ├── tool/               # Tool trait and registry
│   │   └── streaming/          # Real-time response streaming
│   ├── commands/              # ~95 slash command implementations
│   ├── tools/                 # ~40 tool implementations
│   ├── state/                 # Session persistence and recovery
│   ├── coordinator/           # Multi-agent swarm orchestration
│   ├── services/              # OAuth, MCP, LSP, analytics
│   └── compat-harness/        # TypeScript source compatibility
└── tests/                     # Integration tests
```

## Quick Start

```bash
cd rust/
cargo build --release
cargo run -- --help
```

## Documentation

| Document | Description |
|----------|-------------|
| [STATUS.md](STATUS.md) | Current project status, test counts, architecture |
| [QUERYENGINE_FEATURES.md](QUERYENGINE_FEATURES.md) | QueryEngine capabilities and configuration |
| [TOOLS_DOCUMENTATION.md](TOOLS_DOCUMENTATION.md) | All ~40 tools documented |
| [COMMANDS_DOCUMENTATION.md](COMMANDS_DOCUMENTATION.md) | All ~95 commands documented |
| [RUNTIME_ARCHITECTURE.md](RUNTIME_ARCHITECTURE.md) | Runtime internals and thread safety |
| [API_INTEGRATION.md](API_INTEGRATION.md) | Cross-crate API patterns |
| [MODULAR_STRUCTURE.md](MODULAR_STRUCTURE.md) | 67-file modular architecture |
| [TESTING_PATTERNS.md](TESTING_PATTERNS.md) | Testing guidelines |
| [MODULAR_REFACTOR.md](MODULAR_REFACTOR.md) | Refactoring from 11 to 67 files |
| [API_CHANGES.md](API_CHANGES.md) | API changes and thread safety fixes |

## Key Features

- **QueryEngine**: Full conversation lifecycle with tool-call loops, streaming, budget enforcement
- **LLM Integration**: Anthropic API client with streaming SSE, model fallback, cost tracking
- **Tool System**: ~40 tools including FileRead, FileEdit, Bash, Glob, Grep, Agent, MCP, LSP
- **Command System**: ~95 slash commands for git, review, session, config, diagnostics
- **Multi-Agent**: Coordinator integration for agent swarms and team management
- **Thread Safety**: All components use `Arc<RwLock<T>>`/`Arc<Mutex<T>>` for async safety
- **Modular Architecture**: 67 files, all under 500 lines (per CLAUDE.md guidelines)

## Development

```bash
# Check all crates
cargo check --workspace

# Run tests
cargo test --workspace

# Run single-threaded (required for tests)
cargo test --workspace -- --test-threads=1

# Format
cargo fmt --all

# Lint
cargo clippy --workspace
```

## Design Principles

1. **Compatibility**: Drop-in replacement for original TypeScript CLI
2. **Modularity**: No file over 500 lines, directory-based modules
3. **Type Safety**: Leverage Rust's type system, no runtime errors
4. **Performance**: Tokio async, efficient streaming, parallel execution
5. **Testability**: Mock LLM client, comprehensive test coverage

## Relationship to Original

The `../src/` directory contains the leaked TypeScript source (~1,900 files, 512K lines). This Rust implementation is a clean-room port preserving the architecture while leveraging Rust's strengths.

## License

This is a port of leaked source code. All original TypeScript source is property of Anthropic.
