//! ConfigTool - Configuration management
//!
//! This tool reads and writes configuration files in various formats:
//! - JSON
//! - YAML
//! - TOML
//! - INI/Properties

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration format types.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum ConfigFormat {
    #[serde(rename = "json")]
    Json,
    #[serde(rename = "yaml")]
    Yaml,
    #[serde(rename = "toml")]
    Toml,
    #[serde(rename = "ini")]
    Ini,
    #[serde(rename = "auto")]
    Auto,
}

impl ConfigFormat {
    /// Detect format from file extension.
    fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "json" => Some(Self::Json),
            "yaml" | "yml" => Some(Self::Yaml),
            "toml" => Some(Self::Toml),
            "ini" | "conf" | "cfg" | "properties" => Some(Self::Ini),
            _ => None,
        }
    }
}

impl Default for ConfigFormat {
    fn default() -> Self {
        Self::Auto
    }
}

/// Input schema for the ConfigTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConfigInput {
    /// The configuration file path.
    pub file_path: String,
    /// The operation to perform: read, write, get, set.
    pub operation: String,
    /// Format of the config file (auto-detected if not specified).
    #[serde(default)]
    pub format: Option<String>,
    /// Key path for get/set operations (dot-notation like "database.host").
    #[serde(default)]
    pub key: Option<String>,
    /// Value for set operation.
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    /// Full configuration data for write operation.
    #[serde(default)]
    pub data: Option<serde_json::Value>,
    /// Whether to create parent directories if they don't exist.
    #[serde(default)]
    pub create_dirs: Option<bool>,
}

/// Output schema for ConfigTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigOutput {
    pub file_path: String,
    pub operation: String,
    pub format: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Full configuration data for read operations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    /// Value for get operation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<serde_json::Value>,
    /// Keys at the current level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
}

/// The ConfigTool manages configuration files.
#[derive(Debug, Clone, Default)]
pub struct ConfigTool;

impl ConfigTool {
    /// Create a new ConfigTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Detect format from file path.
    fn detect_format(&self, file_path: &str) -> Option<ConfigFormat> {
        std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .and_then(ConfigFormat::from_extension)
    }

    /// Parse configuration file based on format.
    fn parse_config(&self, content: &str, format: &ConfigFormat) -> Result<serde_json::Value, String> {
        match format {
            ConfigFormat::Json => {
                serde_json::from_str(content).map_err(|e| format!("JSON parse error: {e}"))
            }
            ConfigFormat::Yaml => {
                // For YAML, we'll parse as JSON-compatible structure
                // In a full implementation, use a YAML parser
                serde_yaml::from_str(content).map_err(|e| format!("YAML parse error: {e}"))
            }
            ConfigFormat::Toml => {
                toml::from_str(content).map_err(|e| format!("TOML parse error: {e}"))
            }
            ConfigFormat::Ini => {
                self.parse_ini(content)
            }
            ConfigFormat::Auto => Err("Cannot auto-detect format during parsing".to_string()),
        }
    }

    /// Serialize configuration to string.
    fn serialize_config(&self, data: &serde_json::Value, format: &ConfigFormat) -> Result<String, String> {
        match format {
            ConfigFormat::Json => {
                serde_json::to_string_pretty(data).map_err(|e| format!("JSON serialize error: {e}"))
            }
            ConfigFormat::Yaml => {
                serde_yaml::to_string(data).map_err(|e| format!("YAML serialize error: {e}"))
            }
            ConfigFormat::Toml => {
                toml::to_string_pretty(data).map_err(|e| format!("TOML serialize error: {e}"))
            }
            ConfigFormat::Ini => {
                self.serialize_ini(data)
            }
            ConfigFormat::Auto => Err("Cannot auto-detect format during serialization".to_string()),
        }
    }

