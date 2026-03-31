use crate::registry::registry::ToolRegistry;
use crate::tool::BoxedTool;

/// Builder for creating a tool registry with specific configurations.
pub struct ToolRegistryBuilder {
    tools: Vec<BoxedTool>,
    deferred: Vec<BoxedTool>,
    disabled: Vec<String>,
}

impl ToolRegistryBuilder {
    /// Create a new registry builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            deferred: Vec::new(),
            disabled: Vec::new(),
        }
    }

    /// Add a tool to the registry.
    pub fn add_tool(mut self, tool: BoxedTool) -> Self {
        if tool.should_defer() {
            self.deferred.push(tool);
        } else {
            self.tools.push(tool);
        }
        self
    }

    /// Add multiple tools.
    pub fn add_tools(mut self, tools: Vec<BoxedTool>) -> Self {
        for tool in tools {
            if tool.should_defer() {
                self.deferred.push(tool);
            } else {
                self.tools.push(tool);
            }
        }
        self
    }

    /// Disable a tool by name.
    pub fn disable_tool(mut self, name: impl Into<String>) -> Self {
        self.disabled.push(name.into());
        self
    }

    /// Build the final registry.
    #[must_use]
    pub fn build(self) -> ToolRegistry {
        let registry = ToolRegistry::new();

        for tool in self.tools {
            if !self.disabled.contains(&tool.name().to_string()) {
                registry.register(tool);
            }
        }

        // Register deferred tools too
        for tool in self.deferred {
            if !self.disabled.contains(&tool.name().to_string()) {
                registry.register(tool);
            }
        }

        registry
    }

    /// Build with a filter function.
    #[must_use]
    pub fn build_filtered<F>(self, filter: F) -> ToolRegistry
    where
        F: Fn(&BoxedTool) -> bool,
    {
        let registry = ToolRegistry::new();

        for tool in self.tools {
            if filter(&tool) && !self.disabled.contains(&tool.name().to_string()) {
                registry.register(tool);
            }
        }

        for tool in self.deferred {
            if filter(&tool) && !self.disabled.contains(&tool.name().to_string()) {
                registry.register(tool);
            }
        }

        registry
    }
}

impl Default for ToolRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
