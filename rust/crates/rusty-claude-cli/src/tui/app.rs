//! Application state machine for the TUI.

use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Margin},
    widgets::{Clear, Paragraph},
    Frame,
};

use crate::tui::event::AppEvent;

/// A single message cell in the conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageCell {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// Scroll state for the chat area.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollState {
    pub offset: usize,
}

/// The TUI application.
#[derive(Debug)]
pub struct App {
    pub messages: Vec<MessageCell>,
    pub scroll: ScrollState,
    pub should_quit: bool,
    pub input_text: String,
}

impl App {
    pub fn new() -> Self {
        Self {
            messages: vec![
                MessageCell {
                    role: Role::System,
                    text: "Welcome to claw TUI! Press 'q' to quit.".to_string(),
                },
                MessageCell {
                    role: Role::User,
                    text: "Hello, this is a mock user message.".to_string(),
                },
                MessageCell {
                    role: Role::Assistant,
                    text: "And this is a mock assistant reply.\n
You can scroll with arrow keys.".to_string(),
                },
            ],
            scroll: ScrollState::default(),
            should_quit: false,
            input_text: String::new(),
        }
    }

    /// Handle an incoming application event.
    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key.code),
            AppEvent::Resize(_, _) => {}
            AppEvent::Quit => self.should_quit = true,
            AppEvent::AssistantDelta(text) => self.append_delta(text),
            AppEvent::ToolStart(_name) => {}
            AppEvent::ToolResult(_name, _result) => {}
            AppEvent::Tick => {}
        }
    }

    fn handle_key(&mut self, code: crossterm::event::KeyCode) {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => self.scroll_page_up(),
            KeyCode::PageDown => self.scroll_page_down(),
            KeyCode::Char(c) => {
                self.input_text.push(c);
            }
            KeyCode::Enter => {
                if !self.input_text.trim().is_empty() {
                    self.messages.push(MessageCell {
                        role: Role::User,
                        text: self.input_text.clone(),
                    });
                    self.input_text.clear();
                    self.scroll_to_bottom();
                }
            }
            _ => {}
        }
    }

    fn scroll_up(&mut self) {
        self.scroll.offset = self.scroll.offset.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        if self.scroll.offset + 1 < self.messages.len() {
            self.scroll.offset += 1;
        }
    }

    fn scroll_page_up(&mut self) {
        self.scroll.offset = self.scroll.offset.saturating_sub(5);
    }

    fn scroll_page_down(&mut self) {
        self.scroll.offset = (self.scroll.offset + 5).min(self.messages.len().saturating_sub(1));
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll.offset = self.messages.len().saturating_sub(1);
    }

    fn append_delta(&mut self, delta: String) {
        if let Some(last) = self.messages.last_mut() {
            if last.role == Role::Assistant {
                last.text.push_str(&delta);
                return;
            }
        }
        self.messages.push(MessageCell {
            role: Role::Assistant,
            text: delta,
        });
    }

    /// Draw the UI to the given frame.
    pub fn draw(&self, f: &mut ratatui::Frame) {
        use ratatui::widgets::{Block, Borders};
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)])
            .split(area);

        let chat_area = chunks[0];
        let input_area = chunks[1];
        let status_area = chunks[2];

        // Chat area: render messages
        let messages_to_show: Vec<_> = self
            .messages
            .iter()
            .skip(self.scroll.offset)
            .collect();

        let message_text = messages_to_show
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.text))
            .collect::<Vec<_>>()
            .join("\n---\n");

        let chat_paragraph = Paragraph::new(message_text);
        f.render_widget(chat_paragraph, chat_area);

        // Input area
        let input_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::ALL)
            .title("Input");
        let input_paragraph = Paragraph::new(self.input_text.as_str()).block(input_block);
        f.render_widget(input_paragraph, input_area);

        // Status bar
        let status = Paragraph::new("claw TUI | q: quit | ↑↓: scroll | Enter: send");
        f.render_widget(status, status_area);
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new() {
        let app = App::new();
        assert_eq!(app.messages.len(), 3);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_scroll_up_down() {
        let mut app = App::new();
        app.scroll.offset = 2;
        app.scroll_up();
        assert_eq!(app.scroll.offset, 1);
        app.scroll_down();
        assert_eq!(app.scroll.offset, 2);
    }

    #[test]
    fn test_append_delta() {
        let mut app = App::new();
        app.append_delta(" more".to_string());
        let last = app.messages.last().unwrap();
        assert_eq!(last.role, Role::Assistant);
        assert!(last.text.ends_with(" more"));
    }
}