    /// Simple INI parser.
    fn parse_ini(&self, content: &str) -> Result<serde_json::Value, String> {
        let mut result = serde_json::Map::new();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Section header
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                if !result.contains_key(&current_section) {
                    result.insert(
                        current_section.clone(),
                        serde_json::Value::Object(serde_json::Map::new()),
                    );
                }
                continue;
            }

            // Key-value pair
            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();

                // Try to parse as number or boolean
                let json_value = if let Ok(num) = value.parse::<i64>() {
                    serde_json::Value::Number(num.into())
                } else if let Ok(num) = value.parse::<f64>() {
                    serde_json::json!(num)
                } else if value.eq_ignore_ascii_case("true") {
                    serde_json::Value::Bool(true)
                } else if value.eq_ignore_ascii_case("false") {
                    serde_json::Value::Bool(false)
                } else {
                    serde_json::Value::String(value)
                };

                if current_section.is_empty() {
                    result.insert(key, json_value);
                } else if let Some(section) = result.get_mut(&current_section) {
                    if let Some(obj) = section.as_object_mut() {
                        obj.insert(key, json_value);
                    }
                }
            }
        }

        Ok(serde_json::Value::Object(result))
    }

    /// Simple INI serializer.
    fn serialize_ini(&self, data: &serde_json::Value) -> Result<String, String> {
        let mut output = String::new();

        if let Some(obj) = data.as_object() {
            // First write top-level keys
            for (key, value) in obj {
                if !value.is_object() {
                    output.push_str(&format!("{} = {}\n", key, self.value_to_ini_string(value)));
                }
            }

            // Then write sections
            for (key, value) in obj {
                if value.is_object() {
                    output.push_str(&format!("\n[{}]\n", key));
                    if let Some(section_obj) = value.as_object() {
                        for (sub_key, sub_value) in section_obj {
                            output.push_str(&format!(
                                "{} = {}\n",
                                sub_key,
                                self.value_to_ini_string(sub_value)
                            ));
                        }
                    }
                }
            }
        }

        Ok(output)
    }

    /// Convert JSON value to INI string representation.
    fn value_to_ini_string(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        }
    }

    /// Get value at key path using dot notation.
    fn get_value_at_path(&self, data: &serde_json::Value, path: &str) -> Option<serde_json::Value> {
        let mut current = data;

        for key in path.split('.') {
            match current {
                serde_json::Value::Object(map) => {
                    current = map.get(key)?;
                }
                _ => return None,
            }
        }

        Some(current.clone())
    }

    /// Set value at key path using dot notation.
    fn set_value_at_path(
        &self,
        data: &mut serde_json::Value,
        path: &str,
        value: serde_json::Value,
    ) -> Result<(), String> {
        let keys: Vec<&str> = path.split('.').collect();
        let mut current = data;

        // Navigate to the parent of the target key
        for key in &keys[..keys.len() - 1] {
            if !current.is_object() {
                return Err(format!("Cannot navigate through non-object at key: {}", key));
            }

            let obj = current.as_object_mut().unwrap();

            if !obj.contains_key(*key) {
                // Create intermediate objects as needed
                obj.insert(key.to_string(), serde_json::Value::Object(serde_json::Map::new()));
            }

            current = obj.get_mut(*key).unwrap();
        }

        // Set the final key
        if let Some(last_key) = keys.last() {
            if let Some(obj) = current.as_object_mut() {
                obj.insert(last_key.to_string(), value);
                Ok(())
            } else {
                Err("Cannot set value on non-object".to_string())
            }
        } else {
            Err("Empty key path".to_string())
        }
    }

    /// List keys at a path.
    fn list_keys(&self, data: &serde_json::Value, path: Option<&str>) -> Option<Vec<String>> {
        let target = if let Some(p) = path {
            self.get_value_at_path(data, p)?
        } else {
            data.clone()
        };

        if let Some(obj) = target.as_object() {
            Some(obj.keys().cloned().collect())
        } else {
            Some(Vec::new())
        }
    }

    /// Execute read operation.
    async fn read_config(&self, file_path: &str, format: ConfigFormat) -> ConfigOutput {
        let content = match tokio::fs::read_to_string(file_path).await {
            Ok(c) => c,
            Err(e) => {
                return ConfigOutput {
                    file_path: file_path.to_string(),
                    operation: "read".to_string(),
                    format: format_str(&format).to_string(),
                    success: false,
                    error: Some(format!("Failed to read file: {e}")),
                    data: None,
                    value: None,
                    keys: None,
                };
            }
        };

        // If format is auto, try to detect from extension
        let actual_format = if format == ConfigFormat::Auto {
            match self.detect_format(file_path) {
                Some(f) => f,
                None => {
                    return ConfigOutput {
                        file_path: file_path.to_string(),
                        operation: "read".to_string(),
                        format: "unknown".to_string(),
                        success: false,
                        error: Some("Could not auto-detect config format from file extension".to_string()),
                        data: None,
                        value: None,
                        keys: None,
                    };
                }
            }
        } else {
            format
        };

        match self.parse_config(&content, &actual_format) {
            Ok(data) => ConfigOutput {
                file_path: file_path.to_string(),
                operation: "read".to_string(),
                format: format_str(&actual_format).to_string(),
                success: true,
                error: None,
                data: Some(data.clone()),
                value: None,
                keys: self.list_keys(&data, None),
            },
            Err(e) => ConfigOutput {
                file_path: file_path.to_string(),
                operation: "read".to_string(),
                format: format_str(&actual_format).to_string(),
                success: false,
                error: Some(e),
                data: None,
                value: None,
                keys: None,
            },
        }
    }

    /// Execute write operation.
    async fn write_config(
        &self,
        file_path: &str,
        format: ConfigFormat,
        data: &serde_json::Value,
        create_dirs: bool,
    ) -> ConfigOutput {
        // If format is auto, try to detect from extension
        let actual_format = if format == ConfigFormat::Auto {
            match self.detect_format(file_path) {
                Some(f) => f,
                None => {
                    return ConfigOutput {
                        file_path: file_path.to_string(),
                        operation: "write".to_string(),
                        format: "unknown".to_string(),
                        success: false,
                        error: Some("Could not auto-detect config format from file extension".to_string()),
                        data: None,
                        value: None,
                        keys: None,
                    };
                }
            }
        } else {
            format
        };

        // Create parent directories if requested
        if create_dirs {
            if let Some(parent) = std::path::Path::new(file_path).parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ConfigOutput {
                        file_path: file_path.to_string(),
                        operation: "write".to_string(),
                        format: format_str(&actual_format).to_string(),
                        success: false,
                        error: Some(format!("Failed to create directories: {e}")),
                        data: None,
                        value: None,
                        keys: None,
                    };
                }
            }
        }

        match self.serialize_config(data, &actual_format) {
            Ok(content) => {
                match tokio::fs::write(file_path, content).await {
                    Ok(_) => ConfigOutput {
                        file_path: file_path.to_string(),
                        operation: "write".to_string(),
                        format: format_str(&actual_format).to_string(),
                        success: true,
                        error: None,
                        data: Some(data.clone()),
                        value: None,
                        keys: self.list_keys(data, None),
                    },
                    Err(e) => ConfigOutput {
                        file_path: file_path.to_string(),
                        operation: "write".to_string(),
                        format: format_str(&actual_format).to_string(),
                        success: false,
                        error: Some(format!("Failed to write file: {e}")),
                        data: None,
                        value: None,
                        keys: None,
                    },
                }
            }
            Err(e) => ConfigOutput {
                file_path: file_path.to_string(),
                operation: "write".to_string(),
                format: format_str(&actual_format).to_string(),
                success: false,
                error: Some(e),
                data: None,
                value: None,
                keys: None,
            },
        }
    }

    /// Execute get operation.
    async fn get_value(
        &self,
        file_path: &str,
        format: ConfigFormat,
        key: &str,
    ) -> ConfigOutput {
        let read_result = self.read_config(file_path, format).await;

        if !read_result.success {
            return read_result;
        }

        let data = read_result.data.unwrap();
        let value = self.get_value_at_path(&data, key);
        let keys = value.as_ref().and_then(|v| self.list_keys(v, None));

        ConfigOutput {
            file_path: file_path.to_string(),
            operation: "get".to_string(),
            format: read_result.format,
            success: value.is_some(),
            error: if value.is_none() {
                Some(format!("Key '{}' not found", key))
            } else {
                None
            },
            data: None,
            value,
            keys,
        }
    }

    /// Execute set operation.
    async fn set_value(
        &self,
        file_path: &str,
        format: ConfigFormat,
        key: &str,
        value: serde_json::Value,
        create_dirs: bool,
    ) -> ConfigOutput {
        let read_result = self.read_config(file_path, format.clone()).await;

        let mut data = if read_result.success {
            read_result.data.unwrap()
        } else {
            // Start with empty object if file doesn't exist
            serde_json::Value::Object(serde_json::Map::new())
        };

        // Set the value
        if let Err(e) = self.set_value_at_path(&mut data, key, value) {
            return ConfigOutput {
                file_path: file_path.to_string(),
                operation: "set".to_string(),
                format: read_result.format,
                success: false,
                error: Some(e),
                data: None,
                value: None,
                keys: None,
            };
        }

        // Write back
        self.write_config(file_path, format, &data, create_dirs).await
    }
}

