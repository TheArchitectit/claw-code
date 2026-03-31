//! Analytics Module
//!
//! Provides analytics tracking and reporting with OpenTelemetry integration:
//! - Event tracking (tool usage, commands)
//! - Session metrics
//! - Performance monitoring
//! - Error reporting
//!
//! # Example
//! ```rust
//! use services::analytics::{AnalyticsClient, EventBuilder};
//!
//! async fn track_usage() {
//!     let client = AnalyticsClient::new("claude-code");
//!
//!     // Track a tool usage event
//!     client.track_event(
//!         EventBuilder::new("tool_execution")
//!             .with_property("tool_name", "file_read")
//!             .with_property("duration_ms", 150)
//!             .build()
//!     ).await;
//! }
//! ```

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Analytics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    /// Enable analytics
    pub enabled: bool,
    /// Service name
    pub service_name: String,
    /// Session ID
    pub session_id: Option<String>,
    /// User ID (if available)
    pub user_id: Option<String>,
    /// Additional tags
    pub tags: HashMap<String, String>,
    /// Endpoint for remote analytics (optional)
    pub endpoint: Option<String>,
    /// Sampling rate (0.0-1.0)
    pub sampling_rate: f64,
    /// Enable console output
    pub console_output: bool,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "claude-code-rust".to_string(),
            session_id: None,
            user_id: None,
            tags: HashMap::new(),
            endpoint: None,
            sampling_rate: 1.0,
            console_output: true,
        }
    }
}

impl AnalyticsConfig {
    /// Create new config
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            ..Default::default()
        }
    }
}

/// Analytics event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEvent {
    /// Event ID
    pub id: String,
    /// Event name/type
    pub name: String,
    /// Event timestamp
    pub timestamp: chrono::DateTime<Utc>,
    /// Event properties
    pub properties: HashMap<String, Value>,
    /// Session ID
    pub session_id: Option<String>,
    /// User ID (if available)
    pub user_id: Option<String>,
}

impl AnalyticsEvent {
    /// Create a new analytics event
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            timestamp: Utc::now(),
            properties: HashMap::new(),
            session_id: None,
            user_id: None,
        }
    }

    /// Add a property
    pub fn with_property(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(value) = serde_json::to_value(value) {
            self.properties.insert(key.into(), value);
        }
        self
    }

    /// Add session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }
}

/// Event builder for fluent API
#[derive(Debug)]
pub struct EventBuilder {
    name: String,
    properties: HashMap<String, Value>,
    session_id: Option<String>,
    user_id: Option<String>,
}

impl EventBuilder {
    /// Create a new event builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            properties: HashMap::new(),
            session_id: None,
            user_id: None,
        }
    }

    /// Add a property
    pub fn with_property(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        if let Ok(value) = serde_json::to_value(value) {
            self.properties.insert(key.into(), value);
        }
        self
    }

    /// Add session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add user ID
    pub fn with_user(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Build the event
    pub fn build(self) -> AnalyticsEvent {
        AnalyticsEvent {
            id: Uuid::new_v4().to_string(),
            name: self.name,
            timestamp: Utc::now(),
            properties: self.properties,
            session_id: self.session_id,
            user_id: self.user_id,
        }
    }
}

/// Session metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetrics {
    /// Session ID
    pub session_id: String,
    /// Start time
    pub start_time: chrono::DateTime<Utc>,
    /// End time (if session ended)
    pub end_time: Option<chrono::DateTime<Utc>>,
    /// Number of commands executed
    pub command_count: u64,
    /// Number of tools used
    pub tool_usage_count: u64,
    /// Total tool execution time (ms)
    pub total_tool_duration_ms: u64,
    /// Number of errors
    pub error_count: u64,
    /// Number of files modified
    pub files_modified: u64,
    /// Number of files read
    pub files_read: u64,
    /// Additional metrics
    #[serde(flatten)]
    pub custom: HashMap<String, Value>,
}

impl SessionMetrics {
    /// Create new session metrics
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            start_time: Utc::now(),
            end_time: None,
            command_count: 0,
            tool_usage_count: 0,
            total_tool_duration_ms: 0,
            error_count: 0,
            files_modified: 0,
            files_read: 0,
            custom: HashMap::new(),
        }
    }

    /// Record command execution
    pub fn record_command(&mut self) {
        self.command_count += 1;
    }

    /// Record tool usage
    pub fn record_tool_usage(&mut self, duration_ms: u64) {
        self.tool_usage_count += 1;
        self.total_tool_duration_ms += duration_ms;
    }

    /// Record error
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Record file modification
    pub fn record_file_modified(&mut self) {
        self.files_modified += 1;
    }

    /// Record file read
    pub fn record_file_read(&mut self) {
        self.files_read += 1;
    }

    /// End the session
    pub fn end_session(&mut self) {
        self.end_time = Some(Utc::now());
    }

    /// Get session duration
    pub fn duration_seconds(&self) -> i64 {
        let end = self.end_time.unwrap_or_else(Utc::now);
        (end - self.start_time).num_seconds()
    }
}

