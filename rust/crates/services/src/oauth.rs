//! OAuth 2.0 Authentication Module
//!
//! Provides OAuth 2.0 flows for GitHub and Google with:
//! - Token storage and refresh
//! - Secure storage using system keyring or encrypted file fallback
//! - PKCE (Proof Key for Code Exchange) for security
//!
//! # Example
//! ```rust
//! use services::oauth::{OAuthManager, OAuthProvider};
//!
//! async fn authenticate() -> anyhow::Result<()> {
//!     let manager = OAuthManager::new().await?;
//!
//!     // Start GitHub OAuth flow
//!     let token = manager.authenticate(OAuthProvider::GitHub).await?;
//!     println!("Authenticated as: {}", token.user_info.name);
//!
//!     // Token is automatically stored securely
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Duration, Utc};
use oauth2::{
    basic::{BasicClient, BasicTokenResponse},
    reqwest::async_http_client,
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use ring::rand::SecureRandom;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

mod secure_storage;
pub use secure_storage::{
    SecureStorage, StorageBackend, KeyringStorage, EncryptedFileStorage,
    EnvironmentStorage, create_default_storage, create_storage
};

/// Supported OAuth providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OAuthProvider {
    /// GitHub OAuth
    #[serde(rename = "github")]
    GitHub,
    /// Google OAuth
    #[serde(rename = "google")]
    Google,
}

impl OAuthProvider {
    /// Get the provider name
    pub fn name(&self) -> &'static str {
        match self {
            OAuthProvider::GitHub => "GitHub",
            OAuthProvider::Google => "Google",
        }
    }

    /// Get the provider identifier (used in storage keys)
    pub fn id(&self) -> &'static str {
        match self {
            OAuthProvider::GitHub => "github",
            OAuthProvider::Google => "google",
        }
    }

    /// Default OAuth2 endpoints for each provider
    fn default_endpoints(&self) -> ProviderEndpoints {
        match self {
            OAuthProvider::GitHub => ProviderEndpoints {
                auth_url: "https://github.com/login/oauth/authorize".to_string(),
                token_url: "https://github.com/login/oauth/access_token".to_string(),
                user_info_url: Some("https://api.github.com/user".to_string()),
                scopes: vec!["read:user".to_string(), "user:email".to_string()],
            },
            OAuthProvider::Google => ProviderEndpoints {
                auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
                token_url: "https://oauth2.googleapis.com/token".to_string(),
                user_info_url: Some("https://www.googleapis.com/oauth2/v2/userinfo".to_string()),
                scopes: vec!["openid".to_string(), "profile".to_string(), "email".to_string()],
            },
        }
    }
}

/// OAuth2 endpoints for a provider
#[derive(Debug, Clone)]
struct ProviderEndpoints {
    auth_url: String,
    token_url: String,
    user_info_url: Option<String>,
    scopes: Vec<String>,
}

/// OAuth token with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    /// Access token
    pub access_token: String,
    /// Refresh token (if available)
    pub refresh_token: Option<String>,
    /// Token type (e.g., "Bearer")
    pub token_type: String,
    /// Expiration time
    pub expires_at: Option<DateTime<Utc>>,
    /// Provider
    pub provider: OAuthProvider,
    /// User information
    pub user_info: UserInfo,
    /// When the token was obtained
    pub obtained_at: DateTime<Utc>,
}

impl OAuthToken {
    /// Check if the token is expired (or about to expire within 5 minutes)
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Utc::now() > expires_at - Duration::minutes(5)
        } else {
            // No expiration, assume valid
            false
        }
    }

    /// Check if the token can be refreshed
    pub fn can_refresh(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// User information from OAuth provider
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID from provider
    pub id: String,
    /// Username/login
    pub username: String,
    /// Display name
    pub name: String,
    /// Email address
    pub email: Option<String>,
    /// Avatar URL
    pub avatar_url: Option<String>,
}

/// OAuth configuration for a provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    /// OAuth provider
    pub provider: OAuthProvider,
    /// Client ID
    pub client_id: String,
    /// Client secret
    pub client_secret: String,
    /// Redirect URL (defaults to http://localhost:8765/callback)
    pub redirect_url: Option<String>,
    /// Custom scopes (defaults to provider defaults)
    pub scopes: Option<Vec<String>>,
    /// Custom authorization URL
    pub auth_url: Option<String>,
    /// Custom token URL
    pub token_url: Option<String>,
}

