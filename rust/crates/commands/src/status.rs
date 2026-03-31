//! Status command for displaying session information.
//!
//! Shows current session details including:
//! - Current working directory
//! - Session ID
//! - Tool registry count
//! - Active configuration

use std::collections::HashMap;
use std::sync::Arc;

use crate::{Command, CommandContext, CommandMetadata, CommandOutput, CommandResult};

/// Mock registry for counting tools - in real implementation this would come from context
#[derive(Debug, Clone, Default)]
pub struct ToolRegistryInfo {
    /// Number of registered tools
    pub tool_count: usize,
    /// Names of registered tools
    pub tool_names: Vec<String>,
}

/// Status command for session information
#[derive(Debug, Clone)]
pub struct StatusCommand {
    metadata: CommandMetadata,
    /// Optional registry info (would be injected in real implementation)
    registry_info: Arc<std::sync::Mutex<ToolRegistryInfo>>,
}

impl StatusCommand {
    /// Create a new status command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("status", "Display current session information")
                .with_alias("info")
                .with_alias("whoami")
                .with_usage("status [--tools] [--env]"),
            registry_info: Arc::new(std::sync::Mutex::new(ToolRegistryInfo::default())),
        }
    }

    /// Create a status command with registry info
    #[must_use]
    pub fn with_registry_info(registry_info: ToolRegistryInfo) -> Self {
        Self {
            metadata: CommandMetadata::new("status", "Display current session information")
                .with_alias("info")
                .with_alias("whoami")
                .with_usage("status [--tools] [--env]"),
            registry_info: Arc::new(std::sync::Mutex::new(registry_info)),
        }
    }

    /// Get current session ID (mock implementation)
    fn get_session_id(&self) -> String {
        // In a real implementation, this would come from the runtime context
        format!("session-{}", std::process::id())
    }

    /// Get system information
    fn get_system_info(&self) -> HashMap<String, String> {
        let mut info = HashMap::new();

        info.insert(
            "os".to_string(),
            std::env::consts::OS.to_string(),
        );
        info.insert(
            "arch".to_string(),
            std::env::consts::ARCH.to_string(),
        );
        info.insert(
            "family".to_string(),
            std::env::consts::FAMILY.to_string(),
        );

        if let Ok(hostname) = std::env::var("HOSTNAME").or_else(|_| std::env::var("COMPUTERNAME")) {
            info.insert("hostname".to_string(), hostname);
        }

        info
    }

    /// Format status output
    fn format_status(&self, ctx: &CommandContext, show_tools: bool, show_env: bool) -> String {
        let mut lines = vec!["Session Status".to_string(), "=".repeat(40)];

        // Basic session info
        lines.push(format!("Session ID: {}", self.get_session_id()));
        lines.push(format!("Working Directory: {}", ctx.cwd.display()));
        lines.push(format!("Process ID: {}", std::process::id()));

        // System info
        lines.push(String::new());
        lines.push("System:".to_string());
        let sys_info = self.get_system_info();
        lines.push(format!("  OS: {}", sys_info.get("os").unwrap_or(&"unknown".to_string())));
        lines.push(format!(
            "  Architecture: {}",
            sys_info.get("arch").unwrap_or(&"unknown".to_string())
        ));
        if let Some(hostname) = sys_info.get("hostname") {
            lines.push(format!("  Hostname: {}", hostname));
        }

        // Tool registry info
        if show_tools {
            lines.push(String::new());
            lines.push("Tool Registry:".to_string());

            if let Ok(registry) = self.registry_info.lock() {
                lines.push(format!("  Registered Tools: {}", registry.tool_count));
                if !registry.tool_names.is_empty() {
                    lines.push("  Tools:".to_string());
                    for name in &registry.tool_names {
                        lines.push(format!("    - {}", name));
                    }
                }
            }
        }

        // Environment info
        if show_env {
            lines.push(String::new());
            lines.push("Environment:".to_string());

            let important_vars = ["HOME", "USER", "SHELL", "EDITOR", "TERM"];
            for var in &important_vars {
                match std::env::var(var) {
                    Ok(value) => lines.push(format!("  {}: {}", var, value)),
                    Err(_) => lines.push(format!("  {}: <not set>", var)),
                }
            }
        }

        // Command arguments if any
        if !ctx.args.is_empty() {
            lines.push(String::new());
            lines.push(format!("Arguments: {:?}", ctx.args));
        }

        // Flags if any
        if !ctx.flags.is_empty() {
            lines.push(String::new());
            lines.push(format!("Flags: {:?}", ctx.flags));
        }

        lines.join("\n")
    }
}

impl Default for StatusCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Command for StatusCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let show_tools = ctx.flags.contains_key("tools") || ctx.flags.contains_key("t");
        let show_env = ctx.flags.contains_key("env") || ctx.flags.contains_key("e");

        let output = self.format_status(ctx, show_tools, show_env);
        Ok(CommandOutput::Text(output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_command_metadata() {
        let cmd = StatusCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "status");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"info".to_string()));
        assert!(meta.aliases.contains(&"whoami".to_string()));
    }

    #[test]
    fn test_status_matches() {
        let cmd = StatusCommand::new();

        assert!(cmd.matches("status"));
        assert!(cmd.matches("info"));
        assert!(cmd.matches("whoami"));
        assert!(!cmd.matches("invalid"));
    }

    #[tokio::test]
    async fn test_status_execution() {
        let cmd = StatusCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Session Status"));
                assert!(text.contains("Working Directory"));
                assert!(text.contains("System:"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_status_with_tools_flag() {
        let registry_info = ToolRegistryInfo {
            tool_count: 3,
            tool_names: vec!["bash".to_string(), "read".to_string(), "write".to_string()],
        };

        let cmd = StatusCommand::with_registry_info(registry_info);
        let mut ctx = CommandContext::new("/tmp");
        ctx.flags.insert("tools".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Tool Registry"));
                assert!(text.contains("Registered Tools: 3"));
                assert!(text.contains("bash"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_status_with_env_flag() {
        let cmd = StatusCommand::new();
        let mut ctx = CommandContext::new("/tmp");
        ctx.flags.insert("env".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Environment:"));
                // Should contain some environment variables
                assert!(text.contains("HOME:") || text.contains("<not set>"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[test]
    fn test_get_session_id() {
        let cmd = StatusCommand::new();
        let session_id = cmd.get_session_id();

        assert!(session_id.starts_with("session-"));
    }

    #[test]
    fn test_get_system_info() {
        let cmd = StatusCommand::new();
        let info = cmd.get_system_info();

        assert!(info.contains_key("os"));
        assert!(info.contains_key("arch"));
        assert!(info.contains_key("family"));
    }
}
