use std::sync::Arc;

use dashmap::DashMap;

use crate::registry::lookup::{levenshtein_distance, ToolLookupResult};
use crate::registry::manifest::{ToolDefinition, ToolManifest, ToolManifestEntrySer};
use crate::tool::{BoxedTool, SharedTool, Tool};
use crate::types::ToolError;

/// A registry for tools that supports efficient lookup by name.
///
/// The registry stores tools in a concurrent hash map for thread-safe
/// access and supports lookup by primary name or alias.
#[derive(Clone)]
pub struct ToolRegistry {
    tools: Arc<DashMap<String, SharedTool>>,
    aliases: Arc<DashMap<String, String>>, // alias -> primary name
}

impl ToolRegistry {
    /// Create a new empty tool registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: Arc::new(DashMap::new()),
            aliases: Arc::new(DashMap::new()),
        }
    }

    /// Create a registry from a list of tools.
    #[must_use]
    pub fn from_tools(tools: Vec<BoxedTool>) -> Self {
        let registry = Self::new();
        for tool in tools {
            registry.register(tool);
        }
        registry
    }

    /// Register a tool in the registry.
    ///
    /// If a tool with the same name already exists, it is replaced.
    pub fn register(&self, tool: BoxedTool) {
        let name = tool.name().to_string();

        // Register aliases before moving the tool
        for alias in tool.aliases() {
            self.aliases.insert(alias.clone(), name.clone());
        }

        // Register the tool wrapped in Arc
        self.tools.insert(name, Arc::new(tool));
    }

    /// Unregister a tool by name.
    ///
    /// Returns true if a tool was removed.
    pub fn unregister(&self, name: &str) -> bool {
        // Remove the tool
        let removed = self.tools.remove(name);

        // Clean up aliases pointing to this tool
        if removed.is_some() {
            let aliases_to_remove: Vec<String> = self
                .aliases
                .iter()
                .filter(|entry| entry.value() == name)
                .map(|entry| entry.key().clone())
                .collect();

            for alias in aliases_to_remove {
                self.aliases.remove(&alias);
            }
        }

        removed.is_some()
    }

    /// Get a tool by name or alias.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<SharedTool> {
        // Try direct lookup first
        if let Some(tool) = self.tools.get(name) {
            return Some(Arc::clone(tool.value()));
        }

        // Try alias lookup
        if let Some(primary_name) = self.aliases.get(name) {
            if let Some(tool) = self.tools.get(primary_name.value()) {
                return Some(Arc::clone(tool.value()));
            }
        }

        None
    }

    /// Check if a tool with the given name exists.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name) || self.aliases.contains_key(name)
    }

    /// Get all tools in the registry.
    #[must_use]
    pub fn all_tools(&self) -> Vec<SharedTool> {
        self.tools
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Get all enabled tools.
    #[must_use]
    pub fn enabled_tools(&self) -> Vec<SharedTool> {
        self.tools
            .iter()
            .filter(|entry| entry.value().is_enabled())
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Get the number of tools in the registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Clear all tools from the registry.
    pub fn clear(&self) {
        self.tools.clear();
        self.aliases.clear();
    }

    /// Find tools matching a predicate.
    #[must_use]
    pub fn find<F>(&self, predicate: F) -> Vec<SharedTool>
    where
        F: Fn(&(dyn Tool + Send + Sync)) -> bool,
    {
        self.tools
            .iter()
            .filter(|entry| predicate(entry.value().as_ref()))
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Filter tools by category.
    #[must_use]
    pub fn filter_by_type(&self, tool_type: ToolType) -> Vec<SharedTool> {
        match tool_type {
            ToolType::Mcp => self.find(|t| t.is_mcp()),
            ToolType::Lsp => self.find(|t| t.is_lsp()),
            ToolType::Builtin => self.find(|t| !t.is_mcp() && !t.is_lsp()),
            ToolType::All => self.all_tools(),
        }
    }

    /// Get deferred tools (those that require ToolSearch).
    #[must_use]
    pub fn deferred_tools(&self) -> Vec<SharedTool> {
        self.find(|t| t.should_defer())
    }

    /// Get always-loaded tools (those that never defer).
    #[must_use]
    pub fn always_loaded_tools(&self) -> Vec<SharedTool> {
        self.find(|t| t.always_load())
    }

    /// Get a reference to a tool without cloning.
    ///
    /// This is more efficient than `get` but requires the DashMap guard.
    pub fn get_ref(&self, name: &str) -> Option<dashmap::mapref::one::Ref<'_, String, SharedTool>> {
        if let Some(tool) = self.tools.get(name) {
            return Some(tool);
        }

        if let Some(primary_name) = self.aliases.get(name) {
            return self.tools.get(primary_name.value());
        }

        None
    }

    /// Execute a tool by name with the given input.
    ///
    /// This is a convenience method for looking up and executing a tool.
    pub async fn execute(
        &self,
        name: &str,
        input: serde_json::Value,
        context: &crate::context::ToolUseContext,
        tool_use_id: crate::types::ToolUseId,
        on_progress: Option<Box<dyn Fn(crate::messages::ProgressData) + Send>>,
    ) -> crate::types::ToolResult<crate::tool::ToolOutput> {
        let tool = self
            .get(name)
            .ok_or_else(|| ToolError::not_found(name))?;

        tool.as_ref().execute(input, context, tool_use_id, on_progress).await
    }

    /// Look up a tool with suggestions if not found.
    ///
    /// This method attempts to find a tool by name, and if not found,
    /// returns suggestions for similar tools based on name similarity.
    #[must_use]
    pub fn get_with_suggestions(&self, name: &str) -> ToolLookupResult {
        // Try direct lookup first
        if let Some(tool) = self.get(name) {
            return ToolLookupResult::Found(tool);
        }

        // Generate suggestions based on name similarity
        let suggestions = self.find_similar_tools(name, 3);

        ToolLookupResult::NotFound {
            requested: name.to_string(),
            suggestions,
        }
    }

    /// Find similar tool names based on string distance.
    ///
    /// Uses a simple edit-distance based approach to find tools with
    /// similar names to the requested one.
    #[must_use]
    pub fn find_similar_tools(&self, name: &str, limit: usize) -> Vec<String> {
        let name_lower = name.to_lowercase();
        let mut candidates: Vec<(String, usize)> = self
            .tools
            .iter()
            .map(|entry| {
                let tool_name = entry.key().to_lowercase();
                let distance = levenshtein_distance(&name_lower, &tool_name);
                (entry.key().clone(), distance)
            })
            .filter(|(_, distance)| *distance <= name.len() / 2) // Only reasonably similar
            .collect();

        // Sort by distance (lower is better)
        candidates.sort_by_key(|(_, distance)| *distance);

        candidates.into_iter().take(limit).map(|(name, _)| name).collect()
    }

    /// Generate tool definitions for all enabled tools.
    ///
    /// This creates ToolDefinition structures suitable for LLM system prompts.
    #[must_use]
    pub fn generate_tool_definitions(&self) -> Vec<ToolDefinition> {
        self.enabled_tools()
            .iter()
            .map(|tool| {
                let hint = tool.search_hint().unwrap_or("");
                let mut def = ToolDefinition::new(tool.name(), hint)
                    .with_input_schema(tool.input_schema());

                if let Some(output_schema) = tool.output_schema() {
                    def = def.with_output_schema(output_schema);
                }

                def
            })
            .collect()
    }

    /// Create a tool manifest for the system prompt.
    ///
    /// Returns a serializable manifest containing metadata for all tools.
    #[must_use]
    pub fn create_manifest(&self) -> ToolManifest {
        let tools = self
            .all_tools()
            .iter()
            .map(|tool| ToolManifestEntrySer {
                name: tool.name().to_string(),
                source: if tool.is_mcp() {
                    "mcp".to_string()
                } else if tool.is_lsp() {
                    "lsp".to_string()
                } else {
                    "builtin".to_string()
                },
                is_deferred: tool.should_defer(),
                is_always_loaded: tool.always_load(),
            })
            .collect();

        ToolManifest { tools }
    }

    /// Filter tools by a predicate.
    ///
    /// Returns tools that match the given filter function.
    #[must_use]
    pub fn filter_by_permission<F>(&self, predicate: F) -> Vec<SharedTool>
    where
        F: Fn(&(dyn Tool + Send + Sync)) -> bool,
    {
        self.find(predicate)
    }

    /// Get tools matching an allowlist.
    ///
    /// Only returns tools whose names are in the provided list.
    #[must_use]
    pub fn filter_by_allowlist(&self, allowed_names: &[String]) -> Vec<SharedTool> {
        self.tools
            .iter()
            .filter(|entry| allowed_names.contains(entry.key()))
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Get tools not matching a denylist.
    ///
    /// Returns all tools except those whose names are in the provided list.
    #[must_use]
    pub fn filter_by_denylist(&self, denied_names: &[String]) -> Vec<SharedTool> {
        self.tools
            .iter()
            .filter(|entry| !denied_names.contains(entry.key()))
            .map(|entry| Arc::clone(entry.value()))
            .collect()
    }

    /// Register a tool dynamically.
    ///
    /// This can be called during runtime to add new tools.
    /// If a tool with the same name already exists, it is replaced.
    pub fn register_dynamic(&self, tool: BoxedTool) {
        self.register(tool);
    }

    /// Update an existing tool.
    ///
    /// This replaces a tool while maintaining its aliases.
    /// Returns true if a tool was updated, false if it didn't exist.
    pub fn update(&self, name: &str, tool: BoxedTool) -> bool {
        if !self.contains(name) {
            return false;
        }

        // Unregister old tool to clean up aliases
        self.unregister(name);

        // Register new tool
        self.register(tool);

        true
    }

    /// Check if a tool version satisfies a constraint.
    ///
    /// Note: This is a placeholder implementation. In a full implementation,
    /// tools would need to expose their version via the Tool trait.
    #[must_use]
    pub fn check_tool_version(&self, _name: &str, _constraint: &VersionConstraint) -> bool {
        // Since Tool trait doesn't yet expose version, we assume any version matches
        // This can be extended when the Tool trait is updated with version info
        true
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<BoxedTool>> for ToolRegistry {
    fn from(tools: Vec<BoxedTool>) -> Self {
        Self::from_tools(tools)
    }
}

/// Tool type categories for filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolType {
    /// MCP tools.
    Mcp,

    /// LSP tools.
    Lsp,

    /// Built-in tools.
    Builtin,

    /// All tools.
    All,
}

/// A version constraint for tool lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionConstraint {
    /// Any version is acceptable.
    Any,

    /// Exact version match required.
    Exact(String),

    /// Version must be at least this version.
    AtLeast(String),

    /// Version must be less than this version.
    LessThan(String),
}

impl VersionConstraint {
    /// Check if a version satisfies this constraint.
    ///
    /// Uses basic semver comparison (major.minor.patch).
    pub fn is_satisfied_by(&self, version: &str) -> bool {
        match self {
            VersionConstraint::Any => true,
            VersionConstraint::Exact(v) => v == version,
            VersionConstraint::AtLeast(min) => {
                version_compare(version, min).map(|c| c >= 0).unwrap_or(false)
            }
            VersionConstraint::LessThan(max) => {
                version_compare(version, max).map(|c| c < 0).unwrap_or(false)
            }
        }
    }
}

/// Compare two semver versions.
/// Returns -1 if v1 < v2, 0 if equal, 1 if v1 > v2.
fn version_compare(v1: &str, v2: &str) -> Option<i32> {
    let parse = |v: &str| -> Option<Vec<u32>> {
        v.split('.')
            .take(3)
            .map(|s| s.parse().ok())
            .collect()
    };

    let parts1 = parse(v1)?;
    let parts2 = parse(v2)?;

    for (p1, p2) in parts1.iter().zip(parts2.iter()) {
        match p1.cmp(p2) {
            std::cmp::Ordering::Less => return Some(-1),
            std::cmp::Ordering::Greater => return Some(1),
            std::cmp::Ordering::Equal => continue,
        }
    }

    Some(0)
}
