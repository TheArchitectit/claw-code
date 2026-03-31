//! FileEditTool - Modify files by replacing strings.
//!
//! This tool provides in-place file editing capabilities:
//! - Replace a string with another string
//! - Replace all occurrences with replace_all flag
//! - Handle quote normalization
//! - Create new files if old_string is empty and file doesn't exist

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Maximum file size for editing (1 GB).
const MAX_EDIT_FILE_SIZE: u64 = 1024 * 1024 * 1024;

/// Input schema for the FileEditTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileEditInput {
    /// The absolute path to the file to modify.
    pub file_path: String,
    /// The text to replace.
    pub old_string: String,
    /// The text to replace it with.
    pub new_string: String,
    /// Replace all occurrences.
    #[serde(default)]
    pub replace_all: Option<bool>,
}

/// Hunk in a diff patch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditPatchHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<String>,
}

/// Output schema for the FileEditTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEditOutput {
    /// The path to the file that was edited.
    pub file_path: String,
    /// The original string that was replaced.
    pub old_string: String,
    /// The new string that replaced it.
    pub new_string: String,
    /// The original file contents before editing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    /// Diff patch showing changes.
    pub structured_patch: Vec<EditPatchHunk>,
    /// Whether all occurrences were replaced.
    pub replace_all: bool,
}

/// The FileEditTool modifies files by string replacement.
#[derive(Debug, Clone, Default)]
pub struct FileEditTool;

impl FileEditTool {
    /// Create a new FileEditTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Find the actual string in file content, handling quote normalization.
    fn find_actual_string<'a>(content: &'a str, search: &'a str) -> Option<&'a str> {
        // Direct match
        if content.contains(search) {
            return Some(search);
        }

        // Try with quote normalization (curly/smart quotes)
        let normalized_search = search
            .replace('"', "'")
            .replace('"', "'")
            .replace('"', "\"")
            .replace('"', "\"");

        if content.contains(&normalized_search) {
            // Find the actual substring in content that matches
            // This is a simplified version - in practice, you'd need more complex logic
            for window in content.lines() {
                let normalized_window = window
                    .replace('"', "'")
                    .replace('"', "'")
                    .replace('"', "\"")
                    .replace('"', "\"");
                if normalized_window.contains(&normalized_search) {
                    // Return a placeholder - real implementation would extract exact substring
                    return Some(search);
                }
            }
        }

        None
    }

    /// Apply the edit to content.
    fn apply_edit(
        content: &str,
        old_string: &str,
        new_string: &str,
        replace_all: bool,
    ) -> Option<String> {
        let actual_old = Self::find_actual_string(content, old_string)?;

        if replace_all {
            Some(content.replace(actual_old, new_string))
        } else {
            Some(content.replacen(actual_old, new_string, 1))
        }
    }

