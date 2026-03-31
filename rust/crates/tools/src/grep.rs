//! GrepTool - Search file contents with regex patterns.
//!
//! This tool provides content searching capabilities:
//! - Regex pattern matching across files
//! - Support for ripgrep-style options (-i, -n, -C, etc.)
//! - Multiple output modes: content, files_with_matches, count
//! - Type filtering and glob filtering

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use regex::RegexBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tokio::fs;

/// Default limit on results.
const DEFAULT_HEAD_LIMIT: usize = 250;

/// Maximum file size to search (10 MB).
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// VCS directories to exclude.
const VCS_DIRECTORIES: &[&str] = &[".git", ".svn", ".hg", ".bzr", ".jj", ".sl"];

/// Input schema for the GrepTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GrepInput {
    /// The regex pattern to search for.
    pub pattern: String,
    /// File or directory to search in.
    #[serde(default)]
    pub path: Option<String>,
    /// Glob pattern to filter files (e.g., "*.rs").
    #[serde(default)]
    pub glob: Option<String>,
    /// Output mode: content, files_with_matches, count.
    #[serde(default)]
    pub output_mode: Option<String>,
    /// Lines before each match.
    #[serde(default, alias = "-B")]
    pub before_context: Option<usize>,
    /// Lines after each match.
    #[serde(default, alias = "-A")]
    pub after_context: Option<usize>,
    /// Lines before and after (alias for before_context and after_context).
    #[serde(default, alias = "-C")]
    pub context: Option<usize>,
    /// Show line numbers.
    #[serde(default = "default_true", alias = "-n")]
    pub show_line_numbers: bool,
    /// Case insensitive search.
    #[serde(default, alias = "-i")]
    pub case_insensitive: bool,
    /// File type filter (e.g., "rust", "python").
    #[serde(default)]
    pub file_type: Option<String>,
    /// Limit output to first N lines/entries.
    #[serde(default)]
    pub head_limit: Option<usize>,
    /// Skip first N lines/entries.
    #[serde(default)]
    pub offset: Option<usize>,
    /// Enable multiline mode.
    #[serde(default)]
    pub multiline: Option<bool>,
}

fn default_true() -> bool {
    true
}

/// A match found in a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Match {
    pub file_path: String,
    pub line_number: usize,
    pub content: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Output schema for the GrepTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrepOutput {
    /// Output mode used.
    pub mode: String,
    /// Number of files with matches.
    pub num_files: usize,
    /// Filenames that matched.
    pub filenames: Vec<String>,
    /// Content output (for content mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Number of lines (for content mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_lines: Option<usize>,
    /// Number of matches (for count mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_matches: Option<usize>,
    /// The limit that was applied (if truncation occurred).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_limit: Option<usize>,
    /// The offset that was applied.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applied_offset: Option<usize>,
}

/// The GrepTool searches file contents with regex patterns.
#[derive(Debug, Clone, Default)]
pub struct GrepTool;

impl GrepTool {
    /// Create a new GrepTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Get the effective context values.
    fn get_context(
        before: Option<usize>,
        after: Option<usize>,
        context: Option<usize>,
    ) -> (usize, usize) {
        if let Some(ctx) = context {
            (ctx, ctx)
        } else {
            (before.unwrap_or(0), after.unwrap_or(0))
        }
    }

    /// Check if a file should be excluded based on VCS patterns.
    fn is_vcs_path(path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();
        for vcs in VCS_DIRECTORIES {
            if path_str.contains(&format!("/{vcs}/")) || path_str.ends_with(&format!("/{vcs}")) {
                return true;
            }
        }
        false
    }

