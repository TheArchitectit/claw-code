# R.A.D. Codicological

**Research & Development - Conversational Development Environment**

A reverse-engineered implementation of an AI-assisted software engineering tool, developed through clean-room analysis of publicly observable protocols and behaviors.

## Overview

R.A.D. Codicological is an open-source, clean-room implementation of a conversational AI development environment. This project was created through independent reverse engineering of publicly documented protocols (MCP, LSP) and observable interaction patterns, without reference to proprietary source code.

**Purpose:** Educational research into AI-assisted software engineering tooling, multi-agent orchestration, and conversational interfaces.

## Architecture

Built in Rust with a modular, async-first architecture:

```
rust/
├── crates/
│   ├── rusty-claude-cli/     # CLI entry with TUI (ratatui)
│   ├── runtime/               # Core: QueryEngine, LLM client, streaming
│   │   ├── query_engine/      # Conversation lifecycle management
│   │   ├── llm_client/        # LLM API abstraction layer
│   │   ├── messages/           # Message protocol handling
│   │   ├── tool/               # Tool trait system
│   │   └── streaming/          # Real-time streaming
│   ├── commands/              # Command implementations
│   ├── tools/                 # Tool implementations
│   ├── state/                 # Session persistence
│   ├── coordinator/           # Multi-agent orchestration
│   └── compat-harness/        # Protocol compatibility testing
```

## Key Components

### QueryEngine
The core conversation orchestrator managing:
- Turn-based LLM interaction loops
- Tool execution lifecycle
- Streaming response handling
- Cost tracking and budget enforcement
- Message normalization

### Tool System
Modular tool trait allowing extensible capabilities:
- File operations (read, write, edit)
- System integration (bash, task management)
- Web capabilities (fetch, search)
- Multi-agent (agent spawning, team coordination)
- Protocol support (MCP, LSP)

### LLM Client
Abstraction over LLM APIs with:
- Streaming SSE support
- Model fallback chains
- Rate limiting and retry logic
- Cost calculation per model

## Quick Start

```bash
cd rust/
cargo build --release
cargo run -- --help
```

## Development

```bash
# Check all crates
cargo check --workspace

# Run tests (single-threaded required)
cargo test --workspace -- --test-threads=1

# Format
cargo fmt --all
```

## Design Principles

1. **Clean Room**: Implemented from public protocol specifications only
2. **Modularity**: No file exceeds 500 lines; directory-based modules
3. **Type Safety**: Leverage Rust's type system for runtime reliability
4. **Protocol Compatibility**: Works with standard protocols (MCP, LSP, Anthropic API)
5. **Research Purpose**: Educational investigation of AI tooling patterns

## Protocol Support

- **MCP (Model Context Protocol)**: Tool and resource provider integration
- **LSP (Language Server Protocol)**: IDE integration capabilities
- **Anthropic API**: LLM backend support
- **OAuth 2.0**: Authentication flows

## Documentation

See `rust/` directory for detailed documentation:
- Architecture guides
- Tool system documentation
- Command reference
- API integration patterns
- Testing guidelines

## Reverse Engineering Methodology

This project was developed through:
1. **Protocol Analysis**: Study of publicly documented APIs (MCP spec, LSP spec, Anthropic API docs)
2. **Behavioral Observation**: Analysis of observable interaction patterns in similar tools
3. **Independent Implementation**: All code written from scratch based on protocol specifications
4. **Compatibility Testing**: Verification against public protocol specifications

**No proprietary source code was referenced in the development of this project.**

## Legal Notice

This is an independent research project implementing publicly documented protocols. All code is original work. Any similarity to existing products stems from:
- Implementation of common industry standards (MCP, LSP)
- Convergent evolution of AI-assisted tooling patterns
- Public API compatibility requirements

This project is for educational and research purposes.

## License

[Specify your license here]

## Acknowledgments

- MCP Protocol specification (modelcontextprotocol.io)
- LSP Specification (microsoft.github.io/language-server-protocol)
- Rust async ecosystem (Tokio, async-trait)
- Terminal UI libraries (ratatui, crossterm)
