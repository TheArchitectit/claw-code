//! Permission request types.
//!
//! A [`PermissionRequest`] represents a request to execute a tool,
//! containing all the information needed to make a permission decision.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tools::{Tool, ToolInput};
use uuid::Uuid;

/// A request for permission to execute a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Unique identifier for this request.
    pub id: String,
    /// The name of the tool being requested.
    pub tool_name: String,
    /// The input to the tool.
    pub input: ToolInput,
    /// The session ID this request belongs to.
    pub session_id: String,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// Additional metadata about the request.
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub metadata: HashMap<String, String>,
}

impl PermissionRequest {
    /// Create a new permission request.
    #[must_use]
    pub fn new(tool: &dyn Tool, input: ToolInput, session_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool_name: tool.metadata().name.clone(),
            input,
            session_id: session_id.into(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new permission request with a specific ID.
    #[must_use]
    pub fn with_id(
        id: impl Into<String>,
        tool_name: impl Into<String>,
        input: ToolInput,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            tool_name: tool_name.into(),
            input,
            session_id: session_id.into(),
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the request.
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Get the age of this request in milliseconds.
    #[must_use]
    pub fn age_ms(&self) -> u64 {
        let now = Utc::now();
        let duration = now.signed_duration_since(self.created_at);
        duration.num_milliseconds().max(0) as u64
    }

    /// Check if this request has expired based on a timeout.
    #[must_use]
    pub fn is_expired(&self, timeout_ms: u64) -> bool {
        self.age_ms() > timeout_ms
    }

    /// Get the tool name.
    #[must_use]
    pub fn tool_name(&self) -> &str {
        &self.tool_name
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the request ID.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Information about a permission request for display purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestInfo {
    /// The request ID.
    pub id: String,
    /// The tool name.
    pub tool_name: String,
    /// A human-readable description of the request.
    pub description: String,
    /// The session ID.
    pub session_id: String,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
}

impl From<&PermissionRequest> for PermissionRequestInfo {
    fn from(request: &PermissionRequest) -> Self {
        Self {
            id: request.id.clone(),
            tool_name: request.tool_name.clone(),
            description: format!("Execute {} tool", request.tool_name),
            session_id: request.session_id.clone(),
            created_at: request.created_at,
        }
    }
}

/// Response to a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionResponse {
    /// The request was approved.
    Approved {
        /// The request ID.
        request_id: String,
        /// Whether this decision should be persisted.
        permanent: bool,
    },
    /// The request was denied.
    Denied {
        /// The request ID.
        request_id: String,
        /// The reason for denial.
        reason: Option<String>,
        /// Whether this decision should be persisted.
        permanent: bool,
    },
    /// The request was cancelled.
    Cancelled {
        /// The request ID.
        request_id: String,
    },
}

impl PermissionResponse {
    /// Create an approval response.
    #[must_use]
    pub fn approved(request_id: impl Into<String>, permanent: bool) -> Self {
        Self::Approved {
            request_id: request_id.into(),
            permanent,
        }
    }

    /// Create a denial response.
    #[must_use]
    pub fn denied(
        request_id: impl Into<String>,
        reason: impl Into<String>,
        permanent: bool,
    ) -> Self {
        Self::Denied {
            request_id: request_id.into(),
            reason: Some(reason.into()),
            permanent,
        }
    }

    /// Create a cancellation response.
    #[must_use]
    pub fn cancelled(request_id: impl Into<String>) -> Self {
        Self::Cancelled {
            request_id: request_id.into(),
        }
    }

    /// Get the request ID.
    #[must_use]
    pub fn request_id(&self) -> &str {
        match self {
            Self::Approved { request_id, .. } => request_id,
            Self::Denied { request_id, .. } => request_id,
            Self::Cancelled { request_id } => request_id,
        }
    }

    /// Check if this is an approval.
    #[must_use]
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved { .. })
    }

    /// Check if this is a denial.
    #[must_use]
    pub fn is_denied(&self) -> bool {
        matches!(self, Self::Denied { .. })
    }

    /// Check if this is a cancellation.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        matches!(self, Self::Cancelled { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tools::{BashTool, FileReadTool, ToolInput};

    #[test]
    fn test_permission_request_new() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo test");
        let request = PermissionRequest::new(&tool, input, "session-1");

        assert!(!request.id.is_empty());
        assert_eq!(request.tool_name, "BashTool");
        assert_eq!(request.session_id, "session-1");
        assert!(request.age_ms() < 1000);
    }

    #[test]
    fn test_permission_request_with_id() {
        let input = ToolInput::new().with_arg("path", "/test");
        let request = PermissionRequest::with_id("custom-id", "TestTool", input, "session-1");

        assert_eq!(request.id, "custom-id");
        assert_eq!(request.tool_name, "TestTool");
    }

    #[test]
    fn test_permission_request_with_metadata() {
        let tool = FileReadTool::new();
        let input = ToolInput::new();
        let request = PermissionRequest::new(&tool, input, "session-1")
            .with_metadata("source", "test");

        assert_eq!(request.metadata.get("source"), Some(&"test".to_string()));
    }

    #[test]
    fn test_permission_request_expired() {
        let tool = BashTool::new();
        let input = ToolInput::new();
        let request = PermissionRequest::new(&tool, input, "session-1");

        // Should not be expired with a long timeout
        assert!(!request.is_expired(300_000));
    }

    #[test]
    fn test_permission_request_info() {
        let tool = BashTool::new();
        let input = ToolInput::new();
        let request = PermissionRequest::new(&tool, input, "session-1");
        let info = PermissionRequestInfo::from(&request);

        assert_eq!(info.id, request.id);
        assert_eq!(info.tool_name, "BashTool");
        assert!(!info.description.is_empty());
    }

    #[test]
    fn test_permission_response_approved() {
        let response = PermissionResponse::approved("req-1", true);
        assert!(response.is_approved());
        assert!(!response.is_denied());
        assert!(!response.is_cancelled());
        assert_eq!(response.request_id(), "req-1");
    }

    #[test]
    fn test_permission_response_denied() {
        let response = PermissionResponse::denied("req-1", "reason", false);
        assert!(!response.is_approved());
        assert!(response.is_denied());
        assert!(!response.is_cancelled());
    }

    #[test]
    fn test_permission_response_cancelled() {
        let response = PermissionResponse::cancelled("req-1");
        assert!(!response.is_approved());
        assert!(!response.is_denied());
        assert!(response.is_cancelled());
    }
}
