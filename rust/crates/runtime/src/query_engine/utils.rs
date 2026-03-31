//! Utility functions for the query engine.

use crate::messages::{ContentBlock, ToolResultContent};
use crate::tool::ToolOutput;

/// Process tool execution results into ContentBlock format for LLM consumption.
///
/// Handles different output types:
/// - Text: Plain text output
/// - JSON: Structured data
/// - Binary: Base64 encoded data
/// - Errors: Formatted with is_error flag
///
/// Also handles oversized results with truncation.
pub fn process_tool_results(
    results: Vec<(crate::types::ToolUseId, ToolOutput)>,
) -> Vec<ContentBlock> {
    const MAX_RESULT_SIZE: usize = 100_000;
    const TRUNCATION_SUFFIX: &str = "\n... (truncated)";

    results
        .into_iter()
        .map(|(tool_use_id, output)| {
            let (content, is_error) = format_tool_output(output, MAX_RESULT_SIZE, TRUNCATION_SUFFIX);

            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error: if is_error { Some(true) } else { None },
            }
        })
        .collect()
}

/// Format a single ToolOutput into ToolResultContent.
///
/// Returns the content vector and a boolean indicating if this is an error.
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

    // Handle binary data (stored as object with "_binary" key and base64 data)
    if let Some(binary_data) = data.get("_binary").and_then(|b| b.as_str()) {
        let content = vec![ToolResultContent::Text {
            text: format!("Binary data (base64): {}", binary_data),
        }];
        return (content, false);
    }

    // Handle different data types
    let content = match &data {
        // String data - treat as text
        serde_json::Value::String(text) => {
            let truncated = truncate_text(text, max_size, truncation_suffix);
            vec![ToolResultContent::Text { text: truncated }]
        }
        // Array or Object - treat as JSON
        serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            // Check if it's a simple array of strings that should be joined
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
            // Otherwise return as JSON
            vec![ToolResultContent::Json { data }]
        }
        // Numbers, booleans, null - convert to string
        _ => {
            let text = data.to_string();
            let truncated = truncate_text(&text, max_size, truncation_suffix);
            vec![ToolResultContent::Text { text: truncated }]
        }
    };

    (content, false)
}

/// Truncate text if it exceeds the maximum size.
pub fn truncate_text(text: &str, max_size: usize, suffix: &str) -> String {
    if text.len() <= max_size {
        text.to_string()
    } else {
        let truncation_point = max_size.saturating_sub(suffix.len());
        let truncated = &text[..truncation_point.min(text.len())];
        format!("{}{}", truncated, suffix)
    }
}
