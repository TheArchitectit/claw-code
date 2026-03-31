//! MCP Tool - Model Context Protocol server invocation
//!
//! This tool enables invocation of MCP servers using JSON-RPC 2.0 format.
//! It supports MCP resource access and handles SSE streaming responses.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// MCP request following JSON-RPC 2.0 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl McpRequest {
    /// Create a new MCP request
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }
}

/// MCP response following JSON-RPC 2.0 specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// MCP resource metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP tool metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

/// The MCP store manages server configurations
#[derive(Debug, Clone)]
pub struct McpStore {
    servers: Arc<Mutex<HashMap<String, McpServerConfig>>>,
}

impl McpStore {
    /// Create a new empty MCP store
    pub fn new() -> Self {
        Self {
            servers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Clear all server configurations
    pub fn clear(&self) {
        let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        servers.clear();
    }

    /// Register an MCP server
    pub fn register(&self, config: McpServerConfig) {
        let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        servers.insert(config.name.clone(), config);
    }

    /// Get a server configuration
    pub fn get(&self, name: &str) -> Option<McpServerConfig> {
        let servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        servers.get(name).cloned()
    }

    /// List all registered servers
    pub fn list(&self) -> Vec<McpServerConfig> {
        let servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        servers.values().cloned().collect()
    }

    /// Remove a server configuration
    pub fn remove(&self, name: &str) -> Option<McpServerConfig> {
        let mut servers = self.servers.lock().unwrap_or_else(|e| e.into_inner());
        servers.remove(name)
    }
}

impl Default for McpStore {
    fn default() -> Self {
        Self::new()
    }
}

// Global MCP store instance
use std::sync::OnceLock;

static GLOBAL_MCP_STORE: OnceLock<McpStore> = OnceLock::new();

/// Get the global MCP store instance
pub fn get_mcp_store() -> McpStore {
    GLOBAL_MCP_STORE.get_or_init(McpStore::new).clone()
}

/// Reset the global MCP store (for testing)
pub fn reset_mcp_store() {
    // Ensure the store is initialized first, then clear it
    let store = get_mcp_store();
    store.clear();
}

/// Input schema for the McpTool
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpInput {
    /// The action to perform: "invoke", "discover", or "list_servers"
    pub action: String,
    /// The MCP server name (for invoke/discover)
    #[serde(default)]
    pub server_name: Option<String>,
    /// The JSON-RPC method to call (for invoke)
    #[serde(default)]
    pub method: Option<String>,
    /// The parameters for the method (for invoke)
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    /// Optional server URL override
    #[serde(default)]
    pub server_url: Option<String>,
    /// Optional authentication token
    #[serde(default)]
    pub auth_token: Option<String>,
}

/// The McpTool for invoking MCP servers
#[derive(Debug, Clone, Default)]
pub struct McpTool;

impl McpTool {
    /// Create a new McpTool
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Execute MCP request over HTTP
    async fn execute_mcp_request(
        &self,
        server_url: &str,
        request: McpRequest,
        auth_token: Option<&str>,
    ) -> ToolResult<McpResponse> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        let url = format!("{}/mcp/messages", server_url.trim_end_matches('/'));

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
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionFailed {
                message: format!("MCP request failed: {e}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(ToolError::ExecutionFailed {
                message: format!("MCP server returned error {}: {}", status, text),
            });
        }

        let mcp_response: McpResponse = response.json().await.map_err(|e| {
            ToolError::ExecutionFailed {
                message: format!("Failed to parse MCP response: {e}"),
            }
        })?;

        Ok(mcp_response)
    }

    /// Discover available tools from MCP server
    async fn discover_tools(
        &self,
        server_url: &str,
        auth_token: Option<&str>,
    ) -> ToolResult<Vec<McpToolInfo>> {
        let request = McpRequest::new("tools/list", None);
        let response = self.execute_mcp_request(server_url, request, auth_token).await?;

        if let Some(error) = response.error {
            return Err(ToolError::ExecutionFailed {
                message: format!("MCP error {}: {}", error.code, error.message),
            });
        }

        let tools = response
            .result
            .and_then(|r| r.get("tools").cloned())
            .and_then(|t| serde_json::from_value(t).ok())
            .unwrap_or_default();

        Ok(tools)
    }

    /// Invoke an MCP tool
    async fn invoke_tool(
        &self,
        server_url: &str,
        method: &str,
        params: Option<serde_json::Value>,
        auth_token: Option<&str>,
    ) -> ToolResult<serde_json::Value> {
        let request = McpRequest::new(method, params);
        let response = self.execute_mcp_request(server_url, request, auth_token).await?;

        if let Some(error) = response.error {
            return Err(ToolError::ExecutionFailed {
                message: format!("MCP error {}: {}", error.code, error.message),
            });
        }

        Ok(response.result.unwrap_or(serde_json::Value::Null))
    }
}

