use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

/// R.A.D Codicological 2.x - Rust CLI Foundation
#[derive(Parser, Debug, Clone)]
#[command(
    name = "claude",
    bin_name = "claude",
    version,
    about = "Claude Code - AI-powered coding assistant",
    long_about = "An interactive AI coding assistant that helps you write, refactor, and understand code."
)]
#[command(
    help_template = "{before-help}{about-with-newline}{usage-heading} {usage}\n\n{options-heading}\n{options}\n{subcommands-heading}\n{subcommands}{after-help}"
)]
pub struct Cli {
    /// Your prompt (optional - starts interactive mode if not provided)
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// Enable debug mode with optional category filtering
    #[arg(short, long, value_name = "FILTER")]
    pub debug: Option<Option<String>>,

    /// Enable debug mode (to stderr)
    #[arg(long, hide = true)]
    pub debug_to_stderr: bool,

    /// Write debug logs to a specific file path
    #[arg(long, value_name = "PATH")]
    pub debug_file: Option<PathBuf>,

    /// Override verbose mode setting from config
    #[arg(long)]
    pub verbose: bool,

    /// Print response and exit (useful for pipes)
    #[arg(short, long)]
    pub print: bool,

    /// Minimal mode: skip hooks, LSP, plugin sync, attribution
    #[arg(long)]
    pub bare: bool,

    /// Output format (only works with --print)
    #[arg(long, value_name = "FORMAT", value_enum)]
    pub output_format: Option<OutputFormat>,

    /// Input format
    #[arg(long, value_name = "FORMAT", value_enum)]
    pub input_format: Option<InputFormat>,

    /// Bypass all permission checks
    #[arg(long)]
    pub dangerously_skip_permissions: bool,

    /// Enable bypassing permissions without enabling by default
    #[arg(long)]
    pub allow_dangerously_skip_permissions: bool,

    /// Permission mode for the session
    #[arg(long, value_name = "MODE", value_enum)]
    pub permission_mode: Option<PermissionMode>,

    /// Continue the most recent conversation
    #[arg(short, long)]
    pub r#continue: bool,

    /// Resume a conversation by session ID
    #[arg(short, long, value_name = "SESSION_ID")]
    pub resume: Option<Option<String>>,

    /// Model for the current session
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Effort level for the session
    #[arg(long, value_enum)]
    pub effort: Option<EffortLevel>,

    /// Agent for the current session
    #[arg(long, value_name = "AGENT")]
    pub agent: Option<String>,

    /// Additional directories to allow tool access
    #[arg(long, value_name = "DIRS", value_delimiter = ',')]
    pub add_dir: Vec<PathBuf>,

    /// Load plugins from a directory
    #[arg(long, value_name = "PATH")]
    pub plugin_dir: Vec<PathBuf>,

    /// Set a display name for this session
    #[arg(short, long, value_name = "NAME")]
    pub name: Option<String>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Option<Commands>,
}

/// Output format options for non-interactive mode
#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Plain text output (default)
    #[default]
    Text,
    /// JSON output
    Json,
    /// Streaming JSON output
    StreamJson,
}

/// Input format options
#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputFormat {
    /// Plain text input (default)
    #[default]
    Text,
    /// Streaming JSON input
    StreamJson,
}

/// Permission mode options
#[derive(Debug, Clone, Copy, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionMode {
    /// Accept all permissions automatically
    #[serde(rename = "accept")]
    Accept,
    /// Reject all permissions automatically
    #[serde(rename = "reject")]
    Reject,
    /// Prompt for each permission
    #[default]
    #[serde(rename = "prompt")]
    Prompt,
    /// Read-only mode, no writes allowed
    #[serde(rename = "read-only")]
    ReadOnly,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PermissionMode::Accept => write!(f, "accept"),
            PermissionMode::Reject => write!(f, "reject"),
            PermissionMode::Prompt => write!(f, "prompt"),
            PermissionMode::ReadOnly => write!(f, "read-only"),
        }
    }
}

