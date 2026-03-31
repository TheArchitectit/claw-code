//! Tool Execution Orchestration Implementation
//!
//! This module provides the tool execution flow that receives tool use requests,
//! executes tools, and returns results.

use std::collections::HashSet;
use futures::future::join_all;
use tracing::debug;

use crate::context::ToolUseContext;
use crate::messages::ToolResultContent;
use crate::registry::ToolRegistry;
use crate::tool::{Tool, ToolOutput};
use crate::types::{QueryResult, ToolError, ToolUseId};

/// Execute a tool and return formatted ToolResultContent.
///
/// This method provides a higher-level interface over `execute_tool`,
/// returning properly formatted content for LLM consumption.
///
/// # Arguments
/// * `name` - The tool name to execute
/// * `params` - The tool parameters as JSON
/// * `tool_use_id` - The unique ID for this tool use
///
/// # Returns
/// * `ToolResultContent` on success (text, json, or error content)
pub async fn execute_tool_with_result(
    tool_registry: &ToolRegistry,
    name: &str,
    params: serde_json::Value,
    tool_use_id: ToolUseId,
    context: &ToolUseContext,
) -> Result<ToolResultContent, ToolError> {
    // Find the tool
    let tool = tool_registry
        .get(name)
        .ok_or_else(|| ToolError::not_found(name))?;

    // Execute the tool
    let output = tool
        .execute(params, context, tool_use_id, None)
        .await?;

    // Format the output into ToolResultContent
    let content = format_output_to_content(output)?;

    Ok(content)
}

/// Execute multiple tools in parallel where safe.
///
/// This method analyzes tool calls to determine which can be executed
/// concurrently (concurrency-safe, read-only tools) and which must
/// be executed sequentially.
///
/// # Arguments
/// * `tool_calls` - List of (tool_use_id, tool_name, tool_input) tuples
/// * `tool_registry` - The registry to look up tools
/// * `context` - The tool use context
///
/// # Returns
/// * Vec of (ToolUseId, result) pairs
pub async fn execute_tools_parallel(
    tool_calls: Vec<(ToolUseId, String, serde_json::Value)>,
    tool_registry: &ToolRegistry,
    context: &ToolUseContext,
) -> Vec<(ToolUseId, Result<ToolOutput, ToolError>)> {
    if tool_calls.is_empty() {
        return Vec::new();
    }

    // Track in-progress tool use IDs
    let ids: HashSet<ToolUseId> = tool_calls.iter().map(|(id, _, _)| id.clone()).collect();
    update_in_progress_tool_ids(&ids).await;

    // Categorize tools: concurrent-safe vs sequential
    let mut concurrent_calls: Vec<(ToolUseId, String, serde_json::Value)> = Vec::new();
    let mut sequential_calls: Vec<(ToolUseId, String, serde_json::Value)> = Vec::new();

    for (tool_use_id, tool_name, tool_input) in tool_calls {
        if let Some(tool) = tool_registry.get(&tool_name) {
            if tool.is_concurrency_safe(&tool_input) && tool.is_read_only(&tool_input) {
                concurrent_calls.push((tool_use_id, tool_name, tool_input));
            } else {
                sequential_calls.push((tool_use_id, tool_name, tool_input));
            }
        } else {
            // Tool not found - add to sequential to handle the error properly
            sequential_calls.push((tool_use_id, tool_name, tool_input));
        }
    }

    let mut results: Vec<(ToolUseId, Result<ToolOutput, ToolError>)> = Vec::new();

    // Execute concurrent-safe tools in parallel
    if !concurrent_calls.is_empty() {
        debug!("Executing {} tools concurrently", concurrent_calls.len());

        let concurrent_futures = concurrent_calls.into_iter().map(|(id, name, input)| {
            let tool = tool_registry.get(&name);
            let context = context.clone();
            async move {
                let result = match tool {
                    Some(t) => t.execute(input, &context, id.clone(), None).await,
                    None => Err(ToolError::not_found(&name)),
                };
                (id, result)
            }
        });

        let concurrent_results = join_all(concurrent_futures).await;
        results.extend(concurrent_results);
    }

    // Execute sequential tools one by one
    for (tool_use_id, tool_name, tool_input) in sequential_calls {
        debug!("Executing tool {} sequentially", tool_name);
        let result = match tool_registry.get(&tool_name) {
            Some(tool) => tool.execute(tool_input, context, tool_use_id.clone(), None).await,
            None => Err(ToolError::not_found(&tool_name)),
        };
        results.push((tool_use_id, result));
    }

    // Clear in-progress tool IDs
    update_in_progress_tool_ids(&HashSet::new()).await;

    results
}

/// Update the set of in-progress tool use IDs.
async fn update_in_progress_tool_ids(ids: &HashSet<ToolUseId>) {
    // This could be extended to broadcast to listeners
    // For now, we just track internally
    let _ = ids;
}

