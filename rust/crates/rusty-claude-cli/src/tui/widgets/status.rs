//! Status and progress widgets for the TUI.
//!
//! This module provides widgets for:
//! - Status bar (model, tokens, mode display)
//! - Progress indicators (spinners, progress bars)
//! - Typing indicator animation

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, Gauge, Paragraph, Widget, Wrap},
};

/// A status bar widget showing session information.
#[derive(Debug, Clone)]
pub struct StatusBarWidget {
    /// Current mode (NORMAL, INSERT, COMMAND).
    pub mode: StatusMode,
    /// Model name being used.
    pub model: String,
    /// Token count information.
    pub tokens: TokenInfo,
    /// Connection status.
    pub connection: ConnectionStatus,
    /// Current working directory.
    pub cwd: String,
    /// Git branch (if in git repo).
    pub git_branch: Option<String>,
    /// Additional status items.
    pub extra_items: Vec<(String, String)>,
}

/// The current mode for the status bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMode {
    /// Normal/navigation mode.
    Normal,
    /// Prompt/input mode.
    Prompt,
    /// Confirmation dialog mode.
    Confirm,
    /// Command mode (for slash commands).
    Command,
}

/// Token usage information.
#[derive(Debug, Clone)]
pub struct TokenInfo {
    /// Input tokens used.
    pub input: u32,
    /// Output tokens used.
    pub output: u32,
    /// Maximum tokens allowed.
    pub max: u32,
    /// Context window percentage used.
    pub context_used: f64,
}

impl Default for TokenInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenInfo {
    /// Create new token info with zeros.
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: 0,
            output: 0,
            max: 200_000, // Default Claude context window
            context_used: 0.0,
        }
    }

    /// Create with specific values.
    #[must_use]
    pub fn with_counts(input: u32, output: u32) -> Self {
        Self {
            input,
            output,
            max: 200_000,
            context_used: (input + output) as f64 / 200_000.0,
        }
    }

    /// Total tokens used.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.input.saturating_add(self.output)
    }

    /// Percentage of context used.
    #[must_use]
    pub fn percent_used(&self) -> f64 {
        (self.total() as f64 / self.max as f64) * 100.0
    }

    /// Get color based on usage level.
    #[must_use]
    pub fn usage_color(&self) -> Color {
        let pct = self.percent_used();
        if pct < 50.0 {
            Color::Green
        } else if pct < 80.0 {
            Color::Yellow
        } else {
            Color::Red
        }
    }
}

/// Connection status to the AI service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    /// Connected and ready.
    Connected,
    /// Currently connecting.
    Connecting,
    /// Disconnected.
    Disconnected,
    /// Connection error.
    Error,
    /// Rate limited.
    RateLimited,
}

impl ConnectionStatus {
    /// Get display text for this status.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Connected => "●",
            Self::Connecting => "◐",
            Self::Disconnected => "○",
            Self::Error => "✖",
            Self::RateLimited => "⏳",
        }
    }

    /// Get color for this status.
    #[must_use]
    pub fn color(&self) -> Color {
        match self {
            Self::Connected => Color::Green,
            Self::Connecting => Color::Yellow,
            Self::Disconnected => Color::Gray,
            Self::Error => Color::Red,
            Self::RateLimited => Color::Yellow,
        }
    }
}