/// Effort level for the session
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    Max,
}

impl std::fmt::Display for EffortLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffortLevel::Low => write!(f, "low"),
            EffortLevel::Medium => write!(f, "medium"),
            EffortLevel::High => write!(f, "high"),
            EffortLevel::Max => write!(f, "max"),
        }
    }
}

/// CLI subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    /// Run a prompt and exit (non-interactive mode)
    Run {
        /// The prompt to execute
        prompt: String,

        /// Output format
        #[arg(short, long, value_enum)]
        output: Option<OutputFormat>,
    },

    /// MCP server management
    #[command(name = "mcp")]
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Plugin management
    #[command(name = "plugin")]
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Authentication management
    #[command(name = "auth")]
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },

    /// Resume a previous session
    #[command(name = "resume")]
    Resume {
        /// Session ID to resume (optional - opens picker if not provided)
        session_id: Option<String>,
    },

    /// List previous sessions
    #[command(name = "sessions")]
    Sessions,

    /// Configuration management
    #[command(name = "config")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    /// Doctor: check system health
    #[command(name = "doctor")]
    Doctor,

    /// Internal: Dump manifests from upstream TypeScript
    #[command(hide = true)]
    DumpManifests,

    /// Internal: Print bootstrap plan
    #[command(hide = true)]
    BootstrapPlan,
}

/// MCP subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum McpCommands {
    /// Add an MCP server
    Add {
        /// Server name
        name: String,
        /// Server URL or command
        source: String,
    },
    /// List configured MCP servers
    List,
    /// Remove an MCP server
    Remove {
        /// Server name to remove
        name: String,
    },
    /// Start MCP server for this session
    Serve,
}

/// Plugin subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum PluginCommands {
    /// Install a plugin
    Install {
        /// Plugin name or URL
        source: String,
    },
    /// List installed plugins
    List,
    /// Uninstall a plugin
    Uninstall {
        /// Plugin name
        name: String,
    },
    /// Update plugins
    Update,
}

/// Auth subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum AuthCommands {
    /// Log in to Claude AI
    Login,
    /// Log out from Claude AI
    Logout,
    /// Show current auth status
    Status,
}

/// Config subcommands
#[derive(Subcommand, Debug, Clone)]
pub enum ConfigCommands {
    /// Get a config value
    Get {
        /// Config key
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key
        key: String,
        /// Config value
        value: String,
    },
    /// List all config values
    List,
}

/// Determine if we should run in interactive mode
pub fn is_interactive_mode(cli: &Cli) -> bool {
    if cli.print {
        return false;
    }
    if cli.command.is_some() {
        // Some commands are interactive, others aren't
        return matches!(
            cli.command,
            None | Some(Commands::Resume { .. }) | Some(Commands::Sessions)
        );
    }
    // No command and no print flag = interactive
    true
}

/// Determine if TTY is available
pub fn is_tty_available() -> bool {
    crossterm::tty::IsTty::is_tty(&std::io::stdout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing() {
        let cli = Cli::parse_from(["claude", "--verbose", "--print", "hello"]);
        assert!(cli.verbose);
        assert!(cli.print);
        assert_eq!(cli.prompt, Some("hello".to_string()));
    }

    #[test]
    fn test_permission_mode_parsing() {
        let cli = Cli::parse_from(["claude", "--permission-mode", "accept"]);
        assert!(matches!(cli.permission_mode, Some(PermissionMode::Accept)));
    }

    #[test]
    fn test_subcommand_parsing() {
        let cli = Cli::parse_from(["claude", "doctor"]);
        assert!(matches!(cli.command, Some(Commands::Doctor)));
    }

    #[test]
    fn test_mcp_subcommands() {
        let cli = Cli::parse_from(["claude", "mcp", "list"]);
        assert!(matches!(
            cli.command,
            Some(Commands::Mcp {
                command: McpCommands::List
            })
        ));
    }
}
