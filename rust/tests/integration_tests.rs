//! Integration tests for R.A.D Codicological
//!
//! These tests verify the complete tool execution flow including:
//! - Tool execution with actual file operations
//! - Command dispatch
//! - Full end-to-end workflows

use tools::{
    BashTool, FileEditTool, FileReadTool, FileWriteTool, GlobTool, GrepTool,
    TaskCreateTool, TaskGetTool, TaskListTool, TaskOutputTool, TaskUpdateTool,
    Tool, ToolInput,
};

/// Helper to create a temp directory for integration tests
async fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

#[tokio::test]
async fn test_file_read_tool_execution() {
    let temp = temp_dir().await;
    let test_file = temp.path().join("test.txt");
    let content = "Hello, integration test!";

    // Create the test file
    tokio::fs::write(&test_file, content)
        .await
        .expect("Failed to write test file");

    // Execute FileReadTool
    let tool = FileReadTool::new();
    let input = ToolInput::new()
        .with_arg("file_path", test_file.to_str().unwrap());

    let output = tool.execute(input).await;

    // Verify the output uses "type" field (not "success")
    assert_eq!(
        output.data.get("type").and_then(|v| v.as_str()),
        Some("text"),
        "FileReadTool should return type 'text'"
    );

    let read_content = output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Output should have content field");

    // Content has line numbers added by the tool
    assert!(read_content.contains("Hello, integration test!"), "File content should match");
}

#[tokio::test]
async fn test_file_write_and_read_roundtrip() {
    let temp = temp_dir().await;
    let test_file = temp.path().join("roundtrip.txt");
    let content = "Testing write and read roundtrip!";

    // Step 1: Write file using FileWriteTool
    let write_tool = FileWriteTool::new();
    let write_input = ToolInput::new()
        .with_arg("file_path", test_file.to_str().unwrap())
        .with_arg("content", content);

    let write_output = write_tool.execute(write_input).await;

    // FileWriteTool uses "operation" field with values "create" or "update"
    assert_eq!(
        write_output.data.get("operation").and_then(|v| v.as_str()),
        Some("create"),
        "FileWriteTool should return operation 'create' for new files"
    );

    // Verify file was actually created
    assert!(test_file.exists(), "File should exist after write");

    // Step 2: Read the file back using FileReadTool
    let read_tool = FileReadTool::new();
    let read_input = ToolInput::new()
        .with_arg("file_path", test_file.to_str().unwrap());

    let read_output = read_tool.execute(read_input).await;

    assert_eq!(
        read_output.data.get("type").and_then(|v| v.as_str()),
        Some("text"),
        "FileReadTool should return type 'text'"
    );

    let read_content = read_output
        .data
        .get("content")
        .and_then(|v| v.as_str())
        .expect("Output should have content field");

    assert!(read_content.contains(content), "Content should match after roundtrip");
}

#[tokio::test]
async fn test_file_edit_tool_execution() {
    let temp = temp_dir().await;
    let test_file = temp.path().join("edit_test.txt");

    // Create initial file
    let initial_content = "Hello, world!\nThis is a test.\nGoodbye!";
    tokio::fs::write(&test_file, initial_content)
        .await
        .expect("Failed to write initial file");

    // Edit the file
    let tool = FileEditTool::new();
    let input = ToolInput::new()
        .with_arg("file_path", test_file.to_str().unwrap())
        .with_arg("old_string", "world")
        .with_arg("new_string", "integration test");

    let output = tool.execute(input).await;

    assert_eq!(
        output.data.get("success").and_then(|v| v.as_bool()),
        Some(true),
        "FileEditTool should return success=true"
    );

    // Verify the file was actually modified
    let modified_content = tokio::fs::read_to_string(&test_file)
        .await
        .expect("Failed to read modified file");

    assert!(
        modified_content.contains("Hello, integration test!"),
        "File should contain the replaced text"
    );
    assert!(
        !modified_content.contains("Hello, world!"),
        "File should not contain the old text"
    );
}

#[tokio::test]
async fn test_glob_tool_execution() {
    let temp = temp_dir().await;
    let base_path = temp.path();

    // Create several files
    tokio::fs::write(base_path.join("file1.txt"), "content1").await.unwrap();
    tokio::fs::write(base_path.join("file2.txt"), "content2").await.unwrap();
    tokio::fs::write(base_path.join("other.rs"), "rust code").await.unwrap();

    // Use GlobTool to find .txt files
    let tool = GlobTool::new();
    let input = ToolInput::new()
        .with_arg("pattern", base_path.join("*.txt").to_str().unwrap());

    let output = tool.execute(input).await;

    assert_eq!(
        output.data.get("num_files").and_then(|v| v.as_u64()),
        Some(2),
        "GlobTool should find 2 .txt files"
    );

    let filenames = output
        .data
        .get("filenames")
        .and_then(|v| v.as_array())
        .expect("Output should have filenames array");

    assert_eq!(filenames.len(), 2, "Should find 2 .txt files");
}

#[tokio::test]
async fn test_grep_tool_execution() {
    let temp = temp_dir().await;
    let test_file = temp.path().join("search_test.txt");

    // Create file with searchable content
    let content = "Line 1: Hello world\nLine 2: Hello integration test\nLine 3: Goodbye world";
    tokio::fs::write(&test_file, content).await.unwrap();

    // Search for "Hello"
    let tool = GrepTool::new();
    let input = ToolInput::new()
        .with_arg("pattern", "Hello")
        .with_arg("path", test_file.to_str().unwrap());

    let output = tool.execute(input).await;

    assert_eq!(
        output.data.get("mode").and_then(|v| v.as_str()),
        Some("files_with_matches"),
        "GrepTool should return mode 'files_with_matches'"
    );

    let filenames = output
        .data
        .get("filenames")
        .and_then(|v| v.as_array())
        .expect("Output should have filenames array");

    assert_eq!(filenames.len(), 1, "Should find 1 file with 'Hello'");
}

