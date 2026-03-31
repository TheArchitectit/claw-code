use std::io;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame, Terminal,
};

pub mod messages;
pub mod render;
pub mod widgets;

/// TUI application state
#[derive(Debug)]
pub struct TuiApp {
    /// Input buffer for the prompt
    pub input: String,
    /// Cursor position in the input
    pub cursor_position: usize,
    /// Message history
    pub messages: Vec<Message>,
    /// Scroll position in the message area
    pub scroll: usize,
    /// Whether the app should exit
    pub should_exit: bool,
    /// Whether we're waiting for a response
    pub waiting: bool,
    /// Current mode
    pub mode: AppMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    Prompt,
    Confirm(String),
}

#[derive(Debug, Clone)]
pub enum Message {
    User(String),
    Assistant(String),
    System(String),
    Tool { name: String, status: ToolStatus, output: Option<String> },
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolStatus {
    Pending,
    Running,
    Success,
    Error,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            messages: Vec::new(),
            scroll: 0,
            should_exit: false,
            waiting: false,
            mode: AppMode::Normal,
        }
    }

    /// Add a user message
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(Message::User(content));
        self.scroll_to_bottom();
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, content: String) {
        self.messages.push(Message::Assistant(content));
        self.scroll_to_bottom();
    }

    /// Add a system message
    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(Message::System(content));
        self.scroll_to_bottom();
    }

    /// Add a tool message
    pub fn add_tool_message(&mut self, name: String) {
        self.messages.push(Message::Tool {
            name,
            status: ToolStatus::Pending,
            output: None,
        });
        self.scroll_to_bottom();
    }

    /// Update the last tool message status
    pub fn update_tool_status(&mut self, status: ToolStatus, output: Option<String>) {
        if let Some(Message::Tool { status: s, output: o, .. }) = self.messages.last_mut() {
            *s = status;
            *o = output;
        }
    }

    /// Add an error message
    pub fn add_error(&mut self, content: String) {
        self.messages.push(Message::Error(content));
        self.scroll_to_bottom();
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll = self.messages.len().saturating_sub(1);
    }

    /// Handle key events for prompt input
    pub fn handle_input_event(&mut self, key: KeyCode) -> Option<String> {
        match key {
            KeyCode::Enter => {
                let input = std::mem::take(&mut self.input);
                self.cursor_position = 0;
                return Some(input);
            }
            KeyCode::Char(c) => {
                self.input.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                    self.input.remove(self.cursor_position);
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input.len() {
                    self.input.remove(self.cursor_position);
                }
            }
            KeyCode::Left => {
                self.cursor_position = self.cursor_position.saturating_sub(1);
            }
            KeyCode::Right => {
                if self.cursor_position < self.input.len() {
                    self.cursor_position += 1;
                }
            }
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            KeyCode::End => {
                self.cursor_position = self.input.len();
            }
            KeyCode::Up => {
                // Could implement history navigation here
            }
            KeyCode::Esc => {
                self.mode = AppMode::Normal;
            }
            _ => {}
        }
        None
    }

    /// Handle navigation keys
    pub fn handle_navigation(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Down => {
                if self.scroll < self.messages.len().saturating_sub(1) {
                    self.scroll += 1;
                }
            }
            KeyCode::PageUp => {
                self.scroll = self.scroll.saturating_sub(10);
            }
            KeyCode::PageDown => {
                self.scroll = (self.scroll + 10).min(self.messages.len().saturating_sub(1));
            }
            _ => {}
        }
    }
}

/// Setup the terminal for TUI mode
pub fn setup_terminal() -> io::Result<Terminal<impl Backend>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to normal mode
pub fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        LeaveAlternateScreen,
        DisableMouseCapture,
        event::DisableBracketedPaste
    )?;
    Ok(())
}

/// Render the UI
pub fn draw_ui(f: &mut Frame, app: &mut TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(3),      // Message area
            Constraint::Length(3),   // Input area
            Constraint::Length(1), // Status bar
        ])
        .split(f.area());

    // Message area
    render_message_area(f, app, chunks[0]);

    // Input area
    render_input_area(f, app, chunks[1]);

    // Status bar
    render_status_bar(f, app, chunks[2]);
}

fn render_message_area(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    // Create inner layout for messages
    let messages: Vec<Line> = app
        .messages
        .iter()
        .skip(app.scroll)
        .flat_map(|msg| render_message(msg))
        .collect();

    let paragraph = Paragraph::new(Text::from(messages))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Claude Code ")
                .title_alignment(Alignment::Center),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);

    // Render scrollbar
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut scrollbar_state = ScrollbarState::new(app.messages.len())
        .position(app.scroll)
        .content_length(app.messages.len().saturating_sub(1));

    f.render_stateful_widget(
        scrollbar,
        area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut scrollbar_state,
    );
}

