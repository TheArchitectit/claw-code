//! Tool registry for managing tool collections.
//!
//! This module provides the `ToolRegistry` for registering and looking up
//! tools by name, as well as filtering and organizing tools.

pub mod builder;
pub mod lookup;
pub mod manifest;
pub mod registry;

pub use builder::ToolRegistryBuilder;
pub use lookup::{ToolLookupResult, MissingToolInfo, find_tool_by_name, tool_matches_name};
pub use manifest::{ToolManifest, ToolManifestEntry, ToolManifestEntrySer, ToolSource, ToolDefinition};
pub use registry::{ToolRegistry, ToolType, VersionConstraint};
