//! NotebookEditTool - Edit Jupyter notebook cells.
//!
//! This tool provides editing capabilities for Jupyter notebook (.ipynb) files.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;

/// Maximum file size for notebooks (10 MB).
const MAX_NOTEBOOK_SIZE: u64 = 10 * 1024 * 1024;

/// Notebook cell types.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CellType {
    Code,
    Markdown,
    Raw,
}

impl Default for CellType {
    fn default() -> Self {
        Self::Code
    }
}

impl std::fmt::Display for CellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Code => write!(f, "code"),
            Self::Markdown => write!(f, "markdown"),
            Self::Raw => write!(f, "raw"),
        }
    }
}

/// A notebook cell output.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct CellOutput {
    #[serde(rename = "output_type")]
    pub output_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evalue: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub traceback: Option<Vec<String>>,
}

/// A notebook cell.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotebookCell {
    #[serde(rename = "cell_type")]
    pub cell_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub source: serde_json::Value, // Can be string or array of strings
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "execution_count")]
    pub execution_count: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<Vec<CellOutput>>,
}

impl Default for NotebookCell {
    fn default() -> Self {
        Self {
            cell_type: "code".to_string(),
            id: None,
            source: serde_json::Value::String(String::new()),
            metadata: Some(HashMap::new()),
            execution_count: None,
            outputs: None,
        }
    }
}

impl NotebookCell {
    /// Get the source as a string.
    pub fn source_string(&self) -> String {
        match &self.source {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join("")
            }
            _ => String::new(),
        }
    }

    /// Set the source from a string.
    pub fn set_source(&mut self, source: impl Into<String>) {
        self.source = serde_json::Value::String(source.into());
    }
}

/// Notebook metadata structure.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct NotebookMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kernelspec: Option<HashMap<String, serde_json::Value>>,
    #[serde(rename = "language_info", skip_serializing_if = "Option::is_none")]
    pub language_info: Option<HashMap<String, serde_json::Value>>,
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Notebook content structure (nbformat).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotebookContent {
    pub metadata: NotebookMetadata,
    pub cells: Vec<NotebookCell>,
    #[serde(rename = "nbformat")]
    pub nbformat: i32,
    #[serde(rename = "nbformat_minor")]
    pub nbformat_minor: i32,
}

impl Default for NotebookContent {
    fn default() -> Self {
        Self {
            metadata: NotebookMetadata::default(),
            cells: Vec::new(),
            nbformat: 4,
            nbformat_minor: 5,
        }
    }
}

impl NotebookContent {
    /// Get the programming language from metadata.
    pub fn language(&self) -> String {
        self.metadata
            .language_info
            .as_ref()
            .and_then(|li| li.get("name"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| "python".to_string())
    }

    /// Find cell index by ID or return None.
    pub fn find_cell_index(&self, cell_id: &str) -> Option<usize> {
        // First try to find by actual ID
        let by_id = self.cells.iter().position(|c| c.id.as_deref() == Some(cell_id));
        if by_id.is_some() {
            return by_id;
        }

        // Try to parse as "cell-N" format
        parse_cell_index(cell_id)
    }

    /// Generate a new cell ID (for nbformat 4.5+).
    pub fn generate_cell_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        format!("cell-{:x}", timestamp)
    }
}

/// Parse a cell ID in "cell-N" format to get the index.
fn parse_cell_index(cell_id: &str) -> Option<usize> {
    let pattern = regex::Regex::new(r"^cell-(\d+)$").ok()?;
    pattern.captures(cell_id)?.get(1)?.as_str().parse().ok()
}

/// Input schema for the NotebookEditTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NotebookEditInput {
    /// The absolute path to the Jupyter notebook file.
    pub notebook_path: String,
    /// The ID of the cell to edit.
    #[serde(default)]
    pub cell_id: Option<String>,
    /// The new source for the cell.
    pub new_source: String,
    /// The type of the cell (code or markdown).
    #[serde(default)]
    pub cell_type: Option<String>,
    /// The type of edit to make.
    #[serde(default)]
    pub edit_mode: Option<String>,
}

/// Output schema for the NotebookEditTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookEditOutput {
    pub new_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_id: Option<String>,
    pub cell_type: String,
    pub language: String,
    pub edit_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub notebook_path: String,
    pub original_file: String,
    pub updated_file: String,
}

/// Edit modes for notebook cells.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EditMode {
    Replace,
    Insert,
    Delete,
}

impl Default for EditMode {
    fn default() -> Self {
        Self::Replace
    }
}

impl std::fmt::Display for EditMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Replace => write!(f, "replace"),
            Self::Insert => write!(f, "insert"),
            Self::Delete => write!(f, "delete"),
        }
    }
}

/// The NotebookEditTool edits Jupyter notebook cells.
#[derive(Debug, Clone, Default)]
pub struct NotebookEditTool;

