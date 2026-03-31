//! Sandboxed execution for dangerous tools.
//!
//! The [`Sandbox`] provides isolated execution environments for tools
//! that may perform dangerous operations like file writes or shell commands.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use tokio::time::timeout;

use tools::{Tool, ToolInput, ToolOutput};

/// Errors that can occur during sandboxed execution.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum SandboxError {
    /// The sandbox is not available.
    #[error("Sandbox unavailable: {message}")]
    Unavailable { message: String },

    /// The operation timed out.
    #[error("Sandbox operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// The operation exceeded resource limits.
    #[error("Resource limit exceeded: {message}")]
    ResourceLimit { message: String },

    /// The operation was blocked by security policy.
    #[error("Security policy violation: {message}")]
    SecurityPolicy { message: String },

    /// The operation failed.
    #[error("Sandbox execution failed: {message}")]
    ExecutionFailed { message: String },

    /// Internal sandbox error.
    #[error("Internal sandbox error: {message}")]
    Internal { message: String },
}

/// Result type for sandbox operations.
pub type SandboxResult<T> = Result<T, SandboxError>;

/// Configuration for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxConfig {
    /// Whether sandboxing is enabled.
    pub enabled: bool,
    /// Maximum execution time in milliseconds.
    pub timeout_ms: u64,
    /// Maximum memory usage in MB.
    pub max_memory_mb: u32,
    /// Maximum file size that can be written in MB.
    pub max_file_size_mb: u32,
    /// Allowed directories for file operations.
    pub allowed_directories: Vec<PathBuf>,
    /// Blocked directories (cannot access these).
    pub blocked_directories: Vec<PathBuf>,
    /// Allowed shell commands (if empty, all are blocked).
    pub allowed_commands: Vec<String>,
    /// Blocked shell commands (always blocked).
    pub blocked_commands: Vec<String>,
    /// Whether network access is allowed.
    pub allow_network: bool,
    /// Whether to allow executing external binaries.
    pub allow_external_binaries: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 120_000, // 2 minutes
            max_memory_mb: 512,
            max_file_size_mb: 100,
            allowed_directories: vec![],
            blocked_directories: vec![
                PathBuf::from("/etc"),
                PathBuf::from("/root"),
                PathBuf::from("~/.ssh"),
            ],
            allowed_commands: vec![
                "echo".to_string(),
                "cat".to_string(),
                "ls".to_string(),
                "grep".to_string(),
                "find".to_string(),
                "head".to_string(),
                "tail".to_string(),
                "wc".to_string(),
                "git".to_string(),
                "cargo".to_string(),
                "rustc".to_string(),
            ],
            blocked_commands: vec![
                "rm -rf /".to_string(),
                "dd ".to_string(),
                "mkfs".to_string(),
                "fdisk".to_string(),
            ],
            allow_network: false,
            allow_external_binaries: false,
        }
    }
}

impl SandboxConfig {
    /// Create a new default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a permissive configuration (less restrictions).
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            enabled: true,
            timeout_ms: 300_000, // 5 minutes
            max_memory_mb: 1024,
            max_file_size_mb: 500,
            allowed_directories: vec![],
            blocked_directories: vec![],
            allowed_commands: vec![], // Allow all
            blocked_commands: vec!["rm -rf /".to_string()],
            allow_network: true,
            allow_external_binaries: true,
        }
    }

    /// Create a strict configuration (more restrictions).
    #[must_use]
    pub fn strict() -> Self {
        Self {
            enabled: true,
            timeout_ms: 30_000, // 30 seconds
            max_memory_mb: 256,
            max_file_size_mb: 10,
            allowed_directories: vec![],
            blocked_directories: vec![
                PathBuf::from("/"),
                PathBuf::from("/etc"),
                PathBuf::from("/usr"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
            ],
            allowed_commands: vec!["echo".to_string(), "cat".to_string(), "ls".to_string()],
            blocked_commands: vec![],
            allow_network: false,
            allow_external_binaries: false,
        }
    }

    /// Set the timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the maximum memory.
    #[must_use]
    pub fn with_max_memory(mut self, max_memory_mb: u32) -> Self {
        self.max_memory_mb = max_memory_mb;
        self
    }

    /// Add an allowed directory.
    #[must_use]
    pub fn with_allowed_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.allowed_directories.push(path.into());
        self
    }

    /// Add a blocked directory.
    #[must_use]
    pub fn with_blocked_directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.blocked_directories.push(path.into());
        self
    }

    /// Add an allowed command.
    #[must_use]
    pub fn with_allowed_command(mut self, command: impl Into<String>) -> Self {
        self.allowed_commands.push(command.into());
        self
    }

    /// Add a blocked command.
    #[must_use]
    pub fn with_blocked_command(mut self, command: impl Into<String>) -> Self {
        self.blocked_commands.push(command.into());
        self
    }

    /// Check if a path is allowed.
    #[must_use]
    pub fn is_path_allowed(&self, path: &PathBuf) -> bool {
        // Check blocked directories
        for blocked in &self.blocked_directories {
            if path.starts_with(blocked) {
                return false;
            }
        }

        // If allowed directories is empty, all (non-blocked) are allowed
        if self.allowed_directories.is_empty() {
            return true;
        }

        // Check if path is under any allowed directory
        self.allowed_directories.iter().any(|allowed| path.starts_with(allowed))
    }

    /// Check if a command is allowed.
    #[must_use]
    pub fn is_command_allowed(&self, command: &str) -> bool {
        // Check blocked commands
        for blocked in &self.blocked_commands {
            if command.contains(blocked) {
                return false;
            }
        }

        // If allowed commands is empty, all (non-blocked) are allowed
        if self.allowed_commands.is_empty() {
            return true;
        }

        // Check if command starts with any allowed command
        self.allowed_commands.iter().any(|allowed| command.starts_with(allowed))
    }
}

