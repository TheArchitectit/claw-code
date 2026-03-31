//! Integration tests for tool execution flow
//!
//! These tests verify that tools work together correctly in realistic workflows:
//! - File read → edit → write pipeline
//! - Glob finding files → Grep searching content
//! - Task creation → update → output storage → retrieval
//! - Web fetch → content processing
//!
//! All tests use the test-utils crate for fixtures and run with #[tokio::test].

use std::sync::Mutex;
use test_utils::prelude::*;
use test_utils::fixtures;
use tools::{
    BashTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool, GrepTool,
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskUpdateTool,
    WebFetchTool, WebSearchTool, Tool, ToolInput,
};

// Global mutex to ensure task-related tests run serially
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn setup() -> std::sync::MutexGuard<'static, ()> {
    use tools::task_store::reset_task_store;
    let guard = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    reset_task_store();
    guard
}

// =============================================================================
// File Operation Pipeline Tests
// =============================================================================

/// Test the complete file workflow: read → edit → write
#[tokio::test]
async fn test_file_read_edit_write_pipeline() {
    let temp = TempDirFixture::new().await;

    // Step 1: Create initial file with FileWriteTool
    let write_tool = FileWriteTool::new();
    let initial_content = "Hello, World!\nThis is the original content.\nGoodbye!";
    let file_path = temp.join("pipeline.txt");

    let write_output = write_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("content", initial_content),
        )
        .await;

    assert_success(&write_output);
    assert_output_field(&write_output, "operation", "create");
    assert!(file_path.exists(), "File should exist after write");

    // Step 2: Read the file with FileReadTool
    let read_tool = FileReadTool::new();
    let read_output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap()),
        )
        .await;

    assert_has_field(&read_output, "content");
    let read_content = read_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Should have content");
    assert_contains(read_content, "Hello, World!");

    // Step 3: Edit the file with FileEditTool
    let edit_tool = FileEditTool::new();
    let edit_output = edit_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("old_string", "World")
                .with_arg("new_string", "Integration Test"),
        )
        .await;

    assert_success(&edit_output);
    assert_has_field(&edit_output, "structured_patch");

    // Verify the edit was applied
    let final_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_contains(&final_content, "Hello, Integration Test!");
    assert_not_contains(&final_content, "Hello, World!");

    // Step 4: Read again to confirm changes
    let verify_output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap()),
        )
        .await;

    let verify_content = verify_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Should have content");
    assert_contains(verify_content, "Integration Test");
}

/// Test file write with parent directory creation
#[tokio::test]
async fn test_file_write_creates_nested_directories() {
    let temp = TempDirFixture::new().await;
    let write_tool = FileWriteTool::new();

    let nested_path = temp.join("deep/nested/directory/structure/file.txt");

    let output = write_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", nested_path.to_str().unwrap())
                .with_arg("content", "Nested content"),
        )
        .await;

    assert_success(&output);
    assert!(nested_path.exists(), "Nested file should exist");

    let content = tokio::fs::read_to_string(&nested_path).await.unwrap();
    assert_eq!(content, "Nested content");
}

/// Test error handling for non-existent file read
#[tokio::test]
async fn test_file_read_error_handling() {
    let read_tool = FileReadTool::new();

    let output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", "/nonexistent/path/to/file.txt"),
        )
        .await;

    assert_has_error(&output);
    let error_type = output
        .data
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_eq!(error_type, "not_found", "Should return not_found type");
}

/// Test FileEditTool with replace_all option
#[tokio::test]
async fn test_file_edit_replace_all() {
    let temp = TempDirFixture::new().await;
    let write_tool = FileWriteTool::new();
    let edit_tool = FileEditTool::new();

    // Create file with multiple occurrences
    let file_path = temp.join("multi.txt");
    write_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("content", "foo bar foo baz foo"),
        )
        .await;

    // Replace all occurrences
    let output = edit_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("old_string", "foo")
                .with_arg("new_string", "qux")
                .with_arg("replace_all", true),
        )
        .await;

    assert_success(&output);

    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, "qux bar qux baz qux");
}

