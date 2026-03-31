//! Settings management with validation
//!
//! Provides:
//! - Settings persistence
//! - Settings validation
//! - Settings source tracking (env, file, default)

use crate::error::{StateError, StateResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Source of a setting value
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    /// Default value
    Default,
    /// From environment variable
    Environment,
    /// From settings file
    File,
    /// From CLI argument
    Cli,
    /// From user prompt/interactive
    Interactive,
}

impl SettingSource {
    /// Priority of this source (higher = overrides lower)
    pub fn priority(&self) -> u8 {
        match self {
            SettingSource::Default => 0,
            SettingSource::File => 10,
            SettingSource::Environment => 20,
            SettingSource::Cli => 30,
            SettingSource::Interactive => 40,
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            SettingSource::Default => "default",
            SettingSource::Environment => "environment",
            SettingSource::File => "settings file",
            SettingSource::Cli => "command line",
            SettingSource::Interactive => "interactive",
        }
    }
}

/// A setting value with its source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingValue {
    pub value: serde_json::Value,
    pub source: SettingSource,
    pub set_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl SettingValue {
    /// Create a new setting value
    pub fn new(value: impl Serialize, source: SettingSource) -> StateResult<Self> {
        Ok(Self {
            value: serde_json::to_value(value)?,
            source,
            set_at: Some(chrono::Utc::now()),
        })
    }

    /// Get value as type T
    pub fn get<T: for<'de> Deserialize<'de>>(&self) -> StateResult<T> {
        serde_json::from_value(self.value.clone())
            .map_err(|e| StateError::Deserialization(e.to_string()))
    }

    /// Check if this value should override another based on source priority
    pub fn should_override(&self, other: &SettingValue) -> bool {
        self.source.priority() >= other.source.priority()
    }
}

/// Settings container
#[derive(Debug, Clone, Default)]
pub struct Settings {
    values: HashMap<String, SettingValue>,
}

impl Settings {
    /// Create empty settings
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Set a value
    pub fn set(&mut self, key: impl Into<String>, value: SettingValue) {
        let key = key.into();
        // Only override if new value has equal or higher priority
        if let Some(existing) = self.values.get(&key) {
            if !value.should_override(existing) {
                return;
            }
        }
        self.values.insert(key, value);
    }

    /// Get a value
    pub fn get(&self, key: &str) -> Option<&SettingValue> {
        self.values.get(key)
    }

    /// Get value as type T
    pub fn get_as<T: for<'de> Deserialize<'de>>(&self, key: &str) -> StateResult<Option<T>> {
        self.get(key)
            .map(|v| v.get::<T>())
            .transpose()
    }

    /// Remove a value
    pub fn remove(&mut self, key: &str) -> Option<SettingValue> {
        self.values.remove(key)
    }

    /// Check if a key exists
    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// Get all keys
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.values.keys()
    }

    /// Merge with another settings object (higher priority wins)
    pub fn merge(&mut self, other: Settings) {
        for (key, value) in other.values {
            self.set(key, value);
        }
    }

    /// Serialize to JSON
    pub fn to_json(&self) -> StateResult<String> {
        let map: HashMap<_, _> = self.values.iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        Ok(serde_json::to_string_pretty(&map)?)
    }
}

/// Settings file structure
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SettingsFile {
    #[serde(flatten)]
    values: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    _version: Option<String>,
}

/// Settings validator function type
type ValidatorFn = Box<dyn Fn(&serde_json::Value) -> Result<(), String> + Send + Sync>;

/// Settings schema entry
pub struct SettingSchema {
    pub key: String,
    pub default: Option<serde_json::Value>,
    pub validator: Option<ValidatorFn>,
    pub description: Option<String>,
}

impl std::fmt::Debug for SettingSchema {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SettingSchema")
            .field("key", &self.key)
            .field("default", &self.default)
            .field("validator", &self.validator.is_some())
            .field("description", &self.description)
            .finish()
    }
}

/// Settings manager
#[derive(Debug, Clone)]
pub struct SettingsManager {
    settings_dir: PathBuf,
    settings: Arc<RwLock<Settings>>,
    schemas: Arc<RwLock<HashMap<String, SettingSchema>>>,
}

impl SettingsManager {
    /// Create a new settings manager
    pub async fn new(state_dir: &Path) -> StateResult<Self> {
        let settings_dir = state_dir.join("settings");
        fs::create_dir_all(&settings_dir).await?;

        let manager = Self {
            settings_dir,
            settings: Arc::new(RwLock::new(Settings::new())),
            schemas: Arc::new(RwLock::new(HashMap::new())),
        };

        // Load default schemas
        manager.register_default_schemas().await?;

        // Load from file
        manager.load_from_file().await?;

        Ok(manager)
    }

