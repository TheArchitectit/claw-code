use crate::types::JsonSchema;

/// A definition of a tool for use in system prompts and LLM APIs.
///
/// This structure contains all the metadata needed to describe a tool
/// to an LLM, including its name, description, and JSON schema.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    /// The tool name.
    pub name: String,

    /// A description of what the tool does.
    pub description: String,

    /// The JSON schema for the tool's input parameters.
    pub input_schema: JsonSchema,

    /// Optional output schema for the tool's return value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<JsonSchema>,

    /// Optional example inputs for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub examples: Option<Vec<serde_json::Value>>,

    /// The tool version (e.g., "1.0.0").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Whether this tool is deprecated.
    #[serde(default)]
    pub deprecated: bool,

    /// Deprecation message if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deprecation_message: Option<String>,
}

impl ToolDefinition {
    /// Create a new tool definition with the given name and description.
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema: JsonSchema::object(),
            output_schema: None,
            examples: None,
            version: None,
            deprecated: false,
            deprecation_message: None,
        }
    }

    /// Set the input schema.
    #[must_use]
    pub fn with_input_schema(mut self, schema: JsonSchema) -> Self {
        self.input_schema = schema;
        self
    }

    /// Set the output schema.
    #[must_use]
    pub fn with_output_schema(mut self, schema: JsonSchema) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Add an example input.
    #[must_use]
    pub fn with_example(mut self, example: serde_json::Value) -> Self {
        self.examples.get_or_insert_with(Vec::new).push(example);
        self
    }

    /// Set the version.
    #[must_use]
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Mark as deprecated.
    #[must_use]
    pub fn with_deprecation(mut self, message: impl Into<String>) -> Self {
        self.deprecated = true;
        self.deprecation_message = Some(message.into());
        self
    }

    /// Convert to JSON schema format for LLM APIs.
    ///
    /// This produces the format expected by Anthropic's Claude API:
    /// ```json
    /// {
    ///   "name": "tool_name",
    ///   "description": "Tool description",
    ///   "input_schema": { ... }
    /// }
    /// ```
    pub fn to_llm_schema(&self) -> serde_json::Value {
        let mut value = serde_json::json!({
            "name": &self.name,
            "description": &self.description,
            "input_schema": self.input_schema.schema(),
        });

        if let Some(ref output) = self.output_schema {
            if let Ok(schema_json) = serde_json::to_value(output.schema()) {
                value["output_schema"] = schema_json;
            }
        }

        value
    }
}

/// A simple tool manifest for serialization.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolManifest {
    /// The tools in the manifest.
    pub tools: Vec<ToolManifestEntrySer>,
}

/// Serializable manifest entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolManifestEntrySer {
    /// The tool name.
    pub name: String,

    /// The source.
    pub source: String,

    /// Whether deferred.
    pub is_deferred: bool,

    /// Whether always loaded.
    pub is_always_loaded: bool,
}

/// A manifest entry for a tool.
#[derive(Debug, Clone)]
pub struct ToolManifestEntry {
    /// The tool name.
    pub name: String,

    /// The source of the tool.
    pub source: ToolSource,

    /// Whether the tool is deferred.
    pub is_deferred: bool,

    /// Whether the tool is always loaded.
    pub is_always_loaded: bool,
}

/// The source of a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSource {
    /// Base tool included by default.
    Base,

    /// Conditional tool loaded based on context.
    Conditional,

    /// MCP tool from an external server.
    Mcp,

    /// LSP tool from a language server.
    Lsp,

    /// Plugin tool from an extension.
    Plugin,
}
