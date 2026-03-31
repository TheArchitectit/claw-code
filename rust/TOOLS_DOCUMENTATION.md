# R.A.D Codicological 2.x - Tool System Documentation

**Location**: `/mnt/ollama/git/claw-code/rust/crates/tools/src/`
**Last Updated**: March 31, 2026
**Total Tools Documented**: 26

---

## Table of Contents

1. [Overview](#overview)
2. [Core Architecture](#core-architecture)
3. [Tool Inventory](#tool-inventory)
   - [File Operations](#file-operations)
   - [System Tools](#system-tools)
   - [Web Tools](#web-tools)
   - [Planning & Task Management](#planning--task-management)
   - [Advanced Tools](#advanced-tools)
4. [Architecture Patterns](#architecture-patterns)
5. [Usage Examples](#usage-examples)
6. [QueryEngine Integration](#queryengine-integration)

---

## Overview

The R.A.D Codicological Tool System is a modular, extensible framework for providing LLM agents with safe, controlled access to system resources. All tools implement the core `Tool` trait and are designed with thread safety, validation, and clear error handling in mind.

### Key Design Principles

- **Two-phase execution**: Validation (sync) → Execution (async)
- **Builder patterns** for inputs and outputs
- **Global state management** via `OnceLock<Arc<Mutex<Store>>>` singletons
- **Read-only vs. mutable** distinction via `ToolMetadata`
- **Concurrency safety** flags for parallel execution control

---

## Core Architecture

### The Tool Trait

All tools implement the `Tool` trait defined in `tool.rs`:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn metadata(&self) -> &ToolMetadata;
    async fn validate(&self, input: &ToolInput) -> ToolResult<()>>;
    async fn execute(&self, input: ToolInput) -> ToolOutput;
    fn name(&self) -> &str { &self.metadata().name }
}
```

### ToolMetadata

Static metadata using `OnceLock` for lazy initialization:

```rust
pub struct ToolMetadata {
    pub name: String,
    pub description: String,
    pub is_read_only: bool,        // Does not modify filesystem
    pub is_concurrency_safe: bool, // Can run concurrently
}
```

### ToolInput / ToolOutput

Generic data containers using `HashMap<String, serde_json::Value>`:

```rust
// Input construction
let input = ToolInput::new()
    .with_arg("file_path", "/path/to/file")
    .with_arg("limit", 100u32);

// Output construction
ToolOutput::new()
    .with_field("content", file_content)
    .with_field("success", true)
    .with_truncated(true)
```

### ToolError Enum

Comprehensive error handling:

```rust
pub enum ToolError {
    ValidationFailed { message: String, error_code: Option<u32> },
    ExecutionFailed { message: String },
    PermissionDenied { message: String },
    NotFound { message: String },
    Timeout { timeout_ms: u64 },
    Internal { message: String },
    Cancelled,
}
```

---

## Tool Inventory

### File Operations

#### FileReadTool (`file_read.rs`)

**Purpose**: Read text files with line limits, handle images, block dangerous paths.

**Input Schema**:
```rust
pub struct FileReadInput {
    pub file_path: String,
    pub offset: Option<usize>,      // 1-indexed starting line
    pub limit: Option<usize>,       // Max lines to read
}
```

**Output Schema**:
```rust
pub struct TextFileOutput {
    pub file_path: String,
    pub content: String,
    pub num_lines: usize,
    pub start_line: usize,
    pub total_lines: usize,
}
```

**Safety Features**:
- Blocks device files (`/dev/zero`, `/dev/random`, etc.)
- 10 MB size limit for text files
- Binary file detection and blocking
- Image file type detection

**Thread Safety**: Read-only (`is_read_only = true`)

---

#### FileWriteTool (`file_write.rs`)

**Purpose**: Create or overwrite files with automatic parent directory creation.

**Input Schema**:
```rust
pub struct FileWriteInput {
    pub file_path: String,
    pub content: String,
}
```

**Output Schema**:
```rust
pub struct FileWriteOutput {
    pub operation: String,          // "create" or "update"
    pub file_path: String,
    pub content: String,
    pub structured_patch: Option<Vec<PatchHunk>>,
    pub original_content: Option<String>,
}
```

**Key Implementation Details**:
- Auto-creates parent directories
- Generates diff output for updates
- 50 MB write size limit

**Thread Safety**: Not concurrency-safe (`is_concurrency_safe = false`)

---

#### FileEditTool (`file_edit.rs`)

**Purpose**: Modify files by string replacement with support for partial edits.

**Input Schema**:
```rust
pub struct FileEditInput {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    pub replace_all: Option<bool>,
}
```

**Output Schema**:
```rust
pub struct FileEditOutput {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    pub original_content: Option<String>,
    pub structured_patch: Vec<EditPatchHunk>,
    pub replace_all: bool,
}
```

**Key Implementation Details**:
- Quote normalization (handles curly/smart quotes)
- Multiple match detection (requires `replace_all` for multiple occurrences)
- Can create new files when `old_string` is empty
- 1 GB maximum file size

**Thread Safety**: Not concurrency-safe

---

#### GlobTool (`glob.rs`)

**Purpose**: Find files by name pattern using glob syntax.

**Input Schema**:
```rust
pub struct GlobInput {
    pub pattern: String,            // e.g., "*.rs", "**/*.md"
    pub path: Option<String>,       // Directory to search in
}
```

**Output Schema**:
```rust
pub struct GlobOutput {
    pub duration_ms: u64,
    pub num_files: usize,
    pub filenames: Vec<String>,
    pub truncated: bool,            // True if results exceeded MAX_RESULTS (100)
}
```

**Thread Safety**: Read-only

---

#### GrepTool (`grep.rs`)

**Purpose**: Search file contents with regex patterns.

**Input Schema**:
```rust
pub struct GrepInput {
    pub pattern: String,
    pub path: Option<String>,
    pub glob: Option<String>,
    pub output_mode: Option<String>,  // "content", "files_with_matches", "count"
    pub before_context: Option<usize>, // -B alias
    pub after_context: Option<usize>,  // -A alias
    pub context: Option<usize>,       // -C alias
    pub show_line_numbers: bool,      // -n (default true)
    pub case_insensitive: bool,         // -i
    pub file_type: Option<String>,
    pub head_limit: Option<usize>,
    pub offset: Option<usize>,
    pub multiline: Option<bool>,
}
```

**Output Schema**:
```rust
pub struct GrepOutput {
    pub mode: String,
    pub num_files: usize,
    pub filenames: Vec<String>,
    pub content: Option<String>,
    pub num_lines: Option<usize>,
    pub num_matches: Option<usize>,
    pub applied_limit: Option<usize>,
    pub applied_offset: Option<usize>,
}
```

**Key Implementation Details**:
- VCS directory exclusion (.git, .svn, etc.)
- 10 MB file size limit
- Type filtering (rust, python, javascript, etc.)

**Thread Safety**: Read-only

---

#### NotebookEditTool (`notebook_edit.rs`)

**Purpose**: Edit Jupyter notebook (.ipynb) cells.

**Input Schema**:
```rust
pub struct NotebookEditInput {
    pub notebook_path: String,
    pub cell_id: Option<String>,
    pub new_source: String,
    pub cell_type: Option<String>,   // "code", "markdown", "raw"
    pub edit_mode: Option<String>,   // "replace", "insert", "delete"
}
```

**Output Schema**:
```rust
pub struct NotebookEditOutput {
    pub new_source: String,
    pub cell_id: Option<String>,
    pub cell_type: String,
    pub language: String,
    pub edit_mode: String,
    pub notebook_path: String,
    pub original_file: String,
    pub updated_file: String,
}
```

**Key Implementation Details**:
- Supports nbformat 4.x with cell IDs
- Resets execution count on code cell edits
- 10 MB notebook size limit

**Thread Safety**: Not concurrency-safe

---

### System Tools

#### BashTool (`bash.rs`)

**Purpose**: Execute shell commands safely with timeout support.

**Input Schema**:
```rust
pub struct BashInput {
    pub command: String,
    pub timeout: Option<u64>,         // Milliseconds (default 5 min, max 30 min)
    pub description: Option<String>,
    pub run_in_background: Option<bool>,
}
```

**Output Schema**:
```rust
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub interrupted: bool,
    pub exit_code: Option<i32>,
    pub duration_ms: Option<u64>,
}
```

**Safety Features**:
- Dangerous pattern detection (`rm -rf /`, fork bombs, etc.)
- Timeout handling with tokio
- Background execution support

**Thread Safety**: Not concurrency-safe

---

### Planning & Task Management

#### TaskStore (`task_store.rs`)

**Purpose**: Shared in-memory storage for task management.

**Core Types**:
```rust
pub enum TaskStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

pub struct Task {
    pub id: String,
    pub subject: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub owner: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub metadata: Option<serde_json::Value>,
}

pub struct TaskStore {
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    outputs: Arc<Mutex<HashMap<String, String>>>,
}
```

**Global Access**:
```rust
static GLOBAL_TASK_STORE: OnceLock<TaskStore> = OnceLock::new();
pub fn get_task_store() -> TaskStore { ... }
```

---

#### TaskCreateTool (`task_create.rs`)

**Purpose**: Create new tasks in the task management system.

**Input Schema**:
```rust
pub struct TaskCreateInput {
    pub subject: String,
    pub description: Option<String>,
    pub status: Option<String>,       // "pending", "in_progress", "completed", "cancelled"
    pub owner: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
```

**Output Schema**:
```rust
pub struct TaskCreateOutput {
    pub task_id: String,
    pub subject: String,
    pub status: String,
    pub created_at: u64,
}
```

**Thread Safety**: Not concurrency-safe

---

#### TaskGetTool (`task_get.rs`)

**Purpose**: Get detailed information about a specific task.

**Input Schema**:
```rust
pub struct TaskGetInput {
    pub task_id: String,
}
```

**Output**: Full `TaskDetails` including description, timestamps, metadata.

**Thread Safety**: Read-only

---

#### TaskListTool (`task_list.rs`)

**Purpose**: List all tasks with optional filtering.

**Input Schema**:
```rust
pub struct TaskListInput {
    pub status: Option<String>,
    pub owner: Option<String>,
}
```

**Output Schema**:
```rust
pub struct TaskListOutput {
    pub tasks: Vec<TaskSummary>,
    pub total: usize,
}
```

**Thread Safety**: Read-only

---

#### TaskUpdateTool (`task_update.rs`)

**Purpose**: Update task status and properties.

**Input Schema**:
```rust
pub struct TaskUpdateInput {
    pub task_id: String,
    pub status: Option<String>,
    pub subject: Option<String>,
    pub description: Option<String>,
    pub owner: Option<String>,
    pub metadata: Option<serde_json::Value>,
}
```

**Output Schema**:
```rust
pub struct TaskUpdateOutput {
    pub task_id: String,
    pub updated_fields: Vec<String>,
    pub new_status: Option<String>,
}
```

**Thread Safety**: Not concurrency-safe

---

#### TaskOutputTool (`task_output.rs`)

**Purpose**: Store and retrieve task output content.

**Input Schema**:
```rust
pub struct TaskOutputInput {
    pub task_id: String,
    pub content: Option<String>,
    pub retrieve: Option<bool>,
    pub append: Option<bool>,
}
```

**Output Schemas**:
```rust
pub struct TaskStoreOutput {
    pub task_id: String,
    pub stored: bool,
    pub bytes_written: usize,
}

pub struct TaskRetrieveOutput {
    pub task_id: String,
    pub content: String,
    pub has_output: bool,
}
```

**Thread Safety**: Not concurrency-safe

---

#### TodoStore (`todo_store.rs`)

**Purpose**: Shared in-memory storage for session-level todos.

**Core Types**:
```rust
pub enum TodoStatus {
    Pending, InProgress, Completed, Cancelled,
}

pub enum TodoPriority {
    Low, Medium, High, Urgent,
}

pub struct Todo {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
    pub priority: TodoPriority,
    pub active_form: Option<String>, // Present continuous description
    pub created_at: u64,
    pub updated_at: u64,
    pub metadata: Option<serde_json::Value>,
}
```

**Key Features**:
- Sorted listing by status and priority
- Batch operations support
- Completion tracking

---

#### TodoWriteTool (`todo_write.rs`)

**Purpose**: Manage session-level todo lists with batch operations.

**Input Schema**:
```rust
pub struct TodoInput {
    pub id: Option<String>,
    pub content: String,
    pub status: Option<String>,
    pub priority: Option<String>,
    pub active_form: Option<String>,
}

pub struct TodoWriteInput {
    pub todos: Vec<TodoInput>,
    pub replace_all: Option<bool>,
}
```

**Output Schema**:
```rust
pub struct TodoWriteOutput {
    pub old_todos: Vec<TodoOutputItem>,
    pub new_todos: Vec<TodoOutputItem>,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub all_completed: bool,
}
```

**Thread Safety**: Not concurrency-safe

---

### Web Tools

#### WebSearchTool (`web_search.rs`)

**Purpose**: Search the web using DuckDuckGo Lite.

**Input Schema**:
```rust
pub struct WebSearchInput {
    pub query: String,
    pub num_results: Option<usize>,
}
```

**Output Schema**:
```rust
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub struct WebSearchOutput {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub success: bool,
    pub error: Option<String>,
}
```

**Key Implementation Details**:
- Uses DuckDuckGo Lite (HTML scraping)
- 30-second timeout
- HTML parsing with `scraper` crate

**Thread Safety**: Read-only

---

#### WebFetchTool (`web_fetch.rs`)

**Purpose**: Fetch web pages and extract readable content.

**Input Schema**:
```rust
pub struct WebFetchInput {
    pub url: String,
    pub max_length: Option<usize>,      // Default 10,000 chars
    pub extract_content: Option<bool>,  // Default true
}
```

**Output Schema**:
```rust
pub struct WebFetchOutput {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub content_type: String,
    pub status_code: u16,
    pub success: bool,
    pub error: Option<String>,
}
```

**Content Extraction**:
- Tries selectors: `main`, `article`, `[role='main']`, `.content`, `#content`, etc.
- Extracts text from: `p`, `h1-6`, `li`, `pre`, `code`
- Falls back to body text if no structured content found

**Thread Safety**: Read-only

---

### Advanced Tools

#### AgentTool (`agent.rs`)

**Purpose**: Spawn and manage sub-agents with color coding.

**Core Types**:
```rust
pub enum AgentColor {
    Blue, Pink, Green, Yellow, Red, Purple, Orange, Cyan,
}

pub enum AgentStatus {
    Idle, Running, Completed, Failed, Stopped,
}

pub struct AgentInfo {
    pub agent_id: String,
    pub name: String,
    pub color: String,
    pub description: String,
    pub status: AgentStatus,
    pub parent_agent: Option<String>,
    pub created_at: u64,
    pub started_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub result: Option<String>,
    pub error: Option<String>,
}
```

**Actions**: `create`, `run`, `stop`, `status`, `list`

**Global Store**: `GLOBAL_AGENT_STORE: OnceLock<AgentStore>`

**Thread Safety**: Not concurrency-safe

---

#### SendMessageTool (`send_message.rs`)

**Purpose**: Inter-agent messaging system with priority queue.

**Core Types**:
```rust
pub enum MessagePriority {
    Low, Normal, High, Urgent,
}

pub struct Message {
    pub message_id: String,
    pub from: String,
    pub to: String,
    pub content: String,
    pub priority: String,
    pub timestamp: u64,
    pub read: bool,
    pub message_type: String,
}
```

**Actions**: `send`, `receive`, `peek`, `list`, `mark_read`

**Key Features**:
- Priority-ordered message queues
- Persistent per-agent queues
- Message preview in summaries (100 char limit)

**Global Store**: `GLOBAL_MESSAGE_STORE: OnceLock<MessageStore>`

---

#### McpTool (`mcp.rs`)

**Purpose**: Invoke MCP (Model Context Protocol) servers using JSON-RPC 2.0.

**Core Types**:
```rust
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

pub struct McpServerConfig {
    pub name: String,
    pub url: String,
    pub auth_token: Option<String>,
    pub enabled: bool,
}
```

**Actions**: `invoke`, `discover`, `list_servers`

**Key Implementation Details**:
- Atomic ID generation with `AtomicU64`
- Bearer token authentication
- 30-second request timeout

**Global Store**: `GLOBAL_MCP_STORE: OnceLock<McpStore>`

---

#### McpRegistryTool (`mcp_registry.rs`)

**Purpose**: Manage MCP resources - list, read, cache MCP server resources.

**Actions**: `list`, `read`, `register_server`, `unregister_server`, `list_servers`

**Features**:
- Resource caching with `McpResourceStore`
- Content caching with timestamp tracking
- URI-based resource addressing

**Global Stores**: `GLOBAL_MCP_STORE`, `GLOBAL_MCP_RESOURCE_STORE`

---

#### LspTool (`lsp.rs`)

**Purpose**: Language Server Protocol integration for code intelligence.

**Operations**:
- `goToDefinition`: Navigate to symbol definitions
- `findReferences`: Find all references to a symbol
- `hover`: Get type information and documentation
- `documentSymbols`: Extract symbols from a document

**Auto-Detection**: Maps file extensions to LSP servers:
- `.rs` → `rust-analyzer`
- `.js/.ts/.jsx/.tsx` → `typescript-language-server`
- `.py` → `pylsp`
- `.go` → `gopls`
- `.c/.cpp/.h/.hpp` → `clangd`
- `.java` → `jdtls`
- `.rb` → `solargraph`
- `.php` → `intelephense`

**Fallback**: Regex-based symbol extraction for common languages.

**Thread Safety**: Read-only

---

#### ConfigTool (`config.rs`)

**Purpose**: Read and write configuration files (JSON, YAML, TOML, INI).

**Operations**: `read`, `write`, `get`, `set`

**Input Schema**:
```rust
pub struct ConfigInput {
    pub file_path: String,
    pub operation: String,
    pub format: Option<String>,     // "json", "yaml", "toml", "ini", "auto"
    pub key: Option<String>,        // Dot-notation path like "database.host"
    pub value: Option<serde_json::Value>,
    pub data: Option<serde_json::Value>,
    pub create_dirs: Option<bool>,
}
```

**Format Auto-Detection**: From file extension (.json, .yaml, .yml, .toml, .ini, .conf)

---

## Architecture Patterns

### 1. Global Store Pattern

All stateful tools use a singleton pattern for cross-invocation persistence:

```rust
use std::sync::{Arc, Mutex, OnceLock};

static GLOBAL_STORE: OnceLock<Store> = OnceLock::new();

pub fn get_store() -> Store {
    GLOBAL_STORE.get_or_init(Store::new).clone()
}

pub fn reset_store() {
    if let Some(store) = GLOBAL_STORE.get() {
        store.clear();
    }
}
```

### 2. Two-Phase Validation/Execution

All tools follow strict separation of concerns:

```rust
#[async_trait]
impl Tool for MyTool {
    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        // 1. Check required fields
        // 2. Validate formats/types
        // 3. Check preconditions (file exists, etc.)
        // NO I/O operations except for validation checks
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        // 1. Perform the actual operation
        // 2. Handle errors gracefully
        // 3. Return structured output
    }
}
```

### 3. Builder Pattern for Data Construction

```rust
// ToolInput construction
let input = ToolInput::new()
    .with_arg("file_path", "/path/to/file")
    .with_arg("limit", 100u32);

// ToolOutput construction
ToolOutput::new()
    .with_field("content", data)
    .with_field("success", true)
    .with_truncated(false)
```

### 4. Test Synchronization

All store-based tests use a global mutex to prevent race conditions:

```rust
#[cfg(test)]
pub static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn setup() -> std::sync::MutexGuard<'static, ()> {
    let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    reset_store();
    guard
}
```

### 5. Error Code System

Validation errors include numeric error codes for programmatic handling:

```rust
Err(ToolError::ValidationFailed {
    message: "Invalid status: pending. Valid: in_progress, completed".to_string(),
    error_code: Some(3),  // Specific error identifier
})
```

---

## Usage Examples

### File Operations

```rust
// Reading a file
let tool = FileReadTool::new();
let input = ToolInput::new()
    .with_arg("file_path", "/path/to/file.rs")
    .with_arg("offset", 1u64)
    .with_arg("limit", 50u64);
let output = tool.execute(input).await;

// Writing a file
let tool = FileWriteTool::new();
let input = ToolInput::new()
    .with_arg("file_path", "/path/to/output.txt")
    .with_arg("content", "Hello, World!");
let output = tool.execute(input).await;

// Editing a file
let tool = FileEditTool::new();
let input = ToolInput::new()
    .with_arg("file_path", "/path/to/file.rs")
    .with_arg("old_string", "fn old_name()")
    .with_arg("new_string", "fn new_name()")
    .with_arg("replace_all", false);
let output = tool.execute(input).await;

// Glob search
let tool = GlobTool::new();
let input = ToolInput::new()
    .with_arg("pattern", "**/*.rs")
    .with_arg("path", "/project/src");
let output = tool.execute(input).await;

// Grep search
let tool = GrepTool::new();
let input = ToolInput::new()
    .with_arg("pattern", "pub fn main")
    .with_arg("path", "/project/src")
    .with_arg("output_mode", "content")
    .with_arg("context", 3u64);
let output = tool.execute(input).await;
```

### Task Management

```rust
// Create a task
let tool = TaskCreateTool::new();
let input = ToolInput::new()
    .with_arg("subject", "Implement feature X")
    .with_arg("description", "Detailed description")
    .with_arg("status", "pending")
    .with_arg("owner", "agent-1");
let output = tool.execute(input).await;
let task_id = output.data.get("task_id").unwrap().as_str().unwrap();

// Update task status
let tool = TaskUpdateTool::new();
let input = ToolInput::new()
    .with_arg("task_id", task_id)
    .with_arg("status", "in_progress");
let output = tool.execute(input).await;

// List tasks
let tool = TaskListTool::new();
let input = ToolInput::new()
    .with_arg("status", "pending");
let output = tool.execute(input).await;
```

### Agent Management

```rust
// Create an agent
let tool = AgentTool::new();
let input = ToolInput::new()
    .with_arg("action", "create")
    .with_arg("name", "Research Agent")
    .with_arg("color", "blue")
    .with_arg("description", "Research implementation details");
let output = tool.execute(input).await;

// Run an agent
let input = ToolInput::new()
    .with_arg("action", "run")
    .with_arg("name", "Worker Agent")
    .with_arg("color", "green")
    .with_arg("description", "Process data files");
let output = tool.execute(input).await;
```

### Web Operations

```rust
// Search the web
let tool = WebSearchTool::new();
let input = ToolInput::new()
    .with_arg("query", "Rust async programming")
    .with_arg("num_results", 10u64);
let output = tool.execute(input).await;

// Fetch a web page
let tool = WebFetchTool::new();
let input = ToolInput::new()
    .with_arg("url", "https://example.com/docs")
    .with_arg("extract_content", true)
    .with_arg("max_length", 5000u64);
let output = tool.execute(input).await;
```

### MCP Integration

```rust
// Register an MCP server
let tool = McpRegistryTool::new();
let input = ToolInput::new()
    .with_arg("action", "register_server")
    .with_arg("server_name", "local-mcp")
    .with_arg("server_url", "http://localhost:8080")
    .with_arg("auth_token", "secret-token");
let output = tool.execute(input).await;

// Invoke an MCP tool
let tool = McpTool::new();
let input = ToolInput::new()
    .with_arg("action", "invoke")
    .with_arg("server_name", "local-mcp")
    .with_arg("method", "tools/list")
    .with_arg("params", serde_json::json!({}));
let output = tool.execute(input).await;
```

---

## QueryEngine Integration

### Tool Registration

Tools are registered with the QueryEngine via a `ToolRegistry`:

```rust
pub struct ToolRegistry {
    entries: Vec<ToolManifestEntry>,
}

pub struct ToolManifestEntry {
    pub name: String,
    pub source: ToolSource,  // Base or Conditional
}
```

### Execution Flow

The QueryEngine orchestrates tool execution during LLM interactions:

1. **Tool Selection**: LLM selects which tool to use based on the current context
2. **Validation**: QueryEngine calls `tool.validate(&input)` before execution
3. **Execution**: On validation success, calls `tool.execute(input).await`
4. **Result Handling**: Output is serialized and added to conversation context

### Concurrency Control

Tools marked `is_concurrency_safe = false` are executed sequentially. Read-only tools can often run in parallel.

### Error Handling

Tool errors are converted to structured responses for the LLM:

```rust
match tool.execute(input).await {
    Ok(output) => {
        // Add successful result to conversation
    }
    Err(e) => {
        // Format error for LLM context
        format!("Tool {} failed: {}", tool.name(), e)
    }
}
```

### Integration with Streaming

Tool results can be streamed back to the user interface through the coordinator integration, allowing real-time feedback during long-running operations (BashTool with background execution, WebFetch for large pages, etc.).

---

## Summary Statistics

| Category | Count | Tools |
|----------|-------|-------|
| File Operations | 6 | FileReadTool, FileWriteTool, FileEditTool, GlobTool, GrepTool, NotebookEditTool |
| System Tools | 1 | BashTool |
| Web Tools | 2 | WebSearchTool, WebFetchTool |
| Task Management | 7 | TaskCreateTool, TaskGetTool, TaskListTool, TaskUpdateTool, TaskOutputTool, TodoWriteTool + stores |
| Advanced Tools | 6 | AgentTool, SendMessageTool, McpTool, McpRegistryTool, LspTool, ConfigTool |
| **Total** | **22** | + 4 backing stores |

---

*End of Tool System Documentation*
