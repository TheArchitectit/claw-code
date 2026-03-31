//! Stream handler state tracking.

use crate::types::ToolUseId;

/// Partial tool use accumulation state
#[derive(Debug, Clone)]
pub struct PartialToolUse {
    /// The tool use ID
    pub(crate) id: ToolUseId,
    /// The tool name
    pub(crate) name: String,
    /// Accumulated JSON input
    pub(crate) partial_json: String,
}