impl OAuthConfig {
    /// Create a new OAuth configuration
    pub fn new(provider: OAuthProvider, client_id: String, client_secret: String) -> Self {
        Self {
            provider,
            client_id,
            client_secret,
            redirect_url: None,
            scopes: None,
            auth_url: None,
            token_url: None,
        }
    }

    /// Set custom redirect URL
    pub fn with_redirect_url(mut self, url: String) -> Self {
        self.redirect_url = Some(url);
        self
    }

    /// Set custom scopes
    pub fn with_scopes(mut self, scopes: Vec<String>) -> Self {
        self.scopes = Some(scopes);
        self
    }
}

/// OAuth error types
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Token expired and refresh failed
    #[error("Token expired and refresh failed: {0}")]
    RefreshFailed(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Network error
    #[error("Network error: {0}")]
    NetworkError(String),

    /// User cancelled
    #[error("Authentication cancelled by user")]
    Cancelled,
}

/// OAuth callback result
#[derive(Debug, Clone)]
struct CallbackResult {
    code: AuthorizationCode,
    state: CsrfToken,
}

/// Internal OAuth client for a provider
#[derive(Debug, Clone)]
struct OAuthClient {
    provider: OAuthProvider,
    client: BasicClient,
    endpoints: ProviderEndpoints,
    config: OAuthConfig,
}

impl OAuthClient {
    fn new(config: OAuthConfig) -> Result<Self> {
        let endpoints = if let (Some(auth), Some(token)) = (&config.auth_url, &config.token_url) {
            ProviderEndpoints {
                auth_url: auth.clone(),
                token_url: token.clone(),
                user_info_url: config.provider.default_endpoints().user_info_url,
                scopes: config
                    .scopes
                    .as_ref()
                    .map(|s| s.iter().map(|s| s.to_string()).collect())
                    .unwrap_or_else(|| config.provider.default_endpoints().scopes.iter().map(|s| s.to_string()).collect()),
            }
        } else {
            config.provider.default_endpoints()
        };

        let auth_url = AuthUrl::new(endpoints.auth_url.clone())
            .map_err(|e| OAuthError::ConfigError(format!("Invalid auth URL: {}", e)))?;
        let token_url = TokenUrl::new(endpoints.token_url.clone())
            .map_err(|e| OAuthError::ConfigError(format!("Invalid token URL: {}", e)))?;

        let redirect_url = RedirectUrl::new(
            config
                .redirect_url
                .clone()
                .unwrap_or_else(|| "http://localhost:8765/callback".to_string()),
        )
        .map_err(|e| OAuthError::ConfigError(format!("Invalid redirect URL: {}", e)))?;

        let client = BasicClient::new(
            ClientId::new(config.client_id.clone()),
            Some(ClientSecret::new(config.client_secret.clone())),
            auth_url,
            Some(token_url),
        )
        .set_redirect_uri(redirect_url);

        Ok(Self {
            provider: config.provider,
            client,
            endpoints,
            config,
        })
    }

    /// Generate PKCE challenge and return (verifier, challenge)
    fn generate_pkce(&self) -> Result<(PkceCodeVerifier, PkceCodeChallenge)> {
        let rng = ring::rand::SystemRandom::new();
        let mut verifier_bytes = [0u8; 32];
        rng.fill(&mut verifier_bytes)
            .map_err(|_| OAuthError::ConfigError("Failed to generate PKCE verifier".to_string()))?;

        let verifier = PkceCodeVerifier::new(
            URL_SAFE_NO_PAD.encode(verifier_bytes).trim_end_matches('=').to_string()
        );

        let challenge = PkceCodeChallenge::from_code_verifier_sha256(&verifier);

        Ok((verifier, challenge))
    }

    /// Build authorization URL
    fn authorization_url(&self, challenge: &PkceCodeChallenge) -> (String, CsrfToken) {
        let mut url_builder = self
            .client
            .authorize_url(CsrfToken::new_random)
            .set_pkce_challenge(challenge.clone());

        for scope in &self.endpoints.scopes {
            url_builder = url_builder.add_scope(Scope::new(scope.to_string()));
        }

        let (url, csrf_token) = url_builder.url();
        (url.to_string(), csrf_token)
    }

