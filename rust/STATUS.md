# R.A.D Codicological 2.x - Project Status

**Date:** 2026-03-31
**Lines of Code:** 45,344 (tools: ~9,058)
**Test Status:** 309+ tests passing (single-threaded)

## Test Counts

| Crate | Tests | Status |
|-------|-------|--------|
| runtime | 90 | passing |
| tools | 147 | passing |
| commands | 34 | passing |
| cli | 57 | passing |
| test-utils | 6 | passing |
| compat-harness | 3 | passing |
| integration | 30 | passing |
| **Total** | **367** | all passing |

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    rusty-claude-cli                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │    CLI      │  │    TUI      │  │   Command Dispatch  │ │
│  │   (clap)    │  │ (ratatui)   │  │                     │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                       commands crate                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│  │ Commit  │ │ Doctor  │ │ Config  │ │  Help   │ │ Status │ │
│  │Command  │ │Command  │ │Command  │ │Command  │ │Command │ │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └────────┘ │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                        tools crate                           │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│  │FileRead │ │FileWrite│ │FileEdit │ │  Glob   │ │  Grep  │ │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘ └────────┘ │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌────────┐ │
│  │  Bash   │ │WebFetch │ │WebSearch│ │Notebook │ │ Task*  │ │
│  └─────────┘ └─────────┘ └─────────┘ │  Edit   │ │        │ │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ └─────────┘ └────────┘ │
│  │  Agent  │ │SendMsg  │ │  Todo   │                        │
│  └─────────┘ └─────────┘ └─────────┘                        │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                       runtime crate                          │
│  ┌─────────────────┐  ┌─────────────────────────────────────┐ │
│  │  ToolRegistry   │  │           Tool Trait               │ │
│  │   (DashMap)     │  │  ┌─────────┐ ┌──────────────────┐  │ │
│  │                 │  │  │metadata │ │  validate()      │  │ │
│  └─────────────────┘  │  │         │ │  execute()       │  │ │
│                       │  └─────────┘ └──────────────────┘  │ │
│  ┌─────────────────┐  └─────────────────────────────────────┘ │
│  │  Message Types  │                                          │
│  │  (User/Assistant│                                          │
│  │   /System/Tool) │                                          │
│  └─────────────────┘                                          │
└─────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────┐
│                     test-utils crate                         │
│  ┌─────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ TempDirFixture  │  │   MockTool      │  │  Assertions  │ │
│  │   (tempfile)    │  │ (test doubles)  │  │  (helpers)   │ │
│  └─────────────────┘  └─────────────────┘  └──────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Implemented Tools (25+)

### File Operations
- **FileReadTool**: Read text, images, with offset/limit
- **FileWriteTool**: Create/overwrite files, parent dir creation
- **FileEditTool**: String replacement with diff generation
- **NotebookEditTool**: Jupyter notebook cell editing

### Search
- **GlobTool**: Pattern-based file finding
- **GrepTool**: Regex search with context/count modes

### System
- **BashTool**: Shell execution with timeout, working dir, env
- **LspTool**: Language Server Protocol integration (goToDefinition, findReferences, hover)
- **ConfigTool**: Settings management

### Task Management
- **TaskCreateTool**: Create tasks with metadata
- **TaskUpdateTool**: Update task status/owner
- **TaskListTool**: List with status/owner filters
- **TaskGetTool**: Get task details
- **TaskOutputTool**: Store/retrieve task output

### Agent System
- **AgentTool**: Spawn sub-agents with color coding (8 colors)
- **SendMessageTool**: Inter-agent messaging (4 priority levels)

### Todo Management
- **TodoWriteTool**: Create/update todo lists

### Web
- **WebFetchTool**: HTTP GET with content extraction
- **WebSearchTool**: Search engine integration

### MCP (Model Context Protocol)
- **McpTool**: Invoke MCP servers
- **McpRegistryTool**: Manage MCP server connections
- **ListMcpResourcesTool**: List available resources
- **ReadMcpResourceTool**: Read resource content

## TUI Widgets (Phase 8 Complete)

### Input Widgets
- **PromptWidget**: Input with history, cursor positioning, placeholder
- **MultilineInputWidget**: Multi-line text with navigation
- **ShortcutHelpWidget**: Keyboard shortcut display

