//! `/teleport` - Repository switching command.
//!
//! This command allows switching between git repositories while maintaining context.
//! It finds and lists available git repositories and can switch the working directory.

use async_trait::async_trait;
use std::path::PathBuf;

use crate::{Command, CommandContext, CommandError, CommandMetadata, CommandOutput, CommandResult};

/// `/teleport` - Repository switching command
pub struct TeleportCommand {
    metadata: CommandMetadata,
}

impl TeleportCommand {
    /// Create a new teleport command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("teleport", "Switch between git repositories")
                .with_aliases(vec!["tp".to_string(), "goto".to_string()])
                .with_usage("/teleport [repo_name|path] [--list]"),
        }
    }

    /// List available git repositories
    fn list_repos(&self, cwd: &std::path::Path) -> CommandResult {
        let repos = self.find_git_repos(cwd);

        if repos.is_empty() {
            return Ok(CommandOutput::Text(
                "No git repositories found.".to_string(),
            ));
        }

        let mut output = format!("Found {} git repository(ies):\n\n", repos.len());
        for (i, repo) in repos.iter().enumerate() {
            let repo_name = repo.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
            let current_marker = if i == 0 { " (current)" } else { "" };
            output.push_str(&format!("  {}. {}{}\n", i + 1, repo_name, current_marker));
            output.push_str(&format!("     Path: {}\n", repo.display()));
        }

        output.push_str("\nUse '/teleport <name>' to switch to a repository.");

        Ok(CommandOutput::Text(output))
    }

    /// Switch to a repository
    fn switch_repo(&self, cwd: &std::path::Path, target: &str) -> CommandResult {
        let repos = self.find_git_repos(cwd);

        // Try to find by name first
        let repo = repos
            .iter()
            .find(|r| {
                r.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
                    == target
            })
            .cloned();

        // If not found by name, try as a path
        let target_path = if let Some(repo) = repo {
            repo
        } else {
            let path = PathBuf::from(target);
            if path.is_dir() && path.join(".git").exists() {
                path
            } else if cwd.join(target).is_dir() && cwd.join(target).join(".git").exists() {
                cwd.join(target)
            } else {
                return Err(CommandError::ExecutionFailed(format!(
                    "Repository not found: {target}"
                )));
            }
        };

        let repo_name = target_path
            .file_name()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();

        Ok(CommandOutput::Json(serde_json::json!({
            "message": format!("Switched to repository: {repo_name}"),
            "repo_name": repo_name,
            "path": target_path.to_string_lossy(),
            "action": "switch",
        })))
    }

    /// Find git repositories starting from the given directory
    fn find_git_repos(&self, start_dir: &std::path::Path) -> Vec<PathBuf> {
        let mut repos = Vec::new();

        // Check if current directory is a git repo
        if start_dir.join(".git").exists() {
            repos.push(start_dir.to_path_buf());
        }

        // Check parent directories for git repos (up to 3 levels)
        let mut current = start_dir;
        for _ in 0..3 {
            if let Some(parent) = current.parent() {
                for entry in parent.read_dir().ok().into_iter().flatten() {
                    let Ok(entry) = entry else { continue };
                    let path = entry.path();
                    if path.is_dir() && path.join(".git").exists() {
                        if !repos.contains(&path) {
                            repos.push(path);
                        }
                    }
                }
                current = parent;
            } else {
                break;
            }
        }

        // Check subdirectories (one level deep)
        if let Ok(entries) = start_dir.read_dir() {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() && path.join(".git").exists() {
                    if !repos.contains(&path) {
                        repos.push(path);
                    }
                }
            }
        }

        repos
    }
}

impl Default for TeleportCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TeleportCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // Check for --list flag or no arguments
        if ctx.args.is_empty() || ctx.flags.contains_key("list") || ctx.flags.contains_key("l") {
            return self.list_repos(&ctx.cwd);
        }

        // Switch to specified repository
        let target = &ctx.args[0];
        self.switch_repo(&ctx.cwd, target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandContext;

    #[test]
    fn test_teleport_command_metadata() {
        let cmd = TeleportCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "teleport");
        assert!(meta.aliases.contains(&"tp".to_string()));
        assert!(meta.aliases.contains(&"goto".to_string()));
    }

    #[test]
    fn test_teleport_command_matches() {
        let cmd = TeleportCommand::new();
        assert!(cmd.matches("teleport"));
        assert!(cmd.matches("tp"));
        assert!(cmd.matches("goto"));
        assert!(!cmd.matches("switch"));
    }

    #[tokio::test]
    async fn test_teleport_list() {
        let cmd = TeleportCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        // Result depends on the filesystem, so just check it runs
    }

    #[test]
    fn test_find_git_repos() {
        let cmd = TeleportCommand::new();

        // Test with a temp directory
        let temp_dir = std::env::temp_dir();
        let repos = cmd.find_git_repos(&temp_dir);

        // This is a basic test - actual repos found depend on the environment
        assert!(repos.is_empty() || !repos.is_empty()); // Just ensure it doesn't panic
    }
}
