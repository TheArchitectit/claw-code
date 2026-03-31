//! Doctor command for environment diagnostics.
//!
//! Checks the health of the development environment including:
//! - Rust toolchain installation
//! - Git configuration
//! - Common environment issues

use std::process::Command as StdCommand;

use crate::{Command, CommandContext, CommandMetadata, CommandOutput, CommandResult};

/// Doctor command for environment diagnostics
#[derive(Debug, Clone)]
pub struct DoctorCommand {
    metadata: CommandMetadata,
}

impl DoctorCommand {
    /// Create a new doctor command
    #[must_use]
    pub fn new() -> Self {
        Self {
            metadata: CommandMetadata::new("doctor", "Check environment health and diagnostics")
                .with_alias("checkup")
                .with_alias("health")
                .with_usage("doctor [--verbose]"),
        }
    }

    /// Check if Rust toolchain is installed and working
    fn check_rust_toolchain(&self) -> (bool, String) {
        match StdCommand::new("rustc").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                (true, format!("Rust toolchain: {version}"))
            }
            Ok(_) => (false, "Rust toolchain: installed but error running".to_string()),
            Err(e) => (false, format!("Rust toolchain: not found ({e})").to_string()),
        }
    }

    /// Check if Cargo is installed
    fn check_cargo(&self) -> (bool, String) {
        match StdCommand::new("cargo").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                (true, format!("Cargo: {version}"))
            }
            Ok(_) => (false, "Cargo: installed but error running".to_string()),
            Err(e) => (false, format!("Cargo: not found ({e})").to_string()),
        }
    }

    /// Check git configuration
    fn check_git_config(&self) -> Vec<(bool, String)> {
        let mut results = Vec::new();

        // Check git is installed
        match StdCommand::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                results.push((true, format!("Git: {version}")));
            }
            _ => {
                results.push((false, "Git: not found".to_string()));
                return results;
            }
        }

        // Check git user.name
        match StdCommand::new("git")
            .args(["config", "--global", "user.name"])
            .output()
        {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
                results.push((true, format!("Git user.name: {name}")));
            }
            _ => results.push((false, "Git user.name: not set".to_string())),
        }

        // Check git user.email
        match StdCommand::new("git")
            .args(["config", "--global", "user.email"])
            .output()
        {
            Ok(output) if output.status.success() && !output.stdout.is_empty() => {
                let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
                results.push((true, format!("Git user.email: {email}")));
            }
            _ => results.push((false, "Git user.email: not set".to_string())),
        }

        results
    }

    /// Check common environment variables
    fn check_env_vars(&self) -> Vec<(bool, String)> {
        let mut results = Vec::new();

        let vars_to_check = ["HOME", "PATH", "EDITOR", "SHELL"];

        for var in &vars_to_check {
            match std::env::var(var) {
                Ok(value) if !value.is_empty() => {
                    if *var == "PATH" {
                        // For PATH, just show count
                        let count = value.split(':').count();
                        results.push((true, format!("{var}: {count} entries")));
                    } else {
                        results.push((true, format!("{var}: {value}")));
                    }
                }
                Ok(_) => results.push((false, format!("{var}: empty"))),
                Err(_) => results.push((false, format!("{var}: not set"))),
            }
        }

        results
    }

    /// Run all diagnostics
    async fn run_diagnostics(&self, verbose: bool) -> CommandOutput {
        let mut output_lines = vec!["Environment Diagnostics".to_string(), "=".repeat(40)];
        let mut all_ok = true;

        // Rust toolchain
        let (ok, msg) = self.check_rust_toolchain();
        output_lines.push(format!("{} {}", if ok { "✓" } else { "✗" }, msg));
        all_ok &= ok;

        // Cargo
        let (ok, msg) = self.check_cargo();
        output_lines.push(format!("{} {}", if ok { "✓" } else { "✗" }, msg));
        all_ok &= ok;

        // Git configuration
        output_lines.push(String::new());
        output_lines.push("Git Configuration:".to_string());
        for (ok, msg) in self.check_git_config() {
            output_lines.push(format!("{} {}", if ok { "✓" } else { "✗" }, msg));
            all_ok &= ok;
        }

        // Environment variables
        output_lines.push(String::new());
        output_lines.push("Environment Variables:".to_string());
        for (ok, msg) in self.check_env_vars() {
            output_lines.push(format!("{} {}", if ok { "✓" } else { "✗" }, msg));
            if !ok && verbose {
                all_ok &= ok;
            }
        }

        // Summary
        output_lines.push(String::new());
        output_lines.push("-".repeat(40));
        if all_ok {
            output_lines.push("✓ All checks passed!".to_string());
        } else {
            output_lines.push("✗ Some checks failed".to_string());
        }

        CommandOutput::Text(output_lines.join("\n"))
    }
}

impl Default for DoctorCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Command for DoctorCommand {
    fn metadata(&self) -> &CommandMetadata {
        &self.metadata
    }

    async fn execute(&self, ctx: &CommandContext) -> CommandResult {
        let verbose = ctx.flags.contains_key("verbose") || ctx.flags.contains_key("v");
        Ok(self.run_diagnostics(verbose).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_command_metadata() {
        let cmd = DoctorCommand::new();
        let meta = cmd.metadata();

        assert_eq!(meta.name, "doctor");
        assert!(!meta.description.is_empty());
        assert!(meta.aliases.contains(&"checkup".to_string()));
        assert!(meta.aliases.contains(&"health".to_string()));
    }

    #[test]
    fn test_doctor_matches() {
        let cmd = DoctorCommand::new();

        assert!(cmd.matches("doctor"));
        assert!(cmd.matches("checkup"));
        assert!(cmd.matches("health"));
        assert!(!cmd.matches("invalid"));
    }

    #[tokio::test]
    async fn test_doctor_execution() {
        let cmd = DoctorCommand::new();
        let ctx = CommandContext::new("/tmp");

        let result = cmd.execute(&ctx).await;
        assert!(result.is_ok());

        match result.unwrap() {
            CommandOutput::Text(text) => {
                assert!(text.contains("Environment Diagnostics"));
                assert!(text.contains("Git Configuration"));
                assert!(text.contains("Environment Variables"));
            }
            _ => panic!("Expected text output"),
        }
    }

    #[test]
    fn test_check_rust_toolchain() {
        let cmd = DoctorCommand::new();
        let (ok, msg) = cmd.check_rust_toolchain();

        // Should report something (may or may not be installed)
        assert!(!msg.is_empty());
        assert!(msg.contains("Rust toolchain"));
    }

    #[test]
    fn test_check_cargo() {
        let cmd = DoctorCommand::new();
        let (ok, msg) = cmd.check_cargo();

        // Should report something
        assert!(!msg.is_empty());
        assert!(msg.contains("Cargo"));
    }

    #[test]
    fn test_check_git_config() {
        let cmd = DoctorCommand::new();
        let results = cmd.check_git_config();

        // Should have at least one result (git version check)
        assert!(!results.is_empty());
    }

    #[test]
    fn test_check_env_vars() {
        let cmd = DoctorCommand::new();
        let results = cmd.check_env_vars();

        // Should check multiple variables
        assert!(!results.is_empty());
    }
}
