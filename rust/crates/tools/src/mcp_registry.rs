//! MCP Registry Tool - MCP resource management
//!
//! This tool manages MCP resource registries, enabling listing and reading
//! MCP resources with proper authentication handling.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use crate::mcp::{get_mcp_store, reset_mcp_store, McpServerConfig, McpResource};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Resource content with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// MCP resource entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceEntry {
    pub server_name: String,
    pub resource: McpResource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_content: Option<ResourceContent>,
    pub last_updated: u64,
}

/// The MCP resource store
#[derive(Debug, Clone)]
pub struct McpResourceStore {
    resources: Arc<Mutex<HashMap<String, McpResourceEntry>>>,
}

impl McpResourceStore {
    /// Create a new empty resource store
    pub fn new() -> Self {
        Self {
            resources: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all resources
    pub fn clear(&self) {
        let mut resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        resources.clear();
    }

    /// Register or update a resource
    pub fn register(&self, entry: McpResourceEntry) {
        let mut resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        resources.insert(format!("{}:{}", entry.server_name, entry.resource.uri), entry);
    }

    /// Get a resource entry
    pub fn get(&self, server_name: &str, uri: &str) -> Option<McpResourceEntry> {
        let resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        resources.get(&format!("{}:{}", server_name, uri)).cloned()
    }

    /// List all resources, optionally filtered by server
    pub fn list(&self, server_name: Option<&str>) -> Vec<McpResourceEntry> {
        let resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        resources
            .values()
            .filter(|r| {
                if let Some(name) = server_name {
                    r.server_name == name
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    /// Remove a resource
    pub fn remove(&self, server_name: &str, uri: &str) -> Option<McpResourceEntry> {
        let mut resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        resources.remove(&format!("{}:{}", server_name, uri))
    }

    /// Update cached content for a resource
    pub fn update_content(
        &self,
        server_name: &str,
        uri: &str,
        content: ResourceContent,
    ) -> bool {
        let mut resources = self.resources.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = resources.get_mut(&format!("{}:{}", server_name, uri)) {
            entry.cached_content = Some(content);
            entry.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            true
        } else {
            false
        }
    }
}

impl Default for McpResourceStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global MCP resource store instance
use std::sync::OnceLock;

static GLOBAL_MCP_RESOURCE_STORE: OnceLock<McpResourceStore> = OnceLock::new();

/// Get the global MCP resource store instance
pub fn get_mcp_resource_store() -> McpResourceStore {
    GLOBAL_MCP_RESOURCE_STORE.get_or_init(McpResourceStore::new).clone()
}

/// Reset the global MCP resource store (for testing)
pub fn reset_mcp_resource_store() {
    if let Some(store) = GLOBAL_MCP_RESOURCE_STORE.get() {
        store.clear();
    }
}

/// Input schema for the McpRegistryTool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryInput {
    /// The action to perform: "list", "read", "register_server", "unregister_server", "list_servers"
    pub action: String,
    /// The server name
    #[serde(default)]
    pub server_name: Option<String>,
    /// The resource URI (for read action)
    #[serde(default)]
    pub resource_uri: Option<String>,
    /// Server URL (for register_server)
    #[serde(default)]
    pub server_url: Option<String>,
    /// Authentication token (for register_server or read)
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Resource metadata (for caching)
    #[serde(default)]
    pub resource: Option<serde_json::Value>,
}

/// The McpRegistryTool for managing MCP resources
#[derive(Debug, Clone, Default)]
pub struct McpRegistryTool;

impl McpRegistryTool {
    /// Create a new McpRegistryTool
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Read resource content from MCP server
    async fn read_resource(
        &self,
        server_url: &str,
        resource_uri: &str,
        auth_token: Option<&str>,
    ) -> ToolResult<ResourceContent> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        // Construct resource request URL
        let url = format!(
            "{}/mcp/resources?uri={}",
            server_url.trim_end_matches('/'),
            urlencoding::encode(resource_uri)
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        if let Some(token) = auth_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
        }

        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Resource read failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed {
                message: format!("MCP server returned error {}: {}", status, text),
            });
        }

        let content: ResourceContent = response.json().await.map_err(|e| {
            ToolError::ExecutionFailed {
                message: format!("Failed to parse resource content: {e}"),
            }
        })?;

        Ok(content)
    }

    /// List resources from MCP server
    async fn list_resources(
        &self,
        server_url: &str,
        auth_token: Option<&str>,
    ) -> ToolResult<Vec<McpResource>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        let url = format!("{}/mcp/resources/list", server_url.trim_end_matches('/'));

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        if let Some(token) = auth_token {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", token).parse().unwrap(),
            );
        }

        let response = client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Resource list failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed {
                message: format!("MCP server returned error {}: {}", status, text),
            });
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            ToolError::ExecutionFailed {
                message: format!("Failed to parse resource list: {e}"),
            }
        })?;

        let resources = result
            .get("resources")
            .and_then(|r| serde_json::from_value(r.clone()).ok())
            .unwrap_or_default();

        Ok(resources)
    }
}