/// Format a ToolOutput into a ToolResultContent.
fn format_output_to_content(output: ToolOutput) -> Result<ToolResultContent, ToolError> {
    let data = output.data;

    // Check if this is an error response
    if let Some(error_msg) = data.get("error").and_then(|e| e.as_str()) {
        return Ok(ToolResultContent::Error {
            message: error_msg.to_string(),
        });
    }

    // Handle different data types
    match data {
        serde_json::Value::String(text) => {
            Ok(ToolResultContent::Text { text })
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Ok(ToolResultContent::Json { data })
        }
        _ => {
            let text = data.to_string();
            Ok(ToolResultContent::Text { text })
        }
    }
}

/// Format a single ToolOutput into a ToolResultContent vector.
///
/// Simplified version for single output formatting with truncation support.
pub fn format_tool_output_single(output: ToolOutput) -> (Vec<ToolResultContent>, bool) {
    const MAX_RESULT_SIZE: usize = 100_000;
    const TRUNCATION_SUFFIX: &str = "\n... (truncated)";

    format_tool_output(output, MAX_RESULT_SIZE, TRUNCATION_SUFFIX)
}

/// Format a ToolOutput into ToolResultContent with truncation.
pub fn format_tool_output(
    output: ToolOutput,
    max_size: usize,
    truncation_suffix: &str,
) -> (Vec<ToolResultContent>, bool) {
    let data = output.data;

    // Check if this is an error response
    if let Some(error_msg) = data.get("error").and_then(|e| e.as_str()) {
        let content = vec![ToolResultContent::Error {
            message: error_msg.to_string(),
        }];
        return (content, true);
    }

    // Handle binary data
    if let Some(binary_data) = data.get("_binary").and_then(|b| b.as_str()) {
        let content = vec![ToolResultContent::Text {
            text: format!("Binary data (base64): {}", binary_data),
        }];
        return (content, false);
    }

    // Handle different data types
    let content = match &data {
        serde_json::Value::String(text) => {
            let truncated = truncate_text(text, max_size, truncation_suffix);
            vec![ToolResultContent::Text { text: truncated }]
        }
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            // Check if it's a simple array of strings
            if let Some(strings) = data.as_array() {
                if strings.iter().all(|v| v.is_string()) {
                    let joined: String = strings
                        .iter()
                        .filter_map(|v| v.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let truncated = truncate_text(&joined, max_size, truncation_suffix);
                    return (vec![ToolResultContent::Text { text: truncated }], false);
                }
            }
            vec![ToolResultContent::Json { data: data.clone() }]
        }
        _ => {
            let text = data.to_string();
            let truncated = truncate_text(&text, max_size, truncation_suffix);
            vec![ToolResultContent::Text { text: truncated }]
        }
    };

    (content, false)
}

/// Truncate text if it exceeds the maximum size.
fn truncate_text(text: &str, max_size: usize, suffix: &str) -> String {
    if text.len() <= max_size {
        text.to_string()
    } else {
        let truncation_point = max_size.saturating_sub(suffix.len());
        let truncated = &text[..truncation_point.min(text.len())];
        format!("{}{}", truncated, suffix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_execute_tool_with_result_not_found() {
        let registry = ToolRegistry::new();
        let context = ToolUseContext::new(ToolUseId::generate(), "/tmp");
        let tool_use_id = ToolUseId::generate();

        let result = execute_tool_with_result(
            &registry,
            "nonexistent_tool",
            json!({}),
            tool_use_id,
            &context,
        ).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound { .. }));
    }

    #[tokio::test]
    async fn test_execute_tools_parallel_empty() {
        let registry = ToolRegistry::new();
        let context = ToolUseContext::new(ToolUseId::generate(), "/tmp");

        let results = execute_tools_parallel(
            Vec::new(),
            &registry,
            &context,
        ).await;

        assert!(results.is_empty());
    }

    #[test]
    fn test_format_tool_output_single() {
        let output = ToolOutput::new("Hello, world!");
        let (content, is_error) = format_tool_output_single(output);

        assert!(!is_error);
        assert_eq!(content.len(), 1);
        match &content[0] {
            ToolResultContent::Text { text } => {
                assert_eq!(text, "Hello, world!");
            }
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_format_tool_output_error() {
        let output = ToolOutput::new_error("Something went wrong");
        let (content, is_error) = format_tool_output_single(output);

        assert!(is_error);
        assert_eq!(content.len(), 1);
        match &content[0] {
            ToolResultContent::Error { message } => {
                assert!(message.contains("Something went wrong"));
            }
            _ => panic!("Expected error content"),
        }
    }

    #[test]
    fn test_truncate_text() {
        let text = "Hello, world!";
        let result = truncate_text(text, 100, "...");
        assert_eq!(result, "Hello, world!");

        let long_text = "a".repeat(200_000);
        let result = truncate_text(&long_text, 100_000, "\n... (truncated)");
        assert!(result.ends_with("\n... (truncated)"));
        assert!(result.len() <= 100_000);
    }
}
