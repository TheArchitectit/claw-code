//! Plugin state management
//!
//! Provides:
//! - Plugin installation tracking
//! - Plugin state persistence
//! - Plugin enable/disable management
//! - Plugin error tracking

use crate::error::{StateError, StateResult};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;

/// Plugin installation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallStatus {
    Pending,
    Installing,
    Installed,
    Failed,
}

/// Plugin state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginState {
    pub id: String,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub install_status: PluginInstallStatus,
    pub install_error: Option<String>,
    pub manifest_path: Option<PathBuf>,
    pub source: String, // e.g., "marketplace", "local", "bundled"
    pub marketplace_id: Option<String>,
    /// Plugin-specific configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Plugin hooks registration
    pub hooks: Vec<String>,
}

impl PluginState {
    /// Create new plugin state
    pub fn new(id: impl Into<String>, name: impl Into<String>, version: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            version: version.into(),
            enabled: false,
            installed_at: now,
            updated_at: now,
            install_status: PluginInstallStatus::Pending,
            install_error: None,
            manifest_path: None,
            source: "unknown".to_string(),
            marketplace_id: None,
            config: HashMap::new(),
            hooks: Vec::new(),
        }
    }

    /// Set installation status
    pub fn set_status(&mut self, status: PluginInstallStatus) {
        self.install_status = status;
        self.updated_at = Utc::now();
        if status == PluginInstallStatus::Installed {
            self.install_error = None;
        }
    }

    /// Set installation error
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.install_error = Some(error.into());
        self.install_status = PluginInstallStatus::Failed;
        self.updated_at = Utc::now();
    }

    /// Enable the plugin
    pub fn enable(&mut self) {
        self.enabled = true;
        self.updated_at = Utc::now();
    }

    /// Disable the plugin
    pub fn disable(&mut self) {
        self.enabled = false;
        self.updated_at = Utc::now();
    }

    /// Set config value
    pub fn set_config(&mut self, key: impl Into<String>, value: impl Serialize) -> StateResult<()> {
        let value = serde_json::to_value(value)?;
        self.config.insert(key.into(), value);
        self.updated_at = Utc::now();
        Ok(())
    }

    /// Get config value
    pub fn get_config<T: for<'de> Deserialize<'de>>(&self, key: &str) -> StateResult<Option<T>> {
        self.config
            .get(key)
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|e| StateError::Deserialization(e.to_string()))
    }

    /// Add a hook
    pub fn add_hook(&mut self, hook: impl Into<String>) {
        let hook = hook.into();
        if !self.hooks.contains(&hook) {
            self.hooks.push(hook);
        }
        self.updated_at = Utc::now();
    }

    /// Remove a hook
    pub fn remove_hook(&mut self, hook: &str) {
        self.hooks.retain(|h| h != hook);
        self.updated_at = Utc::now();
    }

    /// Check if plugin is fully installed and ready
    pub fn is_ready(&self) -> bool {
        self.enabled && self.install_status == PluginInstallStatus::Installed
    }
}

/// Plugin installation tracking
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PluginInstallationState {
    pub plugins: HashMap<String, PluginState>,
    pub marketplaces: HashMap<String, MarketplaceState>,
    pub needs_refresh: bool,
    pub last_refresh: Option<DateTime<Utc>>,
}

/// Marketplace state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketplaceState {
    pub name: String,
    pub url: String,
    pub status: PluginInstallStatus,
    pub installed_at: DateTime<Utc>,
    pub error: Option<String>,
}

/// Plugin state manager
#[derive(Debug, Clone)]
pub struct PluginStateManager {
    plugins_dir: PathBuf,
    state: Arc<RwLock<PluginInstallationState>>,
    state_path: PathBuf,
}

impl PluginStateManager {
    /// Create a new plugin state manager
    pub async fn new(state_dir: &Path) -> StateResult<Self> {
        let plugins_dir = state_dir.join("plugins");
        fs::create_dir_all(&plugins_dir).await?;

        let state_path = plugins_dir.join("plugins.json");

        let state = if state_path.exists() {
            let content = fs::read_to_string(&state_path).await?;
            serde_json::from_str(&content)?
        } else {
            PluginInstallationState::default()
        };

        Ok(Self {
            plugins_dir,
            state: Arc::new(RwLock::new(state)),
            state_path,
        })
    }

