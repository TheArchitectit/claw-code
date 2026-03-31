//! Tool helper functions.

use super::r#trait::{BoxedTool, Tool};

/// Check if a tool name matches (primary name or alias).
#[must_use]
pub fn tool_matches_name(tool: &(dyn Tool + Send + Sync), name: &str) -> bool {
    if tool.name() == name {
        return true;
    }
    tool.aliases().iter().any(|alias| alias == name)
}

/// Find a tool by name or alias from a list of tools.
#[must_use]
pub fn find_tool_by_name<'a>(
    tools: &'a [BoxedTool],
    name: &str,
) -> Option<&'a (dyn Tool + Send + Sync)> {
    tools
        .iter()
        .find(|tool| tool_matches_name(tool.as_ref(), name))
        .map(|t| t.as_ref())
}
