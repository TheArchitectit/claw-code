# R.A.D. Codicological

**Research & Development - Conversational Development Environment**

A clean-room implementation of an AI-assisted software engineering tool, built through reverse engineering of public protocols and observable behaviors.

## Overview

This project is a reverse-engineered implementation of conversational AI development tooling. Created through independent analysis of:
- Public protocol specifications (MCP, LSP)
- Observable API behaviors
- Industry-standard interaction patterns

**Status:** Core runtime complete with ~40 tools, full QueryEngine, multi-agent support.

## Architecture

```
rust/
├── crates/
│   ├── rusty-claude-cli/     # CLI with TUI
│   ├── runtime/               # QueryEngine, LLM client, streaming
│   │   ├── query_engine/      # Conversation lifecycle
│   │   ├── llm_client/        # API abstraction
│   │   ├── messages/           # Message protocols
│   │   └── tool/               # Tool system
│   ├── commands/              # Command implementations
│   ├── tools/                 # Tool implementations
│   ├── state/                 # Session persistence
│   └── coordinator/           # Multi-agent orchestration
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
| [STATUS.md](STATUS.md) | Project status and architecture |
| [QUERYENGINE_FEATURES.md](QUERYENGINE_FEATURES.md) | QueryEngine capabilities |
| [TOOLS_DOCUMENTATION.md](TOOLS_DOCUMENTATION.md) | Tool system reference |
| [COMMANDS_DOCUMENTATION.md](COMMANDS_DOCUMENTATION.md) | Command reference |
| [RUNTIME_ARCHITECTURE.md](RUNTIME_ARCHITECTURE.md) | Runtime internals |
| [API_INTEGRATION.md](API_INTEGRATION.md) | Integration patterns |
| [MODULAR_STRUCTURE.md](MODULAR_STRUCTURE.md) | Codebase organization |
| [TESTING_PATTERNS.md](TESTING_PATTERNS.md) | Testing guidelines |

## Key Features

- **QueryEngine**: Conversation lifecycle with streaming, cost tracking
- **Tool System**: Modular tool trait with 40+ implementations
- **Multi-Agent**: Coordinator for agent swarms
- **Protocol Support**: MCP, LSP integration
- **Thread Safety**: Async-safe with Arc<RwLock>/Arc<Mutex>

## Development

```bash
# Build
cargo build --release

# Test (single-threaded)
cargo test --workspace -- --test-threads=1

# Format
cargo fmt --all
```

## Design Principles

1. **Clean Room**: From public specs only
2. **Modularity**: <500 lines per file
3. **Type Safety**: Rust's compile-time guarantees
4. **Protocol Compatible**: Standard protocols only
5. **Research Purpose**: Educational investigation

## Reverse Engineering Methodology

Developed through:
1. Protocol analysis of public specs (MCP, LSP, Anthropic API)
2. Behavioral observation of interaction patterns
3. Independent implementation from specifications
4. Protocol compliance verification

**No proprietary code referenced.**

## Legal Notice

Independent research project implementing public protocols. Original code. Similarities stem from common standards and convergent evolution.

For educational and research purposes.