    /// Register a new plugin
    pub async fn register(&self, plugin: PluginState) -> StateResult<()> {
        let mut state = self.state.write().await;
        state.plugins.insert(plugin.id.clone(), plugin);
        self.persist_state(&state).await?;
        Ok(())
    }

    /// Get a plugin by ID
    pub async fn get(&self, id: &str) -> StateResult<Option<PluginState>> {
        let state = self.state.read().await;
        Ok(state.plugins.get(id).cloned())
    }

    /// Update a plugin
    pub async fn update<F>(&self, id: &str, updater: F) -> StateResult<()>
    where
        F: FnOnce(&mut PluginState),
    {
        let mut state = self.state.write().await;

        if let Some(plugin) = state.plugins.get_mut(id) {
            updater(plugin);
            self.persist_state(&state).await?;
            Ok(())
        } else {
            Err(StateError::PluginNotFound(id.to_string()))
        }
    }

    /// Remove a plugin
    pub async fn remove(&self, id: &str) -> StateResult<()> {
        let mut state = self.state.write().await;
        state.plugins.remove(id);
        self.persist_state(&state).await?;
        Ok(())
    }

    /// Enable a plugin
    pub async fn enable(&self, id: &str) -> StateResult<()> {
        self.update(id, |p| p.enable()).await
    }

    /// Disable a plugin
    pub async fn disable(&self, id: &str) -> StateResult<()> {
        self.update(id, |p| p.disable()).await
    }

    /// List all plugins
    pub async fn list(&self) -> StateResult<Vec<PluginState>> {
        let state = self.state.read().await;
        Ok(state.plugins.values().cloned().collect())
    }

    /// List enabled plugins
    pub async fn list_enabled(&self) -> StateResult<Vec<PluginState>> {
        let all = self.list().await?;
        Ok(all.into_iter().filter(|p| p.enabled).collect())
    }

    /// List ready plugins (enabled and installed)
    pub async fn list_ready(&self) -> StateResult<Vec<PluginState>> {
        let all = self.list().await?;
        Ok(all.into_iter().filter(|p| p.is_ready()).collect())
    }

    /// List plugins by installation status
    pub async fn list_by_status(&self, status: PluginInstallStatus) -> StateResult<Vec<PluginState>> {
        let all = self.list().await?;
        Ok(all.into_iter().filter(|p| p.install_status == status).collect())
    }

    /// Add a marketplace
    pub async fn add_marketplace(&self, marketplace: MarketplaceState) -> StateResult<()> {
        let mut state = self.state.write().await;
        state.marketplaces.insert(marketplace.name.clone(), marketplace);
        self.persist_state(&state).await?;
        Ok(())
    }

    /// Get a marketplace
    pub async fn get_marketplace(&self, name: &str) -> StateResult<Option<MarketplaceState>> {
        let state = self.state.read().await;
        Ok(state.marketplaces.get(name).cloned())
    }

    /// List all marketplaces
    pub async fn list_marketplaces(&self) -> StateResult<Vec<MarketplaceState>> {
        let state = self.state.read().await;
        Ok(state.marketplaces.values().cloned().collect())
    }

    /// Set refresh flag
    pub async fn set_needs_refresh(&self, needs: bool) -> StateResult<()> {
        let mut state = self.state.write().await;
        state.needs_refresh = needs;
        if !needs {
            state.last_refresh = Some(Utc::now());
        }
        self.persist_state(&state).await?;
        Ok(())
    }

    /// Check if refresh is needed
    pub async fn needs_refresh(&self) -> bool {
        let state = self.state.read().await;
        state.needs_refresh
    }

    /// Get last refresh time
    pub async fn last_refresh(&self) -> Option<DateTime<Utc>> {
        let state = self.state.read().await;
        state.last_refresh
    }

    /// Save all state to disk
    pub async fn save_all(&self) -> StateResult<()> {
        let state = self.state.read().await;
        self.persist_state(&state).await
    }

    /// Load all state from disk
    pub async fn load_all(&self) -> StateResult<()> {
        if self.state_path.exists() {
            let content = fs::read_to_string(&self.state_path).await?;
            let loaded: PluginInstallationState = serde_json::from_str(&content)?;

            let mut state = self.state.write().await;
            *state = loaded;
        }
        Ok(())
    }

