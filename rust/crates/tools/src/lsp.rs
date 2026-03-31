//! LspTool - Language Server Protocol integration
//!
//! This tool provides LSP client capabilities for code intelligence:
//! - goToDefinition: Navigate to symbol definitions
//! - findReferences: Find all references to a symbol
//! - hover: Get type information and documentation
//! - documentSymbols: Extract symbols from a document

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// LSP operation types.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LspOperation {
    GoToDefinition,
    FindReferences,
    Hover,
    DocumentSymbols,
}

impl std::str::FromStr for LspOperation {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "goToDefinition" | "definition" => Ok(LspOperation::GoToDefinition),
            "findReferences" | "references" => Ok(LspOperation::FindReferences),
            "hover" => Ok(LspOperation::Hover),
            "documentSymbols" | "symbols" => Ok(LspOperation::DocumentSymbols),
            _ => Err(format!("Invalid LSP operation: {s}")),
        }
    }
}

/// Position in a document (0-indexed).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspPosition {
    /// Line number (0-indexed).
    pub line: u32,
    /// Character offset in the line (0-indexed).
    pub character: u32,
}

/// Location of a symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspLocation {
    pub uri: String,
    pub range: LspRange,
}

/// Range in a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspRange {
    pub start: LspPosition,
    pub end: LspPosition,
}

/// Symbol information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub location: LspLocation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
}

/// Hover information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverInfo {
    pub contents: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LspRange>,
}

/// Input schema for the LspTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LspInput {
    /// The file path to analyze.
    pub file_path: String,
    /// The LSP operation to perform.
    pub operation: String,
    /// Position for operations that require it (line).
    #[serde(default)]
    pub line: Option<u32>,
    /// Position for operations that require it (character).
    #[serde(default)]
    pub character: Option<u32>,
    /// Language server command (optional, will auto-detect if not provided).
    #[serde(default)]
    pub server_command: Option<String>,
}

/// Output schema for LSP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspOutput {
    pub operation: String,
    pub file_path: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub definitions: Option<Vec<LspLocation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<LspLocation>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover: Option<HoverInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbols: Option<Vec<SymbolInfo>>,
}

/// The LspTool provides code intelligence via LSP.
#[derive(Debug, Clone, Default)]
pub struct LspTool;

impl LspTool {
    /// Create a new LspTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Auto-detect LSP server command based on file extension.
    fn detect_server_command(&self, file_path: &str) -> Option<String> {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())?;

        let cmd = match ext {
            "rs" => "rust-analyzer",
            "js" | "ts" | "jsx" | "tsx" => "typescript-language-server",
            "py" => "pylsp",
            "go" => "gopls",
            "c" | "cpp" | "h" | "hpp" => "clangd",
            "java" => "jdtls",
            "rb" => "solargraph",
            "php" => "intelephense",
            _ => return None,
        };

