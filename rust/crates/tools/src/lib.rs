pub mod agent;
pub mod bash;
pub mod config;
pub mod file_edit;
pub mod file_read;
pub mod file_write;
pub mod glob;
pub mod grep;
pub mod lsp;
pub mod mcp;
pub mod mcp_registry;
pub mod notebook_edit;
pub mod send_message;
pub mod task_create;
pub mod task_get;
pub mod task_list;
pub mod task_output;
pub mod task_store;
pub mod task_update;
pub mod todo_store;
pub mod todo_write;
pub mod tool;
pub mod web_fetch;
pub mod web_search;

pub use tool::{BoxedTool, Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolManifestEntry {
    pub name: String,
    pub source: ToolSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSource {
    Base,
    Conditional,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolRegistry {
    entries: Vec<ToolManifestEntry>,
}

impl ToolRegistry {
    #[must_use]
    pub fn new(entries: Vec<ToolManifestEntry>) -> Self {
        Self { entries }
    }

    #[must_use]
    pub fn entries(&self) -> &[ToolManifestEntry] {
        &self.entries
    }
}
