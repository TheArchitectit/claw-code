//! FileReadTool - Read files with support for text, images, and line limits.
//!
//! This tool provides comprehensive file reading capabilities including:
//! - Text file reading with optional offset and limit
//! - Basic image handling (returns file info)
//! - Line number formatting
//! - Device file blocking for safety

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Maximum file size for text files (10 MB).
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Device files that should be blocked (produce infinite output or block).
const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
    "/dev/fd/0",
    "/dev/fd/1",
    "/dev/fd/2",
];

/// Common image extensions.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "bmp"];

/// Binary file extensions that should be blocked.
const BINARY_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "bin", "o", "a", "lib", "class", "jar",
];

/// Input schema for the FileReadTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileReadInput {
    /// The absolute path to the file to read.
    pub file_path: String,

    /// The line number to start reading from (1-indexed).
    #[serde(default)]
    pub offset: Option<usize>,

    /// The number of lines to read.
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Output schema for text files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextFileOutput {
    pub file_path: String,
    pub content: String,
    pub num_lines: usize,
    pub start_line: usize,
    pub total_lines: usize,
}

/// Output schema for image files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageFileOutput {
    pub file_path: String,
    pub file_type: String,
    pub size_bytes: u64,
}

/// The FileReadTool reads files from the filesystem.
#[derive(Debug, Clone, Default)]
pub struct FileReadTool;

impl FileReadTool {
    /// Create a new FileReadTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Check if a path is a blocked device file.
    fn is_blocked_device_path(path: &str) -> bool {
        let path_normalized = path.replace("//", "/");

        // Check exact matches
        for blocked in BLOCKED_DEVICE_PATHS {
            if path_normalized == *blocked {
                return true;
            }
        }

        // Check /proc/*/fd/ patterns
        if path_normalized.starts_with("/proc/") {
            if path_normalized.ends_with("/fd/0")
                || path_normalized.ends_with("/fd/1")
                || path_normalized.ends_with("/fd/2")
            {
                return true;
            }
        }

        false
    }

