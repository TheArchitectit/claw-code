//! BashTool - Execute shell commands safely with timeout support.
//!
//! This tool provides a safe way to execute shell commands, with proper
//! timeout handling and output capture.

use crate::tool::{Tool, ToolError, ToolInput, ToolMetadata, ToolOutput, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// Default timeout for bash commands (5 minutes).
const DEFAULT_TIMEOUT_MS: u64 = 300_000;

/// Maximum allowed timeout (30 minutes).
const MAX_TIMEOUT_MS: u64 = 1_800_000;

/// Input schema for the BashTool.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BashInput {
    /// The command to execute.
    pub command: String,

    /// Optional timeout in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,

    /// Description of what the command does.
    #[serde(default)]
    pub description: Option<String>,

    /// Whether to run in the background.
    #[serde(default)]
    pub run_in_background: Option<bool>,
}

/// Output schema for the BashTool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashOutput {
    /// The standard output of the command.
    pub stdout: String,

    /// The standard error output.
    pub stderr: String,

    /// Whether the command was interrupted (timed out).
    pub interrupted: bool,

    /// The exit code of the command (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Duration of execution in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl BashOutput {
    /// Create an error output.
    #[must_use]
    fn error(message: impl Into<String>) -> Self {
        Self {
            stdout: String::new(),
            stderr: message.into(),
            interrupted: false,
            exit_code: None,
            duration_ms: None,
        }
    }
}

/// The BashTool executes shell commands.
#[derive(Debug, Clone, Default)]
pub struct BashTool;

impl BashTool {
    /// Create a new BashTool.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Validate that a command doesn't contain dangerous patterns.
    fn validate_command_safety(command: &str) -> ToolResult<()> {
        // Check for commands that could be dangerous
        let dangerous_patterns = [
            "rm -rf /",
            "rm -rf /*",
            ":(){ :|:& };:", // Fork bomb
            "> /dev/sda",
            "dd if=/dev/zero of=/dev/sda",
        ];

        let cmd_lower = command.to_lowercase();
        for pattern in dangerous_patterns {
            if cmd_lower.contains(pattern) {
                return Err(ToolError::ValidationFailed {
                    message: format!("Potentially dangerous command pattern detected: {pattern}"),
                    error_code: Some(100),
                });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for BashTool {
    fn metadata(&self) -> &ToolMetadata {
        // This is stored in a static to avoid reallocation
        static METADATA: std::sync::OnceLock<ToolMetadata> = std::sync::OnceLock::new();
        METADATA.get_or_init(|| {
            ToolMetadata::new("BashTool", "Execute shell commands safely with timeout support")
                .concurrency_safe(false)
        })
    }

    async fn validate(&self, input: &ToolInput) -> ToolResult<()> {
        // Check for required command argument
        let command = input
            .require("command")?
            .as_str()
            .ok_or_else(|| ToolError::ValidationFailed {
                message: "Command must be a string".to_string(),
                error_code: Some(1),
            })?;

        if command.is_empty() {
            return Err(ToolError::ValidationFailed {
                message: "Command cannot be empty".to_string(),
                error_code: Some(2),
            });
        }

        // Validate command safety
        Self::validate_command_safety(command)?;

        // Validate timeout if provided
        if let Some(timeout_val) = input.get("timeout") {
            if let Some(timeout_ms) = timeout_val.as_u64() {
                if timeout_ms > MAX_TIMEOUT_MS {
                    return Err(ToolError::ValidationFailed {
                        message: format!("Timeout cannot exceed {MAX_TIMEOUT_MS}ms"),
                        error_code: Some(3),
                    });
                }
            }
        }

        Ok(())
    }

    async fn execute(&self, input: ToolInput) -> ToolOutput {
        let start_time = std::time::Instant::now();

        // Parse input
        let command = match input.get("command") {
            Some(v) => match v.as_str() {
                Some(s) => s.to_string(),
                None => {
                    return ToolOutput::new()
                        .with_field("error", "Command must be a string")
                        .with_field("stdout", "")
                        .with_field("stderr", "Invalid command type")
                        .with_field("interrupted", false);
                }
            },
            None => {
                return ToolOutput::new()
                    .with_field("error", "Missing required argument: command")
                    .with_field("stdout", "")
                    .with_field("stderr", "Command not provided")
                    .with_field("interrupted", false);
            }
        };

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS);

        // Execute the command using sh -c for proper shell interpretation
        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let result = match timeout(Duration::from_millis(timeout_ms), cmd.output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let duration_ms = start_time.elapsed().as_millis() as u64;

                ToolOutput::new()
                    .with_field("stdout", stdout)
                    .with_field("stderr", stderr)
                    .with_field("interrupted", false)
                    .with_field("exit_code", output.status.code())
                    .with_field("duration_ms", duration_ms)
            }
            Ok(Err(e)) => {
                ToolOutput::new()
                    .with_field("error", format!("Failed to execute command: {e}"))
                    .with_field("stdout", "")
                    .with_field("stderr", "")
                    .with_field("interrupted", false)
            }
            Err(_) => {
                // Timeout
                ToolOutput::new()
                    .with_field("error", format!("Command timed out after {timeout_ms}ms"))
                    .with_field("stdout", "")
                    .with_field("stderr", "")
                    .with_field("interrupted", true)
            }
        };

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bash_tool_echo() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo 'Hello World'");

        let output = tool.execute(input).await;

        let stdout = output
            .data
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(stdout.contains("Hello World"));
        assert!(!output
            .data
            .get("interrupted")
            .and_then(|v| v.as_bool())
            .unwrap_or(true));
    }

    #[tokio::test]
    async fn test_bash_tool_validation_empty_command() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bash_tool_validation_dangerous_command() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "rm -rf /");

        let result = tool.validate(&input).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bash_tool_stderr() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "echo 'error' >&2");

        let output = tool.execute(input).await;

        let stderr = output
            .data
            .get("stderr")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(stderr.contains("error"));
    }

    #[tokio::test]
    async fn test_bash_tool_exit_code() {
        let tool = BashTool::new();
        let input = ToolInput::new().with_arg("command", "exit 42");

        let output = tool.execute(input).await;

        let exit_code = output
            .data
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        assert_eq!(exit_code, 42);
    }
}