impl NotebookEditTool {
    /// Create a new NotebookEditTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse the edit mode string.
    fn parse_edit_mode(mode: Option<&str>) -> EditMode {
        match mode {
            Some("insert") => EditMode::Insert,
            Some("delete") => EditMode::Delete,
            _ => EditMode::Replace,
        }
    }

    /// Read and parse a notebook file.
    async fn read_notebook(path: &PathBuf) -> ToolResult<NotebookContent> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound {
                    message: format!("Notebook file does not exist: {}", path.display()),
                }
            } else {
                ToolError::ExecutionFailed {
                    message: format!("Failed to read notebook: {e}"),
                }
            }
        })?;

        serde_json::from_str(&content).map_err(|e| ToolError::ValidationFailed {
            message: format!("Notebook is not valid JSON: {e}"),
            error_code: Some(6),
        })
    }

    /// Serialize notebook to JSON string.
    fn serialize_notebook(notebook: &NotebookContent) -> String {
        serde_json::to_string_pretty(notebook)
            .unwrap_or_default()
            .replace("\n    ", "\n ") // Reduce indentation
            .replace("\n        ", "\n  ")
            .replace("\n            ", "\n   ")
    }
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "NotebookEditTool",
                "Edit Jupyter notebook cells (replace, insert, delete)",
            )
            .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        let notebook_path = input
            .require("notebook_path")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "notebook_path must be a string".to_string(),
                error_code: Some(1),
            })?;

        if notebook_path.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "notebook_path cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Check file extension
        if !notebook_path.to_lowercase().ends_with(".ipynb") {
            return Err(ToolError::ValidationFailed {
                message: "File must be a Jupyter notebook (.ipynb file).".to_string(),
                error_code: Some(2),
            });
        }

        let edit_mode = Self::parse_edit_mode(
            input.get("edit_mode").and_then(|v| v.as_str())
        );

        // Validate edit_mode
        if let Some(mode_str) = input.get("edit_mode").and_then(|v| v.as_str()) {
            if mode_str != "replace" && mode_str != "insert" && mode_str != "delete" {
                return Err(ToolError::ValidationFailed {
                    message: "Edit mode must be replace, insert, or delete.".to_string(),
                    error_code: Some(4),
                });
            }
        }

        // Cell type required for insert mode
        if edit_mode == EditMode::Insert && input.get("cell_type").is_none() {
            return Err(ToolError::ValidationFailed {
                message: "Cell type is required when using edit_mode=insert.".to_string(),
                error_code: Some(5),
            });
        }

        // Validate cell type if provided
        if let Some(cell_type) = input.get("cell_type").and_then(|v| v.as_str()) {
            if cell_type != "code" && cell_type != "markdown" && cell_type != "raw" {
                return Err(ToolError::ValidationFailed {
                    message: "Cell type must be code, markdown, or raw.".to_string(),
                    error_code: Some(6),
                });
            }
        }

        let path = PathBuf::from(notebook_path);

        // Check file exists and get metadata
        let metadata = fs::metadata(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::NotFound {
                    message: "Notebook file does not exist.".to_string(),
                }
            } else {
                ToolError::ExecutionFailed {
                    message: format!("Failed to access file: {e}"),
                }
            }
        })?;

        // Check file size
        if metadata.len() > MAX_NOTEBOOK_SIZE {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Notebook size ({} bytes) exceeds maximum allowed ({} bytes).",
                    metadata.len(),
                    MAX_NOTEBOOK_SIZE
                ),
                error_code: Some(3),
            });
        }

        // Read and parse the notebook
        let notebook = Self::read_notebook(&path).await?;

        // Validate cell_id
        let cell_id = input.get("cell_id").and_then(|v| v.as_str());

        if cell_id.is_none() && edit_mode != EditMode::Insert {
            return Err(ToolError::ValidationFailed {
                message: "Cell ID must be specified when not inserting a new cell.".to_string(),
                error_code: Some(7),
            });
        }

        if let Some(id) = cell_id {
            if notebook.find_cell_index(id).is_none() {
                return Err(ToolError::ValidationFailed {
                    message: format!(r#"Cell with ID "{}" not found in notebook."#, id),
                    error_code: Some(8),
                });
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let notebook_path = match input.get("notebook_path") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "notebook_path must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: notebook_path")
                    .with_field("success", false);
            }
        };

        let new_source = match input.get("new_source") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "new_source must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: new_source")
                    .with_field("success", false);
            }
        };

        let cell_id = input.get("cell_id").and_then(|v| v.as_str()).map(String::from);
        let cell_type = input.get("cell_type").and_then(|v| v.as_str()).map(String::from);
        let edit_mode = Self::parse_edit_mode(
            input.get("edit_mode").and_then(|v| v.as_str())
        );

        let path = PathBuf::from(&notebook_path);

        // Read the original content first
        let original_content = match fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                return ToolOutput::new()
                    .with_field("error", format!("Failed to read notebook: {e}"))
                    .with_field("success", false);
            }
        };

        // Parse the notebook
        let mut notebook: NotebookContent = match serde_json::from_str(&original_content) {
            Ok(n) => n,
            Err(e) => {
                return ToolOutput::new()
                    .with_field("error", format!("Notebook is not valid JSON: {e}"))
                    .with_field("success", false);
            }
        };

        let language = notebook.language();

        // Determine cell index
        let cell_index = if let Some(ref id) = cell_id {
            // First try to find by actual ID
            let mut idx = notebook.cells.iter().position(|c| c.id.as_deref() == Some(id));

            // If not found, try to parse as numeric index
            if idx.is_none() {
                idx = parse_cell_index(id);
            }

            idx.unwrap_or(0)
        } else {
            0 // Default to beginning if no cell_id
        };

        // Determine insert position
        let insert_index = if edit_mode == EditMode::Insert && cell_id.is_some() {
            cell_index + 1 // Insert after the cell with this ID
        } else {
            cell_index
        };

        // Handle auto-convert replace to insert when at end
        let actual_edit_mode = if edit_mode == EditMode::Replace && insert_index >= notebook.cells.len() {
            EditMode::Insert
        } else {
            edit_mode.clone()
        };

        // Determine if we need to generate cell IDs (nbformat >= 4.5)
        let needs_cell_id = notebook.nbformat > 4 || (notebook.nbformat == 4 && notebook.nbformat_minor >= 5);

        // Perform the edit operation
        let new_cell_id = match actual_edit_mode {
            EditMode::Delete => {
                if insert_index < notebook.cells.len() {
                    notebook.cells.remove(insert_index);
                }
                None
            }
            EditMode::Insert => {
                let cell_type_str = cell_type.as_deref().unwrap_or("code");
                let new_id = if needs_cell_id {
                    Some(NotebookContent::generate_cell_id())
                } else {
                    None
                };

                let new_cell = if cell_type_str == "markdown" {
                    NotebookCell {
                        cell_type: "markdown".to_string(),
                        id: new_id.clone(),
                        source: serde_json::Value::String(new_source.clone()),
                        metadata: Some(HashMap::new()),
                        execution_count: None,
                        outputs: None,
                    }
                } else {
                    NotebookCell {
                        cell_type: "code".to_string(),
                        id: new_id.clone(),
                        source: serde_json::Value::String(new_source.clone()),
                        metadata: Some(HashMap::new()),
                        execution_count: Some(serde_json::Value::Null),
                        outputs: Some(Vec::new()),
                    }
                };

                notebook.cells.insert(insert_index.min(notebook.cells.len()), new_cell);
                new_id
            }
            EditMode::Replace => {
                if insert_index < notebook.cells.len() {
                    let target_cell = &mut notebook.cells[insert_index];
                    target_cell.set_source(&new_source);

                    // Reset execution count and clear outputs for code cells
                    if target_cell.cell_type == "code" {
                        target_cell.execution_count = Some(serde_json::Value::Null);
                        target_cell.outputs = Some(Vec::new());
                    }

                    // Update cell type if provided
                    if let Some(ref ct) = cell_type {
                        target_cell.cell_type = ct.clone();
                    }

                    target_cell.id.clone()
                } else {
                    None
                }
            }
        };

        // Serialize the updated notebook
        let updated_content = Self::serialize_notebook(&notebook);

        // Write back to file
        if let Err(e) = fs::write(&path, &updated_content).await {
            return ToolOutput::new()
                .with_field("error", format!("Failed to write notebook: {e}"))
                .with_field("success", false);
        }

        // Build output
        ToolOutput::new()
            .with_field("success", true)
            .with_field("new_source", new_source)
            .with_field("cell_type", cell_type.unwrap_or_else(|| "code".to_string()))
            .with_field("language", language)
            .with_field("edit_mode", actual_edit_mode.to_string())
            .with_field("cell_id", new_cell_id.or(cell_id))
            .with_field("notebook_path", notebook_path)
            .with_field("original_file", original_content)
            .with_field("updated_file", updated_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_notebook() -> NotebookContent {
        NotebookContent {
            metadata: NotebookMetadata {
                kernelspec: Some({
                    let mut map = HashMap::new();
                    map.insert("name".to_string(), serde_json::json!("python3"));
                    map.insert("display_name".to_string(), serde_json::json!("Python 3"));
                    map
                }),
                language_info: Some({
                    let mut map = HashMap::new();
                    map.insert("name".to_string(), serde_json::json!("python"));
                    map
                }),
                extra: HashMap::new(),
            },
            nbformat: 4,
            nbformat_minor: 5,
            cells: vec![
                NotebookCell {
                    cell_type: "markdown".to_string(),
                    id: Some("cell-0".to_string()),
                    source: serde_json::Value::String("# Test Notebook".to_string()),
                    metadata: Some(HashMap::new()),
                    execution_count: None,
                    outputs: None,
                },
                NotebookCell {
                    cell_type: "code".to_string(),
                    id: Some("cell-1".to_string()),
                    source: serde_json::Value::String("print('hello')".to_string()),
                    metadata: Some(HashMap::new()),
                    execution_count: Some(serde_json::Value::Number(1.into())),
                    outputs: Some(vec![CellOutput {
                        output_type: "stream".to_string(),
                        text: Some(serde_json::Value::String("hello\n".to_string())),
                        data: None,
                        ename: None,
                        evalue: None,
                        traceback: None,
                    }]),
                },
            ],
        }
    }

    #[tokio::test]
    async fn test_notebook_edit_replace_cell() {
        let tool = NotebookEditTool::new();
        let dir = tempdir().unwrap();
        let notebook_path = dir.path().join("test.ipynb");

        // Create a test notebook
        let notebook = create_test_notebook();
        fs::write(&notebook_path, serde_json::to_string_pretty(&notebook).unwrap())
            .await
            .unwrap();

        let input = ToolInput::new()
            .with_arg("notebook_path", notebook_path.to_string_lossy().to_string())
            .with_arg("cell_id", "cell-1")
            .with_arg("new_source", "print('updated')")
            .with_arg("edit_mode", "replace");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Verify the cell was updated
        let updated_content = fs::read_to_string(&notebook_path).await.unwrap();
        let updated_notebook: NotebookContent = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_notebook.cells[1].source_string(), "print('updated')");
        // Execution count should be reset (null or None)
        assert!(
            updated_notebook.cells[1].execution_count.is_none() ||
            updated_notebook.cells[1].execution_count == Some(serde_json::Value::Null)
        );
    }

    #[tokio::test]
    async fn test_notebook_edit_insert_cell() {
        let tool = NotebookEditTool::new();
        let dir = tempdir().unwrap();
        let notebook_path = dir.path().join("test.ipynb");

        let notebook = create_test_notebook();
        fs::write(&notebook_path, serde_json::to_string_pretty(&notebook).unwrap())
            .await
            .unwrap();

        let input = ToolInput::new()
            .with_arg("notebook_path", notebook_path.to_string_lossy().to_string())
            .with_arg("cell_id", "cell-0")
            .with_arg("new_source", "# New Cell")
            .with_arg("cell_type", "markdown")
            .with_arg("edit_mode", "insert");

        let output = tool.execute(input).await;

        assert_eq!(
            output.data.get("success").and_then(|v| v.as_bool()),
            Some(true)
        );

        // Verify the cell was inserted after cell-0 (at index 1)
        let updated_content = fs::read_to_string(&notebook_path).await.unwrap();
        let updated_notebook: NotebookContent = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_notebook.cells.len(), 3);
        assert_eq!(updated_notebook.cells[1].cell_type, "markdown");
        assert_eq!(updated_notebook.cells[1].source_string(), "# New Cell");
    }

    #[tokio::test]
    async fn test_notebook_validation_not_ipynb() {
        let tool = NotebookEditTool::new();

        let input = ToolInput::new()
            .with_arg("notebook_path", "/path/to/file.txt")
            .with_arg("new_source", "test");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_cell_index() {
        assert_eq!(parse_cell_index("cell-0"), Some(0));
        assert_eq!(parse_cell_index("cell-5"), Some(5));
        assert_eq!(parse_cell_index("cell-123"), Some(123));
        assert_eq!(parse_cell_index("invalid"), None);
        assert_eq!(parse_cell_index("cell-"), None);
        assert_eq!(parse_cell_index("cell-abc"), None);
    }

    #[test]
    fn test_cell_source_string() {
        let cell = NotebookCell {
            cell_type: "code".to_string(),
            id: None,
            source: serde_json::Value::String("hello world".to_string()),
            metadata: None,
            execution_count: None,
            outputs: None,
        };
        assert_eq!(cell.source_string(), "hello world");

        // Array source (multi-line)
        let cell2 = NotebookCell {
            cell_type: "code".to_string(),
            id: None,
            source: serde_json::Value::Array(vec![
                serde_json::json!("line1\n"),
                serde_json::json!("line2\n"),
            ]),
            metadata: None,
            execution_count: None,
            outputs: None,
        };
        assert_eq!(cell2.source_string(), "line1\nline2\n");
    }

    #[tokio::test]
    async fn test_notebook_language_detection() {
        let mut notebook = create_test_notebook();
        assert_eq!(notebook.language(), "python");

        // Test with no language_info
        notebook.metadata.language_info = None;
        assert_eq!(notebook.language(), "python"); // Default
    }
}