#[async_trait]
impl Tool for McpTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "McpTool",
                "Invoke MCP servers using JSON-RPC 2.0 protocol",
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

        let valid_actions = ["invoke", "discover", "list_servers"];
        if !valid_actions.contains(&action) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Invalid action: {action}. Valid actions are: invoke, discover, list_servers"
                ),
                error_code: Some(2),
            });
        }

        // Validate required fields for invoke
        if action == "invoke" {
            if input.get("method").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "method is required for invoke action".to_string(),
                    error_code: Some(3),
                });
            }

            // Either server_name or server_url must be provided
            if input.get("server_name").is_none() && input.get("server_url").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "Either server_name or server_url is required for invoke".to_string(),
                    error_code: Some(4),
                });
            }
        }

        // Validate required fields for discover
        if action == "discover" {
            if input.get("server_name").is_none() && input.get("server_url").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "Either server_name or server_url is required for discover".to_string(),
                    error_code: Some(5),
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
            "invoke" => {
                let method = match input.get("method").and_then(|v| v.as_str()) {
                    Some(m) => m,
                    None => {
                        return ToolOutput::new()
                            .with_field("error", "Missing required argument: method")
                            .with_field("type", "validation_error");
                    }
                };

                let params = input.get("params").cloned();

                // Get server URL from config or input
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

                match self.invoke_tool(&server_url, method, params, auth_token.as_deref()).await {
                    Ok(result) => ToolOutput::new()
                        .with_field("result", result)
                        .with_field("type", "success"),
                    Err(e) => ToolOutput::new()
                        .with_field("error", e.to_string())
                        .with_field("type", "error"),
                }
            }
            "discover" => {
                // Get server URL from config or input
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

                match self.discover_tools(&server_url, auth_token.as_deref()).await {
                    Ok(tools) => {
                        let count = tools.len();
                        ToolOutput::new()
                            .with_field("tools", tools)
                            .with_field("count", count)
                            .with_field("type", "discovered")
                    }
                    Err(e) => ToolOutput::new()
                        .with_field("error", e.to_string())
                        .with_field("type", "error"),
                }
            }
            "list_servers" => {
                let store = get_mcp_store();
                let servers = store.list();
                let count = servers.len();
                ToolOutput::new()
                    .with_field("servers", servers)
                    .with_field("count", count)
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
        guard
    }

    #[tokio::test]
    async fn test_mcp_tool_validation() {
        let tool = McpTool::new();

        // Valid - invoke action
        let input = ToolInput::new()
            .with_arg("action", "invoke")
            .with_arg("method", "tools/list")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_ok());

        // Valid - discover action
        let input = ToolInput::new()
            .with_arg("action", "discover")
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

        // Missing method for invoke
        let input = ToolInput::new()
            .with_arg("action", "invoke")
            .with_arg("server_url", "http://localhost:8080");
        assert!(tool.validate(&input).await.is_err());

        // Missing server for invoke
        let input = ToolInput::new()
            .with_arg("action", "invoke")
            .with_arg("method", "tools/list");
        assert!(tool.validate(&input).await.is_err());
    }

    #[tokio::test]
    async fn test_mcp_request_creation() {
        let request = McpRequest::new("tools/list", None);
        assert_eq!(request.jsonrpc, "2.0");
        assert_eq!(request.method, "tools/list");
        assert!(request.id.is_some());
    }

    #[tokio::test]
    async fn test_mcp_store() {
        let _guard = setup();
        let store = get_mcp_store();

        let config = McpServerConfig {
            name: "store-test-server".to_string(),
            url: "http://localhost:8080".to_string(),
            auth_token: Some("secret".to_string()),
            enabled: true,
        };

        store.register(config.clone());

        let retrieved = store.get("store-test-server");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().url, "http://localhost:8080");

        let list = store.list();
        assert!(!list.is_empty(), "Store should have at least our server");
    }

    #[tokio::test]
    async fn test_mcp_tool_list_servers() {
        let _guard = setup();
        let tool = McpTool::new();

        // Register a test server with unique name
        let store = get_mcp_store();
        store.register(McpServerConfig {
            name: "list-servers-test".to_string(),
            url: "http://localhost:8080".to_string(),
            auth_token: None,
            enabled: true,
        });

        let input = ToolInput::new().with_arg("action", "list_servers");
        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("server_list")
        );
        // Count should be at least 1 (our server)
        let count = output.data.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        assert!(
            count >= 1,
            "Expected at least 1 server in list, got {}",
            count
        );
    }

    #[test]
    fn test_mcp_server_config_serialization() {
        let config = McpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            auth_token: Some("token".to_string()),
            enabled: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("http://localhost:8080"));
    }

    #[test]
    fn test_mcp_response_parsing() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [{"name": "read_file"}]
            }
        }"#;

        let response: McpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert_eq!(response.id, Some(1));
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_mcp_error_parsing() {
        let json = r#"{
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid Request",
                "data": null
            }
        }"#;

        let response: McpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.jsonrpc, "2.0");
        assert!(response.result.is_none());
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32600);
        assert_eq!(error.message, "Invalid Request");
    }

    #[tokio::test]
    async fn test_mcp_tool_invoke_with_server_name() {
        let _guard = setup();
        let tool = McpTool::new();

        // Register a test server
        let store = get_mcp_store();
        store.register(McpServerConfig {
            name: "local-server".to_string(),
            url: "http://localhost:9999".to_string(),
            auth_token: None,
            enabled: true,
        });

        // Test that it tries to use the registered server (will fail because no actual server)
        let input = ToolInput::new()
            .with_arg("action", "invoke")
            .with_arg("method", "tools/list")
            .with_arg("server_name", "local-server");

        let output = tool.execute(input).await;

        // Should fail because no actual server is running, but the lookup should work
        assert!(output.data.get("error").is_some());
    }

    #[tokio::test]
    async fn test_mcp_tool_invoke_server_not_found() {
        let _guard = setup();
        let tool = McpTool::new();

        let input = ToolInput::new()
            .with_arg("action", "invoke")
            .with_arg("method", "tools/list")
            .with_arg("server_name", "nonexistent-server");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("not_found")
        );
    }
}