    /// Register default setting schemas
    async fn register_default_schemas(&self) -> StateResult<()> {
        let mut schemas = self.schemas.write().await;

        // Common settings
        schemas.insert(
            "auto_memory_enabled".to_string(),
            SettingSchema {
                key: "auto_memory_enabled".to_string(),
                default: Some(serde_json::Value::Bool(true)),
                validator: Some(Box::new(|v| {
                    if v.is_boolean() {
                        Ok(())
                    } else {
                        Err("Must be a boolean".to_string())
                    }
                })),
                description: Some("Enable automatic memory directory".to_string()),
            },
        );

        schemas.insert(
            "verbose".to_string(),
            SettingSchema {
                key: "verbose".to_string(),
                default: Some(serde_json::Value::Bool(false)),
                validator: Some(Box::new(|v| {
                    if v.is_boolean() {
                        Ok(())
                    } else {
                        Err("Must be a boolean".to_string())
                    }
                })),
                description: Some("Enable verbose output".to_string()),
            },
        );

        schemas.insert(
            "model".to_string(),
            SettingSchema {
                key: "model".to_string(),
                default: None,
                validator: Some(Box::new(|v| {
                    if v.is_string() || v.is_null() {
                        Ok(())
                    } else {
                        Err("Must be a string or null".to_string())
                    }
                })),
                description: Some("Default model to use".to_string()),
            },
        );

        schemas.insert(
            "thinking_enabled".to_string(),
            SettingSchema {
                key: "thinking_enabled".to_string(),
                default: Some(serde_json::Value::Bool(false)),
                validator: Some(Box::new(|v| {
                    if v.is_boolean() {
                        Ok(())
                    } else {
                        Err("Must be a boolean".to_string())
                    }
                })),
                description: Some("Enable thinking mode by default".to_string()),
            },
        );

        schemas.insert(
            "permission_mode".to_string(),
            SettingSchema {
                key: "permission_mode".to_string(),
                default: Some(serde_json::Value::String("default".to_string())),
                validator: Some(Box::new(|v| {
                    if let Some(s) = v.as_str() {
                        if ["default", "accept", "reject", "prompt", "read_only"].contains(&s) {
                            Ok(())
                        } else {
                            Err("Invalid permission mode".to_string())
                        }
                    } else {
                        Err("Must be a string".to_string())
                    }
                })),
                description: Some("Default permission mode".to_string()),
            },
        );

        Ok(())
    }

    /// Register a custom schema
    pub async fn register_schema(&self, schema: SettingSchema) -> StateResult<()> {
        let mut schemas = self.schemas.write().await;
        schemas.insert(schema.key.clone(), schema);
        Ok(())
    }

    /// Get a setting value
    pub async fn get(&self, key: &str) -> StateResult<Option<SettingValue>> {
        let settings = self.settings.read().await;

        if let Some(value) = settings.get(key) {
            return Ok(Some(value.clone()));
        }

        // Return default from schema
        let schemas = self.schemas.read().await;
        if let Some(schema) = schemas.get(key) {
            if let Some(default) = &schema.default {
                return Ok(Some(SettingValue {
                    value: default.clone(),
                    source: SettingSource::Default,
                    set_at: None,
                }));
            }
        }

        Ok(None)
    }

    /// Get a setting value as type T
    pub async fn get_as<T: for<'de> Deserialize<'de>>(&self, key: &str) -> StateResult<Option<T>> {
        self.get(key).await?.map(|v| v.get::<T>()).transpose()
    }

    /// Set a setting value
    pub async fn set(&self, key: impl Into<String>, value: SettingValue) -> StateResult<()> {
        let key = key.into();

        // Validate against schema
        self.validate(&key, &value.value).await?;

        let mut settings = self.settings.write().await;
        settings.set(key, value);

        Ok(())
    }

    /// Set from any serializable value with source
    pub async fn set_value(
        &self,
        key: impl Into<String>,
        value: impl Serialize,
        source: SettingSource,
    ) -> StateResult<()> {
        let value = SettingValue::new(value, source)?;
        self.set(key, value).await
    }

    /// Validate a value against schema
    async fn validate(&self, key: &str, value: &serde_json::Value) -> StateResult<()> {
        let schemas = self.schemas.read().await;
        if let Some(schema) = schemas.get(key) {
            if let Some(validator) = &schema.validator {
                validator(value).map_err(StateError::Validation)?;
            }
        }
        Ok(())
    }