    /// Persist state to disk
    async fn persist_state(&self, state: &PluginInstallationState) -> StateResult<()> {
        let content = serde_json::to_string_pretty(state)?;
        fs::write(&self.state_path, content).await?;
        Ok(())
    }

    /// Get the plugins directory
    pub fn plugins_dir(&self) -> &Path {
        &self.plugins_dir
    }

    /// Create a plugin directory
    pub async fn create_plugin_dir(&self, plugin_id: &str) -> StateResult<PathBuf> {
        let dir = self.plugins_dir.join(plugin_id);
        fs::create_dir_all(&dir).await?;
        Ok(dir)
    }

    /// Clear installation errors
    pub async fn clear_errors(&self) -> StateResult<()> {
        let mut state = self.state.write().await;
        for plugin in state.plugins.values_mut() {
            if plugin.install_status == PluginInstallStatus::Failed {
                plugin.install_error = None;
                plugin.install_status = PluginInstallStatus::Pending;
            }
        }
        self.persist_state(&state).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_plugin_state_creation() {
        let plugin = PluginState::new("test-id", "Test Plugin", "1.0.0");
        assert_eq!(plugin.id, "test-id");
        assert!(!plugin.enabled);
        assert_eq!(plugin.install_status, PluginInstallStatus::Pending);
    }

    #[test]
    fn test_plugin_state_transitions() {
        let mut plugin = PluginState::new("test", "Test", "1.0.0");

        plugin.set_status(PluginInstallStatus::Installing);
        assert_eq!(plugin.install_status, PluginInstallStatus::Installing);

        plugin.set_status(PluginInstallStatus::Installed);
        assert_eq!(plugin.install_status, PluginInstallStatus::Installed);
        assert!(plugin.install_error.is_none());

        plugin.set_error("Install failed");
        assert_eq!(plugin.install_status, PluginInstallStatus::Failed);
        assert!(plugin.install_error.is_some());
    }

    #[test]
    fn test_plugin_ready() {
        let mut plugin = PluginState::new("test", "Test", "1.0.0");
        assert!(!plugin.is_ready());

        plugin.enable();
        assert!(!plugin.is_ready()); // Not installed yet

        plugin.set_status(PluginInstallStatus::Installed);
        assert!(plugin.is_ready());
    }

    #[tokio::test]
    async fn test_plugin_manager() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginStateManager::new(temp_dir.path()).await.unwrap();

        // Register plugin
        let plugin = PluginState::new("test-id", "Test Plugin", "1.0.0");
        manager.register(plugin).await.unwrap();

        // Get plugin
        let retrieved = manager.get("test-id").await.unwrap();
        assert!(retrieved.is_some());

        // Enable
        manager.enable("test-id").await.unwrap();
        let plugin = manager.get("test-id").await.unwrap().unwrap();
        assert!(plugin.enabled);

        // List
        let list = manager.list().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_plugin_persistence() {
        let temp_dir = TempDir::new().unwrap();

        // Register and save
        {
            let manager = PluginStateManager::new(temp_dir.path()).await.unwrap();
            let plugin = PluginState::new("test", "Test", "1.0.0");
            manager.register(plugin).await.unwrap();
            manager.save_all().await.unwrap();
        }

        // Load in new instance
        {
            let manager = PluginStateManager::new(temp_dir.path()).await.unwrap();
            manager.load_all().await.unwrap();

            let plugin = manager.get("test").await.unwrap();
            assert!(plugin.is_some());
        }
    }

    #[tokio::test]
    async fn test_marketplace_management() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginStateManager::new(temp_dir.path()).await.unwrap();

        let marketplace = MarketplaceState {
            name: "test-market".to_string(),
            url: "https://example.com".to_string(),
            status: PluginInstallStatus::Installed,
            installed_at: Utc::now(),
            error: None,
        };

        manager.add_marketplace(marketplace).await.unwrap();

        let list = manager.list_marketplaces().await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_needs_refresh() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PluginStateManager::new(temp_dir.path()).await.unwrap();

        assert!(!manager.needs_refresh().await);

        manager.set_needs_refresh(true).await.unwrap();
        assert!(manager.needs_refresh().await);

        manager.set_needs_refresh(false).await.unwrap();
        assert!(!manager.needs_refresh().await);
        assert!(manager.last_refresh().await.is_some());
    }
}