impl Default for StatusBarWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBarWidget {
    /// Create a new status bar.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: StatusMode::Normal,
            model: "claude-3-opus".to_string(),
            tokens: TokenInfo::new(),
            connection: ConnectionStatus::Connected,
            cwd: String::new(),
            git_branch: None,
            extra_items: Vec::new(),
        }
    }

    /// Set the mode.
    #[must_use]
    pub fn with_mode(mut self, mode: StatusMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set token info.
    #[must_use]
    pub fn with_tokens(mut self, tokens: TokenInfo) -> Self {
        self.tokens = tokens;
        self
    }

    /// Set connection status.
    #[must_use]
    pub fn with_connection(mut self, status: ConnectionStatus) -> Self {
        self.connection = status;
        self
    }

    /// Set working directory.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = cwd.into();
        self
    }

    /// Set git branch.
    #[must_use]
    pub fn with_git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }

    /// Add an extra status item.
    pub fn add_item(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.extra_items.push((key.into(), value.into()));
    }

    /// Get mode display text.
    #[must_use]
    pub fn mode_text(&self) -> &'static str {
        match self.mode {
            StatusMode::Normal => "NORMAL",
            StatusMode::Prompt => "PROMPT",
            StatusMode::Confirm => "CONFIRM",
            StatusMode::Command => "COMMAND",
        }
    }

    /// Get mode color.
    #[must_use]
    pub fn mode_color(&self) -> Color {
        match self.mode {
            StatusMode::Normal => Color::Blue,
            StatusMode::Prompt => Color::Green,
            StatusMode::Confirm => Color::Yellow,
            StatusMode::Command => Color::Magenta,
        }
    }

    /// Format token count for display.
    #[must_use]
    pub fn format_tokens(&self) -> String {
        let total = self.tokens.total();
        if total < 1000 {
            format!("{} tokens", total)
        } else if total < 1_000_000 {
            format!("{:.1}k tokens", total as f64 / 1000.0)
        } else {
            format!("{:.1}M tokens", total as f64 / 1_000_000.0)
        }
    }

    /// Calculate height needed.
    #[must_use]
    pub fn height(&self) -> u16 {
        1
    }
}

impl Widget for StatusBarWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mode_style = Style::default()
            .fg(Color::Black)
            .bg(self.mode_color())
            .add_modifier(Modifier::BOLD);

        let mode_text = format!(" {} ", self.mode_text());

        // Build left side: mode + connection + model
        let mut left_spans = vec![
            Span::styled(mode_text, mode_style),
            Span::raw(" "),
            Span::styled(
                self.connection.as_str(),
                Style::default().fg(self.connection.color()),
            ),
            Span::raw(" "),
            Span::styled(&self.model, Style::default().fg(Color::Cyan)),
        ];

        // Add git branch if available
        if let Some(branch) = &self.git_branch {
            left_spans.push(Span::raw(" "));
            left_spans.push(Span::styled(
                format!("({})", branch),
                Style::default().fg(Color::Magenta),
            ));
        }

        // Build center: extra items
        let center_spans: Vec<Span<'_>> = self
            .extra_items
            .iter()
            .flat_map(|(k, v)| {
                vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("{}:", k),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(v, Style::default().fg(Color::White)),
                ]
            })
            .collect();

        // Build right side: tokens + cwd
        let token_color = self.tokens.usage_color();
        let right_text = format!(
            "{} | {}",
            self.format_tokens(),
            if self.cwd.len() > 30 {
                format!("...{}", &self.cwd[self.cwd.len() - 30..])
            } else {
                self.cwd.clone()
            }
        );

        let right_spans = vec![Span::styled(
            right_text,
            Style::default().fg(token_color),
        )];

        // Combine all spans
        let mut all_spans = left_spans;
        all_spans.extend(center_spans);
        all_spans.push(Span::raw(" "));
        all_spans.extend(right_spans);

        let line = Line::from(all_spans);
        let text = Text::from(vec![line]);

        // Render with background
        let bg_style = Style::default().bg(Color::Black);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_style(bg_style);
            }
        }

        Paragraph::new(text).render(area, buf);
    }
}

/// A typing indicator with animated dots.
#[derive(Debug, Clone)]
pub struct TypingIndicatorWidget {
    /// Current animation frame (0-3).
    pub frame: usize,
    /// Text prefix before the dots.
    pub prefix: String,
    /// Style for the indicator.
    pub style: Style,
}

impl Default for TypingIndicatorWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl TypingIndicatorWidget {
    /// Create a new typing indicator.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame: 0,
            prefix: "Claude is thinking".to_string(),
            style: Style::default().fg(Color::Green),
        }
    }

    /// Set the prefix text.
    #[must_use]
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Set the style.
    #[must_use]
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Advance to the next frame.
    pub fn next_frame(&mut self) {
        self.frame = (self.frame + 1) % 4;
    }

    /// Get the current dot pattern.
    #[must_use]
    pub fn dots(&self) -> &'static str {
        match self.frame {
            0 => "   ",
            1 => ".  ",
            2 => ".. ",
            3 => "...",
            _ => "   ",
        }
    }

    /// Get the full display text.
    #[must_use]
    pub fn text(&self) -> String {
        format!("{}{}", self.prefix, self.dots())
    }
}