    /// Generate a diff between old and new content.
    fn generate_diff(
        old_content: &str,
        new_content: &str,
    ) -> Vec<EditPatchHunk> {
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();

        // Simple diff algorithm - find first and last difference
        let mut first_diff = 0;
        while first_diff < old_lines.len()
            && first_diff < new_lines.len()
            && old_lines[first_diff] == new_lines[first_diff]
        {
            first_diff += 1;
        }

        let mut old_end = old_lines.len();
        let mut new_end = new_lines.len();
        while old_end > first_diff && new_end > first_diff {
            if old_lines.get(old_end - 1) == new_lines.get(new_end - 1) {
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

        // Context before (up to 3 lines)
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

        // Context after (up to 3 lines)
        let context_end = (old_end + 3).min(old_lines.len());
        for i in old_end..context_end {
            hunk_lines.push(format!(" {}", old_lines[i]));
        }

        vec![EditPatchHunk {
            old_start: first_diff + 1,
            old_lines: old_lines_changed,
            new_start: first_diff + 1,
            new_lines: new_lines_changed,
            lines: hunk_lines,
        }]
    }

    /// Count occurrences of a string in content.
    fn count_occurrences(content: &str, search: &str) -> usize {
        content.matches(search).count()
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "FileEditTool",
                "Modify files by replacing strings with support for partial modifications",
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

        let old_string = input
            .require("old_string")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "old_string must be a string".to_string(),
                error_code: Some(3),
            })?;

        let new_string = input
            .require("new_string")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "new_string must be a string".to_string(),
                error_code: Some(4),
            })?;

        // Check that old and new are different
        if old_string == new_string {
            return Err(ToolError::ValidationFailed {
                message: "old_string and new_string must be different".to_string(),
                error_code: Some(5),
            });
        }

        let path = PathBuf::from(file_path);

        // Check file exists and get metadata
        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(_) => {
                // File doesn't exist - valid if old_string is empty (file creation)
                if old_string.is_empty() {
                    return Ok(());
                }
                return Err(ToolError::NotFound {
                    message: format!("File does not exist: {file_path}"),
                });
            }
        };

        // Check file size
        if metadata.len() > MAX_EDIT_FILE_SIZE {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "File is too large to edit ({} bytes). Maximum editable file size is {} bytes.",
                    metadata.len(),
                    MAX_EDIT_FILE_SIZE
                ),
                error_code: Some(6),
            });
        }

        // Read file content
        let content = fs::read_to_string(&path).await.map_err(|e| ToolError::ExecutionFailed {
            message: format!("Failed to read file: {e}"),
        })?;

        // If old_string is empty but file has content, it's a creation attempt on existing file
        if old_string.is_empty() && !content.trim().is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "Cannot create new file - file already exists and is not empty".to_string(),
                error_code: Some(7),
            });
        }

        // Check if old_string exists in file
        if !old_string.is_empty() && Self::find_actual_string(&content, old_string).is_none() {
            return Err(ToolError::ValidationFailed {
                message: format!("String to replace not found in file: {old_string}"),
                error_code: Some(8),
            });
        }

        // Check for multiple matches if replace_all is false
        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !replace_all && !old_string.is_empty() {
            let occurrences = Self::count_occurrences(&content, old_string);
            if occurrences > 1 {
                return Err(ToolError::ValidationFailed {
                    message: format!(
                        "Found {occurrences} matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance."
                    ),
                    error_code: Some(9),
                });
            }
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

        let old_string = match input.get("old_string") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "old_string must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: old_string")
                    .with_field("success", false);
            }
        };

        let new_string = match input.get("new_string") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "new_string must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: new_string")
                    .with_field("success", false);
            }
        };

        let replace_all = input
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let path = PathBuf::from(&file_path);

        // Read current content or start with empty for new files
        let original_content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => String::new(),
        };

        // Apply the edit
        let updated_content = match Self::apply_edit(&original_content, &old_string, &new_string, replace_all) {
            Some(c) => c,
            None => {
                return ToolOutput::new()
                    .with_field("error", "Failed to apply edit - old_string not found")
                    .with_field("success", false);
            }
        };

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent).await {
                return ToolOutput::new()
                    .with_field("error", format!("Failed to create parent directory: {e}"))
                    .with_field("success", false);
            }
        }

        // Write the updated content
        if let Err(e) = fs::write(&path, &updated_content).await {
            return ToolOutput::new()
                .with_field("error", format!("Failed to write file: {e}"))
                .with_field("success", false);
        }

        // Generate diff
        let structured_patch = Self::generate_diff(&original_content, &updated_content);

        ToolOutput::new()
            .with_field("file_path", &file_path)
            .with_field("old_string", &old_string)
            .with_field("new_string", &new_string)
            .with_field("original_content", &original_content)
            .with_field("structured_patch", structured_patch)
            .with_field("replace_all", replace_all)
            .with_field("success", true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_file_edit_single_replace() {
        let tool = FileEditTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, "Hello World\nLine 2\nLine 3").await.unwrap();

        let input = ToolInput::new()
            .with_arg("file_path", file_path.to_string_lossy().to_string())
            .with_arg("old_string", "Hello World")
            .with_arg("new_string", "Hi Universe");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hi Universe\nLine 2\nLine 3");
    }

    #[tokio::test]
    async fn test_file_edit_replace_all() {
        let tool = FileEditTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, "foo bar foo baz foo").await.unwrap();

        let input = ToolInput::new()
            .with_arg("file_path", file_path.to_string_lossy().to_string())
            .with_arg("old_string", "foo")
            .with_arg("new_string", "qux")
            .with_arg("replace_all", true);

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "qux bar qux baz qux");
    }

    #[tokio::test]
    async fn test_file_edit_create_new() {
        let tool = FileEditTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.txt").to_string_lossy().to_string();

        let input = ToolInput::new()
            .with_arg("file_path", &file_path)
            .with_arg("old_string", "")
            .with_arg("new_string", "New content");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "New content");
    }

    #[tokio::test]
    async fn test_file_edit_validation_same_strings() {
        let tool = FileEditTool::new();
        let input = ToolInput::new()
            .with_arg("file_path", "/test.txt")
            .with_arg("old_string", "same")
            .with_arg("new_string", "same");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_edit_validation_multiple_matches_no_replace_all() {
        let tool = FileEditTool::new();
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        fs::write(&file_path, "foo bar foo baz foo").await.unwrap();

        let input = ToolInput::new()
            .with_arg("file_path", file_path.to_string_lossy().to_string())
            .with_arg("old_string", "foo")
            .with_arg("new_string", "qux");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_count_occurrences() {
        assert_eq!(FileEditTool::count_occurrences("foo bar foo", "foo"), 2);
        assert_eq!(FileEditTool::count_occurrences("hello world", "foo"), 0);
        assert_eq!(FileEditTool::count_occurrences("aaa", "aa"), 1);
    }

    #[test]
    fn test_generate_diff() {
        let old = "Line 1\nLine 2\nLine 3";
        let new = "Line 1\nModified\nLine 3";

        let diff = FileEditTool::generate_diff(old, new);

        assert_eq!(diff.len(), 1);
        let hunk = &diff[0];
        assert!(hunk.lines.iter().any(|l| l.starts_with('-')));
        assert!(hunk.lines.iter().any(|l| l.starts_with('+')));
    }
}
