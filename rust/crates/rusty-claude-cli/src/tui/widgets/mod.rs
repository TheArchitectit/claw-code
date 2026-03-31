//! TUI Widgets module
//!
//! This module provides organized widget components for the terminal UI:
//! - Input widgets: Prompt handling, multiline input, history
//! - Status widgets: Status bar, progress indicators, typing animation
//! - Message widgets: User, Assistant, System, Tool messages

pub mod input;
pub mod status;

// Re-export message widgets from parent messages module
pub use super::messages::{
    AssistantMessageWidget, MessageWidget, ProgressMessageWidget, SystemMessageWidget,
    ToolCallWidget, ToolResultWidget, UserMessageWidget,
};

// Re-export render utilities
pub use super::render::{
    calculate_total_height, render_messages, EmptyState, MessageList, MessageListState, TypingIndicator,
};