### Status Widgets
- **StatusBarWidget**: Mode, model, tokens, cwd, git branch display
- **TypingIndicatorWidget**: Animated "Claude is thinking..." dots
- **SpinnerWidget**: Dots/line/braille animation variants
- **ProgressWidget**: Progress bar with percentage
- **ToolStatusWidget**: Tool execution state with spinner

## Testing

```bash
# Run all tests (single-threaded configured in .cargo/config.toml)
cargo test --workspace

# Or use the alias
cargo t
```

**Test Count:**
- runtime: 90
- tools: 147
- commands: 34
- cli: 57 TUI widget tests
- test-utils: 6
- integration: 30
- compat-harness: 3
- **Total: 367+ passing**

## Key Design Decisions

1. **Tool Trait**: Async with `async_trait`, returning `ToolOutput`
2. **Registry**: `DashMap` for concurrent access, `Arc<dyn Tool>` storage
3. **TUI**: ratatui with `MessageWidget` enum for different message types
4. **Test Isolation**: `TempDirFixture` for temp files, `MockTool` for doubles
5. **Task Store**: Global singleton with `OnceLock`, mutex-recovery for tests

## Remaining Work (Phase 5-14 from Plan)

### In Progress
- Phase 5: Advanced Commands (/tasks, /skills, /teleport, /init) - operator dispatched
- Phase 6: QueryEngine LLM Integration (streaming, tool-call loop) - operator dispatched
- Phase 7: Services Layer (API, MCP, OAuth, Analytics clients) - operator dispatched

### Completed Recently
- Phase 2: Core Tools (File, Bash, Search, Web) - 147 tools tests passing
- Phase 3: Advanced Tools (MCP, LSP, Agent, Config) - COMPLETE
- Phase 4: Core Commands (/commit, /doctor, /diff, /cost, /review, /compact, /status) - 34 tests, COMPLETE
- Phase 8: TUI Components (PromptWidget, StatusBarWidget, Spinner, Progress) - 57 tests, COMPLETE
- Runtime test fixes - 90 tests passing (was 44 with 12 compilation errors)

### Core Systems (Pending)
- QueryEngine with LLM orchestration (Phase 6 in progress)
- Service layer (API, OAuth, Analytics) (Phase 7 in progress)
- Bridge system (IDE integration)
- Coordinator (multi-agent)
- State & Persistence
- Permissions System

## Notes

**Recent Fixes:**
- Fixed runtime test compilation errors (86 → 0 errors)
- Restored commands/src/lib.rs after accidental overwrite (34 tests restored)
- Fixed compat-harness API compatibility with LegacyCommandRegistry
- Fixed MCP store test isolation issues (changed `reset_mcp_store` to ensure initialization)
- Fixed SystemMessage API in tests (changed from `content` to `subtype` field)
- Fixed 3 mcp_registry tests to handle shared state (test_mcp_tool_list_servers, test_mcp_registry_unregister_server, test_mcp_registry_list_servers)

**MCP Review Findings (Completed):**

*From R.A.D.1.C.A.1 (Public - 113 tools):*
- Plugin trait with lifecycle (init/start/stop/health)
- Bridge pattern for Tool/Skill adaptation
- Capability-based security model
- JSON Schema validation for tool parameters
- Project context auto-discovery

*From R.A.D.1.C.L-private (Advanced Patterns):*
- **Tiered Tool Visibility** (PUBLIC/INTERNAL/ADMIN) - solves tool overload
- **Two-Tier Specialist System** (3b always-loaded, 7b on-demand)
- **Reflex Arc** - mandatory snapshot-before-write safety
- **Tool Dispatcher** - central routing hub with visibility control
- **Agentic RAG** with CRAG/GRASP self-correcting retrieval
- **Swarm V2.0** - polyglot multi-agent with Phoenix Protocol recovery
- **A2A Protocol** - Google Agent-to-Agent implementation

**Potential Adoptions for R.A.D Codicological 2.x:**
- ToolDispatcher for unified tool routing
- ReflexArc for file safety (snapshot-before-write)
- Two-tier model system for performance
- Agentic RAG pipeline for memory/context