impl Widget for TypingIndicatorWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text = self.text();
        let paragraph = Paragraph::new(text).style(self.style);
        paragraph.render(area, buf);
    }
}

/// A spinner widget for showing activity.
#[derive(Debug, Clone)]
pub struct SpinnerWidget {
    /// Current frame index.
    pub frame: usize,
    /// Spinner style.
    pub style: Style,
}

/// Built-in spinner animations.
pub const SPINNER_DOTS: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
pub const SPINNER_LINE: &[&str] = &["-", "\\", "|", "/"];
pub const SPINNER_BRAILLE: &[&str] = &["⢀", "⡀", "⠄", "⠂", "⠁", "⠈", "⠐", "⠠"];

impl Default for SpinnerWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl SpinnerWidget {
    /// Create a new spinner.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frame: 0,
            style: Style::default().fg(Color::Cyan),
        }
    }

    /// Set the style.
    #[must_use]
    pub fn with_style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    /// Advance to the next frame.
    pub fn next_frame(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    /// Get the current spinner character (dots style).
    #[must_use]
    pub fn char(&self) -> &'static str {
        SPINNER_DOTS[self.frame % SPINNER_DOTS.len()]
    }

    /// Get a specific style of spinner.
    #[must_use]
    pub fn char_with_style<'a>(&self, style: &'a [&'a str]) -> &'a str {
        style[self.frame % style.len()]
    }
}

impl Widget for SpinnerWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let spinner = self.char();
        let text = Text::from(Line::from(Span::styled(spinner, self.style)));
        Paragraph::new(text).render(area, buf);
    }
}

/// A progress bar widget with text.
#[derive(Debug, Clone)]
pub struct ProgressWidget {
    /// Progress percentage (0-100).
    pub percent: f64,
    /// Label text.
    pub label: String,
    /// Show percentage.
    pub show_percent: bool,
    /// Bar color.
    pub color: Color,
}

impl Default for ProgressWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressWidget {
    /// Create a new progress widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            percent: 0.0,
            label: String::new(),
            show_percent: true,
            color: Color::Cyan,
        }
    }

    /// Set the progress percentage.
    #[must_use]
    pub fn with_percent(mut self, percent: f64) -> Self {
        self.percent = percent.clamp(0.0, 100.0);
        self
    }

    /// Set the label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set whether to show percentage.
    #[must_use]
    pub fn with_show_percent(mut self, show: bool) -> Self {
        self.show_percent = show;
        self
    }

    /// Set the color.
    #[must_use]
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Update progress from current/total.
    pub fn update(&mut self, current: u64, total: u64) {
        if total > 0 {
            self.percent = (current as f64 / total as f64) * 100.0;
        }
    }
}

impl Widget for ProgressWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Label line
        let label_spans = vec![
            Span::styled(&self.label, Style::default()),
            if self.show_percent {
                Span::styled(
                    format!(" {:.1}%", self.percent),
                    Style::default().fg(self.color),
                )
            } else {
                Span::raw("")
            },
        ];

        let label_line = Line::from(label_spans);
        let label_para = Paragraph::new(Text::from(vec![label_line]));
        label_para.render(
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            buf,
        );

        // Progress bar
        if area.height > 1 {
            let bar_area = Rect {
                x: area.x,
                y: area.y + 1,
                width: area.width,
                height: area.height - 1,
            };

            let filled = ((area.width as f64 * self.percent) / 100.0) as u16;

            for x in 0..area.width {
                let style = if x < filled {
                    Style::default()
                        .bg(self.color)
                        .fg(Color::Black)
                } else {
                    Style::default().bg(Color::DarkGray)
                };

                if bar_area.y < area.y + area.height {
                    buf[(bar_area.x + x, bar_area.y)].set_symbol(" ");
                    buf[(bar_area.x + x, bar_area.y)].set_style(style);
                }
            }
        }
    }
}

/// A tool execution status widget.
#[derive(Debug, Clone)]
pub struct ToolStatusWidget {
    /// Tool name.
    pub tool_name: String,
    /// Current status.
    pub status: ToolExecutionStatus,
    /// Progress message.
    pub message: String,
    /// Spinner frame.
    pub spinner_frame: usize,
}