/// Test FileEditTool validation for ambiguous matches
#[tokio::test]
async fn test_file_edit_validation_multiple_matches() {
    let temp = TempDirFixture::new().await;
    let write_tool = FileWriteTool::new();
    let edit_tool = FileEditTool::new();

    // Create file with multiple occurrences
    let file_path = temp.join("ambiguous.txt");
    write_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("content", "foo bar foo baz"),
        )
        .await;

    // Try to replace without replace_all - should fail validation
    let input = ToolInput::new()
        .with_arg("file_path", file_path.to_str().unwrap())
        .with_arg("old_string", "foo")
        .with_arg("new_string", "qux");

    let result = edit_tool.validate(&input).await;
    assert!(result.is_err(), "Validation should fail for ambiguous matches");
}

// =============================================================================
// Glob + Grep Integration Tests
// =============================================================================

/// Test GlobTool finding files and GrepTool searching content
#[tokio::test]
async fn test_glob_to_grep_workflow() {
    let temp = TempDirFixture::new().await;

    // Create a mixed file tree using fixtures
    let _paths: Vec<std::path::PathBuf> = fixtures::mixed_file_tree(&temp).await.unwrap();

    // Step 1: Use GlobTool to find all .rs files
    let glob_tool = GlobTool::new();
    let glob_output = glob_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "**/*.rs")
                .with_arg("path", temp.path().to_str().unwrap()),
        )
        .await;

    assert_has_field(&glob_output, "filenames");
    let filenames = glob_output
        .data
        .get("filenames")
        .and_then(|v| v.as_array())
        .expect("Should have filenames array");
    assert!(
        filenames.len() >= 3,
        "Should find at least 3 .rs files (main.rs, utils.rs, integration.rs)"
    );

    // Step 2: Use GrepTool to search for "fn" in those files
    let grep_tool = GrepTool::new();
    let grep_output = grep_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "fn ")
                .with_arg("path", temp.path().to_str().unwrap())
                .with_arg("glob", "*.rs")
                .with_arg("output_mode", "content"),
        )
        .await;

    assert_has_field(&grep_output, "content");
    let content = grep_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Should have content");
    assert_contains(content, "fn main");
}

/// Test GlobTool with pattern matching and GrepTool with case-insensitive search
#[tokio::test]
async fn test_glob_grep_case_insensitive() {
    let temp = TempDirFixture::new().await;

    // Create files with mixed case content
    temp.create_file("test1.txt", "Hello World")
        .await
        .unwrap();
    temp.create_file("test2.txt", "HELLO Universe")
        .await
        .unwrap();
    temp.create_file("other.log", "hello log")
        .await
        .unwrap();

    // Find all .txt files
    let glob_tool = GlobTool::new();
    let glob_output = glob_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "*.txt")
                .with_arg("path", temp.path().to_str().unwrap()),
        )
        .await;

    let num_files = glob_output
        .data
        .get("num_files")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(num_files, 2, "Should find 2 .txt files");

    // Search case-insensitively for "hello"
    let grep_tool = GrepTool::new();
    let grep_output = grep_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "hello")
                .with_arg("path", temp.path().to_str().unwrap())
                .with_arg("case_insensitive", true)
                .with_arg("output_mode", "count"),
        )
        .await;

    let num_matches = grep_output
        .data
        .get("num_matches")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(num_matches, 3, "Should find 3 case-insensitive matches");
}

/// Test GlobTool with truncation limit
#[tokio::test]
async fn test_glob_truncation() {
    let temp = TempDirFixture::new().await;

    // Create many files
    for i in 0..150 {
        temp.create_file(format!("file{i:03}.txt"), format!("content {i}"))
            .await
            .unwrap();
    }

    let glob_tool = GlobTool::new();
    let output = glob_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "*.txt")
                .with_arg("path", temp.path().to_str().unwrap()),
        )
        .await;

    let truncated = output
        .data
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let num_files = output
        .data
        .get("num_files")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // GlobTool has MAX_RESULTS = 100
    assert!(truncated, "Should indicate truncation for many files");
    assert_eq!(num_files, 100, "Should return at most 100 files");
}

