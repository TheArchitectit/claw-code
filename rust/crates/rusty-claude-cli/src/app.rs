use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::cli::{Cli, PermissionMode};

/// Application state and configuration
#[derive(Clone)]
pub struct AppState {
    /// Session identifier
    pub session_id: String,
    /// Current working directory
    pub cwd: PathBuf,
    /// Configuration
    pub config: AppConfig,
    /// Runtime state
    pub runtime: RuntimeState,
    /// Analytics client
    pub analytics: services::AnalyticsClient,
    /// OAuth manager
    pub oauth: Arc<services::OAuthManager>,
    /// Telemetry instance
    pub telemetry: Option<Arc<services::Telemetry>>,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("session_id", &self.session_id)
            .field("cwd", &self.cwd)
            .field("config", &self.config)
            .field("runtime", &self.runtime)
            .field("analytics", &self.analytics)
            .field("oauth", &self.oauth)
            .field("telemetry", &self.telemetry.is_some())
            .finish()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppConfig {
    /// Selected model
    pub model: Option<String>,
    /// Effort level
    pub effort: Option<String>,
    /// Permission mode
    pub permission_mode: PermissionMode,
    /// Verbose output
    pub verbose: bool,
    /// Debug mode
    pub debug: bool,
    /// Plugin directories
    pub plugin_dirs: Vec<PathBuf>,
    /// Additional allowed directories
    pub allowed_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeState {
    /// Whether the runtime is initialized
    pub initialized: bool,
    /// Pending operations
    pub pending_operations: Vec<String>,
    /// Active tools
    pub active_tools: Vec<String>,
}

impl AppState {
    /// Initialize the application state from CLI arguments
    pub async fn from_cli(cli: &Cli) -> anyhow::Result<Self> {
        debug!("Initializing application state from CLI");

        let session_id = generate_session_id();
        let cwd = std::env::current_dir()?;

        let config = AppConfig {
            model: cli.model.clone(),
            effort: cli.effort.map(|e| e.to_string()),
            permission_mode: cli.permission_mode.unwrap_or(PermissionMode::Prompt),
            verbose: cli.verbose,
            debug: cli.debug.is_some(),
            plugin_dirs: cli.plugin_dir.clone(),
            allowed_dirs: cli.add_dir.clone(),
        };

        // Initialize analytics
        let analytics_config = services::AnalyticsConfig {
            enabled: true,
            service_name: "rusty-claude".to_string(),
            session_id: Some(session_id.clone()),
            user_id: None,
            tags: std::collections::HashMap::new(),
            endpoint: None,
            sampling_rate: 1.0,
            console_output: true,
        };
        let analytics = services::AnalyticsClient::with_config(analytics_config);

        // Initialize OAuth manager
        let oauth = Arc::new(services::OAuthManager::default());

        // Initialize telemetry (optional)
        let telemetry = match init_telemetry(&session_id).await {
            Ok(t) => {
                info!("Telemetry initialized");
                Some(t)
            }
            Err(e) => {
                debug!("Failed to initialize telemetry: {}", e);
                None
            }
        };

        let state = Self {
            session_id,
            cwd,
            config,
            runtime: RuntimeState::default(),
            analytics,
            oauth,
            telemetry,
        };

        info!("Application state initialized: session_id={}", state.session_id);
        Ok(state)
    }

    /// Track a tool execution via analytics
    pub async fn track_tool(&self, tool_name: &str, duration_ms: u64, success: bool) {
        if let Err(e) = self.analytics.track_tool(tool_name, duration_ms, success).await {
            debug!("Failed to track tool usage: {}", e);
        }
    }

    /// Track a command execution via analytics
    pub async fn track_command(&self, command: &str, args: &[&str]) {
        if let Err(e) = self.analytics.track_command(command, args).await {
            debug!("Failed to track command: {}", e);
        }
    }

    /// Track an error via analytics
    pub async fn track_error(&self, error_type: &str, message: &str) {
        if let Err(e) = self.analytics.track_error(error_type, message).await {
            debug!("Failed to track error: {}", e);
        }
    }

    /// End the session and flush analytics
    pub async fn end_session(&self) -> anyhow::Result<()> {
        info!("Ending session: {}", self.session_id);
        self.analytics.end_session().await?;
        if let Some(ref telemetry) = self.telemetry {
            let _ = telemetry.shutdown().await;
        }
        Ok(())
    }

    /// Get the config directory path
    pub fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|d| d.join("claude"))
    }

    /// Get the data directory path
    pub fn data_dir() -> Option<PathBuf> {
        dirs::data_dir().map(|d| d.join("claude"))
    }

    /// Load global configuration from disk
    pub fn load_global_config() -> anyhow::Result<GlobalConfig> {
        if let Some(config_dir) = Self::config_dir() {
            let config_file = config_dir.join("config.json");
            if config_file.exists() {
                let content = std::fs::read_to_string(&config_file)?;
                let config: GlobalConfig = serde_json::from_str(&content)?;
                return Ok(config);
            }
        }

        Ok(GlobalConfig::default())
    }

    /// Save global configuration to disk
    pub fn save_global_config(config: &GlobalConfig) -> anyhow::Result<()> {
        if let Some(config_dir) = Self::config_dir() {
            std::fs::create_dir_all(&config_dir)?;
            let config_file = config_dir.join("config.json");
            let content = serde_json::to_string_pretty(config)?;
            std::fs::write(&config_file, content)?;
        }
        Ok(())
    }
}

/// Initialize telemetry with default configuration
async fn init_telemetry(session_id: &str) -> anyhow::Result<Arc<services::Telemetry>> {
    let config = services::TelemetryConfig::new("rusty-claude")
        .with_session_id(session_id)
        .with_environment(if cfg!(debug_assertions) {
            "development"
        } else {
            "production"
        })
        .with_console_output(true);

    let telemetry = services::Telemetry::init(config).await?;
    Ok(Arc::new(telemetry))
}

/// Global configuration (saved to disk)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default model
    pub default_model: Option<String>,
    /// Default permission mode
    pub default_permission_mode: Option<PermissionMode>,
    /// API key (encrypted)
    pub api_key: Option<String>,
    /// OAuth token
    pub oauth_token: Option<String>,
    /// Migration version
    pub migration_version: Option<u32>,
    /// Verbose mode default
    pub verbose: Option<bool>,
    /// Analytics configuration
    pub analytics: Option<services::AnalyticsConfig>,
    /// Telemetry configuration
    pub telemetry: Option<services::TelemetryConfig>,
    /// Custom settings
    #[serde(flatten)]
    pub custom: serde_json::Map<String, serde_json::Value>,
}

/// Generate a unique session ID
fn generate_session_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let random: u32 = rand::random();

    format!("session_{:x}_{:x}", timestamp, random)
}

/// Initialize logging based on CLI arguments
pub fn init_logging(cli: &Cli) -> anyhow::Result<()> {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    let filter = if cli.debug.is_some() {
        EnvFilter::new("debug")
    } else if cli.verbose {
        EnvFilter::new("info")
    } else {
        EnvFilter::new("warn")
    };

    let fmt_layer = tracing_subscriber::fmt::layer().with_target(false);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();
        assert!(!id1.is_empty());
        assert!(!id2.is_empty());
        assert_ne!(id1, id2); // Should be unique
    }

    #[test]
    fn test_global_config_default() {
        let config = GlobalConfig::default();
        assert!(config.default_model.is_none());
        assert!(config.api_key.is_none());
    }
}
