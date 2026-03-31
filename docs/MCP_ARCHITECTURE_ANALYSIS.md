# R.A.D.1.C.A.1 MCP Architecture Analysis

**Source**: mcp-reviewer-public agent analysis
**Date**: 2026-03-31
**Tools Found**: 113

---

## Core Architecture Patterns

### 1. Plugin System with Skill-Based Capabilities

The R.A.D.1.C.A.1 uses a plugin-based architecture where tools are exposed as "skills":

```rust
trait Plugin {
    fn init(&self, config: PluginConfig) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn stop(&self) -> Result<()>;
    fn health(&self) -> HealthStatus;
    fn list_skills(&self) -> Vec<SkillDescriptor>;
    fn invoke_skill(&self, request: SkillRequest) -> Result<SkillResponse>;
}
```

**SkillDescriptor includes:**
- JSON Schema for input/output validation
- Cost hints: `latency_ms`, `memory_mb`
- Capability-based permissions (FileRead, Network, etc.)
- Dependency graph for topological startup ordering

### 2. Bridge Pattern for Tool Registration

```rust
struct PluginToolBridge {
    plugin: Arc<dyn Plugin>,
    skill_id: SkillId,
    skill_descriptor: SkillDescriptor,
}

impl Tool for PluginToolBridge {
    async fn execute(&self, args: Value) -> Result<Value> {
        self.plugin.invoke_skill(request).await
    }
}
```

This allows dynamic plugin tools to integrate with the tool registry seamlessly.

### 3. Error Categorization System

```rust
enum PluginErrorCategory {
    Fatal,           // Plugin must be unloaded
    Recoverable,     // Can retry with backoff
    Configuration,   // Fix config and reload
    Security,        // Permission violation
}

enum Severity {
    Critical, High, Medium, Low
}
```

Enables appropriate handling strategies based on error type.

### 4. Dual Registration Strategy

- **Dynamic-first**: Load from `plugins/` directory at startup
- **Fallback**: Hard-coded router plugins if none loaded
- **Duplicate prevention**: Track registered tool names, skip duplicates
- **Hot-reload support**: Watch directory, load/unload on changes

### 5. Input Validation Pattern

```rust
#[derive(Deserialize, Validate)]
struct ReadFileArgs {
    #[validate(length(min = 1, max = 4096))]
    path: String,
    #[validate(range(min = 1, max = 10_000_000))]
    offset: Option<usize>,
    #[validate(range(min = 1, max = 100_000))]
    limit: Option<usize>,
}
```

Uses `validator` crate with derive macros for declarative validation.

### 6. MCP Protocol Implementation

**Transports:**
- SSE: `GET /mcp/sse` (Server-Sent Events)
- WebSocket: `GET /mcp/ws`
- Streamable HTTP: `POST /mcp`