fn format_str(format: &ConfigFormat) -> &'static str {
    match format {
        ConfigFormat::Json => "json",
        ConfigFormat::Yaml => "yaml",
        ConfigFormat::Toml => "toml",
        ConfigFormat::Ini => "ini",
        ConfigFormat::Auto => "auto",
    }
}

#[async_trait]
impl Tool for ConfigTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "ConfigTool",
                "Read and write configuration files (JSON, YAML, TOML, INI)",
            )
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let file_path = input
            .require("file_path")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "file_path must be a string".to_string(),
                error_code: Some(1),
            })?;

        if file_path.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "file_path cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        let operation = input
            .require("operation")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "operation must be a string".to_string(),
                error_code: Some(3),
            })?;

        let valid_ops = ["read", "write", "get", "set"];
        if !valid_ops.contains(&operation) {
            return Err(ToolError::ValidationFailed {
                message: format!("Invalid operation: {}. Valid: {:?}", operation, valid_ops),
                error_code: Some(4),
            });
        }

        // Validate format if provided
        if let Some(format) = input.get("format").and_then(|v| v.as_str()) {
            let valid_formats = ["json", "yaml", "toml", "ini", "auto"];
            if !valid_formats.contains(&format) {
                return Err(ToolError::ValidationFailed {
                    message: format!("Invalid format: {}. Valid: {:?}", format, valid_formats),
                    error_code: Some(5),
                });
            }
        }

        // Key required for get/set
        if (operation == "get" || operation == "set") && input.get("key").is_none() {
            return Err(ToolError::ValidationFailed {
                message: "key is required for get/set operations".to_string(),
                error_code: Some(6),
            });
        }

        // Value required for set
        if operation == "set" && input.get("value").is_none() {
            return Err(ToolError::ValidationFailed {
                message: "value is required for set operation".to_string(),
                error_code: Some(7),
            });
        }

        // Data required for write
        if operation == "write" && input.get("data").is_none() {
            return Err(ToolError::ValidationFailed {
                message: "data is required for write operation".to_string(),
                error_code: Some(8),
            });
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let file_path = match input.get("file_path") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "file_path must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: file_path")
                    .with_field("success", false);
            }
        };

        let operation = match input.get("operation") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "operation must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: operation")
                    .with_field("success", false);
            }
        };

        let format = input
            .get("format")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "json" => Some(ConfigFormat::Json),
                "yaml" => Some(ConfigFormat::Yaml),
                "toml" => Some(ConfigFormat::Toml),
                "ini" => Some(ConfigFormat::Ini),
                "auto" => Some(ConfigFormat::Auto),
                _ => Some(ConfigFormat::Auto),
            })
            .unwrap_or(ConfigFormat::Auto);

        let key = input.get("key").and_then(|v| v.as_str());
        let value = input.get("value").cloned();
        let data = input.get("data").cloned();
        let create_dirs = input.get("create_dirs").and_then(|v| v.as_bool()).unwrap_or(false);

        let result = match operation.as_str() {
            "read" => self.read_config(&file_path, format).await,
            "write" => {
                if let Some(d) = data {
                    self.write_config(&file_path, format, &d, create_dirs).await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "data is required for write operation")
                        .with_field("success", false);
                }
            }
            "get" => {
                if let Some(k) = key {
                    self.get_value(&file_path, format, k).await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "key is required for get operation")
                        .with_field("success", false);
                }
            }
            "set" => {
                if let (Some(k), Some(v)) = (key, value) {
                    self.set_value(&file_path, format, k, v, create_dirs).await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "key and value are required for set operation")
                        .with_field("success", false);
                }
            }
            _ => {
                return ToolOutput::new()
                    .with_field("error", format!("Unknown operation: {}", operation))
                    .with_field("success", false);
            }
        };

        ToolOutput::new()
            .with_field("file_path", result.file_path)
            .with_field("operation", result.operation)
            .with_field("format", result.format)
            .with_field("success", result.success)
            .with_field("error", result.error)
            .with_field("data", result.data)
            .with_field("value", result.value)
            .with_field("keys", result.keys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_validation() {
        let tool = ConfigTool::new();

        // Valid read operation
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/config.json")
            .with_arg("operation", "read");
        assert!(tool.validate(&input).await.is_ok());

        // Valid get with key
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/config.json")
            .with_arg("operation", "get")
            .with_arg("key", "database.host");
        assert!(tool.validate(&input).await.is_ok());

        // Missing file_path
        let input = ToolInput::new().with_arg("operation", "read");
        assert!(tool.validate(&input).await.is_err());

        // Invalid operation
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/config.json")
            .with_arg("operation", "invalid");
        assert!(tool.validate(&input).await.is_err());

        // Invalid format
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/config.json")
            .with_arg("operation", "read")
            .with_arg("format", "xml");
        assert!(tool.validate(&input).await.is_err());
    }

    #[test]
    fn test_config_metadata() {
        let tool = ConfigTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "ConfigTool");
    }

    #[test]
    fn test_format_detection() {
        let tool = ConfigTool::new();

        assert_eq!(
            tool.detect_format("config.json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            tool.detect_format("config.yaml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            tool.detect_format("config.yml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            tool.detect_format("config.toml"),
            Some(ConfigFormat::Toml)
        );
        assert_eq!(
            tool.detect_format("config.ini"),
            Some(ConfigFormat::Ini)
        );
        assert_eq!(tool.detect_format("config.unknown"), None);
    }

    #[test]
    fn test_parse_and_serialize_json() {
        let tool = ConfigTool::new();

        let json = r#"{"name": "test", "value": 42, "enabled": true}"#;
        let parsed = tool.parse_config(json, &ConfigFormat::Json).unwrap();

        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], 42);
        assert_eq!(parsed["enabled"], true);

        let serialized = tool.serialize_config(&parsed, &ConfigFormat::Json).unwrap();
        let reparsed: serde_json::Value = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reparsed["name"], "test");
    }

    #[test]
    fn test_parse_and_serialize_toml() {
        let tool = ConfigTool::new();

        let toml = r#"
name = "test"
value = 42
enabled = true

[database]
host = "localhost"
port = 5432
"#;
        let parsed = tool.parse_config(toml, &ConfigFormat::Toml).unwrap();

        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["database"]["host"], "localhost");

        let serialized = tool.serialize_config(&parsed, &ConfigFormat::Toml).unwrap();
        assert!(serialized.contains("name = \"test\""));
        assert!(serialized.contains("[database]"));
    }

    #[test]
    fn test_parse_and_serialize_ini() {
        let tool = ConfigTool::new();

        let ini = r#"
# Global settings
name = test
debug = true

[database]
host = localhost
port = 5432

[cache]
enabled = false
ttl = 3600
"#;
        let parsed = tool.parse_config(ini, &ConfigFormat::Ini).unwrap();

        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["debug"], true);
        assert_eq!(parsed["database"]["host"], "localhost");
        assert_eq!(parsed["database"]["port"], 5432);

        let serialized = tool.serialize_config(&parsed, &ConfigFormat::Ini).unwrap();
        assert!(serialized.contains("name = test"));
        assert!(serialized.contains("[database]"));
    }

    #[test]
    fn test_get_value_at_path() {
        let tool = ConfigTool::new();

        let data = serde_json::json!({
            "database": {
                "host": "localhost",
                "port": 5432,
                "credentials": {
                    "username": "admin",
                    "password": "secret"
                }
            },
            "debug": true
        });

        assert_eq!(
            tool.get_value_at_path(&data, "database.host"),
            Some(serde_json::json!("localhost"))
        );
        assert_eq!(
            tool.get_value_at_path(&data, "database.port"),
            Some(serde_json::json!(5432))
        );
        assert_eq!(
            tool.get_value_at_path(&data, "database.credentials.username"),
            Some(serde_json::json!("admin"))
        );
        assert_eq!(
            tool.get_value_at_path(&data, "debug"),
            Some(serde_json::json!(true))
        );
        assert_eq!(tool.get_value_at_path(&data, "nonexistent"), None);
        assert_eq!(tool.get_value_at_path(&data, "database.nonexistent"), None);
    }

    #[test]
    fn test_set_value_at_path() {
        let tool = ConfigTool::new();

        let mut data = serde_json::json!({
            "database": {
                "host": "localhost"
            }
        });

        tool.set_value_at_path(&mut data, "database.port", serde_json::json!(5432))
            .unwrap();
        assert_eq!(data["database"]["port"], 5432);

        tool.set_value_at_path(&mut data, "debug", serde_json::json!(true))
            .unwrap();
        assert_eq!(data["debug"], true);

        tool.set_value_at_path(
            &mut data,
            "new.nested.key",
            serde_json::json!("value"),
        )
        .unwrap();
        assert_eq!(data["new"]["nested"]["key"], "value");
    }

    #[tokio::test]
    async fn test_read_write_json() {
        let tool = ConfigTool::new();

        let temp_file = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        // Write config
        let data = serde_json::json!({
            "name": "test",
            "value": 42
        });
        let result = tool
            .write_config(file_path, ConfigFormat::Json, &data, false)
            .await;
        assert!(result.success);

        // Read it back
        let result = tool.read_config(file_path, ConfigFormat::Auto).await;
        assert!(result.success);
        assert_eq!(result.data.unwrap()["name"], "test");
    }

    #[tokio::test]
    async fn test_get_and_set_operations() {
        let tool = ConfigTool::new();

        let temp_file = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        // Write initial config
        let data = serde_json::json!({
            "database": {
                "host": "localhost",
                "port": 5432
            }
        });
        tool.write_config(file_path, ConfigFormat::Json, &data, false)
            .await;

        // Get value
        let result = tool
            .get_value(file_path, ConfigFormat::Auto, "database.host")
            .await;
        assert!(result.success);
        assert_eq!(result.value, Some(serde_json::json!("localhost")));

        // Set new value
        let result = tool
            .set_value(
                file_path,
                ConfigFormat::Auto,
                "database.port",
                serde_json::json!(3306),
                false,
            )
            .await;
        assert!(result.success);

        // Verify it was set
        let result = tool
            .get_value(file_path, ConfigFormat::Auto, "database.port")
            .await;
        assert_eq!(result.value, Some(serde_json::json!(3306)));
    }
}