/// Tool execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolExecutionStatus {
    /// Tool is pending execution.
    Pending,
    /// Tool is currently running.
    Running,
    /// Tool completed successfully.
    Success,
    /// Tool execution failed.
    Error,
}

impl ToolExecutionStatus {
    /// Get icon for this status.
    #[must_use]
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Pending => "◯",
            Self::Running => "◐",
            Self::Success => "✓",
            Self::Error => "✖",
        }
    }

    /// Get color for this status.
    #[must_use]
    pub fn color(&self) -> Color {
        match self {
            Self::Pending => Color::Gray,
            Self::Running => Color::Yellow,
            Self::Success => Color::Green,
            Self::Error => Color::Red,
        }
    }
}

impl Default for ToolStatusWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolStatusWidget {
    /// Create a new tool status widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tool_name: String::new(),
            status: ToolExecutionStatus::Pending,
            message: String::new(),
            spinner_frame: 0,
        }
    }

    /// Set the tool name.
    #[must_use]
    pub fn with_tool(mut self, name: impl Into<String>) -> Self {
        self.tool_name = name.into();
        self
    }

    /// Set the status.
    #[must_use]
    pub fn with_status(mut self, status: ToolExecutionStatus) -> Self {
        self.status = status;
        self
    }

    /// Set the message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = message.into();
        self
    }

    /// Set the spinner frame.
    #[must_use]
    pub fn with_frame(mut self, frame: usize) -> Self {
        self.spinner_frame = frame;
        self
    }

    /// Advance spinner frame (if running).
    pub fn tick(&mut self) {
        if self.status == ToolExecutionStatus::Running {
            self.spinner_frame = self.spinner_frame.wrapping_add(1);
        }
    }

    /// Get the current spinner character.
    #[must_use]
    pub fn spinner(&self) -> &'static str {
        if self.status == ToolExecutionStatus::Running {
            SPINNER_DOTS[self.spinner_frame % SPINNER_DOTS.len()]
        } else {
            self.status.icon()
        }
    }
}