    /// Exchange code for token
    async fn exchange_code(
        &self,
        code: AuthorizationCode,
        verifier: PkceCodeVerifier,
    ) -> Result<BasicTokenResponse> {
        let token_response = self
            .client
            .exchange_code(code)
            .set_pkce_verifier(verifier)
            .request_async(async_http_client)
            .await
            .map_err(|e| OAuthError::AuthenticationFailed(format!("Token exchange failed: {}", e)))?;

        Ok(token_response)
    }

    /// Refresh token
    async fn refresh_token(&self, refresh_token: &str) -> Result<BasicTokenResponse> {
        let token = oauth2::RefreshToken::new(refresh_token.to_string());

        let token_response = self
            .client
            .exchange_refresh_token(&token)
            .request_async(async_http_client)
            .await
            .map_err(|e| OAuthError::RefreshFailed(format!("Token refresh failed: {}", e)))?;

        Ok(token_response)
    }

    /// Fetch user info from provider
    async fn fetch_user_info(&self, access_token: &str) -> Result<UserInfo> {
        let url = self
            .endpoints
            .user_info_url
            .as_ref()
            .ok_or_else(|| OAuthError::ConfigError("No user info URL".to_string()))?;

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| OAuthError::NetworkError(format!("User info request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(OAuthError::NetworkError(format!(
                "User info request failed: {} - {}",
                status, body
            ))
            .into());
        }

        let user_data: serde_json::Value = response.json().await.map_err(|e| {
            OAuthError::NetworkError(format!("Failed to parse user info: {}", e))
        })?;

        let user_info = match self.provider {
            OAuthProvider::GitHub => self.parse_github_user_info(user_data),
            OAuthProvider::Google => self.parse_google_user_info(user_data),
        };

        Ok(user_info)
    }

    fn parse_github_user_info(&self, data: serde_json::Value) -> UserInfo {
        UserInfo {
            id: data["id"].as_i64().map(|i| i.to_string()).unwrap_or_default(),
            username: data["login"].as_str().unwrap_or("").to_string(),
            name: data["name"]
                .as_str()
                .unwrap_or(data["login"].as_str().unwrap_or(""))
                .to_string(),
            email: data["email"].as_str().map(|s| s.to_string()),
            avatar_url: data["avatar_url"].as_str().map(|s| s.to_string()),
        }
    }

    fn parse_google_user_info(&self, data: serde_json::Value) -> UserInfo {
        UserInfo {
            id: data["id"].as_str().unwrap_or("").to_string(),
            username: data["email"]
                .as_str()
                .map(|e| e.split('@').next().unwrap_or(e).to_string())
                .unwrap_or_default(),
            name: data["name"].as_str().unwrap_or("").to_string(),
            email: data["email"].as_str().map(|s| s.to_string()),
            avatar_url: data["picture"].as_str().map(|s| s.to_string()),
        }
    }
}

/// OAuth manager that handles authentication and token storage
#[derive(Debug)]
pub struct OAuthManager {
    storage: Arc<dyn SecureStorage>,
    clients: Arc<RwLock<HashMap<OAuthProvider, OAuthClient>>>,
}

