//! # Test Utilities for R.A.D Codicological
//!
//! This crate provides shared testing utilities for the R.A.D Codicological project,
//! including:
//!
//! - Temporary directory helpers with automatic cleanup
//! - Mock tool implementations for testing
//! - Test fixtures for common scenarios
//! - Assertion helpers for tool output validation

#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tempfile::{tempdir, TempDir};

pub use runtime::{
    Tool, ToolRegistry, ToolRegistryBuilder, ToolUseContext, SessionId, ToolUseId,
    ToolOutput as RuntimeToolOutput, ToolResult as RuntimeToolResult,
};
pub use tools::{
    Tool as SimpleTool, ToolError, ToolInput, ToolMetadata, ToolOutput,
    ToolResult,
};

/// A temporary directory fixture that automatically cleans up after tests.
///
/// # Example
///
/// ```rust
/// use test_utils::TempDirFixture;
///
/// # tokio_test::block_on(async {
/// let temp = TempDirFixture::new().await;
/// let file_path = temp.join("test.txt");
/// tokio::fs::write(&file_path, "hello").await.unwrap();
///
/// // File is automatically deleted when `temp` goes out of scope
/// # });
/// ```
pub struct TempDirFixture {
    temp_dir: TempDir,
}

impl TempDirFixture {
    /// Create a new temporary directory fixture.
    ///
    /// # Panics
    ///
    /// Panics if the temporary directory cannot be created.
    #[must_use]
    pub async fn new() -> Self {
        Self {
            temp_dir: tempdir().expect("Failed to create temp directory"),
        }
    }

    /// Get the path to the temporary directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Join a path component to the temporary directory.
    #[must_use]
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.temp_dir.path().join(path)
    }

    /// Create a file with the given content in the temporary directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created.
    pub async fn create_file<P: AsRef<Path>>(
        &self,
        name: P,
        content: impl AsRef<[u8]>,
    ) -> tokio::io::Result<PathBuf> {
        let path = self.join(&name);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, content).await?;
        Ok(path)
    }

    /// Create a nested directory structure.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub async fn create_dir<P: AsRef<Path>>(&self, name: P) -> tokio::io::Result<PathBuf> {
        let path = self.join(&name);
        tokio::fs::create_dir_all(&path).await?;
        Ok(path)
    }

    /// Create multiple files with the given contents.
    ///
    /// # Errors
    ///
    /// Returns an error if any file cannot be created.
    pub async fn create_files(
        &self,
        files: impl IntoIterator<Item = (impl AsRef<Path>, impl AsRef<[u8]>)>,
    ) -> tokio::io::Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        for (name, content) in files {
            let path = self.create_file(name, content).await?;
            paths.push(path);
        }
        Ok(paths)
    }
}

impl Default for TempDirFixture {
    fn default() -> Self {
        // This is a synchronous default implementation for convenience.
        // In async contexts, prefer `TempDirFixture::new().await`.
        Self {
            temp_dir: tempdir().expect("Failed to create temp directory"),
        }
    }
}

/// A mock tool implementation for testing.
///
/// This allows creating simple tools with predefined behavior for unit tests.
/// The mock tool stores its own metadata, avoiding lifetime issues.
pub struct MockTool {
    metadata: ToolMetadata,
    handler: Box<dyn Fn(&ToolInput) -> ToolOutput + Send + Sync>,
}

impl MockTool {
    /// Create a new mock tool with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: ToolMetadata::new(name, "A mock tool for testing"),
            handler: Box::new(|_| ToolOutput::new().with_field("result", "mock")),
        }
    }

    /// Set the description for this mock tool.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = desc.into();
        self
    }

    /// Set whether this tool is read-only.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.metadata.is_read_only = read_only;
        self
    }

    /// Set whether this tool is concurrency-safe.
    #[must_use]
    pub fn concurrency_safe(mut self, safe: bool) -> Self {
        self.metadata.is_concurrency_safe = safe;
        self
    }

    /// Set a custom handler for this mock tool.
    #[must_use]
    pub fn with_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ToolInput) -> ToolOutput + Send + Sync + 'static,
    {
        self.handler = Box::new(handler);
        self
    }
}

#[async_trait]
impl SimpleTool for MockTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn validate(&self, _input: &ToolInput) -> ToolResult<()> {
        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        (self.handler)(&input)
    }
}

/// A more advanced mock tool that tracks invocations.
pub struct TrackedMockTool {
    metadata: ToolMetadata,
    invocations: std::sync::Arc<tokio::sync::Mutex<Vec<ToolInput>>>,
    handler: Box<dyn Fn(&ToolInput) -> ToolOutput + Send + Sync>,
}