    /// Collect files to search in a directory.
    async fn collect_files(
        path: &PathBuf,
        glob_pattern: Option<&str>,
        file_type: Option<&str>,
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();

        if path.is_file() {
            files.push(path.clone());
            return files;
        }

        let pattern = if let Some(glob) = glob_pattern {
            if glob.starts_with("*.") {
                // Extension filter
                glob.trim_start_matches("*").to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let type_filter = file_type.map(|t| t.to_lowercase());

        let mut entries = match fs::read_dir(path).await {
            Ok(entries) => entries,
            Err(_) => return files,
        };

        while let Ok(Some(entry)) = entries.next_entry().await {
            let entry_path = entry.path();

            if Self::is_vcs_path(&entry_path) {
                continue;
            }

            if entry_path.is_dir() {
                // Recursively collect from subdirectories
                let sub_files = Box::pin(Self::collect_files(&entry_path, glob_pattern, file_type)).await;
                files.extend(sub_files);
            } else if entry_path.is_file() {
                // Check file extension against pattern
                let should_include = if pattern.is_empty() && type_filter.is_none() {
                    true
                } else if !pattern.is_empty() {
                    entry_path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| format!(".{e}") == pattern)
                        .unwrap_or(false)
                } else if let Some(ref ft) = type_filter {
                    Self::matches_file_type(&entry_path, ft)
                } else {
                    false
                };

                if should_include {
                    files.push(entry_path);
                }
            }
        }

        files
    }

    /// Check if a file matches a type filter.
    fn matches_file_type(path: &std::path::Path, file_type: &str) -> bool {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match file_type {
            "rust" => ext == "rs",
            "python" => ext == "py" || ext == "pyw",
            "javascript" => ext == "js",
            "typescript" => ext == "ts" || ext == "tsx",
            "go" => ext == "go",
            "java" => ext == "java",
            "c" => ext == "c" || ext == "h",
            "cpp" => ext == "cpp" || ext == "cc" || ext == "cxx" || ext == "hpp",
            _ => false,
        }
    }

    /// Search a single file for pattern matches.
    async fn search_file(
        path: &PathBuf,
        regex: &regex::Regex,
        before_context: usize,
        after_context: usize,
    ) -> Vec<Match> {
        let mut matches = Vec::new();

        // Check file size
        let metadata = match fs::metadata(path).await {
            Ok(m) => m,
            Err(_) => return matches,
        };

        if metadata.len() > MAX_FILE_SIZE {
            return matches;
        }

        let content = match fs::read_to_string(path).await {
            Ok(c) => c,
            Err(_) => return matches, // Binary or unreadable file
        };

        let lines: Vec<&str> = content.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let line_number = line_idx + 1;

                // Get context before
                let context_start = line_idx.saturating_sub(before_context);
                let context_before: Vec<String> = lines[context_start..line_idx]
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();

                // Get context after
                let context_end = (line_idx + after_context + 1).min(lines.len());
                let context_after: Vec<String> = lines[line_idx + 1..context_end]
                    .iter()
                    .map(|&s| s.to_string())
                    .collect();

                matches.push(Match {
                    file_path: path.to_string_lossy().to_string(),
                    line_number,
                    content: line.to_string(),
                    context_before,
                    context_after,
                });
            }
        }

