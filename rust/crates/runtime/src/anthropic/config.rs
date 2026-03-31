//! Anthropic API client configuration.

use std::time::Duration;

use crate::anthropic::error::AnthropicError;

/// The Anthropic API base URL.
const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";

/// Default model to use.
const DEFAULT_MODEL: &str = "claude-sonnet-4-6";

/// Anthropic API client configuration.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    /// The API base URL.
    pub api_base: String,
    /// The API key.
    pub api_key: String,
    /// Default model.
    pub default_model: String,
    /// Request timeout in seconds.
    pub timeout: Duration,
    /// API version.
    pub api_version: String,
}

impl AnthropicConfig {
    /// Create a new config with the given API key.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_base: ANTHROPIC_API_BASE.to_string(),
            api_key: api_key.into(),
            default_model: DEFAULT_MODEL.to_string(),
            timeout: Duration::from_secs(120),
            api_version: "2023-06-01".to_string(),
        }
    }

    /// Load config from environment variable.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("ANTHROPIC_API_KEY").ok().map(Self::new)
    }

    /// Set the API base URL.
    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// Set the default model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set the timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// Anthropic API client.
#[derive(Debug, Clone)]
pub struct AnthropicClient {
    pub(crate) config: AnthropicConfig,
    pub(crate) http_client: reqwest::Client,
}

impl AnthropicClient {
    /// Create a new Anthropic client.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be created.
    pub fn new(config: AnthropicConfig) -> Result<Self, AnthropicError> {
        let http_client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| AnthropicError::Http(e.to_string()))?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// Create a new Anthropic client from environment.
    ///
    /// # Errors
    /// Returns an error if ANTHROPIC_API_KEY is not set.
    pub fn from_env() -> Result<Self, AnthropicError> {
        let config = AnthropicConfig::from_env()
            .ok_or(AnthropicError::MissingApiKey)?;
        Self::new(config)
    }

    /// Get the API headers.
    pub(crate) fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-api-key",
            reqwest::header::HeaderValue::from_str(&self.config.api_key)
                .expect("API key must be valid ASCII"),
        );
        headers.insert(
            "anthropic-version",
            reqwest::header::HeaderValue::from_static("2023-06-01"),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json"),
        );
        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_config_new() {
        let config = AnthropicConfig::new("test-key");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.api_base, ANTHROPIC_API_BASE);
        assert_eq!(config.default_model, DEFAULT_MODEL);
    }

    #[test]
    fn test_anthropic_config_builder() {
        let config = AnthropicConfig::new("test-key")
            .with_api_base("https://custom.api.com")
            .with_model("claude-opus-4");

        assert_eq!(config.api_base, "https://custom.api.com");
        assert_eq!(config.default_model, "claude-opus-4");
    }

    #[test]
    fn test_client_new() {
        let config = AnthropicConfig::new("test-key");
        let client = AnthropicClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_headers() {
        let config = AnthropicConfig::new("test-key");
        let client = AnthropicClient::new(config).unwrap();
        let headers = client.headers();

        assert_eq!(
            headers.get("x-api-key").unwrap().to_str().unwrap(),
            "test-key"
        );
        assert_eq!(
            headers.get("anthropic-version").unwrap().to_str().unwrap(),
            "2023-06-01"
        );
    }
}
