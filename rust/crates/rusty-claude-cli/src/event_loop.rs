use std::collections::VecDeque;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::cli::{Cli, PermissionMode};
use crate::tui::{TuiApp, UserAction};

/// Events that drive the application
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User submitted a prompt
    UserPrompt(String),
    /// Runtime produced a response chunk
    ResponseChunk(String),
    /// Tool execution started
    ToolStart { name: String, id: String },
    /// Tool execution completed
    ToolComplete { id: String, output: String, success: bool },
    /// Permission request from a tool
    PermissionRequest { tool: String, operation: String, tx: mpsc::Sender<bool> },
    /// System message
    SystemMessage(String),
    /// Error occurred
    Error(String),
    /// User wants to quit
    Quit,
}

/// Main event loop handler
pub struct EventLoop {
    /// Event queue
    events: VecDeque<AppEvent>,
    /// Application configuration
    cli: Cli,
    /// Current permission mode
    permission_mode: PermissionMode,
    /// Pending tool executions
    pending_tools: Vec<ToolExecution>,
}

#[derive(Debug, Clone)]
struct ToolExecution {
    id: String,
    name: String,
    status: ToolStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolStatus {
    Pending,
    Running,
    Complete,
    Failed,
}

impl EventLoop {
    pub fn new(cli: Cli) -> Self {
        let permission_mode = cli.permission_mode.unwrap_or(PermissionMode::Prompt);

        Self {
            events: VecDeque::new(),
            cli,
            permission_mode,
            pending_tools: Vec::new(),
        }
    }

    /// Initialize the event loop with startup events
    pub fn initialize(&mut self) {
        info!("Initializing event loop");

        // Check if we have an initial prompt
        if let Some(prompt) = &self.cli.prompt {
            self.events.push_back(AppEvent::UserPrompt(prompt.clone()));
        }

        // Add startup message
        self.events.push_back(AppEvent::SystemMessage(
            "Welcome to Claude Code (Rust implementation)".to_string(),
        ));

        if self.cli.dangerously_skip_permissions {
            self.events.push_back(AppEvent::SystemMessage(
                "Warning: Running with --dangerously-skip-permissions".to_string(),
            ));
        }
    }

    /// Process the next event in the queue
    pub async fn process_next(&mut self) -> Option<AppEvent> {
        if let Some(event) = self.events.pop_front() {
            debug!("Processing event: {:?}", event);
            self.handle_event(&event).await;
            return Some(event);
        }
        None
    }

    /// Handle a specific event type
    async fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::UserPrompt(prompt) => {
                info!("Processing user prompt: {}", prompt);
                // In a full implementation, this would:
                // 1. Send to the runtime/API
                // 2. Stream back responses
                // For now, we echo a placeholder response
                self.events.push_back(AppEvent::ResponseChunk(
                    "This is a placeholder response from the Rust CLI implementation.\n\n".to_string(),
                ));
                self.events.push_back(AppEvent::ResponseChunk(
                    "The full runtime integration will connect to the AI backend here.".to_string(),
                ));
            }
            AppEvent::ToolStart { name, id } => {
                info!("Tool started: {} ({})", name, id);
                self.pending_tools.push(ToolExecution {
                    id: id.clone(),
                    name: name.clone(),
                    status: ToolStatus::Running,
                });
            }
            AppEvent::ToolComplete { id, output: _output, success } => {
                if let Some(tool) = self.pending_tools.iter_mut().find(|t| t.id == *id) {
                    tool.status = if *success {
                        ToolStatus::Complete
                    } else {
                        ToolStatus::Failed
                    };
                }
                info!("Tool completed: {} (success: {})", id, success);
            }
            AppEvent::PermissionRequest { tool, operation, tx } => {
                debug!("Permission request for {}: {}", tool, operation);
                let allowed = self.check_permission(tool, operation);
                if let Err(e) = tx.send(allowed).await {
                    warn!("Failed to send permission response: {}", e);
                }
            }
            _ => {}
        }
    }

    /// Check if an operation should be allowed based on permission mode
    fn check_permission(&self, tool: &str, _operation: &str) -> bool {
        match self.permission_mode {
            PermissionMode::Accept => true,
            PermissionMode::Reject => false,
            PermissionMode::ReadOnly => !is_write_operation(tool),
            PermissionMode::Prompt => {
                // In interactive mode, this would trigger a UI prompt
                // For now, we'll deny in non-interactive contexts
                !self.cli.print
            }
        }
    }

    /// Add an event to the queue
    pub fn push_event(&mut self, event: AppEvent) {
        self.events.push_back(event);
    }

    /// Check if there are more events to process
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Get the current permission mode
    pub fn permission_mode(&self) -> PermissionMode {
        self.permission_mode
    }

    /// Get CLI reference
    pub fn cli(&self) -> &Cli {
        &self.cli
    }
}