fn render_message(msg: &Message) -> Vec<Line> {
    match msg {
        Message::User(content) => {
            let style = Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD);
            vec![
                Line::from(Span::styled("You: ", style)),
                Line::from(content.clone()),
                Line::from(""),
            ]
        }
        Message::Assistant(content) => {
            let style = Style::default().fg(Color::Green);
            vec![
                Line::from(Span::styled("Claude: ", style)),
                Line::from(content.clone()),
                Line::from(""),
            ]
        }
        Message::System(content) => {
            let style = Style::default().fg(Color::Yellow);
            vec![
                Line::from(Span::styled("System: ", style)),
                Line::from(content.clone()),
                Line::from(""),
            ]
        }
        Message::Tool { name, status, output } => {
            let (status_style, status_text) = match status {
                ToolStatus::Pending => (Style::default().fg(Color::Yellow), "..."),
                ToolStatus::Running => (Style::default().fg(Color::Cyan), "running"),
                ToolStatus::Success => (Style::default().fg(Color::Green), "done"),
                ToolStatus::Error => (Style::default().fg(Color::Red), "error"),
            };

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Tool: ", Style::default().fg(Color::Magenta)),
                    Span::raw(name.clone()),
                    Span::raw(" ["),
                    Span::styled(status_text, status_style),
                    Span::raw("]"),
                ]),
            ];

            if let Some(out) = output {
                lines.push(Line::from(out.clone()));
            }
            lines.push(Line::from(""));
            lines
        }
        Message::Error(content) => {
            let style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);
            vec![
                Line::from(Span::styled("Error: ", style)),
                Line::from(content.clone()),
                Line::from(""),
            ]
        }
    }
}

fn render_input_area(f: &mut Frame, app: &TuiApp, area: Rect) {
    let input = Paragraph::new(app.input.clone())
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(if app.waiting { " Thinking... " } else { " Prompt " }),
        );

    f.render_widget(input, area);

    // Position cursor
    f.set_cursor_position((
        area.x + app.cursor_position as u16 + 1,
        area.y + 1,
    ));
}

fn render_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let mode_text = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Prompt => "PROMPT",
        AppMode::Confirm(_) => "CONFIRM",
    };

    let status = format!(
        " {} | Messages: {} | Scroll: {} | Ctrl+C to quit | Enter to submit ",
        mode_text,
        app.messages.len(),
        app.scroll
    );

    let paragraph = Paragraph::new(status)
        .style(Style::default().fg(Color::White).bg(Color::Blue))
        .alignment(Alignment::Center);

    f.render_widget(paragraph, area);
}

/// Handle a single event from crossterm
pub fn handle_event(_app: &mut TuiApp) -> io::Result<Option<Event>> {
    if event::poll(std::time::Duration::from_millis(100))? {
        let event = event::read()?;
        return Ok(Some(event));
    }
    Ok(None)
}

/// Process an event and return any resulting action
pub fn process_event(app: &mut TuiApp, event: Event) -> Option<UserAction> {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            // Global shortcuts
            match key.code {
                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    app.should_exit = true;
                    return None;
                }
                KeyCode::Char('q') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                    app.should_exit = true;
                    return None;
                }
                _ => {}
            }

            match app.mode {
                AppMode::Normal => {
                    match key.code {
                        KeyCode::Char(':') | KeyCode::Char('i') => {
                            app.mode = AppMode::Prompt;
                            return None;
                        }
                        _ => {
                            app.handle_navigation(key.code);
                            return None;
                        }
                    }
                }
                AppMode::Prompt => {
                    if let Some(input) = app.handle_input_event(key.code) {
                        return Some(UserAction::SubmitPrompt(input));
                    }
                    return None;
                }
                AppMode::Confirm(_) => {
                    match key.code {
                        KeyCode::Char('y') => {
                            app.mode = AppMode::Normal;
                            return Some(UserAction::Confirm(true));
                        }
                        KeyCode::Char('n') | KeyCode::Esc => {
                            app.mode = AppMode::Normal;
                            return Some(UserAction::Confirm(false));
                        }
                        _ => return None,
                    }
                }
            }
        }
        Event::Mouse(mouse_event) => {
            // Handle mouse events for scrolling
            match mouse_event.kind {
                event::MouseEventKind::ScrollUp => {
                    app.scroll = app.scroll.saturating_sub(1);
                }
                event::MouseEventKind::ScrollDown => {
                    if app.scroll < app.messages.len().saturating_sub(1) {
                        app.scroll += 1;
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    None
}

/// User actions that can result from event processing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserAction {
    SubmitPrompt(String),
    Confirm(bool),
    Quit,
}

/// Run a confirmation dialog and return the user's choice
pub fn confirm_dialog(f: &mut Frame, message: &str) {
    let area = centered_rect(60, 20, f.area());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .title_alignment(Alignment::Center);

    let text = Paragraph::new(format!("{}\n\n[y/n]", message))
        .block(block)
        .alignment(Alignment::Center);

    f.render_widget(Clear, area);
    f.render_widget(text, area);
}

/// Helper to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

// Re-export message rendering widgets
pub use messages::{
    AssistantMessageWidget, MessageWidget, ProgressMessageWidget, SystemMessageWidget,
    ToolCallWidget, ToolResultWidget, UserMessageWidget,
};

// Re-export render utilities
pub use render::{
    calculate_total_height, render_messages, EmptyState, MessageList, MessageListState, TypingIndicator,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = TuiApp::new();
        assert!(app.input.is_empty());
        assert!(app.messages.is_empty());
        assert!(!app.should_exit);
    }

    #[test]
    fn test_add_messages() {
        let mut app = TuiApp::new();
        app.add_user_message("Hello".to_string());
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(app.messages[0], Message::User(_)));
    }

    #[test]
    fn test_handle_input() {
        let mut app = TuiApp::new();
        app.handle_input_event(KeyCode::Char('h'));
        app.handle_input_event(KeyCode::Char('i'));
        assert_eq!(app.input, "hi");
        assert_eq!(app.cursor_position, 2);
    }

    #[test]
    fn test_handle_backspace() {
        let mut app = TuiApp::new();
        app.input = "hi".to_string();
        app.cursor_position = 2;
        app.handle_input_event(KeyCode::Backspace);
        assert_eq!(app.input, "h");
        assert_eq!(app.cursor_position, 1);
    }
}