**Format:** JSON-RPC 2.0
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "read_file",
    "arguments": {"path": "/etc/hosts"}
  }
}
```

**Session Management:** `DashMap<String, mpsc::Sender<Message>>`

---

## Tool Categories (113 Total)

### File System Operations (4 tools)
- `read_file` - Read file contents with offset/limit
- `write_file` - Write/overwrite files
- `edit_file` - Partial string replacement
- `list_directory` - List directory contents

### Git Operations (8 tools)
- `git_status` - Working tree status
- `git_diff` - Show changes
- `git_log` - Commit history
- `git_commit` - Create commit
- `git_push` - Push to remote
- `git_pull` - Pull from remote
- `git_branch` - Branch operations
- `git_checkout` - Switch branches

### Code Analysis (6 tools)
- `get_file_symbols` - Extract symbols via Tree-sitter
- `find_references` - Find symbol references
- `code_rag_search` - RAG search on code
- `code_parse` - Parse source to AST
- `kg_lookup` - Knowledge graph symbol lookup
- `kg_relationships` - Find symbol relationships

### Memory & Context (10 tools)
- `save_to_memory` - Store with embeddings
- `recall_memory` - Semantic search
- `search_memory` - Vector similarity search
- `memory_stats` - Storage statistics
- `compact_session` - Compress old context
- `should_compact` - Check compaction threshold
- `auto_checkpoint` - Save checkpoint
- `get_checkpoint` - Retrieve checkpoint
- `list_checkpoints` - List all checkpoints
- `delete_checkpoint` - Remove checkpoint

### Knowledge Graph (5 tools)
- `kg_lookup` - Symbol lookup
- `kg_relationships` - Traversal search
- `kg_community_search` - Cluster detection
- `kg_validate` - Integrity checking
- `multi_hop_search` - Multi-hop graph traversal

### RAG Operations (8 tools)
- `smart_search` - Auto-routed search (code/docs)
- `agentic_search` - Self-correcting search
- `multi_query_search` - Reciprocal rank fusion
- `search_with_hyde` - HyDE search
- `raptor_search` - RAPTOR tree search
- `kg_hybrid_search` - KG + RAG combined
- `build_raptor_tree` - Build RAPTOR hierarchy
- `crag_search` - Corrective RAG with web fallback

### Query Enhancement (5 tools)
- `query_enhance` - Multi-query + HyDE combined
- `analyze_query_complexity` - Complexity analysis
- `classify_query` - Intent classification
- `recommend_operator` - Operator selection
- `critique_answer` - Self-RAG evaluation

### Session Management (6 tools)
- `create_session` - New development session
- `update_session` - Modify session
- `get_session_summary` - Comprehensive summary
- `list_sessions_needing_compaction` - Find stale sessions
- `add_session_event` - Log activity
- `get_recent_context` - Recent session context

### Checkpoints (14 tools)
- `create_checkpoint` - Save compressed context
- `get_checkpoint` - Retrieve by ID
- `list_checkpoints` - List with filters
- `delete_checkpoint` - Soft/hard delete
- `archive_checkpoints` - Cleanup old data
- `compare_checkpoints` - Show differences
- `restore_checkpoint` - Restore file state
- + 7 more checkpoint utilities

### Snapshots (6 tools)
- `snapshot_file` - Create backup
- `restore_snapshot` - Restore from backup
- `get_snapshot_stats` - Usage statistics
- `list_snapshots` - List all snapshots
- `delete_snapshot` - Remove snapshot
- `enforce_snapshot` - Apply safety rules

### Task Management (8 tools)
- `create_mission_task` - Create kanban task
- `update_mission_task` - Modify task
- `todo_write` - Sync todos
- `todo_read` - Read todo list
- `create_failure` - Record failures
- `resolve_failure` - Mark resolved
- `get_prevention_rules` - Get guardrails
- `record_deployment` - DORA deployment tracking

### Agent Orchestration (3 tools)
- `run_agent_task` - Single agent dispatch
- `run_swarm` - Multi-agent swarm
- `dispatch_to_specialist` - A2A specialist routing

### Skills Framework (3 tools)
- `list_skills` - Available skills
- `load_skill` - Load by name
- `skill_resources` - Get resource paths

### Specialist Dispatch (5 tools)
- `recommend_specialist` - Best agent for task
- `get_specialist_profiles` - All specialists
- `get_specialist_status` - Health/metrics
- `warmup_specialist` - Pre-warm models
- `dispatch_operator` - Operator selection

### Mission Control (12 tools)
- `get_mission_status` - System health
- `get_project_context` - Project state
- `get_active_failures` - Unresolved failures
- `create_session` / `update_session` / `add_session_event`
- `sync_todos` - Todo synchronization
- `create_mission_task` - Task creation
- `update_mission_task` - Task updates
- `record_deployment` - Deployment logging
- `record_deployment_failure` - Failure logging
- `record_incident` / `resolve_incident` - Incident tracking

### DORA Metrics (5 tools)
- `get_dora_metrics` - Calculate metrics
- `record_deployment` - Track deployments
- `record_deployment_failure` - Track failures
- `record_incident` - Track incidents
- `resolve_incident` - Mark resolved

### Operators (6 tools)
- `list_operators` - Available operators
- `operator_health` - Health check
- `dispatch_operator` - Task dispatch
- `cancel_task` - Cancel running task
- `get_task_status` - Task status
- `get_task_progress_history` - Progress tracking

### Daemon Operations (3 tools)
- `daemon_start` - Start background daemon
- `daemon_stop` - Stop daemon
- `daemon_status` - Check status

### GitHub Integration (2 tools)
- `github_pr_create` - Create pull request
- `github_issue_list` - List issues

---

## Unique Features for R.A.D Codicological 2.x

### 1. Trident Memory Compaction
3-stage process for context management:
- **Supersede**: Replace old similar documents
- **Collapse**: Compress multiple docs into summary
- **Cluster**: Group related documents

Plus 90-day lifecycle: compress → archive → retire

### 2. Knowledge Graph
- Tree-sitter symbol extraction
- Multi-hop relationship traversal
- Community detection (clusters)
- RAG + graph hybrid search

### 3. DORA Metrics Tracking
- Deployment frequency
- Lead time for changes
- Mean time to recovery (MTTR)
- Change failure rate

### 4. Background Daemon
9 concurrent jobs:
1. `git_watch` - Monitor repo changes
2. `raptor_enrich` - Build RAPTOR summaries
3. `memory_compact` - Compress old sessions
4. `kg_build` - Update knowledge graph
5. `faiss_sync` - Sync vector index
6. `model_warmup` - Keep models hot
7. `auto_checkpoint` - Periodic checkpoints
8. + 2 more internal jobs

### 5. A2A Protocol Support
- OAuth 2.0 authentication
- Webhooks for async callbacks
- SSE for streaming updates
- JSON-RPC 2.0 message format

### 6. Path Traversal Protection
```rust
fn validate_path(path: &str, root_dir: &Path) -> Result<PathBuf> {
    let canonical = Path::new(path).canonicalize()?;
    if !canonical.starts_with(root_dir) {
        return Err(Error::PathEscape);
    }
    Ok(canonical)
}
```

---

## Key Insights for Implementation

1. **Plugin system enables extensibility** without recompiling core
2. **Bridge pattern** allows gradual migration from static to dynamic tools
3. **MCP protocol** provides standardized tool interface
4. **Background daemon** handles expensive operations asynchronously
5. **DORA metrics** enable data-driven improvement tracking
6. **Memory compaction** critical for long-running sessions
7. **Knowledge graph** adds semantic understanding beyond text search

---

*Analysis by mcp-reviewer-public agent*
*Saved to project: 2026-03-31*
