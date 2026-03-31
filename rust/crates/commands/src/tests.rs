//! Comprehensive tests for the commands crate.
//!
//! These tests cover all commands, the registry, and command dispatch.

#[cfg(test)]
mod command_tests {
    use crate::*;
    use std::collections::HashMap;

    // =========================================================================
    // CommandRegistry Tests
    // =========================================================================

    #[test]
    fn test_registry_empty_by_default() {
        let registry = CommandRegistry::new();
        assert!(registry.all().is_empty());
        assert!(!registry.has("help"));
    }

    #[test]
    fn test_registry_register_and_find() {
        let mut registry = CommandRegistry::new();
        registry.register(HelpCommand::simple());

        assert!(registry.has("help"));
        assert!(registry.has("?")); // alias
        assert!(!registry.has("unknown"));

        let cmd = registry.find("help");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().metadata().name, "help");
    }

    #[test]
    fn test_registry_find_by_alias() {
        let mut registry = CommandRegistry::new();
        registry.register(CommitCommand::new());

        let cmd_by_name = registry.find("commit");
        let cmd_by_alias = registry.find("c");

        assert!(cmd_by_name.is_some());
        assert!(cmd_by_alias.is_some());
    }

    #[test]
    fn test_registry_visible_commands_excludes_hidden() {
        let mut registry = CommandRegistry::new();
        registry.register(HelpCommand::simple());
        registry.register(CommitCommand::new());

        let visible = registry.visible_commands();
        assert_eq!(visible.len(), 2);

        // All should be non-hidden
        for cmd in &visible {
            assert!(!cmd.metadata().hidden);
        }
    }

    #[tokio::test]
    async fn test_registry_execute_found() {
        let mut registry = CommandRegistry::new();
        registry.register(HelpCommand::simple());

        let ctx = CommandContext::new("/tmp");
        let result = registry.execute("help", &ctx).await;

        assert!(result.is_ok());
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(!text.is_empty());
            }
            _ => panic!("Expected text output"),
        }
    }

    #[tokio::test]
    async fn test_registry_execute_not_found() {
        let registry = CommandRegistry::new();
        let ctx = CommandContext::new("/tmp");
        let result = registry.execute("nonexistent", &ctx).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            CommandError::NotFound(name) => {
                assert_eq!(name, "nonexistent");
            }
            _ => panic!("Expected NotFound error"),
        }
    }

    // =========================================================================
    // HelpCommand Tests
    // =========================================================================

    #[test]
    fn test_help_command_metadata() {
        let cmd = HelpCommand::simple();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "help");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"?".to_string()));
        assert!(meta.aliases.contains(&"h".to_string()));
    }

    #[test]
    fn test_help_matches() {
        let cmd = HelpCommand::simple();

        assert!(cmd.matches("help"));
        assert!(cmd.matches("?"));
        assert!(cmd.matches("h"));
        assert!(!cmd.matches("commit"));
    }

    #[tokio::test]
    async fn test_help_execution() {
        let cmd = HelpCommand::simple();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("help") || text.contains("commands"));
            }
            _ => panic!("Expected text output"),
        }
    }

    // =========================================================================
    // CommitCommand Tests
    // =========================================================================

    #[test]
    fn test_commit_command_metadata() {
        let cmd = CommitCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "commit");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"c".to_string()));
    }

    #[test]
    fn test_commit_matches() {
        let cmd = CommitCommand::new();

        assert!(cmd.matches("commit"));
        assert!(cmd.matches("c"));
        assert!(!cmd.matches("help"));
    }

    #[tokio::test]
    async fn test_commit_in_non_git_directory() {
        let cmd = CommitCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;

        // Should fail because /tmp is likely not a git repo
        assert!(result.is_err());
        match result.unwrap_err() {
            CommandError::Git(_) => {}
            _ => {}
        }
    }

    // =========================================================================
    // DoctorCommand Tests
    // =========================================================================

    #[test]
    fn test_doctor_command_metadata() {
        let cmd = DoctorCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "doctor");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"diag".to_string()));
        assert!(meta.aliases.contains(&"check".to_string()));
    }

    #[test]
    fn test_doctor_matches() {
        let cmd = DoctorCommand::new();

        assert!(cmd.matches("doctor"));
        assert!(cmd.matches("diag"));
        assert!(cmd.matches("check"));
        assert!(!cmd.matches("help"));
    }

    #[test]
    fn test_doctor_is_available() {
        let cmd = DoctorCommand::new();
        // Doctor should always be available
        assert!(cmd.is_available());
    }

    #[tokio::test]
    async fn test_doctor_execution() {
        let cmd = DoctorCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("✓") || text.contains("✗"));
                assert!(text.contains("Git"));
            }
            _ => panic!("Expected text output"),
        }
    }

    // =========================================================================
    // DiffCommand Tests
    // =========================================================================

    #[test]
    fn test_diff_command_metadata() {
        let cmd = DiffCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "diff");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"d".to_string()));
        assert!(meta.aliases.contains(&"changes".to_string()));
    }

    #[test]
    fn test_diff_matches() {
        let cmd = DiffCommand::new();

        assert!(cmd.matches("diff"));
        assert!(cmd.matches("d"));
        assert!(cmd.matches("changes"));
        assert!(!cmd.matches("commit"));
    }

    #[test]
    fn test_diff_is_available() {
        let cmd = DiffCommand::new();
        // Availability depends on git being installed
        // Just verify the method doesn't panic
        let _ = cmd.is_available();
    }

    #[tokio::test]
    async fn test_diff_in_non_git_directory() {
        let cmd = DiffCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        // Should return error message, not crash
        match result.unwrap() {
            CommandOutput::Error(_) => {}
            CommandOutput::Text(text) => {
                assert!(text.contains("No changes") || text.contains("Not a git repository"));
            }
            _ => {}
        }
    }

    // =========================================================================
    // CostCommand Tests
    // =========================================================================

    #[test]
    fn test_cost_command_metadata() {
        let cmd = CostCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "cost");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"price".to_string()));
        assert!(meta.aliases.contains(&"$".to_string()));
    }

    #[test]
    fn test_cost_matches() {
        let cmd = CostCommand::new();

        assert!(cmd.matches("cost"));
        assert!(cmd.matches("price"));
        assert!(cmd.matches("$"));
        assert!(!cmd.matches("help"));
    }

    #[tokio::test]
    async fn test_cost_execution() {
        let cmd = CostCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Json(data) => {
                assert!(data.get("total_cost").is_some());
                assert!(data.get("input_tokens").is_some());
                assert!(data.get("output_tokens").is_some());
            }
            _ => panic!("Expected JSON output"),
        }
    }

    // =========================================================================
    // ReviewCommand Tests
    // =========================================================================

    #[test]
    fn test_review_command_metadata() {
        let cmd = ReviewCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "review");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"r".to_string()));
        assert!(meta.aliases.contains(&"pr".to_string()));
    }

    #[test]
    fn test_review_matches() {
        let cmd = ReviewCommand::new();

        assert!(cmd.matches("review"));
        assert!(cmd.matches("r"));
        assert!(cmd.matches("pr"));
        assert!(!cmd.matches("commit"));
    }

    #[tokio::test]
    async fn test_review_execution() {
        let cmd = ReviewCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        // Should return something about reviewing or no files
        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(!text.is_empty());
            }
            _ => {}
        }
    }

    #[tokio::test]
    async fn test_review_with_staged_flag() {
        let cmd = ReviewCommand::new();
        let mut ctx = CommandContext::new("/tmp");
        ctx.flags.insert("staged".to_string(), "true".to_string());
        ctx.flags.insert("s".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // CompactCommand Tests
    // =========================================================================

    #[test]
    fn test_compact_command_metadata() {
        let cmd = CompactCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "compact");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"compress".to_string()));
        assert!(meta.aliases.contains(&"summary".to_string()));
    }

    #[test]
    fn test_compact_matches() {
        let cmd = CompactCommand::new();

        assert!(cmd.matches("compact"));
        assert!(cmd.matches("compress"));
        assert!(cmd.matches("summary"));
        assert!(!cmd.matches("review"));
    }

    #[tokio::test]
    async fn test_compact_execution() {
        let cmd = CompactCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("compacted") || text.contains("Conversation"));
            }
            _ => panic!("Expected text output"),
        }
    }

    // =========================================================================
    // StatusCommand Tests
    // =========================================================================

    #[test]
    fn test_status_command_metadata() {
        let cmd = StatusCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "status");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"info".to_string()));
        assert!(meta.aliases.contains(&"state".to_string()));
    }

    #[test]
    fn test_status_matches() {
        let cmd = StatusCommand::new();

        assert!(cmd.matches("status"));
        assert!(cmd.matches("info"));
        assert!(cmd.matches("state"));
        assert!(!cmd.matches("help"));
    }

    #[tokio::test]
    async fn test_status_execution() {
        let cmd = StatusCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Json(data) => {
                assert!(data.get("session").is_some());
            }
            _ => panic!("Expected JSON output"),
        }
    }

    #[tokio::test]
    async fn test_status_with_tools_flag() {
        let cmd = StatusCommand::new();
        let mut ctx = CommandContext::new("/tmp");
        ctx.flags.insert("tools".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_status_with_env_flag() {
        let cmd = StatusCommand::new();
        let mut ctx = CommandContext::new("/tmp");
        ctx.flags.insert("env".to_string(), "true".to_string());

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());
    }

    // =========================================================================
    // CommandContext Tests
    // =========================================================================

    #[test]
    fn test_context_creation() {
        let ctx = CommandContext::new("/tmp");
        assert_eq!(ctx.cwd, std::path::PathBuf::from("/tmp"));
        assert!(ctx.args.is_empty());
        assert!(ctx.flags.is_empty());
    }

    #[test]
    fn test_context_with_args() {
        let ctx = CommandContext::new("/tmp")
            .with_args(vec!["arg1".to_string(), "arg2".to_string()]);

        assert_eq!(ctx.args.len(), 2);
        assert_eq!(ctx.args[0], "arg1");
        assert_eq!(ctx.args[1], "arg2");
    }

    #[test]
    fn test_context_with_flags() {
        let mut flags = HashMap::new();
        flags.insert("verbose".to_string(), "true".to_string());

        let ctx = CommandContext::new("/tmp")
            .with_flags(flags);

        assert!(ctx.flags.contains_key("verbose"));
        assert_eq!(ctx.flags.get("verbose"), Some(&"true".to_string()));
    }

    // =========================================================================
    // CommandOutput Tests
    // =========================================================================

    #[test]
    fn test_output_text_display() {
        let output = CommandOutput::Text("Hello".to_string());
        let display = format!("{}", output);
        assert_eq!(display, "Hello");
    }

    #[test]
    fn test_output_json_display() {
        let output = CommandOutput::Json(serde_json::json!({"key": "value"}));
        let display = format!("{}", output);
        assert!(display.contains("key"));
        assert!(display.contains("value"));
    }

    #[test]
    fn test_output_error_display() {
        let output = CommandOutput::Error("Something went wrong".to_string());
        let display = format!("{}", output);
        assert!(display.contains("Error"));
        assert!(display.contains("Something went wrong"));
    }

    #[test]
    fn test_output_none_display() {
        let output = CommandOutput::None;
        let display = format!("{}", output);
        assert!(display.is_empty());
    }

    // =========================================================================
    // CommandMetadata Tests
    // =========================================================================

    #[test]
    fn test_metadata_creation() {
        let meta = CommandMetadata::new("test", "A test command");

        assert_eq!(meta.name, "test");
        assert_eq!(meta.description, "A test command");
        assert!(meta.aliases.is_empty());
        assert!(!meta.hidden);
    }

    #[test]
    fn test_metadata_with_alias() {
        let meta = CommandMetadata::new("test", "A test command")
            .with_alias("t");

        assert!(meta.aliases.contains(&"t".to_string()));
    }

    #[test]
    fn test_metadata_with_aliases() {
        let meta = CommandMetadata::new("test", "A test command")
            .with_aliases(vec!["t".to_string(), "tst".to_string()]);

        assert_eq!(meta.aliases.len(), 2);
    }

    #[test]
    fn test_metadata_with_usage() {
        let meta = CommandMetadata::new("test", "A test command")
            .with_usage("/test [args]");

        assert_eq!(meta.usage, "/test [args]");
    }

    #[test]
    fn test_metadata_hidden() {
        let meta = CommandMetadata::new("test", "A test command")
            .hidden();

        assert!(meta.hidden);
    }

    // =========================================================================
    // LegacyCommandRegistry Tests
    // =========================================================================

    #[test]
    fn test_legacy_registry_creation() {
        let registry = LegacyCommandRegistry::new();
        assert!(registry.entries().is_empty());
    }

    #[test]
    fn test_legacy_registry_register() {
        let mut registry = LegacyCommandRegistry::new();
        registry.register("test", CommandSource::Builtin);

        assert_eq!(registry.entries().len(), 1);
        assert_eq!(registry.entries()[0].name, "test");
        assert_eq!(registry.entries()[0].source, CommandSource::Builtin);
    }

    #[test]
    fn test_legacy_registry_with_builtin_commands() {
        let registry = LegacyCommandRegistry::with_builtin_commands();

        let entries = registry.entries();
        assert!(!entries.is_empty());

        let names: Vec<String> = entries.iter().map(|e| e.name.clone()).collect();
        assert!(names.contains(&"help".to_string()));
        assert!(names.contains(&"commit".to_string()));
        assert!(names.contains(&"doctor".to_string()));
        assert!(names.contains(&"diff".to_string()));
    }

    #[test]
    fn test_legacy_registry_extract_manifest() {
        let registry = LegacyCommandRegistry::with_builtin_commands();
        let manifest = registry.extract_manifest();

        assert_eq!(manifest.len(), registry.entries().len());
    }
}
