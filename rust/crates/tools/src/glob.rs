//! GlobTool - Find files matching a pattern using glob syntax.
//!
//! This tool provides file pattern matching:
//! - Support for glob patterns like "*.rs", "**/*.md"
//! - Optional path parameter to search in specific directory
//! - Return file list with count and duration info

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Instant;

/// Maximum number of results to return.
const MAX_RESULTS: usize = 100;

/// Input schema for the GlobTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GlobInput {
    /// The glob pattern to match files against.
    pub pattern: String,
    /// The directory to search in (optional, defaults to current directory).
    #[serde(default)]
    pub path: Option<String>,
}

/// Output schema for the GlobTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobOutput {
    /// Time taken to execute in milliseconds.
    pub duration_ms: u64,
    /// Total number of files found.
    pub num_files: usize,
    /// Array of file paths that match the pattern.
    pub filenames: Vec<String>,
    /// Whether results were truncated.
    pub truncated: bool,
}

/// The GlobTool finds files matching a glob pattern.
#[derive(Debug, Clone, Default)]
pub struct GlobTool;

impl GlobTool {
    /// Create a new GlobTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Expand the path if provided, otherwise use current directory.
    fn get_search_path(input_path: Option<&str>) -> PathBuf {
        input_path.map_or_else(
            || std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            PathBuf::from,
        )
    }

    /// Perform the glob search.
    fn glob_search(pattern: &str, base_path: &PathBuf, limit: usize) -> (Vec<String>, bool) {
        let pattern_path = if pattern.starts_with('/') || pattern.starts_with("./") || pattern.starts_with("../") {
            PathBuf::from(pattern)
        } else {
            base_path.join(pattern)
        };

        let pattern_str = pattern_path.to_string_lossy();

        let mut matches = Vec::new();
        let mut truncated = false;

        match glob::glob(&pattern_str) {
            Ok(paths) => {
                for entry in paths.flatten() {
                    if matches.len() >= limit {
                        truncated = true;
                        break;
                    }

                    if let Ok(metadata) = std::fs::metadata(&entry) {
                        if metadata.is_file() {
                            // Convert to relative path if under current directory
                            let path_str = entry.to_string_lossy().to_string();
                            matches.push(path_str);
                        }
                    }
                }
            }
            Err(_) => {
                // Invalid pattern - return empty results
            }
        }

        // Sort by path for consistent output
        matches.sort();

        (matches, truncated)
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new("GlobTool", "Find files by name pattern or wildcard").read_only()
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let pattern = input
            .require("pattern")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "pattern must be a string".to_string(),
                error_code: Some(1),
            })?;

        if pattern.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "pattern cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Validate path if provided
        if let Some(path_val) = input.get("path") {
            if let Some(path) = path_val.as_str() {
                if !path.is_empty() {
                    let path_buf = PathBuf::from(path);
                    match tokio::fs::metadata(&path_buf).await {
                        Ok(metadata) => {
                            if !metadata.is_dir() {
                                return Err(ToolError::ValidationFailed {
                                    message: format!("Path is not a directory: {path}"),
                                    error_code: Some(3),
                                });
                            }
                        }
                        Err(_) => {
                            return Err(ToolError::NotFound {
                                message: format!("Directory does not exist: {path}"),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let start = Instant::now();

        let pattern = match input.get("pattern") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "pattern must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: pattern")
                    .with_field("success", false);
            }
        };

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .map(String::from);

        let base_path = Self::get_search_path(path.as_deref());

        // Perform the glob search
        let (filenames, truncated) = Self::glob_search(&pattern, &base_path, MAX_RESULTS);

        let duration_ms = start.elapsed().as_millis() as u64;
        let num_files = filenames.len();

        ToolOutput::new()
            .with_field("filenames", filenames)
            .with_field("num_files", num_files)
            .with_field("duration_ms", duration_ms)
            .with_field("truncated", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_glob_find_files() {
        let tool = GlobTool::new();
        let dir = tempdir().unwrap();

        // Create some test files
        let file1 = dir.path().join("test1.txt");
        let file2 = dir.path().join("test2.txt");
        let file3 = dir.path().join("other.rs");

        std::fs::write(&file1, "content1").unwrap();
        std::fs::write(&file2, "content2").unwrap();
        std::fs::write(&file3, "content3").unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "*.txt")
            .with_arg("path", dir.path().to_string_lossy().to_string());

        let output = tool.execute(input).await;

        let filenames: Vec<String> = output
            .data
            .get("filenames")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        assert_eq!(filenames.len(), 2);
        assert!(filenames.iter().any(|f| f.contains("test1.txt")));
        assert!(filenames.iter().any(|f| f.contains("test2.txt")));
    }

    #[tokio::test]
    async fn test_glob_no_matches() {
        let tool = GlobTool::new();
        let dir = tempdir().unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "*.nonexistent")
            .with_arg("path", dir.path().to_string_lossy().to_string());

        let output = tool.execute(input).await;

        let filenames: Vec<String> = output
            .data
            .get("filenames")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        assert!(filenames.is_empty());
    }

    #[tokio::test]
    async fn test_glob_validation_invalid_path() {
        let tool = GlobTool::new();

        let input = ToolInput::new()
            .with_arg("pattern", "*.txt")
            .with_arg("path", "/nonexistent/path/12345");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_glob_validation_empty_pattern() {
        let tool = GlobTool::new();

        let input = ToolInput::new().with_arg("pattern", "");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_glob_recursive_pattern() {
        let tool = GlobTool::new();
        let dir = tempdir().unwrap();

        // Create nested files
        let subdir = dir.path().join("subdir");
        std::fs::create_dir(&subdir).unwrap();

        std::fs::write(dir.path().join("root.txt"), "root").unwrap();
        std::fs::write(subdir.join("nested.txt"), "nested").unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "**/*.txt")
            .with_arg("path", dir.path().to_string_lossy().to_string());

        let output = tool.execute(input).await;

        let filenames: Vec<String> = output
            .data
            .get("filenames")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        assert_eq!(filenames.len(), 2);
        assert!(filenames.iter().any(|f| f.contains("root.txt")));
        assert!(filenames.iter().any(|f| f.contains("nested.txt")));
    }
}
