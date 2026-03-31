//! WebSearchTool - Search the web using search engines
//!
//! This tool performs web searches using DuckDuckGo Lite.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Input schema for the WebSearchTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSearchInput {
    /// The search query.
    pub query: String,
    /// Maximum number of results to return.
    #[serde(default)]
    pub num_results: Option<usize>,
}

/// A search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Output schema for web search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchOutput {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The WebSearchTool searches the web using DuckDuckGo.
#[derive(Debug, Clone, Default)]
pub struct WebSearchTool;

impl WebSearchTool {
    /// Create a new WebSearchTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Search using DuckDuckGo Lite.
    async fn search_duckduckgo(&self, query: &str, num_results: usize) -> WebSearchOutput {
        let client = match reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                return WebSearchOutput {
                    query: query.to_string(),
                    results: Vec::new(),
                    success: false,
                    error: Some(format!("Failed to create HTTP client: {e}")),
                };
            }
        };

        // Build DuckDuckGo Lite search URL
        let encoded_query = urlencoding::encode(query);
        let url = format!("https://lite.duckduckgo.com/lite/?q={}", encoded_query);

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                return WebSearchOutput {
                    query: query.to_string(),
                    results: Vec::new(),
                    success: false,
                    error: Some(format!("Failed to search: {e}")),
                };
            }
        };

        let status = response.status();
        if !status.is_success() {
            return WebSearchOutput {
                query: query.to_string(),
                results: Vec::new(),
                success: false,
                error: Some(format!("HTTP error: {status}")),
            };
        }

        let html = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                return WebSearchOutput {
                    query: query.to_string(),
                    results: Vec::new(),
                    success: false,
                    error: Some(format!("Failed to read response: {e}")),
                };
            }
        };

        let results = self.parse_duckduckgo_results(&html, num_results);

        WebSearchOutput {
            query: query.to_string(),
            results,
            success: true,
            error: None,
        }
    }

    /// Parse DuckDuckGo Lite HTML results.
    fn parse_duckduckgo_results(&self, html: &str, max_results: usize) -> Vec<SearchResult> {
        use scraper::{Html, Selector};

        let document = Html::parse_document(html);
        let mut results = Vec::new();

        // DuckDuckGo Lite uses table rows with class "result-link" or similar
        // Try multiple selectors for robustness
        let selectors = [
            "table.result-link",
            ".result-link",
            ".result",
            "tr.result",
        ];

        for selector_str in &selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector).take(max_results) {
                    // Try to find link and title
                    let link_selector = Selector::parse("a").unwrap();
                    if let Some(link) = element.select(&link_selector).next() {
                        let title = link
                            .text()
                            .collect::<String>()
                            .trim()
                            .to_string();

                        let url = link
                            .value()
                            .attr("href")
                            .map(|s| s.to_string())
                            .unwrap_or_default();

                        // Skip empty results
                        if title.is_empty() || url.is_empty() {
                            continue;
                        }

                        // Try to find snippet - simplified approach
                        // Extract snippet from parent or sibling text
                        let snippet = element
                            .parent()
                            .and_then(|p| p.next_sibling())
                            .and_then(|sibling| {
                                // Collect text from all text nodes in siblings
                                let mut text_parts = Vec::new();
                                for node in sibling.children() {
                                    if let Some(text) = node.value().as_text() {
                                        text_parts.push(text.trim().to_string());
                                    }
                                }
                                let text = text_parts.join(" ");
                                if text.is_empty() { None } else { Some(text) }
                            })
                            .unwrap_or_default();

                        results.push(SearchResult {
                            title,
                            url,
                            snippet,
                        });

                        if results.len() >= max_results {
                            break;
                        }
                    }
                }

                if !results.is_empty() {
                    break;
                }
            }
        }

        // Fallback: try generic result parsing
        if results.is_empty() {
            // Look for any links that look like search results
            let link_selector = Selector::parse("a[href^='http']").unwrap();
            for link in document.select(&link_selector).take(max_results * 2) {
                let title = link.text().collect::<String>().trim().to_string();
                let url = link.value().attr("href").unwrap_or("").to_string();

                // Filter out navigation links
                if title.len() > 10 && url.starts_with("http") && !url.contains("duckduckgo") {
                    results.push(SearchResult {
                        title,
                        url,
                        snippet: String::new(),
                    });

                    if results.len() >= max_results {
                        break;
                    }
                }
            }
        }

        results
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new("WebSearchTool", "Search the web using search engines").read_only()
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let query = input
            .require("query")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "query must be a string".to_string(),
                error_code: Some(1),
            })?;

        if query.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "query cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let query = match input.get("query") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "query must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: query")
                    .with_field("success", false);
            }
        };

        let num_results = input
            .get("num_results")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(10);

        let result = self.search_duckduckgo(&query, num_results).await;

        ToolOutput::new()
            .with_field("query", result.query)
            .with_field("results", result.results)
            .with_field("success", result.success)
            .with_field("error", result.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_search_validation() {
        let tool = WebSearchTool::new();

        // Valid query
        let input = ToolInput::new().with_arg("query", "rust programming");
        assert!(tool.validate(&input).await.is_ok());

        // Missing query
        let input = ToolInput::new();
        assert!(tool.validate(&input).await.is_err());

        // Empty query
        let input = ToolInput::new().with_arg("query", "");
        assert!(tool.validate(&input).await.is_err());
    }

    #[test]
    fn test_web_search_metadata() {
        let tool = WebSearchTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "WebSearchTool");
        assert!(meta.is_read_only);
    }

    #[test]
    fn test_parse_duckduckgo_results() {
        let tool = WebSearchTool::new();
        let html = r#"
            <html>
            <body>
                <table>
                    <tr class="result-link">
                        <td><a href="https://example.com">Example Site</a></td>
                    </tr>
                    <tr><td>This is a description</td></tr>
                </table>
            </body>
            </html>
        "#;

        let results = tool.parse_duckduckgo_results(html, 5);

        // Should find at least one result
        assert!(!results.is_empty());
        assert_eq!(results[0].title, "Example Site");
        assert_eq!(results[0].url, "https://example.com");
    }
}
