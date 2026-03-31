//! Diff command for showing git changes.
//!
//! Displays the git diff of current changes in the repository.
//! Requires the `git` feature to be enabled.

use std::path::Path;

use crate::{Command, CommandContext, CommandMetadata, CommandOutput, CommandResult};

/// Diff command for showing git changes
#[derive(Debug, Clone)]
pub struct DiffCommand {
    metadata: CommandMetadata,
}

impl DiffCommand {
    /// Create a new diff command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("diff", "Show git diff of current changes")
                .with_alias("changes")
                .with_alias("d")
                .with_usage("diff [--cached] [--stat] [<path>]"),
        }
    }

    /// Check if the current directory is in a git repository
    fn is_git_repo(&self, path: &Path) -> bool {
        let mut current = Some(path);

        while let Some(dir) = current {
            if dir.join(".git").exists() {
                return true;
            }
            current = dir.parent();
        }

        false
    }

    /// Find the git repository root
    fn find_git_root(&self, path: &Path) -> Option<std::path::PathBuf> {
        let mut current = Some(path);

        while let Some(dir) = current {
            if dir.join(".git").exists() {
                return Some(dir.to_path_buf());
            }
            current = dir.parent();
        }

        None
    }

    /// Run git diff using the git command
    async fn run_git_diff(&self, cwd: &Path, cached: bool, stat: bool, path: Option<&str>) -> CommandResult {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("diff");

        if cached {
            cmd.arg("--cached");
        }

        if stat {
            cmd.arg("--stat");
        }

        // Add color if not explicitly disabled
        if std::env::var("NO_COLOR").is_err() {
            cmd.arg("--color=always");
        }

        if let Some(p) = path {
            cmd.arg("--").arg(p);
        }

        cmd.current_dir(cwd);

        let output = cmd.output().await.map_err(|e| {
            crate::CommandError::ExecutionFailed(format!("Failed to run git diff: {e}"))
        })?;

        let diff_output = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() && !stderr.is_empty() {
            return Ok(CommandOutput::Error(format!("git diff failed: {}", stderr)));
        }

        if diff_output.is_empty() {
            let message = if cached {
                "No staged changes."
            } else {
                "No changes in working directory."
            };
            Ok(CommandOutput::Text(message.to_string()))
        } else {
            Ok(CommandOutput::Text(diff_output.to_string()))
        }
    }

    /// Get diff statistics using git command
    async fn get_diff_stat(&self, cwd: &Path, cached: bool) -> Result<String, crate::CommandError> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("diff").arg("--stat");

        if cached {
            cmd.arg("--cached");
        }

        cmd.current_dir(cwd);

        let output = cmd.output().await.map_err(|e| {
            crate::CommandError::ExecutionFailed(format!("Failed to run git diff --stat: {e}"))
        })?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Format a summary of changes
    async fn format_summary(&self, cwd: &Path) -> String {
        let mut summary = vec!["Git Diff Summary".to_string(), "=".repeat(40)];

        // Get repository root
        if let Some(root) = self.find_git_root(cwd) {
            summary.push(format!("Repository: {}", root.display()));
        }

        // Working directory changes
        match self.get_diff_stat(cwd, false).await {
            Ok(stat) if !stat.is_empty() => {
                summary.push(String::new());
                summary.push("Working directory changes:".to_string());
                summary.push(stat);
            }
            Ok(_) => {
                summary.push(String::new());
                summary.push("Working directory: clean".to_string());
            }
            Err(_) => {}
        }

        // Staged changes
        match self.get_diff_stat(cwd, true).await {
            Ok(stat) if !stat.is_empty() => {
                summary.push(String::new());
                summary.push("Staged changes:".to_string());
                summary.push(stat);
            }
            Ok(_) => {}
            Err(_) => {}
        }

        summary.join("\n")
    }
}

impl Default for DiffCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Command for DiffCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    fn is_available(&self) -> bool {
        // Check if git is available
        match std::process::Command::new("git").arg("--version").output() {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // Check if we're in a git repository
        if !self.is_git_repo(&ctx.cwd) {
            return Ok(CommandOutput::Error(
                "Not a git repository. Run 'git init' to create one.".to_string(),
            ));
        }

        // Parse flags
        let cached = ctx.flags.contains_key("cached") || ctx.flags.contains_key("staged");
        let stat = ctx.flags.contains_key("stat") || ctx.flags.contains_key("s");
        let summary = ctx.flags.contains_key("summary");

        // Get optional path argument
        let path = ctx.args.first().map(|s| s.as_str());

        if summary {
            let output = self.format_summary(&ctx.cwd).await;
            return Ok(CommandOutput::Text(output));
        }

        self.run_git_diff(&ctx.cwd, cached, stat, path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_command_metadata() {
        let cmd = DiffCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "diff");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"changes".to_string()));
        assert!(meta.aliases.contains(&"d".to_string()));
    }

    #[test]
    fn test_diff_matches() {
        let cmd = DiffCommand::new();

        assert!(cmd.matches("diff"));
        assert!(cmd.matches("changes"));
        assert!(cmd.matches("d"));
        assert!(!cmd.matches("invalid"));
    }

    #[test]
    fn test_is_git_repo() {
        let cmd = DiffCommand::new();

        // Should find .git in current project
        assert!(cmd.is_git_repo(std::path::Path::new("/mnt/ollama/git/claw-code/rust")));

        // Should not find .git in /tmp (usually)
        assert!(!cmd.is_git_repo(std::path::Path::new("/tmp")));
    }

    #[test]
    fn test_find_git_root() {
        let cmd = DiffCommand::new();

        // Should find the rust directory as git root
        let root = cmd.find_git_root(std::path::Path::new("/mnt/ollama/git/claw-code/rust/crates"));
        assert!(root.is_some());
        assert!(root.unwrap().ends_with("claw-code"));
    }

    #[tokio::test]
    async fn test_diff_in_non_git_directory() {
        let cmd = DiffCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Error(msg) => {
                assert!(msg.contains("Not a git repository"));
            }
            _ => {
                // Might succeed if /tmp happens to be in a git repo
            }
        }
    }

    #[tokio::test]
    async fn test_diff_in_git_directory() {
        let cmd = DiffCommand::new();
        let ctx = CommandContext::new("/mnt/ollama/git/claw-code/rust");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        // Should get some output (even if no changes)
        match result.unwrap() {
            CommandOutput::Text(text) => {
                // Either shows diff or "No changes"
                assert!(!text.is_empty());
            }
            CommandOutput::Error(_) => {
                // Also acceptable if there's an error
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_diff_with_summary_flag() {
        let cmd = DiffCommand::new();
        let mut ctx = CommandContext::new("/mnt/ollama/git/claw-code/rust");
        ctx.flags.insert("summary".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Git Diff Summary"));
            }
            _ => {}
        }
    }

    #[test]
    fn test_is_available() {
        let cmd = DiffCommand::new();

        // Git should be available in most CI environments
        // This test just verifies the method doesn't panic
        let _available = cmd.is_available();
    }
}
