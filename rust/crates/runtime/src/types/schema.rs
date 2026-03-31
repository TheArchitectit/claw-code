//! JSON Schema types for the runtime crate.
//!
//! This module provides JSON Schema definitions used for tool input/output
//! validation and type definitions.

use serde::{Deserialize, Serialize};

/// A JSON Schema definition for tool input/output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSchema {
    /// The schema type (typically "object").
    #[serde(rename = "type")]
    pub schema_type: String,

    /// The schema properties.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Map<String, serde_json::Value>>,

    /// Required fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,

    /// Additional properties allowed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_properties: Option<bool>,

    /// Schema description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl JsonSchema {
    /// Create a new JSON schema for an object type.
    #[must_use]
    pub fn object() -> Self {
        Self {
            schema_type: "object".to_string(),
            properties: Some(serde_json::Map::new()),
            required: None,
            additional_properties: Some(false),
            description: None,
        }
    }

    /// Add a property to the schema.
    pub fn with_property(
        mut self,
        name: impl Into<String>,
        schema: serde_json::Value,
        required: bool,
    ) -> Self {
        let name = name.into();
        if let Some(ref mut props) = self.properties {
            props.insert(name.clone(), schema);
        }
        if required {
            self.required
                .get_or_insert_with(Vec::new)
                .push(name);
        }
        self
    }

    /// Set the schema description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Create a new JSON schema for an array type.
    #[must_use]
    pub fn array() -> Self {
        Self {
            schema_type: "array".to_string(),
            properties: None,
            required: None,
            additional_properties: None,
            description: None,
        }
    }

    /// Create a new JSON schema for a string type.
    #[must_use]
    pub fn string() -> Self {
        Self {
            schema_type: "string".to_string(),
            properties: None,
            required: None,
            additional_properties: None,
            description: None,
        }
    }

    /// Create a new JSON schema for a number type.
    #[must_use]
    pub fn number() -> Self {
        Self {
            schema_type: "number".to_string(),
            properties: None,
            required: None,
            additional_properties: None,
            description: None,
        }
    }

    /// Create a new JSON schema for a boolean type.
    #[must_use]
    pub fn boolean() -> Self {
        Self {
            schema_type: "boolean".to_string(),
            properties: None,
            required: None,
            additional_properties: None,
            description: None,
        }
    }

    /// Create a new JSON schema for a null type.
    #[must_use]
    pub fn null() -> Self {
        Self {
            schema_type: "null".to_string(),
            properties: None,
            required: None,
            additional_properties: None,
            description: None,
        }
    }

    /// Convert the schema to a JSON value.
    #[must_use]
    pub fn schema(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({"type": "object"}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_schema_variants() {
        let object = JsonSchema::object();
        assert_eq!(object.schema_type, "object");

        let array = JsonSchema::array();
        assert_eq!(array.schema_type, "array");

        let string = JsonSchema::string();
        assert_eq!(string.schema_type, "string");

        let number = JsonSchema::number();
        assert_eq!(number.schema_type, "number");

        let boolean = JsonSchema::boolean();
        assert_eq!(boolean.schema_type, "boolean");

        let null = JsonSchema::null();
        assert_eq!(null.schema_type, "null");
    }

    #[test]
    fn test_json_schema_builder() {
        let schema = JsonSchema::object()
            .with_property(
                "name",
                serde_json::json!({"type": "string"}),
                true,
            )
            .with_property(
                "age",
                serde_json::json!({"type": "integer"}),
                false,
            )
            .with_description("A person object");

        assert_eq!(schema.schema_type, "object");
        assert!(schema.description.as_ref().unwrap().contains("person"));
    }

    #[test]
    fn test_json_schema_with_required() {
        let schema = JsonSchema::object()
            .with_property("req1", serde_json::json!({"type": "string"}), true)
            .with_property("req2", serde_json::json!({"type": "string"}), true)
            .with_property("opt1", serde_json::json!({"type": "string"}), false);

        assert_eq!(schema.required.as_ref().unwrap().len(), 2);
        assert!(schema.required.as_ref().unwrap().contains(&"req1".to_string()));
        assert!(schema.required.as_ref().unwrap().contains(&"req2".to_string()));
    }
}