/// Analytics sink trait for different backends
#[async_trait]
pub trait AnalyticsSink: Send + Sync + std::fmt::Debug {
    /// Send an event
    async fn send_event(&self, event: AnalyticsEvent) -> Result<()>;
    /// Flush any pending events
    async fn flush(&self) -> Result<()>;
}

/// Console sink for development/debugging
#[derive(Debug)]
pub struct ConsoleSink;

#[async_trait]
impl AnalyticsSink for ConsoleSink {
    async fn send_event(&self, event: AnalyticsEvent) -> Result<()> {
        let json = serde_json::to_string_pretty(&event)?;
        info!("Analytics Event: {}", json);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// In-memory sink for testing
#[derive(Debug, Clone)]
pub struct MemorySink {
    events: Arc<RwLock<Vec<AnalyticsEvent>>>,
}

impl MemorySink {
    /// Create a new memory sink
    pub fn new() -> Self {
        Self {
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get all recorded events
    pub async fn get_events(&self) -> Vec<AnalyticsEvent> {
        self.events.read().await.clone()
    }

    /// Clear all events
    pub async fn clear(&self) {
        self.events.write().await.clear();
    }
}

#[async_trait]
impl AnalyticsSink for MemorySink {
    async fn send_event(&self, event: AnalyticsEvent) -> Result<()> {
        self.events.write().await.push(event);
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// HTTP sink for remote analytics
#[derive(Debug)]
pub struct HttpSink {
    client: reqwest::Client,
    endpoint: String,
    api_key: Option<String>,
}

impl HttpSink {
    /// Create a new HTTP sink
    pub fn new(endpoint: impl Into<String>, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            endpoint: endpoint.into(),
            api_key,
        }
    }
}

#[async_trait]
impl AnalyticsSink for HttpSink {
    async fn send_event(&self, event: AnalyticsEvent) -> Result<()> {
        let mut request = self.client.post(&self.endpoint).json(&event);

        if let Some(ref api_key) = self.api_key {
            request = request.header("X-API-Key", api_key);
        }

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Failed to send analytics: {}",
                response.status()
            ));
        }

        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        Ok(())
    }
}

/// Analytics client for tracking events and metrics
#[derive(Debug, Clone)]
pub struct AnalyticsClient {
    config: AnalyticsConfig,
    sink: Arc<dyn AnalyticsSink>,
    metrics: Arc<RwLock<SessionMetrics>>,
}

impl AnalyticsClient {
    /// Create a new analytics client
    pub fn new(service_name: impl Into<String>) -> Self {
        let config = AnalyticsConfig {
            service_name: service_name.into(),
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Create with configuration
    pub fn with_config(config: AnalyticsConfig) -> Self {
        let sink: Arc<dyn AnalyticsSink> = if let Some(ref endpoint) = config.endpoint {
            Arc::new(HttpSink::new(endpoint.clone(), None))
        } else {
            Arc::new(ConsoleSink)
        };

        let metrics = Arc::new(RwLock::new(SessionMetrics::new(
            config.session_id.clone().unwrap_or_default(),
        )));

        Self {
            config,
            sink,
            metrics,
        }
    }

    /// Create with custom sink
    pub fn with_sink(config: AnalyticsConfig, sink: Arc<dyn AnalyticsSink>) -> Self {
        let metrics = Arc::new(RwLock::new(SessionMetrics::new(
            config.session_id.clone().unwrap_or_default(),
        )));

        Self {
            config,
            sink,
            metrics,
        }
    }

    /// Check if analytics is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Enable analytics
    pub fn enable(&mut self) {
        self.config.enabled = true;
    }

    /// Disable analytics
    pub fn disable(&mut self) {
        self.config.enabled = false;
    }

    /// Track an event
    pub async fn track_event(&self, event: impl Into<AnalyticsEvent>) -> Result<()> {
        if !self.config.enabled {
            debug!("Analytics disabled, skipping event");
            return Ok(());
        }

        // Sampling
        if self.config.sampling_rate < 1.0 {
            let random: f64 = rand::random();
            if random > self.config.sampling_rate {
                debug!("Event sampled out");
                return Ok(());
            }
        }

        let mut event: AnalyticsEvent = event.into();

        // Add session/user IDs from config if not present
        if event.session_id.is_none() {
            event.session_id = self.config.session_id.clone();
        }
        if event.user_id.is_none() {
            event.user_id = self.config.user_id.clone();
        }

        // Add service name
        event
            .properties
            .insert("service_name".to_string(), self.config.service_name.clone().into());

        // Add config tags
        for (key, value) in &self.config.tags {
            event
                .properties
                .entry(key.clone())
                .or_insert_with(|| value.clone().into());
        }

        debug!("Tracking event: {}", event.name);
        self.sink.send_event(event).await?;

        Ok(())
    }

    /// Track a tool execution
    pub async fn track_tool(&self, tool_name: &str, duration_ms: u64, success: bool) -> Result<()> {
        let event = EventBuilder::new("tool_execution")
            .with_property("tool_name", tool_name)
            .with_property("duration_ms", duration_ms)
            .with_property("success", success)
            .with_session(self.config.session_id.clone().unwrap_or_default())
            .build();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.record_tool_usage(duration_ms);
        }

        self.track_event(event).await
    }

    /// Track a command execution
    pub async fn track_command(&self, command: &str, args: &[&str]) -> Result<()> {
        let event = EventBuilder::new("command_execution")
            .with_property("command", command)
            .with_property("args", args)
            .with_session(self.config.session_id.clone().unwrap_or_default())
            .build();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.record_command();
        }

        self.track_event(event).await
    }

    /// Track an error
    pub async fn track_error(&self, error_type: &str, message: &str) -> Result<()> {
        let event = EventBuilder::new("error")
            .with_property("error_type", error_type)
            .with_property("message", message)
            .with_session(self.config.session_id.clone().unwrap_or_default())
            .build();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.record_error();
        }

        self.track_event(event).await
    }

    /// Track file access
    pub async fn track_file(&self, path: &str, operation: FileOperation) -> Result<()> {
        let event = EventBuilder::new("file_access")
            .with_property("path", path)
            .with_property("operation", format!("{:?}", operation))
            .with_session(self.config.session_id.clone().unwrap_or_default())
            .build();

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            match operation {
                FileOperation::Read => metrics.record_file_read(),
                FileOperation::Write | FileOperation::Modify => metrics.record_file_modified(),
            }
        }

        self.track_event(event).await
    }