impl TrackedMockTool {
    /// Create a new tracked mock tool.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            metadata: ToolMetadata::new(name, "A tracked mock tool"),
            invocations: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
            handler: Box::new(|_| ToolOutput::new().with_field("result", "mock")),
        }
    }

    /// Set the description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.metadata.description = desc.into();
        self
    }

    /// Set a custom handler.
    #[must_use]
    pub fn with_handler<F>(mut self, handler: F) -> Self
    where
        F: Fn(&ToolInput) -> ToolOutput + Send + Sync + 'static,
    {
        self.handler = Box::new(handler);
        self
    }

    /// Get the number of times this tool was invoked.
    pub async fn invocation_count(&self) -> usize {
        self.invocations.lock().await.len()
    }

    /// Get a clone of all invocations.
    pub async fn get_invocations(&self) -> Vec<ToolInput> {
        self.invocations.lock().await.clone()
    }

    /// Clear all recorded invocations.
    pub async fn clear_invocations(&self) {
        self.invocations.lock().await.clear();
    }

    /// Assert that the tool was invoked exactly N times.
    ///
    /// # Panics
    ///
    /// Panics if the invocation count doesn't match.
    pub async fn assert_invoked_times(&self, expected: usize) {
        let actual = self.invocation_count().await;
        assert_eq!(
            actual, expected,
            "Expected tool '{}' to be invoked {} times, but was invoked {} times",
            self.metadata.name, expected, actual
        );
    }
}

#[async_trait]
impl SimpleTool for TrackedMockTool {
    fn metadata(&self) -> &ToolMetadata {
        &self.metadata
    }

    async fn validate(&self, _input: &ToolInput) -> ToolResult<()> {
        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        self.invocations.lock().await.push(input.clone());
        (self.handler)(&input)
    }
}

/// Helpers for creating common test fixtures.
pub mod fixtures {
    use super::*;

    /// Create a simple Rust project structure in the temp directory.
    ///
    /// Creates:
    /// - Cargo.toml with basic metadata
    /// - src/main.rs with a hello world program
    /// - src/lib.rs with a simple function
    pub async fn rust_project(temp: &TempDirFixture) -> tokio::io::Result<HashMap<String, PathBuf>> {
        let cargo_toml = temp
            .create_file(
                "Cargo.toml",
                r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
            )
            .await?;

        let main_rs = temp
            .create_file(
                "src/main.rs",
                r#"fn main() {
    println!("Hello, world!");
}
"#,
            )
            .await?;

        let lib_rs = temp
            .create_file(
                "src/lib.rs",
                r#"pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }
}
"#,
            )
            .await?;

        let mut map = HashMap::new();
        map.insert("Cargo.toml".to_string(), cargo_toml);
        map.insert("src/main.rs".to_string(), main_rs);
        map.insert("src/lib.rs".to_string(), lib_rs);

        Ok(map)
    }

    /// Create a nested directory structure with various file types.
    pub async fn mixed_file_tree(temp: &TempDirFixture) -> tokio::io::Result<Vec<PathBuf>> {
        let files = vec![
            ("README.md", "# Test Project\n\nThis is a test."),
            ("src/main.rs", "fn main() {}"),
            ("src/utils.rs", "pub fn helper() {}"),
            ("tests/integration.rs", "#[test] fn test() {}"),
            ("docs/guide.md", "# Guide\n\nInstructions here."),
            ("config.json", r#"{"key": "value"}"#),
            (".gitignore", "target/\n*.log"),
        ];

        let paths = temp.create_files(files).await?;
        Ok(paths)
    }

    /// Create a large text file with the specified number of lines.
    pub async fn large_text_file(
        temp: &TempDirFixture,
        name: impl AsRef<Path>,
        lines: usize,
    ) -> tokio::io::Result<PathBuf> {
        let content: String = (1..=lines)
            .map(|i| format!("Line {i}: This is test content for line number {i}\n"))
            .collect();

        temp.create_file(name, content).await
    }

    /// Create a binary file with sequential bytes.
    pub async fn binary_file(
        temp: &TempDirFixture,
        name: impl AsRef<Path>,
        size_bytes: usize,
    ) -> tokio::io::Result<PathBuf> {
        let content: Vec<u8> = (0..size_bytes)
            .map(|i| (i % 256) as u8)
            .collect();

        temp.create_file(name, content).await
    }
}

/// Assertion helpers for test validation.
pub mod assertions {
    use serde::Serialize;
    use serde_json::Value;

    use tools::ToolOutput;

    /// Assert that a tool output contains a field with the expected value.
    ///
    /// # Panics
    ///
    /// Panics if the field doesn't exist or has a different value.
    pub fn assert_output_field(output: &ToolOutput, field: &str, expected: impl Serialize) {
        let expected = serde_json::to_value(expected).unwrap();
        let actual = output
            .data
            .get(field)
            .unwrap_or_else(|| panic!("Expected output to have field '{}'", field));

        assert_eq!(
            actual, &expected,
            "Field '{}' does not match expected value",
            field
        );
    }

