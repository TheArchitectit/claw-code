//! Content block types for messages.
//!
//! This module defines the various content blocks that can appear in messages,
//! including text, tool use, tool results, thinking blocks, and images.

use serde::{Deserialize, Serialize};

use crate::types::ToolUseId;

/// A content block in a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text {
        /// The text content.
        text: String,

        /// Whether this is a citation.
        #[serde(skip_serializing_if = "Option::is_none")]
        citation: Option<bool>,
    },

    /// A tool use block.
    ToolUse {
        /// The tool use ID.
        id: ToolUseId,

        /// The tool name.
        name: String,

        /// The tool input.
        input: serde_json::Value,
    },

    /// A tool result block.
    ToolResult {
        /// The tool use ID this is a result for.
        tool_use_id: ToolUseId,

        /// The result content.
        content: Vec<ToolResultContent>,

        /// Whether this is an error result.
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },

    /// Thinking block.
    Thinking {
        /// The thinking content.
        thinking: String,

        /// Signature for verification.
        #[serde(skip_serializing_if = "Option::is_none")]
        signature: Option<String>,
    },

    /// Redacted thinking block.
    RedactedThinking {
        /// The redacted data.
        data: String,
    },

    /// Image content.
    Image {
        /// The image source.
        source: ImageSource,
    },
}

impl ContentBlock {
    /// Get the text content if this is a text block.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text, .. } => Some(text),
            _ => None,
        }
    }

    /// Check if this is a text block.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self, Self::Text { .. })
    }

    /// Check if this is a tool use block.
    #[must_use]
    pub fn is_tool_use(&self) -> bool {
        matches!(self, Self::ToolUse { .. })
    }

    /// Check if this is a tool result block.
    #[must_use]
    pub fn is_tool_result(&self) -> bool {
        matches!(self, Self::ToolResult { .. })
    }

    /// Check if this is a thinking block.
    #[must_use]
    pub fn is_thinking(&self) -> bool {
        matches!(self, Self::Thinking { .. })
    }
}

/// Tool result content types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolResultContent {
    /// Text result.
    Text {
        /// The text content.
        text: String,
    },

    /// Image result.
    Image {
        /// The image source.
        source: ImageSource,
    },

    /// Error result.
    Error {
        /// The error message.
        message: String,
    },

    /// JSON result.
    Json {
        /// The JSON data.
        data: serde_json::Value,
    },
}

/// Image source types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64 encoded image.
    Base64 {
        /// The media type.
        media_type: String,

        /// The base64 data.
        data: String,
    },

    /// Image from a URL.
    Url {
        /// The URL.
        url: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
            citation: None,
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("text"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_content_block_variants() {
        let text = ContentBlock::Text {
            text: "Hello".to_string(),
            citation: Some(true),
        };

        let tool_use = ContentBlock::ToolUse {
            id: ToolUseId::new("test"),
            name: "Bash".to_string(),
            input: serde_json::json!({"command": "echo hi"}),
        };

        let tool_result = ContentBlock::ToolResult {
            tool_use_id: ToolUseId::new("test"),
            content: vec![ToolResultContent::Text {
                text: "result".to_string(),
            }],
            is_error: Some(false),
        };

        let thinking = ContentBlock::Thinking {
            thinking: "thinking".to_string(),
            signature: Some("sig".to_string()),
        };

        let redacted = ContentBlock::RedactedThinking {
            data: "data".to_string(),
        };

        let image = ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "data".to_string(),
            },
        };

        // Test serialization of all variants
        let _ = serde_json::to_string(&text).unwrap();
        let _ = serde_json::to_string(&tool_use).unwrap();
        let _ = serde_json::to_string(&tool_result).unwrap();
        let _ = serde_json::to_string(&thinking).unwrap();
        let _ = serde_json::to_string(&redacted).unwrap();
        let _ = serde_json::to_string(&image).unwrap();
    }

    #[test]
    fn test_tool_result_content_variants() {
        let text = ToolResultContent::Text {
            text: "result".to_string(),
        };
        let error = ToolResultContent::Error {
            message: "error".to_string(),
        };
        let json = ToolResultContent::Json {
            data: serde_json::json!({"key": "value"}),
        };
        let image = ToolResultContent::Image {
            source: ImageSource::Base64 {
                media_type: "image/png".to_string(),
                data: "data".to_string(),
            },
        };

        // Test serialization
        let _ = serde_json::to_string(&text).unwrap();
        let _ = serde_json::to_string(&error).unwrap();
        let _ = serde_json::to_string(&json).unwrap();
        let _ = serde_json::to_string(&image).unwrap();
    }

    #[test]
    fn test_image_source_variants() {
        let base64 = ImageSource::Base64 {
            media_type: "image/png".to_string(),
            data: "base64data".to_string(),
        };

        let url = ImageSource::Url {
            url: "http://example.com/image.png".to_string(),
        };

        let _ = serde_json::to_string(&base64).unwrap();
        let _ = serde_json::to_string(&url).unwrap();
    }
}