    /// Remove a setting
    pub async fn remove(&self, key: &str) -> StateResult<Option<SettingValue>> {
        let mut settings = self.settings.write().await;
        Ok(settings.remove(key))
    }

    /// Get all settings as JSON
    pub async fn to_json(&self) -> StateResult<String> {
        let settings = self.settings.read().await;
        settings.to_json()
    }

    /// Load settings from file
    pub async fn load_from_file(&self) -> StateResult<()> {
        let path = self.settings_file_path();
        if !path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&path).await?;
        let file: SettingsFile = serde_json::from_str(&content)?;

        let mut settings = self.settings.write().await;
        for (key, value) in file.values {
            let setting_value = SettingValue {
                value,
                source: SettingSource::File,
                set_at: Some(chrono::Utc::now()),
            };
            settings.set(key, setting_value);
        }

        Ok(())
    }

    /// Save settings to file
    pub async fn save_to_file(&self) -> StateResult<()> {
        let settings = self.settings.read().await;

        let mut values = HashMap::new();
        for (key, value) in settings.keys().map(|k| (k, settings.get(k).unwrap())) {
            // Only save non-default values
            if value.source != SettingSource::Default {
                values.insert(key.clone(), value.value.clone());
            }
        }

        let file = SettingsFile {
            values,
            _version: Some("1.0".to_string()),
        };

        let path = self.settings_file_path();
        let content = serde_json::to_string_pretty(&file)?;
        fs::write(path, content).await?;

        Ok(())
    }

    /// Get all setting keys
    pub async fn keys(&self) -> Vec<String> {
        let settings = self.settings.read().await;
        settings.keys().cloned().collect()
    }

    /// Get all settings with their sources
    pub async fn all(&self) -> Vec<(String, SettingValue)> {
        let settings = self.settings.read().await;
        settings
            .keys()
            .map(|k| (k.clone(), settings.get(k).unwrap().clone()))
            .collect()
    }

    /// Get settings file path
    fn settings_file_path(&self) -> PathBuf {
        self.settings_dir.join("settings.json")
    }

    /// Reset to defaults
    pub async fn reset(&self) -> StateResult<()> {
        let mut settings = self.settings.write().await;
        *settings = Settings::new();

        let path = self.settings_file_path();
        if path.exists() {
            fs::remove_file(path).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_setting_source_priority() {
        assert!(SettingSource::Cli.priority() > SettingSource::Environment.priority());
        assert!(SettingSource::Environment.priority() > SettingSource::File.priority());
        assert!(SettingSource::File.priority() > SettingSource::Default.priority());
    }

    #[test]
    fn test_setting_value_override() {
        let default = SettingValue::new(false, SettingSource::Default).unwrap();
        let env = SettingValue::new(true, SettingSource::Environment).unwrap();

        assert!(env.should_override(&default));
        assert!(!default.should_override(&env));
    }

    #[tokio::test]
    async fn test_settings_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SettingsManager::new(temp_dir.path()).await.unwrap();

        // Test default
        let auto_memory = manager.get("auto_memory_enabled").await.unwrap();
        assert!(auto_memory.is_some());
        assert_eq!(auto_memory.unwrap().source, SettingSource::Default);

        // Test set and get
        manager
            .set_value("verbose", true, SettingSource::File)
            .await
            .unwrap();

        let verbose = manager.get_as::<bool>("verbose").await.unwrap();
        assert_eq!(verbose, Some(true));
    }

    #[tokio::test]
    async fn test_settings_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Set and save
        {
            let manager = SettingsManager::new(temp_dir.path()).await.unwrap();
            manager
                .set_value("model", "claude-4-opus", SettingSource::File)
                .await
                .unwrap();
            manager.save_to_file().await.unwrap();
        }

        // Load in new instance
        {
            let manager = SettingsManager::new(temp_dir.path()).await.unwrap();
            let model = manager.get_as::<String>("model").await.unwrap();
            assert_eq!(model, Some("claude-4-opus".to_string()));
        }
    }

    #[tokio::test]
    async fn test_validation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SettingsManager::new(temp_dir.path()).await.unwrap();

        // Valid permission mode
        manager
            .set_value("permission_mode", "accept", SettingSource::File)
            .await
            .unwrap();

        // Invalid value should fail validation
        let result = manager
            .set_value("permission_mode", "invalid", SettingSource::File)
            .await;
        assert!(result.is_err());
    }
}
