//! WebFetchTool - Fetch web pages and extract content
//!
//! This tool fetches web pages using HTTP and extracts readable content.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the WebFetchTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebFetchInput {
    /// The URL to fetch.
    pub url: String,
    /// Maximum length of content to return.
    #[serde(default)]
    pub max_length: Option<usize>,
    /// Whether to extract main content only (removes navigation, ads, etc).
    #[serde(default)]
    pub extract_content: Option<bool>,
}

/// Output schema for web fetch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFetchOutput {
    pub url: String,
    pub title: Option<String>,
    pub content: String,
    pub content_type: String,
    pub status_code: u16,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The WebFetchTool fetches web pages using HTTP.
#[derive(Debug, Clone, Default)]
pub struct WebFetchTool;

impl WebFetchTool {
    /// Create a new WebFetchTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Fetch content from a URL.
    async fn fetch_url(&self, url: &str, max_length: usize, extract_content: bool) -> WebFetchOutput {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return WebFetchOutput {
                    url: url.to_string(),
                    title: None,
                    content: String::new(),
                    content_type: String::new(),
                    status_code: 0,
                    success: false,
                    error: Some(format!("Failed to create HTTP client: {e}")),
                };
            }
        };

        let response = match client.get(url).send().await {
            Ok(r) => r,
            Err(e) => {
                return WebFetchOutput {
                    url: url.to_string(),
                    title: None,
                    content: String::new(),
                    content_type: String::new(),
                    status_code: 0,
                    success: false,
                    error: Some(format!("Failed to fetch URL: {e}")),
                };
            }
        };

        let status_code = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "text/html".to_string());

        if !response.status().is_success() {
            return WebFetchOutput {
                url: url.to_string(),
                title: None,
                content: String::new(),
                content_type,
                status_code,
                success: false,
                error: Some(format!("HTTP error: {}", response.status())),
            };
        }

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                return WebFetchOutput {
                    url: url.to_string(),
                    title: None,
                    content: String::new(),
                    content_type,
                    status_code,
                    success: false,
                    error: Some(format!("Failed to read response body: {e}")),
                };
            }
        };

        // Extract title and content
        let (title, content) = if extract_content && content_type.contains("text/html") {
            self.extract_html_content(&body, max_length)
        } else {
            (None, body.chars().take(max_length).collect())
        };

        WebFetchOutput {
            url: url.to_string(),
            title,
            content,
            content_type,
            status_code,
            success: true,
            error: None,
        }
    }

    /// Extract readable content from HTML.
    fn extract_html_content(&self, html: &str, max_length: usize) -> (Option<String>, String) {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);

        // Extract title
        let title = document
            .select(&Selector::parse("title").unwrap())
            .next()
            .map(|e| e.text().collect::<String>().trim().to_string());

        // Try to find main content area
        let content_selectors = [
            "main",
            "article",
            "[role='main']",
            ".content",
            "#content",
            ".main-content",
            "#main-content",
            "body",
        ];

        let mut extracted_text = String::new();

        for selector_str in &content_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = document.select(&selector).next() {
                    // Extract text from paragraphs and headings
                    let text_selector = Selector::parse("p, h1, h2, h3, h4, h5, h6, li, pre, code").unwrap();
                    for text_element in element.select(&text_selector) {
                        let text = text_element.text().collect::<String>();
                        let trimmed = text.trim();
                        if !trimmed.is_empty() && !trimmed.chars().all(|c| c.is_whitespace()) {
                            extracted_text.push_str(trimmed);
                            extracted_text.push('\n');
                        }
                        if extracted_text.len() >= max_length {
                            break;
                        }
                    }
                    if !extracted_text.is_empty() {
                        break;
                    }
                }
            }
        }

        // If no structured content found, fallback to all text
        if extracted_text.is_empty() {
            extracted_text = document.root_element().text().collect::<String>();
        }

        // Clean up whitespace
        let cleaned = extracted_text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect::<Vec<_>>()
            .join("\n");

        let truncated: String = cleaned.chars().take(max_length).collect();

        (title, truncated)
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new("WebFetchTool", "Fetch web pages and extract content").read_only()
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let url = input
            .require("url")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "url must be a string".to_string(),
                error_code: Some(1),
            })?;

        if url.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "url cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ToolError::ValidationFailed {
                message: "URL must start with http:// or https://".to_string(),
                error_code: Some(3),
            });
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let url = match input.get("url") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "url must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: url")
                    .with_field("success", false);
            }
        };

        let max_length = input
            .get("max_length")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(10000);

        let extract_content = input
            .get("extract_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let result = self.fetch_url(&url, max_length, extract_content).await;

        ToolOutput::new()
            .with_field("url", result.url)
            .with_field("title", result.title)
            .with_field("content", result.content)
            .with_field("content_type", result.content_type)
            .with_field("status_code", result.status_code)
            .with_field("success", result.success)
            .with_field("error", result.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_fetch_validation() {
        let tool = WebFetchTool::new();

        // Valid URL
        let input = ToolInput::new().with_arg("url", "https://example.com");
        assert!(tool.validate(&input).await.is_ok());

        // Missing URL
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Empty URL
        let input = ToolInput::new().with_arg("url", "");
        assert!(tool.validate(&input).await.is_err());

        // Invalid protocol
        let input = ToolInput::new().with_arg("url", "ftp://example.com");
        assert!(tool.validate(&input).await.is_err());
    }

    #[test]
    fn test_web_fetch_metadata() {
        let tool = WebFetchTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "WebFetchTool");
        assert!(meta.is_read_only);
    }

    #[test]
    fn test_extract_html_content() {
        let tool = WebFetchTool::new();
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page</title></head>
            <body>
                <h1>Main Heading</h1>
                <p>This is a paragraph.</p>
                <p>Another paragraph with more content.</p>
            </body>
            </html>
        "#;

        let (title, content) = tool.extract_html_content(html, 1000);

        assert_eq!(title, Some("Test Page".to_string()));
        assert!(content.contains("Main Heading"));
        assert!(content.contains("This is a paragraph."));
    }
}