impl Widget for ToolStatusWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let color = self.status.color();
        let icon = self.spinner();

        let spans = vec![
            Span::styled(
                format!("{} ", icon),
                Style::default().fg(color),
            ),
            Span::styled(
                &self.tool_name,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            if !self.message.is_empty() {
                Span::styled(
                    format!(" - {}", self.message),
                    Style::default().fg(Color::Gray),
                )
            } else {
                Span::raw("")
            },
        ];

        let line = Line::from(spans);
        Paragraph::new(Text::from(vec![line])).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_bar_new() {
        let bar = StatusBarWidget::new();
        assert_eq!(bar.mode, StatusMode::Normal);
        assert_eq!(bar.model, "claude-3-opus");
        assert_eq!(bar.connection, ConnectionStatus::Connected);
    }

    #[test]
    fn test_status_bar_builder() {
        let bar = StatusBarWidget::new()
            .with_mode(StatusMode::Prompt)
            .with_model("claude-3-sonnet")
            .with_connection(ConnectionStatus::Connecting)
            .with_cwd("/home/user")
            .with_git_branch("main");

        assert_eq!(bar.mode, StatusMode::Prompt);
        assert_eq!(bar.model, "claude-3-sonnet");
        assert_eq!(bar.connection, ConnectionStatus::Connecting);
        assert_eq!(bar.cwd, "/home/user");
        assert_eq!(bar.git_branch, Some("main".to_string()));
    }

    #[test]
    fn test_token_info() {
        let info = TokenInfo::with_counts(1000, 500);
        assert_eq!(info.input, 1000);
        assert_eq!(info.output, 500);
        assert_eq!(info.total(), 1500);
        assert!(info.percent_used() > 0.0);
        assert_eq!(info.usage_color(), Color::Green);
    }

    #[test]
    fn test_token_info_high_usage() {
        let info = TokenInfo::with_counts(150_000, 30_000);
        assert_eq!(info.usage_color(), Color::Red);
    }

    #[test]
    fn test_connection_status() {
        assert_eq!(ConnectionStatus::Connected.as_str(), "●");
        assert_eq!(ConnectionStatus::Connected.color(), Color::Green);

        assert_eq!(ConnectionStatus::Error.as_str(), "✖");
        assert_eq!(ConnectionStatus::Error.color(), Color::Red);
    }

    #[test]
    fn test_typing_indicator() {
        let mut indicator = TypingIndicatorWidget::new();
        assert_eq!(indicator.dots(), "   ");
        assert_eq!(indicator.text(), "Claude is thinking   ");

        indicator.next_frame();
        assert_eq!(indicator.dots(), ".  ");

        indicator.next_frame();
        indicator.next_frame();
        assert_eq!(indicator.dots(), "...");

        indicator.next_frame();
        assert_eq!(indicator.dots(), "   "); // Back to start
    }

    #[test]
    fn test_typing_indicator_with_prefix() {
        let indicator = TypingIndicatorWidget::new()
            .with_prefix("Loading");
        assert_eq!(indicator.text(), "Loading   ");
    }

    #[test]
    fn test_spinner() {
        let mut spinner = SpinnerWidget::new();
        assert_eq!(spinner.char(), SPINNER_DOTS[0]);

        spinner.next_frame();
        assert_eq!(spinner.char(), SPINNER_DOTS[1]);

        // Wrap around
        for _ in 0..20 {
            spinner.next_frame();
        }
        assert_eq!(spinner.char(), SPINNER_DOTS[0]);
    }

    #[test]
    fn test_spinner_styles() {
        let spinner = SpinnerWidget::new();
        assert_eq!(spinner.char_with_style(SPINNER_LINE), SPINNER_LINE[0]);
        assert_eq!(
            spinner.char_with_style(SPINNER_BRAILLE),
            SPINNER_BRAILLE[0]
        );
    }

    #[test]
    fn test_progress_widget() {
        let progress = ProgressWidget::new()
            .with_percent(50.0)
            .with_label("Loading")
            .with_color(Color::Green);

        assert_eq!(progress.percent, 50.0);
        assert_eq!(progress.label, "Loading");
        assert_eq!(progress.color, Color::Green);
    }

    #[test]
    fn test_progress_clamp() {
        let progress = ProgressWidget::new().with_percent(150.0);
        assert_eq!(progress.percent, 100.0);

        let progress = ProgressWidget::new().with_percent(-10.0);
        assert_eq!(progress.percent, 0.0);
    }

    #[test]
    fn test_progress_update() {
        let mut progress = ProgressWidget::new();
        progress.update(50, 100);
        assert_eq!(progress.percent, 50.0);

        progress.update(3, 4);
        assert_eq!(progress.percent, 75.0);
    }

    #[test]
    fn test_tool_status() {
        let mut tool = ToolStatusWidget::new()
            .with_tool("ReadFile")
            .with_status(ToolExecutionStatus::Running)
            .with_message("Reading...")
            .with_frame(5);

        assert_eq!(tool.tool_name, "ReadFile");
        assert_eq!(tool.status, ToolExecutionStatus::Running);
        assert_eq!(tool.message, "Reading...");
        assert_eq!(tool.spinner_frame, 5);

        // Spinner should be animated when running
        assert!(tool.spinner() != "✓");
    }

    #[test]
    fn test_tool_status_spinner_static() {
        let tool_pending = ToolStatusWidget::new()
            .with_status(ToolExecutionStatus::Pending);
        assert_eq!(tool_pending.spinner(), "◯");

        let tool_success = ToolStatusWidget::new()
            .with_status(ToolExecutionStatus::Success);
        assert_eq!(tool_success.spinner(), "✓");

        let tool_error = ToolStatusWidget::new()
            .with_status(ToolExecutionStatus::Error);
        assert_eq!(tool_error.spinner(), "✖");
    }

    #[test]
    fn test_status_bar_extra_items() {
        let mut bar = StatusBarWidget::new();
        bar.add_item("key", "value");
        bar.add_item("files", "5");

        assert_eq!(bar.extra_items.len(), 2);
        assert_eq!(bar.extra_items[0], ("key".to_string(), "value".to_string()));
    }

    #[test]
    fn test_format_tokens() {
        let bar = StatusBarWidget::new()
            .with_tokens(TokenInfo::with_counts(500, 500));
        assert!(bar.format_tokens().contains("1,000"));

        let bar = StatusBarWidget::new()
            .with_tokens(TokenInfo::with_counts(1500, 500));
        assert!(bar.format_tokens().contains("2k"));
    }
}
