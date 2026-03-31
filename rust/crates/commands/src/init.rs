//! `/init` - Initialize project command.
//!
//! This command initializes a new project structure including:
//! - Create necessary config files
//! - Set up initial state
//! - Create project directory structure

use async_trait::async_trait;
use std::path::Path;

use crate::{Command, CommandContext, CommandError, CommandMetadata, CommandOutput, CommandResult};

/// `/init` - Initialize project command
pub struct InitCommand {
    metadata: CommandMetadata,
}

impl InitCommand {
    /// Create a new init command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("init", "Initialize a new project")
                .with_aliases(vec!["initialize".to_string(), "new".to_string()])
                .with_usage("/init [project_name] [--template=<template>]"),
        }
    }

    /// Initialize a new project
    async fn init_project(&self, cwd: &Path, project_name: Option<&str>) -> CommandResult {
        let project_dir = if let Some(name) = project_name {
            let dir = cwd.join(name);
            if dir.exists() {
                return Err(CommandError::ExecutionFailed(format!(
                    "Directory '{}' already exists",
                    dir.display()
                )));
            }
            tokio::fs::create_dir_all(&dir)
                .await
                .map_err(|e| CommandError::Io(e))?;
            dir
        } else {
            cwd.to_path_buf()
        };

        // Create basic directory structure
        let dirs = vec![
            project_dir.join("src"),
            project_dir.join("tests"),
            project_dir.join("docs"),
        ];

        for dir in &dirs {
            tokio::fs::create_dir_all(dir)
                .await
                .map_err(|e| CommandError::Io(e))?;
        }

        // Create README.md
        let readme_path = project_dir.join("README.md");
        let project_display = project_name.unwrap_or("Project");
        let readme_content = format!(
            r#"# {}

This project was initialized with Claude Code.

## Getting Started

- Source code lives in `src/`
- Tests live in `tests/`
- Documentation lives in `docs/`

## Commands

Use the following commands to work with this project:
- `/status` - Check project status
- `/doctor` - Diagnose issues
- `/compact` - Compact conversation history
"#,
            project_display
        );

        tokio::fs::write(&readme_path, readme_content)
            .await
            .map_err(|e| CommandError::Io(e))?;

        // Create .claude directory for Claude Code specific files
        let claude_dir = project_dir.join(".claude");
        tokio::fs::create_dir_all(&claude_dir)
            .await
            .map_err(|e| CommandError::Io(e))?;

        // Create a simple config file
        let config_path = claude_dir.join("config.json");
        let config = serde_json::json!({
            "project_name": project_display,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "version": "0.1.0",
        });

        tokio::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap())
            .await
            .map_err(|e| CommandError::Io(e))?;

        // Initialize git if requested and not already a repo
        let git_dir = project_dir.join(".git");
        if !git_dir.exists() {
            let output = tokio::process::Command::new("git")
                .arg("init")
                .current_dir(&project_dir)
                .output()
                .await
                .map_err(|e| CommandError::Git(format!("Failed to initialize git: {e}")))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CommandError::Git(format!(
                    "Git init failed: {stderr}"
                )));
            }
        }

        let project_name_str = project_name.unwrap_or("current directory");
        Ok(CommandOutput::Text(format!(
            "Initialized project '{}' in {}\n\nCreated:\n  - README.md\n  - src/\n  - tests/\n  - docs/\n  - .claude/config.json\n  - .git/",
            project_name_str,
            project_dir.display()
        )))
    }

    /// Check if current directory is already initialized
    fn check_initialized(&self, cwd: &Path) -> bool {
        cwd.join(".claude").exists() || cwd.join(".git").exists()
    }
}

impl Default for InitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for InitCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // Parse project name from args
        let project_name = ctx.args.first().map(|s| s.as_str());

        // Check for --force flag
        let force = ctx.flags.contains_key("force") || ctx.flags.contains_key("f");

        // Check if already initialized (unless forcing)
        if project_name.is_none() && !force && self.check_initialized(&ctx.cwd) {
            return Ok(CommandOutput::Text(
                "Project already initialized in this directory.\nUse --force to reinitialize."
                    .to_string(),
            ));
        }

        self.init_project(&ctx.cwd, project_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandContext;

    #[test]
    fn test_init_command_metadata() {
        let cmd = InitCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "init");
        assert!(meta.aliases.contains(&"initialize".to_string()));
        assert!(meta.aliases.contains(&"new".to_string()));
    }

    #[test]
    fn test_init_command_matches() {
        let cmd = InitCommand::new();
        assert!(cmd.matches("init"));
        assert!(cmd.matches("initialize"));
        assert!(cmd.matches("new"));
        assert!(!cmd.matches("create"));
    }

    #[tokio::test]
    async fn test_init_command_already_initialized() {
        let cmd = InitCommand::new();
        // Use temp dir which likely doesn't have .claude or .git
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        // Should succeed in /tmp (no .claude/.git)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_init_project_with_name() {
        let cmd = InitCommand::new();
        let temp_dir = std::env::temp_dir().join(format!("test-init-{}", std::process::id()));
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let ctx = CommandContext::new(&temp_dir).with_args(vec!["test-project".to_string()]);

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        // Cleanup
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