#[tokio::test]
async fn test_bash_tool_execution() {
    let tool = BashTool::new();
    let input = ToolInput::new()
        .with_arg("command", "echo 'Hello from bash'");

    let output = tool.execute(input).await;

    // BashTool returns stdout directly, no 'type' field
    assert!(
        output.data.contains_key("stdout"),
        "BashTool output should contain stdout field"
    );

    let stdout = output
        .data
        .get("stdout")
        .and_then(|v| v.as_str())
        .expect("Output should have stdout");

    assert!(stdout.contains("Hello from bash"), "Should capture command output");
}

#[tokio::test]
async fn test_task_management_flow() {
    use tools::task_store::reset_task_store;

    // Reset task store for clean test
    reset_task_store();

    // Step 1: Create a task
    let create_tool = TaskCreateTool::new();
    let create_input = ToolInput::new()
        .with_arg("subject", "Integration Test Task")
        .with_arg("description", "Testing the full task flow")
        .with_arg("owner", "test-agent");

    let create_output = create_tool.execute(create_input).await;
    let task_id = create_output
        .data
        .get("task_id")
        .and_then(|v| v.as_str())
        .expect("Should have task_id")
        .to_string();

    assert_eq!(
        create_output.data.get("type").and_then(|v| v.as_str()),
        Some("created")
    );

    // Step 2: Get the task
    let get_tool = TaskGetTool::new();
    let get_input = ToolInput::new().with_arg("task_id", &task_id);

    let get_output = get_tool.execute(get_input).await;

    assert_eq!(
        get_output.data.get("type").and_then(|v| v.as_str()),
        Some("found")
    );
    assert_eq!(
        get_output.data.get("subject").and_then(|v| v.as_str()),
        Some("Integration Test Task")
    );

    // Step 3: Update task status
    let update_tool = TaskUpdateTool::new();
    let update_input = ToolInput::new()
        .with_arg("task_id", &task_id)
        .with_arg("status", "in_progress");

    let update_output = update_tool.execute(update_input).await;

    assert_eq!(
        update_output.data.get("type").and_then(|v| v.as_str()),
        Some("updated")
    );

    // Verify status was updated
    let get_output2 = get_tool.execute(ToolInput::new().with_arg("task_id", &task_id)).await;
    assert_eq!(
        get_output2.data.get("status").and_then(|v| v.as_str()),
        Some("in_progress")
    );

    // Step 4: Store task output
    let output_tool = TaskOutputTool::new();
    let output_input = ToolInput::new()
        .with_arg("task_id", &task_id)
        .with_arg("content", "Task execution results here");

    let output_result = output_tool.execute(output_input).await;

    assert_eq!(
        output_result.data.get("type").and_then(|v| v.as_str()),
        Some("stored")
    );

    // Step 5: List tasks
    let list_tool = TaskListTool::new();
    let list_output = list_tool.execute(ToolInput::new()).await;

    let total = list_output
        .data
        .get("total")
        .and_then(|v| v.as_u64())
        .expect("Should have total count");

    assert!(total >= 1, "Should have at least 1 task in list");
}

#[tokio::test]
async fn test_tool_validation_failure() {
    // Test FileReadTool with missing required parameter
    let tool = FileReadTool::new();
    let input = ToolInput::new(); // Missing file_path

    let result = tool.validate(&input).await;

    assert!(result.is_err(), "Validation should fail for missing file_path");
}

#[tokio::test]
async fn test_tool_execution_error() {
    // Test FileReadTool with non-existent file
    let tool = FileReadTool::new();
    let input = ToolInput::new()
        .with_arg("file_path", "/nonexistent/path/to/file.txt");

    let output = tool.execute(input).await;

    assert_eq!(
        output.data.get("type").and_then(|v| v.as_str()),
        Some("not_found"),
        "Should fail for non-existent file with type 'not_found'"
    );

    assert!(
        output.data.contains_key("error"),
        "Output should contain error field"
    );
}

#[tokio::test]
async fn test_all_tools_are_available() {
    // Verify all expected tools can be instantiated and have correct metadata
    let tools: Vec<Box<dyn Tool>> = vec![
        Box::new(FileReadTool::new()),
        Box::new(FileWriteTool::new()),
        Box::new(FileEditTool::new()),
        Box::new(BashTool::new()),
        Box::new(GlobTool::new()),
        Box::new(GrepTool::new()),
        Box::new(TaskCreateTool::new()),
        Box::new(TaskListTool::new()),
        Box::new(TaskGetTool::new()),
        Box::new(TaskUpdateTool::new()),
        Box::new(TaskOutputTool::new()),
    ];

    let expected_names = vec![
        "FileReadTool",
        "FileWriteTool",
        "FileEditTool",
        "BashTool",
        "GlobTool",
        "GrepTool",
        "TaskCreateTool",
        "TaskListTool",
        "TaskGetTool",
        "TaskUpdateTool",
        "TaskOutputTool",
    ];

    assert_eq!(tools.len(), expected_names.len(), "Should have all tools");

    for (i, tool) in tools.iter().enumerate() {
        let meta = tool.metadata();
        assert_eq!(
            meta.name, expected_names[i],
            "Tool at index {} should be {}",
            i, expected_names[i]
        );
        assert!(
            !meta.description.is_empty(),
            "{} should have a description",
            meta.name
        );
    }
}