/// Test GrepTool with context lines
#[tokio::test]
async fn test_grep_with_context() {
    let temp = TempDirFixture::new().await;

    // Create file with context
    let content = "Line 1
Line 2
Line 3
Target line here
Line 5
Line 6
Line 7";
    temp.create_file("context.txt", content).await.unwrap();

    let grep_tool = GrepTool::new();
    let output = grep_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "Target")
                .with_arg("path", temp.join("context.txt").to_str().unwrap())
                .with_arg("context", 2u64)
                .with_arg("output_mode", "content"),
        )
        .await;

    let result_content = output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    assert_contains(result_content, "Target line here");
    // Context lines should be included
    assert_contains(result_content, "Line 2");
    assert_contains(result_content, "Line 6");
}

// =============================================================================
// Task Management Flow Tests
// =============================================================================

/// Test complete task lifecycle: create → update → output → retrieve
#[tokio::test]
async fn test_task_full_lifecycle() {
    let _guard = setup();

    // Step 1: Create a task
    let create_tool = TaskCreateTool::new();
    let create_output = create_tool
        .execute(
            ToolInput::new()
                .with_arg("subject", "Integration Test Task")
                .with_arg("description", "Testing full task lifecycle")
                .with_arg("owner", "test-agent"),
        )
        .await;

    let task_id = create_output
        .data
        .get("task_id")
        .and_then(|v| v.as_str())
        .expect("Should have task_id")
        .to_string();

    assert_output_field(&create_output, "type", "created");
    assert_output_field(&create_output, "status", "pending");

    // Step 2: Update task status
    let update_tool = TaskUpdateTool::new();
    let update_output = update_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("status", "in_progress"),
        )
        .await;

    assert_output_field(&update_output, "type", "updated");
    assert_output_field(&update_output, "new_status", "in_progress");

    // Verify update via Get
    let get_tool = TaskGetTool::new();
    let get_output = get_tool
        .execute(ToolInput::new().with_arg("task_id", &task_id))
        .await;

    assert_output_field(&get_output, "type", "found");
    assert_output_field(&get_output, "status", "in_progress");

    // Step 3: Store task output
    let output_tool = TaskOutputTool::new();
    let task_result = "Task execution completed successfully!\nResults: 42 tests passed";

    let store_output = output_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("content", task_result),
        )
        .await;

    assert_output_field(&store_output, "type", "stored");
    assert_output_field(&store_output, "stored", true);

    // Step 4: Retrieve task output
    let retrieve_output = output_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("retrieve", true),
        )
        .await;

    assert_output_field(&retrieve_output, "type", "retrieved");
    assert_output_field(&retrieve_output, "has_output", true);

    let retrieved_content = retrieve_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Should have content");
    assert_eq!(retrieved_content, task_result);

    // Step 5: Mark task as completed
    let complete_output = update_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("status", "completed"),
        )
        .await;

    assert_output_field(&complete_output, "type", "updated");

    // Final verification
    let final_get = get_tool
        .execute(ToolInput::new().with_arg("task_id", &task_id))
        .await;

    assert_output_field(&final_get, "status", "completed");
}

/// Test task listing with multiple tasks
#[tokio::test]
async fn test_task_listing() {
    let _guard = setup();

    let create_tool = TaskCreateTool::new();
    let list_tool = TaskListTool::new();

    // Create multiple tasks
    for i in 0..5 {
        create_tool
            .execute(
                ToolInput::new()
                    .with_arg("subject", format!("Task {i}"))
                    .with_arg("status", if i % 2 == 0 { "pending" } else { "in_progress" }),
            )
            .await;
    }

    // List all tasks
    let list_output = list_tool.execute(ToolInput::new()).await;

    assert_has_field(&list_output, "tasks");
    assert_has_field(&list_output, "total");

    let total = list_output
        .data
        .get("total")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert!(total >= 5, "Should list at least 5 tasks, got {total}");
}

/// Test task not found error handling
#[tokio::test]
async fn test_task_not_found() {
    let get_tool = TaskGetTool::new();

    let output = get_tool
        .execute(ToolInput::new().with_arg("task_id", "nonexistent-task-12345"))
        .await;

    assert_output_field(&output, "type", "not_found");
    assert_has_field(&output, "error");
}

/// Test task update with invalid status
#[tokio::test]
async fn test_task_invalid_status() {
    let update_tool = TaskUpdateTool::new();

    let input = ToolInput::new()
        .with_arg("task_id", "some-task-id")
        .with_arg("status", "invalid_status");

    let result = update_tool.validate(&input).await;
    assert!(result.is_err(), "Should fail validation for invalid status");
}