        Some(cmd.to_string())
    }

    /// Execute goToDefinition operation (stub implementation).
    async fn goto_definition(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        _server_cmd: Option<&str>,
    ) -> LspOutput {
        // Stub: In full implementation, this would:
        // 1. Start the LSP server if not running
        // 2. Send textDocument/definition request
        // 3. Return the result

        LspOutput {
            operation: "goToDefinition".to_string(),
            file_path: file_path.to_string(),
            success: true,
            error: Some(
                "LSP server integration not yet implemented - stub returning empty result".to_string(),
            ),
            definitions: Some(Vec::new()),
            references: None,
            hover: None,
            symbols: None,
        }
    }

    /// Execute findReferences operation (stub implementation).
    async fn find_references(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        _server_cmd: Option<&str>,
    ) -> LspOutput {
        LspOutput {
            operation: "findReferences".to_string(),
            file_path: file_path.to_string(),
            success: true,
            error: Some(
                "LSP server integration not yet implemented - stub returning empty result".to_string(),
            ),
            definitions: None,
            references: Some(Vec::new()),
            hover: None,
            symbols: None,
        }
    }

    /// Execute hover operation (stub implementation).
    async fn hover(
        &self,
        file_path: &str,
        line: u32,
        character: u32,
        _server_cmd: Option<&str>,
    ) -> LspOutput {
        LspOutput {
            operation: "hover".to_string(),
            file_path: file_path.to_string(),
            success: true,
            error: Some(
                "LSP server integration not yet implemented - stub returning empty result".to_string(),
            ),
            definitions: None,
            references: None,
            hover: Some(HoverInfo {
                contents: String::new(),
                range: None,
            }),
            symbols: None,
        }
    }

    /// Execute documentSymbols operation (stub implementation).
    async fn document_symbols(&self, file_path: &str, _server_cmd: Option<&str>) -> LspOutput {
        // Try to extract basic symbols using regex patterns as fallback
        let symbols = self.extract_symbols_fallback(file_path).await;

        LspOutput {
            operation: "documentSymbols".to_string(),
            file_path: file_path.to_string(),
            success: symbols.is_ok(),
            error: symbols.as_ref().err().map(|e| e.to_string()),
            definitions: None,
            references: None,
            hover: None,
            symbols: symbols.ok(),
        }
    }

    /// Fallback symbol extraction using regex patterns.
    async fn extract_symbols_fallback(&self, file_path: &str) -> Result<Vec<SymbolInfo>, String> {
        use regex::Regex;

        let content = std::fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read file: {e}"))?;

        let mut symbols = Vec::new();
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match ext {
            "rs" => {
                // Pattern for Rust: fn, struct, enum, trait, impl, mod, const, static, type
                let patterns = [
                    (r"^\s*(?:pub\s+)?(?:async\s+)?fn\s+(\w+)", "function"),
                    (r"^\s*(?:pub\s+)?struct\s+(\w+)", "struct"),
                    (r"^\s*(?:pub\s+)?enum\s+(\w+)", "enum"),
                    (r"^\s*(?:pub\s+)?trait\s+(\w+)", "trait"),
                    (r"^\s*(?:pub\s+)?type\s+(\w+)", "typeAlias"),
                    (r"^\s*(?:pub\s+)?const\s+(\w+)", "constant"),
                    (r"^\s*(?:pub\s+)?static\s+(\w+)", "constant"),
                    (r"^\s*(?:pub\s+)?mod\s+(\w+)", "module"),
                    (r"^\s*(?:pub\s+)?use\s+([^;]+)", "import"),
                ];

                for (pattern, kind) in &patterns {
                    let regex = Regex::new(pattern).map_err(|e| e.to_string())?;
                    for (i, line) in content.lines().enumerate() {
                        if let Some(cap) = regex.captures(line) {
                            if let Some(name) = cap.get(1) {
                                symbols.push(SymbolInfo {
                                    name: name.as_str().to_string(),
                                    kind: (*kind).to_string(),
                                    detail: Some(line.trim().to_string()),
                                    location: LspLocation {
                                        uri: file_path.to_string(),
                                        range: LspRange {
                                            start: LspPosition {
                                                line: i as u32,
                                                character: 0,
                                            },
                                            end: LspPosition {
                                                line: i as u32,
                                                character: line.len() as u32,
                                            },
                                        },
                                    },
                                    container_name: None,
                                });
                            }
                        }
                    }
                }
            }
            "js" | "ts" | "jsx" | "tsx" => {
                let patterns = [
                    (r"^\s*(?:export\s+)?(?:async\s+)?function\s+(\w+)", "function"),
                    (r"^\s*(?:export\s+)?class\s+(\w+)", "class"),
                    (r"^\s*(?:export\s+)?(?:const|let|var)\s+(\w+)", "variable"),
                    (r"^\s*(?:export\s+)?interface\s+(\w+)", "interface"),
                    (r"^\s*(?:export\s+)?type\s+(\w+)", "typeAlias"),
                    (r"^\s*(?:export\s+)?enum\s+(\w+)", "enum"),
                ];

                for (pattern, kind) in &patterns {
                    let regex = Regex::new(pattern).map_err(|e| e.to_string())?;
                    for (i, line) in content.lines().enumerate() {
                        if let Some(cap) = regex.captures(line) {
                            if let Some(name) = cap.get(1) {
                                symbols.push(SymbolInfo {
                                    name: name.as_str().to_string(),
                                    kind: (*kind).to_string(),
                                    detail: Some(line.trim().to_string()),
                                    location: LspLocation {
                                        uri: file_path.to_string(),
                                        range: LspRange {
                                            start: LspPosition {
                                                line: i as u32,
                                                character: 0,
                                            },
                                            end: LspPosition {
                                                line: i as u32,
                                                character: line.len() as u32,
                                            },
                                        },
                                    },
                                    container_name: None,
                                });
                            }
                        }
                    }
                }
            }
            "py" => {
                let patterns = [
                    (r"^\s*(?:async\s+)?def\s+(\w+)", "function"),
                    (r"^\s*class\s+(\w+)", "class"),
                    (r"^\s*(\w+)\s*=", "variable"),
                ];

                for (pattern, kind) in &patterns {
                    let regex = Regex::new(pattern).map_err(|e| e.to_string())?;
                    for (i, line) in content.lines().enumerate() {
                        if let Some(cap) = regex.captures(line) {
                            if let Some(name) = cap.get(1) {
                                symbols.push(SymbolInfo {
                                    name: name.as_str().to_string(),
                                    kind: (*kind).to_string(),
                                    detail: Some(line.trim().to_string()),
                                    location: LspLocation {
                                        uri: file_path.to_string(),
                                        range: LspRange {
                                            start: LspPosition {
                                                line: i as u32,
                                                character: 0,
                                            },
                                            end: LspPosition {
                                                line: i as u32,
                                                character: line.len() as u32,
                                            },
                                        },
                                    },
                                    container_name: None,
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                // Generic pattern for other languages
                let regex = Regex::new(r"^\s*(?:def|function|fn|func|method|sub|procedure)\s+(\w+)")
                    .map_err(|e| e.to_string())?;
                for (i, line) in content.lines().enumerate() {
                    if let Some(cap) = regex.captures(line) {
                        if let Some(name) = cap.get(1) {
                            symbols.push(SymbolInfo {
                                name: name.as_str().to_string(),
                                kind: "function".to_string(),
                                detail: Some(line.trim().to_string()),
                                location: LspLocation {
                                    uri: file_path.to_string(),
                                    range: LspRange {
                                        start: LspPosition {
                                            line: i as u32,
                                            character: 0,
                                        },
                                        end: LspPosition {
                                            line: i as u32,
                                            character: line.len() as u32,
                                        },
                                    },
                                },
                                container_name: None,
                            });
                        }
                    }
                }
            }
        }

        // Sort by line number
        symbols.sort_by(|a, b| a.location.range.start.line.cmp(&b.location.range.start.line));

        // Remove duplicates
        symbols.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

        Ok(symbols)
    }
}

#[async_trait]
impl Tool for LspTool {
    fn metadata(&self) -> &ToolMetadata {
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new(
                "LspTool",
                "Language Server Protocol integration for code intelligence",
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

        let operation = input
            .require("operation")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "operation must be a string".to_string(),
                error_code: Some(3),
            })?;

        // Validate operation is valid
        let valid_ops = ["goToDefinition", "findReferences", "hover", "documentSymbols"];
        if !valid_ops.contains(&operation) {
            return Err(ToolError::ValidationFailed {
                message: format!(
                    "Invalid operation: {}. Valid: {:?}",
                    operation, valid_ops
                ),
                error_code: Some(4),
            });
        }

        // Check that position fields are provided for operations that need them
        let needs_position = operation == "goToDefinition"
            || operation == "findReferences"
            || operation == "hover";
        if needs_position {
            if input.get("line").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "line is required for this operation".to_string(),
                    error_code: Some(5),
                });
            }
            if input.get("character").is_none() {
                return Err(ToolError::ValidationFailed {
                    message: "character is required for this operation".to_string(),
                    error_code: Some(6),
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

        let operation = match input.get("operation") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "operation must be a string")
                        .with_field("success", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: operation")
                    .with_field("success", false);
            }
        };

        let line = input.get("line").and_then(|v| v.as_u64()).map(|v| v as u32);
        let character = input
            .get("character")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32);
        let server_command = input
            .get("server_command")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Auto-detect server if not provided
        let server_cmd = server_command.or_else(|| self.detect_server_command(&file_path));

        // Parse operation and execute
        let result = match operation.as_str() {
            "goToDefinition" => {
                if let (Some(l), Some(c)) = (line, character) {
                    self.goto_definition(&file_path, l, c, server_cmd.as_deref())
                        .await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "line and character are required")
                        .with_field("success", false);
                }
            }
            "findReferences" => {
                if let (Some(l), Some(c)) = (line, character) {
                    self.find_references(&file_path, l, c, server_cmd.as_deref())
                        .await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "line and character are required")
                        .with_field("success", false);
                }
            }
            "hover" => {
                if let (Some(l), Some(c)) = (line, character) {
                    self.hover(&file_path, l, c, server_cmd.as_deref()).await
                } else {
                    return ToolOutput::new()
                        .with_field("error", "line and character are required")
                        .with_field("success", false);
                }
            }
            "documentSymbols" => self.document_symbols(&file_path, server_cmd.as_deref()).await,
            _ => {
                return ToolOutput::new()
                    .with_field("error", format!("Unknown operation: {}", operation))
                    .with_field("success", false);
            }
        };

        ToolOutput::new()
            .with_field("operation", result.operation)
            .with_field("file_path", result.file_path)
            .with_field("success", result.success)
            .with_field("error", result.error)
            .with_field("definitions", result.definitions)
            .with_field("references", result.references)
            .with_field("hover", result.hover)
            .with_field("symbols", result.symbols)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_lsp_validation() {
        let tool = LspTool::new();

        // Valid documentSymbols
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/test.rs")
            .with_arg("operation", "documentSymbols");
        assert!(tool.validate(&input).await.is_ok());

        // Valid goToDefinition with position
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/test.rs")
            .with_arg("operation", "goToDefinition")
            .with_arg("line", 10)
            .with_arg("character", 5);
        assert!(tool.validate(&input).await.is_ok());

        // Missing file_path
        let input = ToolInput::new().with_arg("operation", "documentSymbols");
        assert!(tool.validate(&input).await.is_err());

        // Invalid operation
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/test.rs")
            .with_arg("operation", "invalidOp");
        assert!(tool.validate(&input).await.is_err());

        // goToDefinition without position
        let input = ToolInput::new()
            .with_arg("file_path", "/tmp/test.rs")
            .with_arg("operation", "goToDefinition");
        assert!(tool.validate(&input).await.is_err());
    }

    #[test]
    fn test_lsp_metadata() {
        let tool = LspTool::new();
        let meta = tool.metadata();
        assert_eq!(meta.name, "LspTool");
        assert!(meta.is_read_only);
    }

    #[test]
    fn test_detect_server_command() {
        let tool = LspTool::new();

        assert_eq!(
            tool.detect_server_command("test.rs"),
            Some("rust-analyzer".to_string())
        );
        assert_eq!(
            tool.detect_server_command("test.ts"),
            Some("typescript-language-server".to_string())
        );
        assert_eq!(
            tool.detect_server_command("test.py"),
            Some("pylsp".to_string())
        );
        assert_eq!(
            tool.detect_server_command("test.go"),
            Some("gopls".to_string())
        );
        assert_eq!(tool.detect_server_command("test.txt"), None);
    }

    #[tokio::test]
    async fn test_extract_symbols_rust() {
        let tool = LspTool::new();

        // Create a temporary Rust file
        let mut temp_file = tempfile::NamedTempFile::with_suffix(".rs").unwrap();
        write!(
            temp_file,
            r#"
pub fn main() {{
    println!("Hello");
}}

struct Point {{
    x: i32,
    y: i32,
}}

pub async fn async_func() {{}}
"#
        )
        .unwrap();

        let symbols = tool
            .extract_symbols_fallback(temp_file.path().to_str().unwrap())
            .await
            .unwrap();

        assert!(!symbols.is_empty());

        // Check for function
        let func = symbols.iter().find(|s| s.name == "main");
        assert!(func.is_some());
        assert_eq!(func.unwrap().kind, "function");

        // Check for struct
        let struct_sym = symbols.iter().find(|s| s.name == "Point");
        assert!(struct_sym.is_some());
        assert_eq!(struct_sym.unwrap().kind, "struct");
    }

    #[tokio::test]
    async fn test_extract_symbols_python() {
        let tool = LspTool::new();

        let mut temp_file = tempfile::NamedTempFile::with_suffix(".py").unwrap();
        write!(
            temp_file,
            r#"
def hello():
    print("Hello")

class MyClass:
    def method(self):
        pass
"#
        )
        .unwrap();

        let symbols = tool
            .extract_symbols_fallback(temp_file.path().to_str().unwrap())
            .await
            .unwrap();

        assert!(!symbols.is_empty());

        let func = symbols.iter().find(|s| s.name == "hello");
        assert!(func.is_some());
        assert_eq!(func.unwrap().kind, "function");

        let class = symbols.iter().find(|s| s.name == "MyClass");
        assert!(class.is_some());
        assert_eq!(class.unwrap().kind, "class");
    }
}
