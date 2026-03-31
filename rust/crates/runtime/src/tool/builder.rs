//! Tool builder.

use crate::types::JsonSchema;

use super::validation::MaxResultSize;

/// Builder for creating tool definitions with defaults.
pub struct ToolBuilder {
    #[allow(dead_code)]
    name: String,
    aliases: Vec<String>,
    search_hint: Option<String>,
    input_schema: JsonSchema,
    output_schema: Option<JsonSchema>,
    enabled: bool,
    max_result_size: MaxResultSize,
    strict: bool,
}

impl ToolBuilder {
    /// Create a new tool builder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            aliases: Vec::new(),
            search_hint: None,
            input_schema: JsonSchema::object(),
            output_schema: None,
            enabled: true,
            max_result_size: MaxResultSize::Limit(100_000),
            strict: false,
        }
    }

    /// Add an alias.
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.aliases.push(alias.into());
        self
    }

    /// Set the search hint.
    pub fn with_search_hint(mut self, hint: impl Into<String>) -> Self {
        self.search_hint = Some(hint.into());
        self
    }

    /// Set the input schema.
    pub fn with_input_schema(mut self, schema: JsonSchema) -> Self {
        self.input_schema = schema;
        self
    }

    /// Set the output schema.
    pub fn with_output_schema(mut self, schema: JsonSchema) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// Set enabled state.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Set max result size.
    pub fn with_max_result_size(mut self, size: MaxResultSize) -> Self {
        self.max_result_size = size;
        self
    }

    /// Set strict mode.
    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }
}