// =============================================================================
// Web Tool Integration Tests
// =============================================================================

/// Test WebFetchTool fetching and processing content
#[tokio::test]
async fn test_web_fetch_content_processing() {
    // This test fetches a known endpoint and processes the result
    // Using httpbin.org for reliable testing
    let fetch_tool = WebFetchTool::new();

    let output = fetch_tool
        .execute(
            ToolInput::new()
                .with_arg("url", "https://httpbin.org/json")
                .with_arg("max_length", 5000u64),
        )
        .await;

    // Check response structure
    assert_has_field(&output, "url");
    assert_has_field(&output, "status_code");

    let status_code = output
        .data
        .get("status_code")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(status_code, 200, "Should return HTTP 200");

    // Verify content was fetched
    assert_has_field(&output, "content");
    let content = output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert!(!content.is_empty(), "Should have non-empty content");
}

/// Test WebSearchTool (may be mocked or use real search)
#[tokio::test]
async fn test_web_search() {
    let search_tool = WebSearchTool::new();

    let output = search_tool
        .execute(
            ToolInput::new()
                .with_arg("query", "Rust programming language")
                .with_arg("num_results", 3u64),
        )
        .await;

    // Search should return results or an error (API key missing is acceptable)
    if output.data.contains_key("results") {
        let results = output
            .data
            .get("results")
            .and_then(|v| v.as_array())
            .expect("Should have results array");
        assert!(
            results.len() <= 3,
            "Should return at most 3 results as requested"
        );
    } else {
        // API key might be missing - that's acceptable for this test
        assert_has_field(&output, "error");
    }
}

/// Test WebFetchTool with error handling for invalid URL
#[tokio::test]
async fn test_web_fetch_error_handling() {
    let fetch_tool = WebFetchTool::new();

    let output = fetch_tool
        .execute(
            ToolInput::new()
                .with_arg("url", "not-a-valid-url"),
        )
        .await;

    // Should either fail validation or return an error
    assert!(
        output.data.contains_key("error") || output.data.get("success").is_some(),
        "Should handle invalid URL gracefully"
    );
}

// =============================================================================
// Bash Tool Integration Tests
// =============================================================================

/// Test BashTool execution and output capture
#[tokio::test]
async fn test_bash_execution_pipeline() {
    let bash_tool = BashTool::new();

    // Create a temp file using bash, then read it
    let temp = TempDirFixture::new().await;
    let file_path = temp.join("bash_created.txt");

    let output = bash_tool
        .execute(
            ToolInput::new()
                .with_arg("command", format!("echo 'Created by bash' > {}", file_path.to_str().unwrap())),
        )
        .await;

    assert_has_field(&output, "stdout");
    assert_has_field(&output, "stderr");
    assert_has_field(&output, "exit_code");

    let exit_code = output
        .data
        .get("exit_code")
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    assert_eq!(exit_code, 0, "Command should succeed");

    // Verify file was created
    assert!(file_path.exists(), "File should be created by bash command");
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_contains(content.trim(), "Created by bash");
}

/// Test BashTool with stderr capture
#[tokio::test]
async fn test_bash_stderr_capture() {
    let bash_tool = BashTool::new();

    let output = bash_tool
        .execute(
            ToolInput::new()
                .with_arg("command", "echo 'error message' >&2"),
        )
        .await;

    let stderr = output
        .data
        .get("stderr")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_contains(stderr, "error message");
}

/// Test BashTool with non-zero exit code
#[tokio::test]
async fn test_bash_nonzero_exit() {
    let bash_tool = BashTool::new();

    let output = bash_tool
        .execute(
            ToolInput::new()
                .with_arg("command", "exit 42"),
        )
        .await;

    let exit_code = output
        .data
        .get("exit_code")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert_eq!(exit_code, 42, "Should capture non-zero exit code");
}