    /// Assert that a tool output contains a field.
    ///
    /// # Panics
    ///
    /// Panics if the field doesn't exist.
    pub fn assert_has_field(output: &ToolOutput, field: &str) {
        assert!(
            output.data.contains_key(field),
            "Expected output to have field '{}'",
            field
        );
    }

    /// Assert that a tool output indicates success.
    ///
    /// # Panics
    ///
    /// Panics if the output doesn't have a success field set to true.
    pub fn assert_success(output: &ToolOutput) {
        let success = output
            .data
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        assert!(success, "Expected output to indicate success");
    }

    /// Assert that a tool output indicates failure.
    ///
    /// # Panics
    ///
    /// Panics if the output has a success field set to true.
    pub fn assert_failure(output: &ToolOutput) {
        let success = output
            .data
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        assert!(!success, "Expected output to indicate failure");
    }

    /// Assert that a tool output contains an error message.
    ///
    /// # Panics
    ///
    /// Panics if there's no error field.
    pub fn assert_has_error(output: &ToolOutput) {
        assert!(
            output.data.contains_key("error") || 
            output.data.get("type").and_then(Value::as_str) == Some("error"),
            "Expected output to contain an error"
        );
    }

    /// Assert that a string contains a substring.
    ///
    /// # Panics
    ///
    /// Panics if the substring is not found.
    pub fn assert_contains(haystack: impl AsRef<str>, needle: impl AsRef<str>) {
        let haystack = haystack.as_ref();
        let needle = needle.as_ref();
        assert!(
            haystack.contains(needle),
            "Expected string to contain '{}', but it didn't. Full content:\n{}",
            needle,
            haystack
        );
    }

    /// Assert that a string does not contain a substring.
    ///
    /// # Panics
    ///
    /// Panics if the substring is found.
    pub fn assert_not_contains(haystack: impl AsRef<str>, needle: impl AsRef<str>) {
        let haystack = haystack.as_ref();
        let needle = needle.as_ref();
        assert!(
            !haystack.contains(needle),
            "Expected string to NOT contain '{}', but it did. Full content:\n{}",
            needle,
            haystack
        );
    }
}

/// Re-export commonly used types for convenience.
pub mod prelude {
    pub use super::{
        assertions::*,
        fixtures::*,
        MockTool, TempDirFixture, TrackedMockTool,
    };
    pub use runtime::{SessionId, ToolRegistry, ToolUseContext, ToolUseId};
    pub use tools::{ToolInput, ToolOutput, ToolResult};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_temp_dir_fixture() {
        let temp = TempDirFixture::new().await;
        let path = temp.path();
        assert!(path.exists());

        let file = temp.create_file("test.txt", "hello").await.unwrap();
        assert!(file.exists());
        assert_eq!(tokio::fs::read_to_string(&file).await.unwrap(), "hello");

        // Test nested directories
        let nested = temp.create_file("a/b/c/nested.txt", "nested").await.unwrap();
        assert!(nested.exists());
    }

    #[tokio::test]
    async fn test_fixture_rust_project() {
        let temp = TempDirFixture::new().await;
        let files = fixtures::rust_project(&temp).await.unwrap();

        assert!(files.contains_key("Cargo.toml"));
        assert!(files.contains_key("src/main.rs"));
        assert!(files.contains_key("src/lib.rs"));

        for path in files.values() {
            assert!(path.exists());
        }
    }

    #[tokio::test]
    async fn test_fixture_mixed_file_tree() {
        let temp = TempDirFixture::new().await;
        let paths = fixtures::mixed_file_tree(&temp).await.unwrap();

        assert_eq!(paths.len(), 7);

        // Check that all paths exist and are under the temp directory
        for path in &paths {
            assert!(path.exists());
            assert!(path.starts_with(temp.path()));
        }
    }

    #[tokio::test]
    async fn test_fixture_large_text_file() {
        let temp = TempDirFixture::new().await;
        let path = fixtures::large_text_file(&temp, "large.txt", 100).await.unwrap();

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 100);
        assert!(lines[0].contains("Line 1:"));
        assert!(lines[99].contains("Line 100:"));
    }

    #[tokio::test]
    async fn test_mock_tool() {
        let tool = MockTool::new("test_tool")
            .with_description("A test tool")
            .with_handler(|input| {
                let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                ToolOutput::new().with_field("greeting", format!("Hello, {name}!"))
            });

        let input = ToolInput::new().with_arg("name", "World");
        let output = tool.execute(input).await;

        assertions::assert_has_field(&output, "greeting");
        assertions::assert_output_field(&output, "greeting", "Hello, World!");
    }

    #[tokio::test]
    async fn test_tracked_mock_tool() {
        let tool = TrackedMockTool::new("tracked");

        // Invoke multiple times
        for i in 0..3 {
            let input = ToolInput::new().with_arg("index", i as u64);
            let _ = tool.execute(input).await;
        }

        tool.assert_invoked_times(3).await;

        let invocations = tool.get_invocations().await;
        assert_eq!(invocations.len(), 3);
    }
}