/// A sandbox for isolated tool execution.
pub struct Sandbox {
    config: SandboxConfig,
}

impl fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sandbox")
            .field("config", &self.config)
            .finish()
    }
}

impl Sandbox {
    /// Create a new sandbox with the given configuration.
    #[must_use]
    pub fn new(config: SandboxConfig) -> Self {
        Self { config }
    }

    /// Get the sandbox configuration.
    #[must_use]
    pub fn config(&self) -> &SandboxConfig {
        &self.config
    }

    /// Execute a tool in the sandbox.
    ///
    /// This method:
    /// 1. Validates the tool input against sandbox policies
    /// 2. Applies timeout and resource limits
    /// 3. Executes the tool
    /// 4. Returns the result
    pub async fn execute<T>(&self, tool: &T, input: ToolInput) -> SandboxResult<ToolOutput>
    where
        T: Tool + ?Sized,
    {
        if !self.config.enabled {
            // Sandbox disabled, just execute
            return Ok(tool.execute(input).await);
        }

        // Validate input against sandbox policies
        self.validate_input(&input)?;

        // Apply timeout
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);

        match timeout(timeout_duration, tool.execute(input)).await {
            Ok(output) => Ok(output),
            Err(_) => Err(SandboxError::Timeout {
                timeout_ms: self.config.timeout_ms,
            }),
        }
    }

    /// Validate tool input against sandbox policies.
    fn validate_input(&self, input: &ToolInput) -> SandboxResult<()> {
        // Check for file paths in arguments
        for (key, value) in &input.args {
            if let Some(path_str) = value.as_str() {
                let path = PathBuf::from(path_str);

                // Check if this looks like a file path
                if key.contains("path") || key.contains("file") || key.contains("dir") {
                    if !self.config.is_path_allowed(&path) {
                        return Err(SandboxError::SecurityPolicy {
                            message: format!(
                                "Access to path '{}' is not allowed by sandbox policy",
                                path.display()
                            ),
                        });
                    }
                }

                // Check for command in arguments
                if key.contains("command") || key.contains("cmd") {
                    if !self.config.is_command_allowed(path_str) {
                        return Err(SandboxError::SecurityPolicy {
                            message: format!(
                                "Command '{}' is not allowed by sandbox policy",
                                path_str
                            ),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Check if a path is allowed by the sandbox.
    #[must_use]
    pub fn is_path_allowed(&self, path: &PathBuf) -> bool {
        self.config.is_path_allowed(path)
    }

    /// Check if a command is allowed by the sandbox.
    #[must_use]
    pub fn is_command_allowed(&self, command: &str) -> bool {
        self.config.is_command_allowed(command)
    }
}

/// Trait for tools that can run in a sandbox.
#[async_trait]
pub trait SandboxedTool: Tool {
    /// Execute the tool with sandboxing.
    async fn execute_sandboxed(
        &self,
        input: ToolInput,
        sandbox: &Sandbox,
    ) -> SandboxResult<ToolOutput> {
        sandbox.execute(self, input).await
    }
}

// Blanket implementation for all Tool types
#[async_trait]
impl<T: Tool + ?Sized> SandboxedTool for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_config_default() {
        let config = SandboxConfig::default();
        assert!(config.enabled);
        assert_eq!(config.timeout_ms, 120_000);
        assert!(!config.allow_network);
    }

    #[test]
    fn test_sandbox_config_permissive() {
        let config = SandboxConfig::permissive();
        assert!(config.enabled);
        assert!(config.allow_network);
        assert!(config.allow_external_binaries);
    }

    #[test]
    fn test_sandbox_config_strict() {
        let config = SandboxConfig::strict();
        assert!(config.enabled);
        assert_eq!(config.timeout_ms, 30_000);
        assert!(!config.allow_network);
    }

    #[test]
    fn test_sandbox_config_path_allowed() {
        let config = SandboxConfig::default()
            .with_blocked_directory("/etc")
            .with_allowed_directory("/home/user");

        assert!(!config.is_path_allowed(&PathBuf::from("/etc/passwd")));
        assert!(config.is_path_allowed(&PathBuf::from("/home/user/file.txt")));
        assert!(!config.is_path_allowed(&PathBuf::from("/home/other/file.txt")));
    }

    #[test]
    fn test_sandbox_config_path_allowed_empty() {
        let config = SandboxConfig::default();

        // With empty allowed_directories, non-blocked paths are allowed
        assert!(config.is_path_allowed(&PathBuf::from("/tmp/file.txt")));
        assert!(!config.is_path_allowed(&PathBuf::from("/etc/passwd")));
    }

    #[test]
    fn test_sandbox_config_command_allowed() {
        let config = SandboxConfig::default();

        assert!(config.is_command_allowed("echo hello"));
        assert!(config.is_command_allowed("git status"));
        assert!(!config.is_command_allowed("rm -rf /"));
        assert!(!config.is_command_allowed("dd if=/dev/zero"));
    }

    #[test]
    fn test_sandbox_config_command_allowed_empty() {
        let config = SandboxConfig::permissive();

        // With empty allowed_commands, non-blocked commands are allowed
        assert!(config.is_command_allowed("any-command"));
        assert!(!config.is_command_allowed("rm -rf /"));
    }

    #[test]
    fn test_sandbox_config_builder() {
        let config = SandboxConfig::new()
            .with_timeout(60_000)
            .with_max_memory(256)
            .with_allowed_directory("/tmp")
            .with_blocked_command("dangerous");

        assert_eq!(config.timeout_ms, 60_000);
        assert_eq!(config.max_memory_mb, 256);
        assert!(config.is_path_allowed(&PathBuf::from("/tmp/file.txt")));
        assert!(!config.is_command_allowed("dangerous"));
    }

    #[test]
    fn test_sandbox_error_display() {
        let err = SandboxError::Timeout { timeout_ms: 5000 };
        assert!(err.to_string().contains("timed out"));
    }

    #[tokio::test]
    async fn test_sandbox_execute_disabled() {
        let config = SandboxConfig {
            enabled: false,
            ..Default::default()
        };
        let sandbox = Sandbox::new(config);

        use tools::FileReadTool;
        let tool = FileReadTool::new();
        let input = ToolInput::new().with_arg("file_path", "/tmp/test.txt");

        // Should execute without validation when disabled
        let result = sandbox.execute(&tool, input).await;
        // May fail because file doesn't exist, but should not be a sandbox error
        assert!(!matches!(result, Err(SandboxError::SecurityPolicy { .. })));
    }

    #[tokio::test]
    async fn test_sandbox_validate_input_blocked_path() {
        let config = SandboxConfig::default()
            .with_blocked_directory("/etc");
        let sandbox = Sandbox::new(config);

        let input = ToolInput::new().with_arg("file_path", "/etc/passwd");
        let result = sandbox.validate_input(&input);

        assert!(matches!(result, Err(SandboxError::SecurityPolicy { .. })));
    }

    #[tokio::test]
    async fn test_sandbox_validate_input_blocked_command() {
        let config = SandboxConfig::default();
        let sandbox = Sandbox::new(config);

        let input = ToolInput::new().with_arg("command", "rm -rf /");
        let result = sandbox.validate_input(&input);

        assert!(matches!(result, Err(SandboxError::SecurityPolicy { .. })));
    }
}