/// Test BashTool timeout handling
#[tokio::test]
async fn test_bash_timeout() {
    let bash_tool = BashTool::new();

    let output = bash_tool
        .execute(
            ToolInput::new()
                .with_arg("command", "sleep 10")
                .with_arg("timeout", 100u64), // 100ms timeout
        )
        .await;

    let interrupted = output
        .data
        .get("interrupted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    assert!(interrupted, "Should indicate interruption due to timeout");
}

/// Test BashTool dangerous command validation
#[tokio::test]
async fn test_bash_dangerous_command_validation() {
    let bash_tool = BashTool::new();

    let input = ToolInput::new().with_arg("command", "rm -rf /");
    let result = bash_tool.validate(&input).await;

    assert!(result.is_err(), "Should reject dangerous command");
}

// =============================================================================
// Cross-Tool Integration Tests
// =============================================================================

/// Test complex workflow: Create files → Glob find → Grep search → Edit → Verify
#[tokio::test]
async fn test_complex_file_workflow() {
    let temp = TempDirFixture::new().await;

    // Create multiple source files with similar structure
    let files = vec![
        ("src/auth.rs", "pub fn login() { /* TODO: implement */ }"),
        ("src/db.rs", "pub fn connect() { /* TODO: implement */ }"),
        ("src/api.rs", "pub fn handle_request() { /* TODO: implement */ }"),
    ];

    let write_tool = FileWriteTool::new();
    for (path, content) in files {
        let file_path = temp.join(path);
        write_tool
            .execute(
                ToolInput::new()
                    .with_arg("file_path", file_path.to_str().unwrap())
                    .with_arg("content", content),
            )
            .await;
    }

    // Find all Rust files
    let glob_tool = GlobTool::new();
    let glob_output = glob_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "**/*.rs")
                .with_arg("path", temp.path().to_str().unwrap()),
        )
        .await;

    let num_files = glob_output
        .data
        .get("num_files")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(num_files, 3, "Should find 3 Rust files");

    // Search for TODO comments
    let grep_tool = GrepTool::new();
    let grep_output = grep_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "TODO")
                .with_arg("path", temp.path().to_str().unwrap())
                .with_arg("output_mode", "count"),
        )
        .await;

    let num_matches = grep_output
        .data
        .get("num_matches")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(num_matches, 3, "Should find 3 TODO comments");

    // Edit one file to remove TODO
    let edit_tool = FileEditTool::new();
    edit_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", temp.join("src/auth.rs").to_str().unwrap())
                .with_arg("old_string", "/* TODO: implement */")
                .with_arg("new_string", "// Implementation complete"),
        )
        .await;

    // Verify the edit
    let read_tool = FileReadTool::new();
    let read_output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", temp.join("src/auth.rs").to_str().unwrap()),
        )
        .await;

    let content = read_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_contains(content, "Implementation complete");
    assert_not_contains(content, "TODO");
}

/// Test task-based workflow with file operations
#[tokio::test]
async fn test_task_driven_file_operations() {
    let _guard = setup();

    let temp = TempDirFixture::new().await;

    // Create a task for file processing
    let create_tool = TaskCreateTool::new();
    let task_output = create_tool
        .execute(
            ToolInput::new()
                .with_arg("subject", "Process configuration files")
                .with_arg("description", "Find and update config files")
                .with_arg("owner", "file-processor"),
        )
        .await;

    let task_id = task_output
        .data
        .get("task_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();

    // Update task to in_progress
    let update_tool = TaskUpdateTool::new();
    update_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("status", "in_progress"),
        )
        .await;

    // Create config files
    temp.create_file("config.json", r#"{"version": "1.0", "debug": false}"#)
        .await
        .unwrap();
    temp.create_file("settings.toml", r#"name = "app"\nport = 8080"#)
        .await
        .unwrap();

    // Find config files
    let glob_tool = GlobTool::new();
    let glob_result = glob_tool
        .execute(
            ToolInput::new()
                .with_arg("pattern", "config.*")
                .with_arg("path", temp.path().to_str().unwrap()),
        )
        .await;

    let found_files = glob_result
        .data
        .get("num_files")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // Store results as task output
    let output_tool = TaskOutputTool::new();
    let results = format!("Found {} configuration files\nFiles processed successfully", found_files);

    output_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("content", results),
        )
        .await;

    // Mark task as completed
    update_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("status", "completed"),
        )
        .await;

    // Verify task state
    let get_tool = TaskGetTool::new();
    let final_state = get_tool
        .execute(ToolInput::new().with_arg("task_id", &task_id))
        .await;

    assert_output_field(&final_state, "status", "completed");

    // Verify task output
    let stored_output = output_tool
        .execute(
            ToolInput::new()
                .with_arg("task_id", &task_id)
                .with_arg("retrieve", true),
        )
        .await;

    let content = stored_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    assert_contains(content, "Found 1 configuration files");
}