impl Default for OAuthManager {
    fn default() -> Self {
        // Create a simple environment-based storage for default
        // This is a placeholder - in production use proper storage
        Self {
            storage: Arc::new(crate::oauth::secure_storage::EnvironmentStorage::new("CLAUDE_OAUTH")),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl OAuthManager {
    /// Create a new OAuth manager with system keyring storage
    pub async fn new() -> Result<Self> {
        let storage = secure_storage::create_default_storage().await?;
        Self::with_storage(storage).await
    }

    /// Create a new OAuth manager with a custom storage backend
    pub async fn with_storage(storage: Arc<dyn SecureStorage>) -> Result<Self> {
        Ok(Self {
            storage,
            clients: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Register an OAuth provider configuration
    pub async fn register_provider(&self, config: OAuthConfig) -> Result<()> {
        let client = OAuthClient::new(config.clone())?;
        let mut clients = self.clients.write().await;
        clients.insert(config.provider, client);
        info!("Registered OAuth provider: {:?}", config.provider);
        Ok(())
    }

    /// Check if a provider is registered
    pub async fn is_registered(&self, provider: OAuthProvider) -> bool {
        let clients = self.clients.read().await;
        clients.contains_key(&provider)
    }

    /// Authenticate with a provider (initiates OAuth flow)
    pub async fn authenticate(&self, provider: OAuthProvider) -> Result<OAuthToken> {
        let client = {
            let clients = self.clients.read().await;
            clients
                .get(&provider)
                .cloned()
                .ok_or_else(|| {
                    OAuthError::ConfigError(format!("Provider {:?} not registered", provider))
                })?
        };

        // Generate PKCE
        let (verifier, challenge) = client.generate_pkce()?;

        // Build authorization URL
        let (auth_url, csrf_token) = client.authorization_url(&challenge);

        info!("Starting OAuth flow for {:?}", provider);
        debug!("Authorization URL: {}", auth_url);

        // Start local HTTP server for callback
        let (tx, mut rx) = mpsc::channel::<CallbackResult>(1);
        let redirect_url = client
            .config
            .redirect_url
            .clone()
            .unwrap_or_else(|| "http://localhost:8765/callback".to_string());
        let addr: SocketAddr = redirect_url
            .replace("http://", "")
            .replace("https://", "")
            .replace("/callback", "")
            .parse()
            .unwrap_or_else(|_| "127.0.0.1:8765".parse().unwrap());

        // Spawn HTTP server
        let tx = Arc::new(tokio::sync::Mutex::new(tx));
        let app = axum::Router::new().route(
            "/callback",
            axum::routing::get(
                move |axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>| {
                    let tx = tx.clone();
                    async move {
                        if let (Some(code), Some(state)) = (params.get("code"), params.get("state"))
                        {
                            let result = CallbackResult {
                                code: AuthorizationCode::new(code.clone()),
                                state: CsrfToken::new(state.clone()),
                            };
                            let _ = tx.lock().await.send(result).await;
                            "Authentication successful! You can close this window."
                        } else {
                            "Authentication failed: missing code or state"
                        }
                    }
                },
            ),
        );

        let listener = tokio::net::TcpListener::bind(addr).await.map_err(|e| {
            OAuthError::ConfigError(format!("Failed to bind callback server: {}", e))
        })?;

        let server = axum::serve(listener, app);
        let server_handle = tokio::spawn(async move {
            server.await.ok();
        });

        // Open browser for authentication
        if let Err(e) = open::that(&auth_url) {
            warn!("Failed to open browser: {}. Please open URL manually.", e);
            println!("Please open this URL in your browser:\n{}", auth_url);
        }

        // Wait for callback
        let callback = tokio::time::timeout(tokio::time::Duration::from_secs(300), rx.recv())
            .await
            .map_err(|_| OAuthError::Cancelled)?
            .ok_or(OAuthError::Cancelled)?;

        // Shutdown server
        server_handle.abort();

        // Exchange code for token
        let token_response = client.exchange_code(callback.code, verifier).await?;

        // Calculate expiration
        let expires_at = token_response.expires_in().map(|d| Utc::now() + d);

        // Fetch user info
        let access_token = token_response.access_token().secret();
        let user_info = client.fetch_user_info(access_token).await?;

        let token = OAuthToken {
            access_token: access_token.to_string(),
            refresh_token: token_response.refresh_token().map(|t| t.secret().to_string()),
            token_type: format!("{:?}", token_response.token_type()),
            expires_at,
            provider,
            user_info,
            obtained_at: Utc::now(),
        };

        // Store token
        self.store_token(&token).await?;

        info!("Successfully authenticated with {:?}", provider);
        Ok(token)
    }

    /// Get a stored token (refreshing if necessary)
    pub async fn get_token(&self, provider: OAuthProvider) -> Result<Option<OAuthToken>> {
        let key = format!("oauth_token_{}", provider.id());
        let data = self.storage.get(&key).await?;

        if let Some(data) = data {
            let mut token: OAuthToken =
                serde_json::from_slice(&data).map_err(|e| {
                    OAuthError::StorageError(format!("Failed to deserialize token: {}", e))
                })?;

            // Refresh if expired
            if token.is_expired() && token.can_refresh() {
                debug!("Token for {:?} expired, refreshing...", provider);
                token = self.refresh_token(token).await?;
                self.store_token(&token).await?;
            }

            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    /// Store a token
    async fn store_token(&self, token: &OAuthToken) -> Result<()> {
        let key = format!("oauth_token_{}", token.provider.id());
        let data = serde_json::to_vec(token)?;
        self.storage.set(&key, &data).await?;
        Ok(())
    }

    /// Refresh an expired token
    async fn refresh_token(&self, token: OAuthToken) -> Result<OAuthToken> {
        let client = {
            let clients = self.clients.read().await;
            clients
                .get(&token.provider)
                .cloned()
                .ok_or_else(|| {
                    OAuthError::ConfigError(format!(
                        "Provider {:?} not registered",
                        token.provider
                    ))
                })?
        };

        let refresh_token = token
            .refresh_token
            .ok_or_else(|| OAuthError::RefreshFailed("No refresh token".to_string()))?;

        let response = client.refresh_token(&refresh_token).await?;

        // Calculate new expiration
        let expires_at = response.expires_in().map(|d| Utc::now() + d);

        // Use new refresh token if provided, otherwise keep old one
        let new_refresh_token = response
            .refresh_token()
            .map(|t| t.secret().to_string())
            .or(Some(refresh_token));

        let new_token = OAuthToken {
            access_token: response.access_token().secret().to_string(),
            refresh_token: new_refresh_token,
            token_type: format!("{:?}", response.token_type()),
            expires_at,
            provider: token.provider,
            user_info: token.user_info.clone(),
            obtained_at: Utc::now(),
        };

        info!("Successfully refreshed token for {:?}", token.provider);
        Ok(new_token)
    }

    /// Revoke and delete a stored token
    pub async fn revoke_token(&self, provider: OAuthProvider) -> Result<()> {
        let key = format!("oauth_token_{}", provider.id());
        self.storage.delete(&key).await?;
        info!("Revoked token for {:?}", provider);
        Ok(())
    }

    /// List all stored tokens
    pub async fn list_tokens(&self) -> Result<Vec<OAuthProvider>> {
        let providers = vec![OAuthProvider::GitHub, OAuthProvider::Google];
        let mut result = Vec::new();

        for provider in providers {
            if let Some(token) = self.get_token(provider).await? {
                // Check if token is valid (not expired or can refresh)
                if !token.is_expired() || token.can_refresh() {
                    result.push(provider);
                }
            }
        }

        Ok(result)
    }
}

/// Convenience function to create OAuth manager from environment variables
pub async fn oauth_from_env() -> Result<OAuthManager> {
    let manager = OAuthManager::new().await?;

    // Check for GitHub OAuth credentials
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("GITHUB_CLIENT_ID"),
        std::env::var("GITHUB_CLIENT_SECRET"),
    ) {
        let config = OAuthConfig::new(OAuthProvider::GitHub, client_id, client_secret);
        manager.register_provider(config).await?;
    }

    // Check for Google OAuth credentials
    if let (Ok(client_id), Ok(client_secret)) = (
        std::env::var("GOOGLE_CLIENT_ID"),
        std::env::var("GOOGLE_CLIENT_SECRET"),
    ) {
        let config = OAuthConfig::new(OAuthProvider::Google, client_id, client_secret);
        manager.register_provider(config).await?;
    }

    Ok(manager)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_oauth_provider_id() {
        assert_eq!(OAuthProvider::GitHub.id(), "github");
        assert_eq!(OAuthProvider::Google.id(), "google");
    }

    #[test]
    fn test_oauth_config_builder() {
        let config = OAuthConfig::new(
            OAuthProvider::GitHub,
            "test_client_id".to_string(),
            "test_client_secret".to_string(),
        )
        .with_redirect_url("http://localhost:8080/callback".to_string())
        .with_scopes(vec!["repo".to_string(), "user".to_string()]);

        assert_eq!(config.provider, OAuthProvider::GitHub);
        assert_eq!(config.client_id, "test_client_id");
        assert_eq!(config.redirect_url, Some("http://localhost:8080/callback".to_string()));
    }

    #[tokio::test]
    async fn test_oauth_token_expiration() {
        let token = OAuthToken {
            access_token: "test".to_string(),
            refresh_token: Some("refresh".to_string()),
            token_type: "Bearer".to_string(),
            expires_at: Some(Utc::now() + Duration::hours(1)),
            provider: OAuthProvider::GitHub,
            user_info: UserInfo::default(),
            obtained_at: Utc::now(),
        };

        assert!(!token.is_expired());
        assert!(token.can_refresh());

        let expired_token = OAuthToken {
            access_token: "test".to_string(),
            refresh_token: None,
            token_type: "Bearer".to_string(),
            expires_at: Some(Utc::now() - Duration::hours(1)),
            provider: OAuthProvider::GitHub,
            user_info: UserInfo::default(),
            obtained_at: Utc::now(),
        };

        assert!(expired_token.is_expired());
        assert!(!expired_token.can_refresh());
    }
}
