//! `/skills` - Skill system command.
//!
//! This command provides skill management functionality including:
//! - List available skills
//! - Enable/disable skills
//! - Show skill details

use async_trait::async_trait;

use crate::{Command, CommandContext, CommandError, CommandMetadata, CommandOutput, CommandResult};

/// Available skill categories
const SKILL_CATEGORIES: &[(&str, &str)] = &[
    ("file", "File operations (read, write, edit, search)"),
    ("git", "Git operations (commit, diff, review)"),
    ("system", "System operations (bash, doctor)"),
    ("web", "Web operations (fetch, search)"),
    ("mcp", "MCP server integration"),
    ("lsp", "LSP language server integration"),
    ("agent", "Agent and task management"),
];

/// `/skills` - Skill system command
pub struct SkillsCommand {
    metadata: CommandMetadata,
}

impl SkillsCommand {
    /// Create a new skills command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("skills", "Manage available skills and capabilities")
                .with_aliases(vec!["skill".to_string()])
                .with_usage("/skills [list|show|enable|disable] [skill_name]"),
        }
    }

    /// List available skills
    fn list_skills(&self) -> CommandResult {
        let mut output = "Available skill categories:\n\n".to_string();

        for (name, description) in SKILL_CATEGORIES {
            let status = "✓ enabled";
            output.push_str(&format!("  {}: {} ({}\n", name, description, status));
        }

        output.push_str("\nNote: Skills are automatically enabled based on context.");

        Ok(CommandOutput::Text(output))
    }

    /// Show skill details
    fn show_skill(&self, skill_name: &str) -> CommandResult {
        let skill = SKILL_CATEGORIES
            .iter()
            .find(|(name, _)| *name == skill_name)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("Skill not found: {skill_name}")))?;

        let details = serde_json::json!({
            "name": skill.0,
            "description": skill.1,
            "status": "enabled",
            "tools": get_tools_for_skill(skill.0),
        });

        Ok(CommandOutput::Json(details))
    }

    /// Enable a skill (stub - skills are always enabled)
    fn enable_skill(&self, skill_name: &str) -> CommandResult {
        if SKILL_CATEGORIES.iter().any(|(name, _)| *name == skill_name) {
            Ok(CommandOutput::Text(format!("Skill '{skill_name}' is now enabled")))
        } else {
            Err(CommandError::ExecutionFailed(format!(
                "Unknown skill: {skill_name}"
            )))
        }
    }

    /// Disable a skill (stub)
    fn disable_skill(&self, skill_name: &str) -> CommandResult {
        if SKILL_CATEGORIES.iter().any(|(name, _)| *name == skill_name) {
            Ok(CommandOutput::Text(format!(
                "Skill '{skill_name}' has been disabled (stub)"
            )))
        } else {
            Err(CommandError::ExecutionFailed(format!(
                "Unknown skill: {skill_name}"
            )))
        }
    }
}

impl Default for SkillsCommand {
    fn default() -> Self {
        Self::new()
    }
}

/// Get tools associated with a skill category
fn get_tools_for_skill(skill: &str) -> Vec<String> {
    match skill {
        "file" => vec![
            "FileRead".to_string(),
            "FileWrite".to_string(),
            "FileEdit".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
            "NotebookEdit".to_string(),
        ],
        "git" => vec!["Commit".to_string(), "Diff".to_string(), "Review".to_string()],
        "system" => vec!["Bash".to_string(), "Doctor".to_string()],
        "web" => vec!["WebFetch".to_string(), "WebSearch".to_string()],
        "mcp" => vec!["Mcp".to_string(), "McpRegistry".to_string()],
        "lsp" => vec!["Lsp".to_string()],
        "agent" => vec![
            "Agent".to_string(),
            "SendMessage".to_string(),
            "TaskCreate".to_string(),
            "TaskUpdate".to_string(),
        ],
        _ => vec![],
    }
}

#[async_trait]
impl Command for SkillsCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        // Parse subcommand from args
        let subcommand = ctx.args.first().map(|s| s.as_str()).unwrap_or("list");

        match subcommand {
            "list" | "ls" => self.list_skills(),
            "show" | "get" | "info" => {
                let skill_name = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Skill name required".to_string()))?;
                self.show_skill(skill_name)
            }
            "enable" | "on" => {
                let skill_name = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Skill name required".to_string()))?;
                self.enable_skill(skill_name)
            }
            "disable" | "off" => {
                let skill_name = ctx
                    .args
                    .get(1)
                    .ok_or_else(|| CommandError::InvalidArguments("Skill name required".to_string()))?;
                self.disable_skill(skill_name)
            }
            _ => {
                // If no subcommand but an arg is provided, treat it as a skill name to show
                if ctx.args.len() == 1 {
                    self.show_skill(subcommand)
                } else {
                    Err(CommandError::InvalidArguments(format!(
                        "Unknown subcommand: {subcommand}. Use: list, show, enable, disable"
                    )))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CommandContext;

    #[test]
    fn test_skills_command_metadata() {
        let cmd = SkillsCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "skills");
        assert!(meta.aliases.contains(&"skill".to_string()));
    }

    #[test]
    fn test_skills_command_matches() {
        let cmd = SkillsCommand::new();
        assert!(cmd.matches("skills"));
        assert!(cmd.matches("skill"));
        assert!(!cmd.matches("task"));
    }

    #[tokio::test]
    async fn test_skills_list() {
        let cmd = SkillsCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Available skill categories"));
                assert!(text.contains("file"));
                assert!(text.contains("git"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_skills_show() {
        let cmd = SkillsCommand::new();
        let ctx = CommandContext::new("/tmp").with_args(vec!["show".to_string(), "file".to_string()]);

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Json(data) => {
                assert_eq!(data.get("name").unwrap().as_str().unwrap(), "file");
            }
            _ => panic!("Expected JSON output"),
        }
    }

    #[tokio::test]
    async fn test_skills_enable() {
        let cmd = SkillsCommand::new();
        let ctx = CommandContext::new("/tmp").with_args(vec!["enable".to_string(), "git".to_string()]);

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("enabled"));
            }
            _ => panic!("Expected text output"),
        }
    }
}