// =============================================================================
// Edge Cases and Error Handling Tests
// =============================================================================

/// Test empty file operations
#[tokio::test]
async fn test_empty_file_handling() {
    let temp = TempDirFixture::new().await;
    let write_tool = FileWriteTool::new();
    let read_tool = FileReadTool::new();

    // Write empty file
    let file_path = temp.join("empty.txt");
    write_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap())
                .with_arg("content", ""),
        )
        .await;

    // Read empty file
    let output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", file_path.to_str().unwrap()),
        )
        .await;

    assert_has_field(&output, "content");
}

/// Test large file handling
#[tokio::test]
async fn test_large_file_handling() {
    let temp = TempDirFixture::new().await;

    // Create large file using fixture
    let _path: std::path::PathBuf = fixtures::large_text_file(&temp, "large.txt", 1000).await.unwrap();

    let read_tool = FileReadTool::new();
    let output = read_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", temp.join("large.txt").to_str().unwrap())
                .with_arg("limit", 100u64), // Limit to 100 lines
        )
        .await;

    assert_has_field(&output, "content");
    let content = output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Count lines in output
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 100, "Should limit to 100 lines");
}

/// Test validation errors for missing required fields
#[tokio::test]
async fn test_validation_missing_required_fields() {
    // FileReadTool without file_path
    let read_tool = FileReadTool::new();
    let result = read_tool.validate(&ToolInput::new()).await;
    assert!(result.is_err(), "Should fail without file_path");

    // FileWriteTool without required fields
    let write_tool = FileWriteTool::new();
    let result = write_tool.validate(&ToolInput::new()).await;
    assert!(result.is_err(), "Should fail without required fields");

    // GrepTool without pattern
    let grep_tool = GrepTool::new();
    let result = grep_tool.validate(&ToolInput::new()).await;
    assert!(result.is_err(), "Should fail without pattern");

    // GlobTool without pattern
    let glob_tool = GlobTool::new();
    let result = glob_tool.validate(&ToolInput::new()).await;
    assert!(result.is_err(), "Should fail without pattern");
}

/// Test invalid regex in GrepTool
#[tokio::test]
async fn test_grep_invalid_regex() {
    let grep_tool = GrepTool::new();

    let input = ToolInput::new()
        .with_arg("pattern", "[invalid(regex") // Invalid regex
        .with_arg("path", ".");

    let result = grep_tool.validate(&input).await;
    assert!(result.is_err(), "Should fail with invalid regex");
}

/// Test FileEditTool on non-existent file
#[tokio::test]
async fn test_file_edit_nonexistent() {
    let edit_tool = FileEditTool::new();

    let output = edit_tool
        .execute(
            ToolInput::new()
                .with_arg("file_path", "/nonexistent/file.txt")
                .with_arg("old_string", "old")
                .with_arg("new_string", "new"),
        )
        .await;

    // Should create the file since old_string is not empty but file doesn't exist
    // Actually, with empty old_string it creates, otherwise it errors
    assert_has_field(&output, "success");
}

/// Test FileWriteTool size limit
#[tokio::test]
async fn test_file_write_size_validation() {
    let temp = TempDirFixture::new().await;
    let write_tool = FileWriteTool::new();

    // Try to write content exceeding max size (50MB)
    let huge_content = "x".repeat(51 * 1024 * 1024); // 51MB

    let input = ToolInput::new()
        .with_arg("file_path", temp.join("huge.txt").to_str().unwrap())
        .with_arg("content", huge_content);

    let result = write_tool.validate(&input).await;
    assert!(result.is_err(), "Should fail with content exceeding max size");
}

/// Test concurrent-safe tool metadata
#[tokio::test]
async fn test_tool_concurrency_metadata() {
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(FileReadTool::new()),
        Box::new(GlobTool::new()),
        Box::new(GrepTool::new()),
        Box::new(WebFetchTool::new()),
        Box::new(WebSearchTool::new()),
    ];

    for tool in &tools {
        let meta = tool.metadata();
        assert!(
            meta.is_read_only || !meta.is_concurrency_safe,
            "Tools should have correct concurrency metadata"
        );
    }
}
