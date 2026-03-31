//! Notification types for the tool use context.

/// Options for setting tool JSX.
#[derive(Debug, Clone)]
pub struct ToolJsxOptions {
    /// The JSX content.
    pub jsx: Option<String>,

    /// Whether to hide the prompt input.
    pub should_hide_prompt_input: bool,

    /// Whether to continue animation.
    pub should_continue_animation: bool,

    /// Whether to show a spinner.
    pub show_spinner: bool,

    /// Whether this is a local JSX command.
    pub is_local_jsx_command: bool,

    /// Whether this is immediate.
    pub is_immediate: bool,

    /// Whether to clear the local JSX.
    pub clear_local_jsx: bool,
}

/// OS notification options.
#[derive(Debug, Clone)]
pub struct OsNotificationOptions {
    /// The notification message.
    pub message: String,

    /// The notification type.
    pub notification_type: String,
}

/// A notification.
#[derive(Debug, Clone)]
pub struct Notification {
    /// The notification type.
    pub notification_type: String,

    /// The notification message.
    pub message: String,

    /// The notification level.
    pub level: NotificationLevel,
}

/// Notification severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    /// Info level.
    Info,

    /// Warning level.
    Warning,

    /// Error level.
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_level_variants() {
        let levels = vec![
            NotificationLevel::Info,
            NotificationLevel::Warning,
            NotificationLevel::Error,
        ];

        // Just verify they exist and can be compared
        assert_eq!(levels.len(), 3);
    }

    #[test]
    fn test_tool_jsx_options() {
        let opts = ToolJsxOptions {
            jsx: Some("<div>test</div>".to_string()),
            should_hide_prompt_input: true,
            should_continue_animation: false,
            show_spinner: true,
            is_local_jsx_command: false,
            is_immediate: false,
            clear_local_jsx: false,
        };
        assert_eq!(opts.jsx, Some("<div>test</div>".to_string()));
        assert!(opts.should_hide_prompt_input);
        assert!(!opts.should_continue_animation);
        assert!(opts.show_spinner);
        assert!(!opts.is_local_jsx_command);
        assert!(!opts.is_immediate);
        assert!(!opts.clear_local_jsx);
    }

    #[test]
    fn test_notification() {
        let notification = Notification {
            notification_type: "info".to_string(),
            message: "test".to_string(),
            level: NotificationLevel::Info,
        };

        assert_eq!(notification.notification_type, "info");
        assert_eq!(notification.message, "test");
    }

    #[test]
    fn test_os_notification_options() {
        let opts = OsNotificationOptions {
            message: "message".to_string(),
            notification_type: "info".to_string(),
        };

        assert_eq!(opts.message, "message");
        assert_eq!(opts.notification_type, "info");
    }
}