/// Check if a tool is a write operation
fn is_write_operation(tool: &str) -> bool {
    matches!(
        tool.to_lowercase().as_str(),
        "edit" | "write" | "create" | "delete" | "bash" | "exec"
    )
}

/// Run the interactive TUI event loop
pub async fn run_interactive(
    mut event_loop: EventLoop,
    mut tui_app: TuiApp,
) -> anyhow::Result<()> {
    use crate::tui::{draw_ui, handle_event, process_event, setup_terminal, restore_terminal};

    // Setup terminal
    let mut terminal = setup_terminal()?;

    // Initialize
    event_loop.initialize();

    // Process initial events
    while event_loop.has_events() {
        if let Some(event) = event_loop.process_next().await {
            update_tui_from_event(&mut tui_app, &event);
        }
    }

    // Main event loop
    loop {
        // Draw UI
        terminal.draw(|f| draw_ui(f, &mut tui_app))?;

        // Check if we should exit
        if tui_app.should_exit {
            break;
        }

        // Handle input events
        match handle_event(&mut tui_app)? {
            Some(event) => {
                if let Some(action) = process_event(&mut tui_app, event) {
                    match action {
                        UserAction::SubmitPrompt(prompt) => {
                            tui_app.add_user_message(prompt.clone());
                            tui_app.waiting = true;
                            event_loop.push_event(AppEvent::UserPrompt(prompt));

                            // Process the prompt event
                            while event_loop.has_events() {
                                if let Some(event) = event_loop.process_next().await {
                                    update_tui_from_event(&mut tui_app, &event);
                                }
                            }
                            tui_app.waiting = false;
                        }
                        UserAction::Confirm(allowed) => {
                            // Handle confirmation result
                            debug!("User confirmed: {}", allowed);
                        }
                        UserAction::Quit => {
                            break;
                        }
                    }
                }
            }
            None => {
                // No event, continue polling
                tokio::task::yield_now().await;
            }
        }
    }

    // Cleanup
    restore_terminal()?;
    Ok(())
}

/// Run in non-interactive (print) mode
pub async fn run_non_interactive(mut event_loop: EventLoop) -> anyhow::Result<()> {
    use std::io::Write;

    event_loop.initialize();

    while let Some(event) = event_loop.process_next().await {
        match event {
            AppEvent::ResponseChunk(text) => {
                print!("{}", text);
                std::io::stdout().flush()?;
            }
            AppEvent::Error(msg) => {
                eprintln!("Error: {}", msg);
            }
            AppEvent::SystemMessage(msg) => {
                if event_loop.cli().verbose {
                    eprintln!("[{}]", msg);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// Update the TUI app state based on an event
fn update_tui_from_event(tui_app: &mut TuiApp, event: &AppEvent) {
    use crate::tui::{Message, ToolStatus};

    match event {
        AppEvent::ResponseChunk(text) => {
            // Append to the last assistant message or create a new one
            if let Some(Message::Assistant(content)) = tui_app.messages.last_mut() {
                content.push_str(text);
            } else {
                tui_app.add_assistant_message(text.clone());
            }
        }
        AppEvent::SystemMessage(msg) => {
            tui_app.add_system_message(msg.clone());
        }
        AppEvent::Error(msg) => {
            tui_app.add_error(msg.clone());
        }
        AppEvent::ToolStart { name, .. } => {
            tui_app.add_tool_message(name.clone());
        }
        AppEvent::ToolComplete { output, success, .. } => {
            let status = if *success {
                ToolStatus::Success
            } else {
                ToolStatus::Error
            };
            tui_app.update_tool_status(status, Some(output.clone()));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_event_loop_new() {
        let cli = Cli::parse_from(["claude"]);
        let event_loop = EventLoop::new(cli);
        assert!(!event_loop.has_events());
    }

    #[test]
    fn test_check_permission_accept() {
        let cli = Cli::parse_from(["claude", "--permission-mode", "accept"]);
        let event_loop = EventLoop::new(cli);
        assert!(event_loop.check_permission("edit", "test.rs"));
    }

    #[test]
    fn test_check_permission_reject() {
        let cli = Cli::parse_from(["claude", "--permission-mode", "reject"]);
        let event_loop = EventLoop::new(cli);
        assert!(!event_loop.check_permission("edit", "test.rs"));
    }

    #[test]
    fn test_is_write_operation() {
        assert!(is_write_operation("Edit"));
        assert!(is_write_operation("bash"));
        assert!(!is_write_operation("Read"));
    }
}