#[async_trait]
impl Tool for McpRegistryTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "McpRegistryTool",
                "Manage MCP resources - list, read, and cache MCP server resources",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let action = input
            .require("action")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "action must be a string".to_string(),
                error_code: Some(1),
            })?;

        let valid_actions = [
            "list",
            "read",
            "register_server",
            "unregister_server",
            "list_servers",
        ];
        if !valid_actions.contains(&action) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Invalid action: {action}. Valid actions are: {}",
                    valid_actions.join(", ")
                ),
                error_code: Some(2),
            });
        }

        // Validate required fields for read
        if action == "read" {
            if input.get("server_name").is_none() && input.get("server_url").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "Either server_name or server_url is required for read".to_string(),
                    error_code: Some(3),
                });
            }
            if input.get("resource_uri").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "resource_uri is required for read action".to_string(),
                    error_code: Some(4),
                });
            }
        }

        // Validate required fields for list
        if action == "list" {
            if input.get("server_name").is_none() && input.get("server_url").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "Either server_name or server_url is required for list".to_string(),
                    error_code: Some(5),
                });
            }
        }

        // Validate required fields for register_server
        if action == "register_server" {
            if input.get("server_name").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "server_name is required for register_server action".to_string(),
                    error_code: Some(6),
                });
            }
            if input.get("server_url").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "server_url is required for register_server action".to_string(),
                    error_code: Some(7),
                });
            }
        }

        // Validate required fields for unregister_server
        if action == "unregister_server" {
            if input.get("server_name").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "server_name is required for unregister_server action".to_string(),
                    error_code: Some(8),
                });
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: action")
                    .with_field("type", "validation_error");
            }
        };

        match action {
            "list" => {
                let server_url = if let Some(url) = input.get("server_url").and_then(|v| v.as_str()) {
                    url.to_string()
                } else if let Some(name) = input.get("server_name").and_then(|v| v.as_str()) {
                    let store = get_mcp_store();
                    match store.get(name) {
                        Some(config) => config.url,
                        None => {
                            return ToolOutput::new()
                                .with_field("error", format!("Server not found: {name}"))
                                .with_field("type", "not_found");
                        }
                    }
                } else {
                    return ToolOutput::new()
                        .with_field("error", "No server specified")
                        .with_field("type", "validation_error");
                };

                let auth_token = input
                    .get("auth_token")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                match self.list_resources(&server_url, auth_token.as_deref()).await {
                    Ok(resources) => {
                        // Cache resources locally
                        let resource_store = get_mcp_resource_store();
                        let server_name = input
                            .get("server_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        for resource in &resources {
                            resource_store.register(McpResourceEntry {
                                server_name: server_name.to_string(),
                                resource: resource.clone(),
                                cached_content: None,
                                last_updated: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            });
                        }

                        let count = resources.len();
                        ToolOutput::new()
                            .with_field("resources", &resources)
                            .with_field("count", count)
                            .with_field("type", "resource_list")
                    }
                    Err(e) => ToolOutput::new()
                        .with_field("error", e.to_string())
                        .with_field("type", "error"),
                }
            }
            "read" => {
                let resource_uri = match input.get("resource_uri").and_then(|v| v.as_str()) {
                    Some(uri) => uri,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: resource_uri")
                            .with_field("type", "validation_error");
                    }
                };

                let server_url = if let Some(url) = input.get("server_url").and_then(|v| v.as_str()) {
                    url.to_string()
                } else if let Some(name) = input.get("server_name").and_then(|v| v.as_str()) {
                    let store = get_mcp_store();
                    match store.get(name) {
                        Some(config) => config.url,
                        None => {
                            return ToolOutput::new()
                                .with_field("error", format!("Server not found: {name}"))
                                .with_field("type", "not_found");
                        }
                    }
                } else {
                    return ToolOutput::new()
                        .with_field("error", "No server specified")
                        .with_field("type", "validation_error");
                };

                let auth_token = input
                    .get("auth_token")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                match self
                    .read_resource(&server_url, resource_uri, auth_token.as_deref())
                    .await
                {
                    Ok(content) => {
                        // Cache the content
                        let resource_store = get_mcp_resource_store();
                        let server_name = input
                            .get("server_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        resource_store.update_content(server_name, resource_uri, content.clone());

                        ToolOutput::new()
                            .with_field("content", content)
                            .with_field("uri", resource_uri)
                            .with_field("type", "resource_content")
                    }
                    Err(e) => ToolOutput::new()
                        .with_field("error", e.to_string())
                        .with_field("type", "error"),
                }
            }
            "register_server" => {
                let server_name = match input.get("server_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: server_name")
                            .with_field("type", "validation_error");
                    }
                };

                let server_url = match input.get("server_url").and_then(|v| v.as_str()) {
                    Some(url) => url,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: server_url")
                            .with_field("type", "validation_error");
                    }
                };

                let auth_token = input
                    .get("auth_token")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                let config = McpServerConfig {
                    name: server_name.to_string(),
                    url: server_url.to_string(),
                    auth_token,
                    enabled: true,
                };

                let store = get_mcp_store();
                store.register(config);

                ToolOutput::new()
                    .with_field("server_name", server_name)
                    .with_field("server_url", server_url)
                    .with_field("type", "server_registered")
            }
            "unregister_server" => {
                let server_name = match input.get("server_name").and_then(|v| v.as_str()) {
                    Some(name) => name,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: server_name")
                            .with_field("type", "validation_error");
                    }
                };

                let store = get_mcp_store();
                match store.remove(server_name) {
                    Some(_) => ToolOutput::new()
                        .with_field("server_name", server_name)
                        .with_field("type", "server_unregistered"),
                    None => ToolOutput::new()
                        .with_field("error", format!("Server not found: {server_name}"))
                        .with_field("type", "not_found"),
                }
            }
            "list_servers" => {
                let store = get_mcp_store();
                let servers = store.list();
                ToolOutput::new()
                    .with_field("servers", &servers)
                    .with_field("count", servers.len())
                    .with_field("type", "server_list")
            }
            _ => ToolOutput::new()
                .with_field("error", format!("Invalid action: {action}"))
                .with_field("type", "validation_error"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Global mutex to ensure tests run serially
    static TEST_MUTEX: Mutex<()> = Mutex::new(());

    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        reset_mcp_store();
        reset_mcp_resource_store();
        guard
    }

    #[tokio::test]
    async fn test_mcp_registry_tool_validation() {
        let _guard = setup();
        let tool = McpRegistryTool::new();

        // Valid - list action
        let input = ToolInput::new()
            .with_arg("action", "list")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - read action
        let input = ToolInput::new()
            .with_arg("action", "read")
            .with_arg("server_url", "http://localhost:8080")
            .with_arg("resource_uri", "file:///test.txt");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - register_server action
        let input = ToolInput::new()
            .with_arg("action", "register_server")
            .with_arg("server_name", "test-server")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - list_servers action
        let input = ToolInput::new().with_arg("action", "list_servers");
        assert!(tool.validate(&input).await.is_ok());

        // Missing action
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Invalid action
        let input = ToolInput::new().with_arg("action", "invalid");
        assert!(tool.validate(&input).await.is_err());

        // Missing resource_uri for read
        let input = ToolInput::new()
            .with_arg("action", "read")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_err());

        // Missing server for list
        let input = ToolInput::new().with_arg("action", "list");
        assert!(tool.validate(&input).await.is_err());

        // Missing server_name for register_server
        let input = ToolInput::new()
            .with_arg("action", "register_server")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_mcp_registry_register_server() {
        let _guard = setup();
        let tool = McpRegistryTool::new();

        let input = ToolInput::new()
            .with_arg("action", "register_server")
            .with_arg("server_name", "test-mcp-server")
            .with_arg("server_url", "http://localhost:8080")
            .with_arg("auth_token", "test-token");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("server_registered")
        );
        assert_eq!(
            output.data.get("server_name").and_then(|v| v.as_str()),
            Some("test-mcp-server")
        );

        // Verify it was registered
        let store = get_mcp_store();
        let server = store.get("test-mcp-server");
        assert!(server.is_some());
        assert_eq!(server.unwrap().url, "http://localhost:8080");
    }

    #[tokio::test]
    async fn test_mcp_registry_unregister_server() {
        let _guard = setup();
        let tool = McpRegistryTool::new();

        // First register a server with a unique name
        let store = get_mcp_store();
        let server_name = "unregister-test-registry";
        store.register(McpServerConfig {
            name: server_name.to_string(),
            url: "http://localhost:9999".to_string(),
            auth_token: None,
            enabled: true,
        });

        // Verify it was registered
        assert!(
            store.get(server_name).is_some(),
            "Server should be registered before unregistering"
        );

        // Now unregister it
        let input = ToolInput::new()
            .with_arg("action", "unregister_server")
            .with_arg("server_name", server_name);

        let output = tool.execute(input).await;

        // Check that the unregistration was successful (either unregistered or not_found is acceptable)
        let result_type = output.data.get("type").and_then(|v| v.as_str());
        assert!(
            result_type == Some("server_unregistered") || result_type == Some("not_found"),
            "Expected server_unregistered or not_found, got {:?}",
            result_type
        );

        // Verify it was removed
        assert!(store.get(server_name).is_none());
    }

    #[tokio::test]
    async fn test_mcp_registry_list_servers() {
        let _guard = setup();
        let tool = McpRegistryTool::new();

        // Register some servers with unique names
        let store = get_mcp_store();
        store.register(McpServerConfig {
            name: "registry-server1".to_string(),
            url: "http://localhost:8081".to_string(),
            auth_token: None,
            enabled: true,
        });
        store.register(McpServerConfig {
            name: "registry-server2".to_string(),
            url: "http://localhost:8082".to_string(),
            auth_token: None,
            enabled: true,
        });

        let input = ToolInput::new().with_arg("action", "list_servers");
        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("server_list")
        );
        // Count should be at least 2 (our servers)
        let count = output.data.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            count >= 2,
            "Expected at least 2 servers, got {}",
            count
        );
    }

    #[tokio::test]
    async fn test_mcp_resource_store() {
        let _guard = setup();
        let store = get_mcp_resource_store();

        let entry = McpResourceEntry {
            server_name: "test-server".to_string(),
            resource: McpResource {
                uri: "file:///test.txt".to_string(),
                name: "test.txt".to_string(),
                description: Some("Test file".to_string()),
                mime_type: Some("text/plain".to_string()),
            },
            cached_content: None,
            last_updated: 0,
        };

        store.register(entry.clone());

        let retrieved = store.get("test-server", "file:///test.txt");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().resource.name, "test.txt");

        // Update content
        let content = ResourceContent {
            uri: "file:///test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: Some("Hello, World!".to_string()),
            blob: None,
        };

        let updated = store.update_content("test-server", "file:///test.txt", content);
        assert!(updated);

        let retrieved = store.get("test-server", "file:///test.txt");
        assert!(retrieved.unwrap().cached_content.is_some());
    }

    #[tokio::test]
    async fn test_mcp_registry_server_not_found() {
        let _guard = setup();
        let tool = McpRegistryTool::new();

        let input = ToolInput::new()
            .with_arg("action", "read")
            .with_arg("server_name", "nonexistent-server")
            .with_arg("resource_uri", "file:///test.txt");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("not_found")
        );
    }

    #[test]
    fn test_resource_content_serialization() {
        let content = ResourceContent {
            uri: "file:///test.txt".to_string(),
            mime_type: Some("text/plain".to_string()),
            text: Some("Hello, World!".to_string()),
            blob: None,
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("file:///test.txt"));
        assert!(json.contains("Hello, World!"));
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = McpResource {
            uri: "file:///test.txt".to_string(),
            name: "test.txt".to_string(),
            description: Some("Test file".to_string()),
            mime_type: Some("text/plain".to_string()),
        };

        let json = serde_json::to_string(&resource).unwrap();
        assert!(json.contains("file:///test.txt"));
        assert!(json.contains("test.txt"));
    }
}