        matches
    }

    /// Format matches as content output.
    fn format_content_output(
        matches: &[Match],
        show_line_numbers: bool,
        limit: usize,
        offset: usize,
    ) -> (String, usize, Option<usize>) {
        let mut lines = Vec::new();

        for m in matches.iter().skip(offset).take(limit) {
            // Context before
            for (i, ctx) in m.context_before.iter().enumerate() {
                let ctx_line = m.line_number - m.context_before.len() + i;
                if show_line_numbers {
                    lines.push(format!("{}-{}-{}", m.file_path, ctx_line, ctx));
                } else {
                    lines.push(ctx.clone());
                }
            }

            // Match line
            if show_line_numbers {
                lines.push(format!("{}:{}:{}", m.file_path, m.line_number, m.content));
            } else {
                lines.push(m.content.clone());
            }

            // Context after
            for (i, ctx) in m.context_after.iter().enumerate() {
                let ctx_line = m.line_number + i + 1;
                if show_line_numbers {
                    lines.push(format!("{}-{}-{}", m.file_path, ctx_line, ctx));
                } else {
                    lines.push(ctx.clone());
                }
            }
        }

        let num_lines = lines.len();
        let truncated = if matches.len() > offset + limit {
            Some(limit)
        } else {
            None
        };

        (lines.join("\n"), num_lines, truncated)
    }

    /// Format matches as files_with_matches output.
    fn format_files_output(matches: &[Match]) -> Vec<String> {
        let mut files: Vec<String> = matches
            .iter()
            .map(|m| m.file_path.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        files.sort();
        files
    }

    /// Format matches as count output.
    fn format_count_output(matches: &[Match]) -> (HashMap<String, usize>, usize) {
        let mut file_counts: HashMap<String, usize> = HashMap::new();
        for m in matches {
            *file_counts.entry(m.file_path.clone()).or_insert(0) += 1;
        }
        let total = matches.len();
        (file_counts, total)
    }

    /// Apply head limit with offset.
    fn apply_head_limit<T>(
        items: &[T],
        limit: Option<usize>,
        offset: usize,
    ) -> (Vec<T>, Option<usize>)
    where
        T: Clone,
    {
        // Explicit 0 = unlimited
        if limit == Some(0) {
            return (items[offset..].to_vec(), None);
        }

        let effective_limit = limit.unwrap_or(DEFAULT_HEAD_LIMIT);
        let sliced: Vec<T> = items.iter().skip(offset).take(effective_limit).cloned().collect();
        let was_truncated = items.len() - offset > effective_limit;

        if was_truncated {
            (sliced, Some(effective_limit))
        } else {
            (sliced, None)
        }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new("GrepTool", "Search file contents with regex patterns").read_only()
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

        // Try to compile the regex to validate it
        let case_insensitive = input
            .get("case_insensitive")
            .or_else(|| input.get("-i"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let multiline = input
            .get("multiline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut builder = RegexBuilder::new(pattern);
        builder.case_insensitive(case_insensitive);
        builder.multi_line(multiline);

        if builder.build().is_err() {
            return Err(ToolError::ValidationFailed {
                message: format!("Invalid regex pattern: {pattern}"),
                error_code: Some(3),
            });
        }

        // Validate path if provided
        if let Some(path_val) = input.get("path") {
            if let Some(path) = path_val.as_str() {
                if !path.is_empty() {
                    let path_buf = PathBuf::from(path);
                    match fs::metadata(&path_buf).await {
                        Ok(_) => {} // Path exists (can be file or directory)
                        Err(_) => {
                            return Err(ToolError::NotFound {
                                message: format!("Path does not exist: {path}"),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let _start = Instant::now();

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
            .map(PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

        let glob_pattern = input.get("glob").and_then(|v| v.as_str());
        let file_type = input.get("type").or_else(|| input.get("file_type")).and_then(|v| v.as_str());
        let output_mode = input
            .get("output_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("files_with_matches");
        let before_context = input
            .get("before_context")
            .or_else(|| input.get("-B"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let after_context = input
            .get("after_context")
            .or_else(|| input.get("-A"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let context = input
            .get("context")
            .or_else(|| input.get("-C"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let show_line_numbers = input
            .get("show_line_numbers")
            .or_else(|| input.get("-n"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let case_insensitive = input
            .get("case_insensitive")
            .or_else(|| input.get("-i"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let head_limit = input
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let offset = input
            .get("offset")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(0);
        let multiline = input
            .get("multiline")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Get context values
        let (before_ctx, after_ctx) = Self::get_context(before_context, after_context, context);

        // Build the regex
        let mut builder = RegexBuilder::new(&pattern);
        builder.case_insensitive(case_insensitive);
        builder.multi_line(multiline);

        let regex = match builder.build() {
            Ok(r) => r,
            Err(e) => {
                return ToolOutput::new()
                    .with_field("error", format!("Invalid regex: {e}"))
                    .with_field("success", false);
            }
        };

        // Collect files to search
        let files = Self::collect_files(&path, glob_pattern, file_type).await;

        // Search each file
        let mut all_matches = Vec::new();
        for file in files {
            let file_matches = Self::search_file(&file, &regex, before_ctx, after_ctx).await;
            all_matches.extend(file_matches);
        }

        // Format output based on mode
        match output_mode {
            "content" => {
                let effective_limit = head_limit.unwrap_or(DEFAULT_HEAD_LIMIT);
                let (content, num_lines, applied_limit) =
                    Self::format_content_output(&all_matches, show_line_numbers, effective_limit, offset);

                let mut files_set: std::collections::HashSet<String> = std::collections::HashSet::new();
                for m in all_matches.iter().skip(offset).take(effective_limit) {
                    files_set.insert(m.file_path.clone());
                }

                ToolOutput::new()
                    .with_field("mode", "content")
                    .with_field("num_files", files_set.len())
                    .with_field("filenames", files_set.into_iter().collect::<Vec<_>>())
                    .with_field("content", content)
                    .with_field("num_lines", num_lines)
                    .with_field("applied_limit", applied_limit)
                    .with_field("applied_offset", if offset > 0 { Some(offset) } else { None })
            }
            "count" => {
                let (file_counts, total) = Self::format_count_output(&all_matches);

                // Apply head_limit to file counts
                let count_entries: Vec<String> = file_counts
                    .iter()
                    .map(|(f, c)| format!("{f}:{c}"))
                    .collect();

                let effective_limit = head_limit.unwrap_or(DEFAULT_HEAD_LIMIT);
                let (limited_entries, applied_limit) =
                    Self::apply_head_limit(&count_entries, Some(effective_limit), offset);

                // Parse back the limited entries
                let mut limited_counts: HashMap<String, usize> = HashMap::new();
                let mut file_count = 0;
                for entry in &limited_entries {
                    if let Some(colon_idx) = entry.rfind(':') {
                        let file = &entry[..colon_idx];
                        if let Ok(count) = entry[colon_idx + 1..].parse::<usize>() {
                            limited_counts.insert(file.to_string(), count);
                            file_count += 1;
                        }
                    }
                }

                ToolOutput::new()
                    .with_field("mode", "count")
                    .with_field("num_files", file_count)
                    .with_field("content", limited_entries.join("\n"))
                    .with_field("num_matches", total)
                    .with_field("applied_limit", applied_limit)
                    .with_field("applied_offset", if offset > 0 { Some(offset) } else { None })
            }
            _ => {
                // files_with_matches mode (default)
                let filenames = Self::format_files_output(&all_matches);

                let effective_limit = head_limit.unwrap_or(DEFAULT_HEAD_LIMIT);
                let (limited_files, applied_limit) =
                    Self::apply_head_limit(&filenames, Some(effective_limit), offset);

                ToolOutput::new()
                    .with_field("mode", "files_with_matches")
                    .with_field("num_files", limited_files.len())
                    .with_field("filenames", limited_files)
                    .with_field("applied_limit", applied_limit)
                    .with_field("applied_offset", if offset > 0 { Some(offset) } else { None })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_grep_find_pattern() {
        let tool = GrepTool::new();
        let dir = tempdir().unwrap();

        // Create test files
        fs::write(dir.path().join("test.txt"), "hello world\nfoo bar\nhello again").await.unwrap();
        fs::write(dir.path().join("other.txt"), "goodbye world\nfoo baz").await.unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "hello")
            .with_arg("path", dir.path().to_string_lossy().to_string());

        let output = tool.execute(input).await;

        let filenames: Vec<String> = output
            .data
            .get("filenames")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        assert_eq!(filenames.len(), 1);
        assert!(filenames[0].contains("test.txt"));
    }

    #[tokio::test]
    async fn test_grep_content_mode() {
        let tool = GrepTool::new();
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("test.txt"), "hello world\nfoo bar\nhello again").await.unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "hello")
            .with_arg("path", dir.path().to_string_lossy().to_string())
            .with_arg("output_mode", "content");

        let output = tool.execute(input).await;

        let content = output
            .data
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        assert!(content.contains("hello world") || content.contains("hello again"));
    }

    #[tokio::test]
    async fn test_grep_count_mode() {
        let tool = GrepTool::new();
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("test.txt"), "hello world\nhello bar\nhello again").await.unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "hello")
            .with_arg("path", dir.path().to_string_lossy().to_string())
            .with_arg("output_mode", "count");

        let output = tool.execute(input).await;

        let num_matches = output.data.get("num_matches").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(num_matches, 3);
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        let tool = GrepTool::new();
        let dir = tempdir().unwrap();

        fs::write(dir.path().join("test.txt"), "Hello World\nHELLO again").await.unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "hello")
            .with_arg("path", dir.path().to_string_lossy().to_string())
            .with_arg("case_insensitive", true)
            .with_arg("output_mode", "count");

        let output = tool.execute(input).await;

        let num_matches = output.data.get("num_matches").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(num_matches, 2);
    }

    #[tokio::test]
    async fn test_grep_with_context() {
        let tool = GrepTool::new();
        let dir = tempdir().unwrap();

        fs::write(
            dir.path().join("test.txt"),
            "line 1\nline 2\nline 3\nhello world\nline 5\nline 6",
        )
        .await
        .unwrap();

        let input = ToolInput::new()
            .with_arg("pattern", "hello")
            .with_arg("path", dir.path().to_string_lossy().to_string())
            .with_arg("context", 2u64)
            .with_arg("output_mode", "content");

        let output = tool.execute(input).await;

        let content = output
            .data
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Should include context lines
        assert!(content.contains("line 2") || content.contains("line 3"));
        assert!(content.contains("hello world"));
        assert!(content.contains("line 5") || content.contains("line 6"));
    }

    #[tokio::test]
    async fn test_grep_validation_invalid_regex() {
        let tool = GrepTool::new();

        let input = ToolInput::new()
            .with_arg("pattern", "[invalid")
            .with_arg("path", ".");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_get_context() {
        assert_eq!(GrepTool::get_context(Some(2), Some(3), None), (2, 3));
        assert_eq!(GrepTool::get_context(None, None, Some(5)), (5, 5));
        assert_eq!(GrepTool::get_context(Some(1), None, Some(5)), (5, 5)); // context takes precedence
    }
}
