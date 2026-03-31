//! FileWriteTool - Create or overwrite files with parent directory creation.
//!
//! This tool provides file writing capabilities:
//! - Create new files with automatic parent directory creation
//! - Overwrite existing files
//! - Return structured diff output

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Maximum file size for writing (50 MB).
const MAX_WRITE_SIZE_BYTES: usize = 50 * 1024 * 1024;

/// Input schema for the FileWriteTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileWriteInput {
    /// The absolute path to the file to write.
    pub file_path: String,

    /// The content to write to the file.
    pub content: String,
}

/// Hunk in a diff patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<String>,
}

/// Output schema for the FileWriteTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWriteOutput {
    /// Whether a new file was created or existing was updated.
    pub operation: String,
    /// The path to the file that was written.
    pub file_path: String,
    /// The content that was written.
    pub content: String,
    /// Diff patch showing changes (empty for new files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured_patch: Option<Vec<PatchHunk>>,
    /// Original file content (null for new files).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
}

/// The FileWriteTool creates or overwrites files.
#[derive(Debug, Clone, Default)]
pub struct FileWriteTool;

impl FileWriteTool {
    /// Create a new FileWriteTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Generate a simple diff between old and new content.
    fn generate_diff(old_content: &str, new_content: &str, _file_path: &str) -> Vec<PatchHunk> {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        // Simple diff: find the first difference
        let mut first_diff = 0;
        while first_diff < old_lines.len()
            && first_diff < new_lines.len()
            && old_lines[first_diff] == new_lines[first_diff]
        {
            first_diff += 1;
        }

        // Find the last difference
        let mut old_end = old_lines.len();
        let mut new_end = new_lines.len();
        while old_end > first_diff && new_end > first_diff {
            if old_lines[old_end - 1] == new_lines[new_end - 1] {
                old_end -= 1;
                new_end -= 1;
            } else {
                break;
            }
        }

        let old_lines_changed = old_end.saturating_sub(first_diff);
        let new_lines_changed = new_end.saturating_sub(first_diff);

        // Generate hunk lines
        let mut hunk_lines = Vec::new();

        // Context before
        let context_start = first_diff.saturating_sub(3);
        for i in context_start..first_diff {
            hunk_lines.push(format!(" {}", old_lines[i]));
        }

        // Removed lines
        for i in first_diff..old_end {
            hunk_lines.push(format!("-{}", old_lines[i]));
        }

        // Added lines
        for i in first_diff..new_end {
            hunk_lines.push(format!("+{}", new_lines[i]));
        }

        // Context after
        let context_end = (old_end + 3).min(old_lines.len());
        for i in old_end..context_end {
            hunk_lines.push(format!(" {}", old_lines[i]));
        }

        vec![PatchHunk {
            old_start: first_diff + 1,
            old_lines: old_lines_changed,
            new_start: first_diff + 1,
            new_lines: new_lines_changed,
            lines: hunk_lines,
        }]
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "FileWriteTool",
                "Create or overwrite files with parent directory creation",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let file_path = input
            .require("file_path")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "file_path must be a string".to_string(),
                error_code: Some(1),
            })?;

        if file_path.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "file_path cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Validate content
        let content = input
            .require("content")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "content must be a string".to_string(),
                error_code: Some(3),
            })?;

        if content.len() > MAX_WRITE_SIZE_BYTES {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Content size ({}) exceeds maximum allowed ({} bytes)",
                    content.len(),
                    MAX_WRITE_SIZE_BYTES
                ),
                error_code: Some(4),
            });
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let file_path = match input.get("file_path") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "file_path must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: file_path")
                    .with_field("success", false);
            }
        };

        let content = match input.get("content") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "content must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: content")
                    .with_field("success", false);
            }
        };

        let path = PathBuf::from(&file_path);

        // Check if file already exists
        let file_exists = fs::metadata(&path).await.is_ok();

        // Store original content for diff if updating
        let original_content = if file_exists {
            match fs::read_to_string(&path).await {
                Ok(c) => Some(c),
                Err(_) => None,
            }
        } else {
            None
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return ToolOutput::new()
                    .with_field("error", format!("Failed to create parent directory: {e}"))
                    .with_field("success", false);
            }
        }

        // Write the file
        if let Err(e) = fs::write(&path, &content).await {
            return ToolOutput::new()
                .with_field("error", format!("Failed to write file: {e}"))
                .with_field("success", false);
        }

        // Generate diff for updates
        let structured_patch = original_content.as_ref().map(|orig| {
            Self::generate_diff(orig, &content, &file_path)
        });

        let operation = if file_exists { "update" } else { "create" };

        ToolOutput::new()
            .with_field("operation", operation)
            .with_field("file_path", &file_path)
            .with_field("content", &content)
            .with_field("success", true)
            .with_field("original_content", original_content)
            .with_field("structured_patch", structured_patch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_write_create_new() {
        let tool = FileWriteTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt").to_string_lossy().to_string();

        let input = ToolInput::new()
            .with_arg("file_path", &file_path)
            .with_arg("content", "Hello, World!");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("operation").and_then(|v| v.as_str()),
            Some("create")
        );
        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Verify file was created
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[tokio::test]
    async fn test_file_write_update() {
        let tool = FileWriteTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        // Create initial file
        fs::write(&file_path, "Original content").await.unwrap();

        let input = ToolInput::new()
            .with_arg("file_path", file_path.to_string_lossy().to_string())
            .with_arg("content", "Updated content");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("operation").and_then(|v| v.as_str()),
            Some("update")
        );

        // Verify file was updated
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Updated content");
    }

    #[tokio::test]
    async fn test_file_write_creates_parent_dirs() {
        let tool = FileWriteTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir
            .path()
            .join("a/b/c/nested.txt")
            .to_string_lossy()
            .to_string();

        let input = ToolInput::new()
            .with_arg("file_path", &file_path)
            .with_arg("content", "Nested content");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Verify file was created in nested directory
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Nested content");
    }

    #[tokio::test]
    async fn test_file_write_validation_missing_path() {
        let tool = FileWriteTool::new();
        let input = ToolInput::new().with_arg("content", "test");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_generate_diff() {
        let old = "Line 1\nLine 2\nLine 3\nLine 4";
        let new = "Line 1\nModified Line 2\nLine 3\nLine 4";

        let diff = FileWriteTool::generate_diff(old, new, "test.txt");

        assert_eq!(diff.len(), 1);
        let hunk = &diff[0];
        assert!(hunk.lines.iter().any(|l| l.starts_with('-')));
        assert!(hunk.lines.iter().any(|l| l.starts_with('+')));
    }
}