    /// Check if a file is an image based on extension.
    fn is_image_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                let ext_lower = ext.to_lowercase();
                IMAGE_EXTENSIONS.contains(&ext_lower.as_str())
            })
            .unwrap_or(false)
    }

    /// Check if a file is binary based on extension.
    fn is_binary_file(path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                let ext_lower = ext.to_lowercase();
                BINARY_EXTENSIONS.contains(&ext_lower.as_str())
            })
            .unwrap_or(false)
    }

    /// Add line numbers to content.
    fn add_line_numbers(content: &str, start_line: usize) -> String {
        content
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{:6}  {}", start_line + i, line))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Read a range of lines from file content.
    fn read_lines_range(
        content: &str,
        offset: usize,
        limit: Option<usize>,
    ) -> (String, usize, usize) {
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // offset is 1-indexed, convert to 0-indexed
        let start = offset.saturating_sub(1);
        let end = limit.map_or(lines.len(), |l| (start + l).min(lines.len()));

        let selected_lines: Vec<&str> = lines[start..end].to_vec();
        let result = selected_lines.join("\n");
        let num_lines = selected_lines.len();

        (result, num_lines, total_lines)
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "FileReadTool",
                "Read text files, handle images, respect line limits",
            )
            .read_only()
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

        // Check for blocked device paths
        if Self::is_blocked_device_path(file_path) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Cannot read '{file_path}': this device file would block or produce infinite output"
                ),
                error_code: Some(9),
            });
        }

        // Validate offset and limit
        if let Some(offset_val) = input.get("offset") {
            if let Some(offset) = offset_val.as_u64() {
                if offset == 0 {
                    return Err(ToolError::ValidationFailed {
                        message: "offset must be at least 1 (1-indexed)".to_string(),
                        error_code: Some(3),
                    });
                }
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
                        .with_field("type", "error");
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: file_path")
                    .with_field("type", "error");
            }
        };

        let path = PathBuf::from(&file_path);

        // Check if file exists
        let metadata = match fs::metadata(&path).await {
            Ok(m) => m,
            Err(e) => {
                return ToolOutput::new()
                    .with_field("error", format!("File does not exist: {file_path}"))
                    .with_field("type", "not_found")
                    .with_field("details", e.to_string());
            }
        };

        // Check if it's a file
        if !metadata.is_file() {
            return ToolOutput::new()
                .with_field("error", format!("Path is not a file: {file_path}"))
                .with_field("type", "not_file");
        }

        let size = metadata.len();

        // Check file size limit for text files
        if size > MAX_FILE_SIZE_BYTES {
            return ToolOutput::new()
                .with_field(
                    "error",
                    format!(
                        "File size ({size} bytes) exceeds maximum allowed ({MAX_FILE_SIZE_BYTES} bytes). Use offset and limit parameters to read specific portions."
                    ),
                )
                .with_field("type", "too_large")
                .with_field("size", size)
                .with_field("max_size", MAX_FILE_SIZE_BYTES);
        }

        // Handle image files
        if Self::is_image_file(&path) {
            return ToolOutput::new()
                .with_field("type", "image")
                .with_field("file_path", &file_path)
                .with_field("file_type", path.extension().unwrap_or_default().to_string_lossy())
                .with_field("size_bytes", size);
        }

        // Check for binary files (non-image)
        if Self::is_binary_file(&path) {
            return ToolOutput::new()
                .with_field("error", format!("Cannot read binary file: {file_path}"))
                .with_field("type", "binary");
        }

        // Read the file content
        let content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                // Try to detect if it's a binary file by read error
                return ToolOutput::new()
                    .with_field("error", format!("Failed to read file: {e}"))
                    .with_field("type", "read_error");
            }
        };

        // Parse offset and limit
        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(1); // Default to start from line 1

        let limit = input
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // Extract the requested lines
        let (selected_content, num_lines, total_lines) =
            Self::read_lines_range(&content, offset, limit);

        // Add line numbers
        let formatted_content = Self::add_line_numbers(&selected_content, offset);

        // Check if we returned less than requested (file shorter than offset)
        let has_content = !selected_content.is_empty() || total_lines == 0;

        if !has_content && offset > total_lines {
            return ToolOutput::new()
                .with_field("error", format!(
                    "The file exists but is shorter than the provided offset ({offset}). The file has {total_lines} lines."
                ))
                .with_field("type", "offset_too_large")
                .with_field("file_path", &file_path)
                .with_field("total_lines", total_lines)
                .with_field("requested_offset", offset);
        }

        ToolOutput::new()
            .with_field("type", "text")
            .with_field("file_path", &file_path)
            .with_field("content", formatted_content)
            .with_field("num_lines", num_lines)
            .with_field("start_line", offset)
            .with_field("total_lines", total_lines)
            .with_field("truncated", limit.map_or(false, |l| total_lines > offset + l - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_file_read_text() {
        let tool = FileReadTool::new();

        // Create a temp file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Line 1").unwrap();
        writeln!(file, "Line 2").unwrap();
        writeln!(file, "Line 3").unwrap();

        let path = file.path().to_string_lossy().to_string();
        let input = ToolInput::new().with_arg("file_path", &path);

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("type").and_then(|v| v.as_str()), Some("text"));
        assert_eq!(output.data.get("total_lines").and_then(|v| v.as_u64()), Some(3));
    }

    #[tokio::test]
    async fn test_file_read_with_offset_and_limit() {
        let tool = FileReadTool::new();

        let mut file = NamedTempFile::new().unwrap();
        for i in 1..=10 {
            writeln!(file, "Line {i}").unwrap();
        }

        let path = file.path().to_string_lossy().to_string();
        let input = ToolInput::new()
            .with_arg("file_path", &path)
            .with_arg("offset", 3u64)
            .with_arg("limit", 2u64);

        let output = tool.execute(input).await;

        assert_eq!(output.data.get("start_line").and_then(|v| v.as_u64()), Some(3));
        assert_eq!(output.data.get("num_lines").and_then(|v| v.as_u64()), Some(2));

        let content = output
            .data
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(content.contains("Line 3"));
        assert!(content.contains("Line 4"));
        assert!(!content.contains("Line 5"));
    }

    #[tokio::test]
    async fn test_file_read_not_found() {
        let tool = FileReadTool::new();
        let input = ToolInput::new().with_arg("file_path", "/nonexistent/path/file.txt");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("type").and_then(|v| v.as_str()),
            Some("not_found")
        );
    }

    #[tokio::test]
    async fn test_file_read_blocked_device() {
        let tool = FileReadTool::new();
        let input = ToolInput::new().with_arg("file_path", "/dev/zero");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_file_read_image() {
        let tool = FileReadTool::new();

        // Create a temp file with .png extension
        let file = NamedTempFile::with_suffix(".png").unwrap();
        let path = file.path().to_string_lossy().to_string();

        let input = ToolInput::new().with_arg("file_path", &path);
        let output = tool.execute(input).await;

        assert_eq!(output.data.get("type").and_then(|v| v.as_str()), Some("image"));
    }
}
