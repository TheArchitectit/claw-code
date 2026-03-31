//! MCP connection and resource types for the tool use context.

/// MCP connection information.
#[derive(Debug, Clone)]
pub struct McpConnection {
    /// The server name.
    pub name: String,

    /// The connection state.
    pub state: McpConnectionState,

    /// Capabilities supported by the server.
    pub capabilities: Vec<String>,
}

impl McpConnection {
    /// Create a new MCP connection with the given name.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            state: McpConnectionState::Disconnected,
            capabilities: Vec::new(),
        }
    }
}

/// MCP connection states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpConnectionState {
    /// The connection is connecting.
    Connecting,

    /// The connection is active.
    Connected,

    /// The connection is disconnected.
    Disconnected,

    /// The connection has an error.
    Error(String),
}

impl std::fmt::Display for McpConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpConnectionState::Connecting => write!(f, "connecting"),
            McpConnectionState::Connected => write!(f, "connected"),
            McpConnectionState::Disconnected => write!(f, "disconnected"),
            McpConnectionState::Error(e) => write!(f, "error: {e}"),
        }
    }
}

/// MCP resource information.
#[derive(Debug, Clone)]
pub struct McpResource {
    /// The resource URI.
    pub uri: String,

    /// The resource name.
    pub name: String,

    /// The MIME type.
    pub mime_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_connection_state() {
        let states = vec![
            McpConnectionState::Disconnected,
            McpConnectionState::Connecting,
            McpConnectionState::Connected,
            McpConnectionState::Error("test".to_string()),
        ];

        for state in states {
            let _ = format!("{}", state);
        }
    }

    #[test]
    fn test_mcp_connection_creation() {
        let conn = McpConnection::new("test-server".to_string());
        assert_eq!(conn.name, "test-server");
        assert!(matches!(conn.state, McpConnectionState::Disconnected));
    }
}