    /// Get current session metrics
    pub async fn get_metrics(&self) -> SessionMetrics {
        self.metrics.read().await.clone()
    }

    /// End the session and report final metrics
    pub async fn end_session(&self) -> Result<()> {
        let metrics = {
            let mut m = self.metrics.write().await;
            m.end_session();
            m.clone()
        };

        // Send final session summary event
        let event = EventBuilder::new("session_end")
            .with_property("duration_seconds", metrics.duration_seconds())
            .with_property("command_count", metrics.command_count)
            .with_property("tool_usage_count", metrics.tool_usage_count)
            .with_property("error_count", metrics.error_count)
            .with_property("files_modified", metrics.files_modified)
            .with_property("files_read", metrics.files_read)
            .with_session(metrics.session_id.clone())
            .build();

        self.track_event(event).await?;
        self.sink.flush().await?;

        Ok(())
    }

    /// Flush pending events
    pub async fn flush(&self) -> Result<()> {
        self.sink.flush().await
    }
}

/// File operation types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    /// File was read
    Read,
    /// File was written
    Write,
    /// File was modified
    Modify,
}

/// Global analytics instance (optional)
static mut GLOBAL_ANALYTICS: Option<AnalyticsClient> = None;

/// Initialize global analytics
pub fn init_global_analytics(client: AnalyticsClient) {
    unsafe {
        GLOBAL_ANALYTICS = Some(client);
    }
}

/// Get global analytics instance
pub fn global_analytics() -> Option<&'static AnalyticsClient> {
    unsafe { GLOBAL_ANALYTICS.as_ref() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_builder() {
        let event = EventBuilder::new("test_event")
            .with_property("key1", "value1")
            .with_property("key2", 42)
            .with_session("session_123")
            .with_user("user_456")
            .build();

        assert_eq!(event.name, "test_event");
        assert_eq!(event.session_id, Some("session_123".to_string()));
        assert_eq!(event.user_id, Some("user_456".to_string()));
        assert_eq!(event.properties.get("key1").unwrap().as_str().unwrap(), "value1");
        assert_eq!(event.properties.get("key2").unwrap().as_i64().unwrap(), 42);
    }

    #[tokio::test]
    async fn test_memory_sink() {
        let sink = MemorySink::new();
        let client = AnalyticsClient::with_sink(AnalyticsConfig::default(), Arc::new(sink.clone()));

        // Track some events
        client.track_event(AnalyticsEvent::new("event1")).await.unwrap();
        client.track_event(AnalyticsEvent::new("event2")).await.unwrap();

        let events = sink.get_events().await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].name, "event1");
        assert_eq!(events[1].name, "event2");
    }

    #[test]
    fn test_session_metrics() {
        let mut metrics = SessionMetrics::new("test_session");

        metrics.record_command();
        metrics.record_command();
        metrics.record_tool_usage(100);
        metrics.record_tool_usage(200);
        metrics.record_error();
        metrics.record_file_read();
        metrics.record_file_modified();

        assert_eq!(metrics.command_count, 2);
        assert_eq!(metrics.tool_usage_count, 2);
        assert_eq!(metrics.total_tool_duration_ms, 300);
        assert_eq!(metrics.error_count, 1);
        assert_eq!(metrics.files_read, 1);
        assert_eq!(metrics.files_modified, 1);
    }
}
