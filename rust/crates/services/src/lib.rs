//! Services Crate
//!
//! This crate provides external service integrations including:
//! - OAuth authentication (GitHub, Google)
//! - Analytics tracking and event reporting
//! - OpenTelemetry integration for distributed tracing
//! - Secure token storage
//!
//! # Example
//! ```rust
//! use services::analytics::{AnalyticsClient, EventBuilder};
//! use services::oauth::{OAuthManager, OAuthProvider};
//! use services::telemetry::{Telemetry, TelemetryConfig};
//!
//! async fn setup_services() {
//!     // Initialize analytics
//!     let analytics = AnalyticsClient::new("my-service");
//!
//!     // Setup OAuth
//!     let oauth = OAuthManager::default();
//!
//!     // Initialize telemetry
//!     let telemetry = Telemetry::init(TelemetryConfig::new("my-service")).await.unwrap();
//! }
//! ```

#![warn(missing_docs)]

/// OAuth module for authentication
pub mod oauth;
/// Analytics module for event tracking and metrics
pub mod analytics;
/// Telemetry module for OpenTelemetry integration
pub mod telemetry;

// Re-export OAuth types
pub use oauth::{
    OAuthManager,
    OAuthProvider,
    OAuthToken,
    OAuthConfig,
};

// Re-export secure storage types from oauth module
pub use oauth::{
    SecureStorage,
    StorageBackend,
    KeyringStorage,
    EncryptedFileStorage,
    EnvironmentStorage,
    create_default_storage,
    create_storage,
};

// Re-export Analytics types
pub use analytics::{
    AnalyticsClient,
    AnalyticsEvent,
    AnalyticsConfig,
    AnalyticsSink,
    ConsoleSink,
    MemorySink,
    HttpSink,
    EventBuilder,
    SessionMetrics,
    FileOperation,
    init_global_analytics,
    global_analytics,
};

// Re-export Telemetry types
pub use telemetry::{
    Telemetry,
    TelemetryClient,
    TelemetryConfig,
    TelemetrySpan,
    SpanBuilder,
    TelemetryValue,
    ContextPropagator,
    init_global_telemetry,
    global_telemetry,
    shutdown_global_telemetry,
};

/// Convenience type aliases
pub type Result<T> = anyhow::Result<T>;

/// Initialize all services with default configuration
///
/// This is a convenience function for quickly setting up all services
/// during application startup.
pub async fn init_all(
    service_name: impl Into<String>,
) -> Result<(AnalyticsClient, Arc<OAuthManager>, Telemetry)> {
    use std::sync::Arc;

    let service_name = service_name.into();

    // Initialize analytics
    let analytics = AnalyticsClient::new(service_name.clone());

    // Initialize OAuth
    let oauth = Arc::new(OAuthManager::default());

    // Initialize telemetry
    let telemetry_config = TelemetryConfig::new(service_name);
    let telemetry = Telemetry::init(telemetry_config).await?;

    Ok((analytics, oauth, telemetry))
}

use std::sync::Arc;
