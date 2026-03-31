//! Input handling widgets for the TUI.
//!
//! This module provides widgets for:
//! - Prompt input with cursor positioning
//! - Multiline text input
//! - Input history management
//! - Keyboard shortcut display

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Widget, Wrap},
};

/// A prompt input widget with history support.
#[derive(Debug, Clone)]
pub struct PromptWidget {
    /// The current input buffer.
    pub input: String,
    /// Cursor position in the input (byte index).
    pub cursor_position: usize,
    /// Input history.
    pub history: Vec<String>,
    /// Current position in history (None = not navigating history).
    pub history_position: Option<usize>,
    /// Saved input when navigating history (to restore on cancel).
    pub saved_input: String,
    /// Whether the prompt is in multiline mode.
    pub multiline: bool,
    /// Placeholder text when input is empty.
    pub placeholder: String,
    /// Title for the prompt block.
    pub title: String,
    /// Whether the prompt is waiting/processing.
    pub is_waiting: bool,
}

impl Default for PromptWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptWidget {
    /// Create a new prompt widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            input: String::new(),
            cursor_position: 0,
            history: Vec::new(),
            history_position: None,
            saved_input: String::new(),
            multiline: false,
            placeholder: "Type your message...".to_string(),
            title: " Prompt ".to_string(),
            is_waiting: false,
        }
    }

    /// Set the placeholder text.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set multiline mode.
    #[must_use]
    pub fn with_multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Set waiting state.
    #[must_use]
    pub fn with_waiting(mut self, waiting: bool) -> Self {
        self.is_waiting = waiting;
        self
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
        // Clear history navigation when typing
        self.history_position = None;
    }

    /// Insert a string at the cursor position.
    pub fn insert_str(&mut self, s: &str) {
        self.input.insert_str(self.cursor_position, s);
        self.cursor_position += s.len();
        self.history_position = None;
    }

    /// Delete character before cursor (backspace).
    pub fn backspace(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous character boundary
            let prev_pos = self.input[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.input.remove(prev_pos);
            self.cursor_position = prev_pos;
        }
    }

    /// Delete character at cursor (delete key).
    pub fn delete(&mut self) {
        if self.cursor_position < self.input.len() {
            self.input.remove(self.cursor_position);
        }
    }

    /// Move cursor left.
    pub fn move_left(&mut self) {
        if self.cursor_position > 0 {
            // Find the previous character boundary
            let prev_pos = self.input[..self.cursor_position]
                .char_indices()
                .last()
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            self.cursor_position = prev_pos;
        }
    }

    /// Move cursor right.
    pub fn move_right(&mut self) {
        if self.cursor_position < self.input.len() {
            // Find the next character boundary
            let next_pos = self.input[self.cursor_position..]
                .chars()
                .next()
                .map(|c| self.cursor_position + c.len_utf8())
                .unwrap_or(self.input.len());
            self.cursor_position = next_pos;
        }
    }

    /// Move cursor to start of line.
    pub fn move_to_start(&mut self) {
        if self.multiline {
            // Find the start of the current line
            let line_start = self.input[..self.cursor_position]
                .rfind('\n')
                .map(|idx| idx + 1)
                .unwrap_or(0);
            self.cursor_position = line_start;
        } else {
            self.cursor_position = 0;
        }
    }

    /// Move cursor to end of line.
    pub fn move_to_end(&mut self) {
        if self.multiline {
            // Find the end of the current line
            let line_end = self.input[self.cursor_position..]
                .find('\n')
                .map(|idx| self.cursor_position + idx)
                .unwrap_or(self.input.len());
            self.cursor_position = line_end;
        } else {
            self.cursor_position = self.input.len();
        }
    }

    /// Move cursor to start of input.
    pub fn move_to_home(&mut self) {
        self.cursor_position = 0;
    }

    /// Move cursor to end of input.
    pub fn move_to_end_of_input(&mut self) {
        self.cursor_position = self.input.len();
    }

    /// Navigate to previous history entry.
    pub fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }

        // Save current input if starting history navigation
        if self.history_position.is_none() {
            self.saved_input = self.input.clone();
        }

        let new_pos = self.history_position.map_or(
            self.history.len() - 1, // Start from most recent
            |pos| pos.saturating_sub(1),
        );

        if new_pos < self.history.len() {
            self.history_position = Some(new_pos);
            self.input = self.history[new_pos].clone();
            self.cursor_position = self.input.len();
        }
    }

    /// Navigate to next history entry.
    pub fn history_next(&mut self) {
        match self.history_position {
            None => {} // Not in history, do nothing
            Some(pos) => {
                let new_pos = pos + 1;
                if new_pos < self.history.len() {
                    self.history_position = Some(new_pos);
                    self.input = self.history[new_pos].clone();
                    self.cursor_position = self.input.len();
                } else {
                    // Restore saved input and exit history navigation
                    self.history_position = None;
                    self.input = std::mem::take(&mut self.saved_input);
                    self.cursor_position = self.input.len();
                }
            }
        }
    }

    /// Submit the current input, adding it to history.
    /// Returns the submitted input and clears the buffer.
    pub fn submit(&mut self) -> String {
        let input = std::mem::take(&mut self.input);
        self.cursor_position = 0;
        self.history_position = None;
        self.saved_input.clear();

        // Add to history if not empty and not a duplicate of the most recent
        if !input.is_empty() {
            if self.history.last() != Some(&input) {
                self.history.push(input.clone());
                // Limit history size
                if self.history.len() > 100 {
                    self.history.remove(0);
                }
            }
        }

        input
    }

    /// Clear the input buffer.
    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
        self.history_position = None;
    }

    /// Check if input is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// Get the current input.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Calculate the cursor position for rendering.
    /// Returns (x, y) coordinates within the given width.
    #[must_use]
    pub fn cursor_coords(&self, width: u16) -> (u16, u16) {
        let display_width = width.saturating_sub(2) as usize; // Account for borders

        if self.multiline {
            // Calculate line and column for multiline input
            let text_before_cursor = &self.input[..self.cursor_position];
            let lines: Vec<&str> = text_before_cursor.split('\n').collect();
            let y = lines.len().saturating_sub(1) as u16;

            let last_line = lines.last().unwrap_or(&"");
            let x = last_line.len().min(display_width) as u16 + 1; // +1 for left border

            (x, y)
        } else {
            // Single line - just use cursor position
            let x = self.cursor_position.min(display_width) as u16 + 1;
            (x, 1)
        }
    }

    /// Calculate the height needed for this widget.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        let content_width = width.saturating_sub(2) as usize;

        if self.multiline {
            // Count lines based on newlines and wrapping
            let lines: Vec<&str> = self.input.split('\n').collect();
            let mut total_lines = 0;
            for line in lines {
                let wrapped_lines = textwrap::wrap(line, content_width);
                total_lines += wrapped_lines.len().max(1);
            }
            total_lines.max(1) as u16 + 2 // +2 for borders
        } else {
            3 // Single line with borders
        }
    }
}

impl Widget for PromptWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let display_title = if self.is_waiting {
            " Thinking... ".to_string()
        } else {
            self.title.clone()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(display_title)
            .border_style(if self.is_waiting {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Blue)
            });

        let inner = block.inner(area);
        block.render(area, buf);

        // Render input or placeholder
        let display_text = if self.input.is_empty() && !self.is_waiting {
            self.placeholder.clone()
        } else {
            self.input.clone()
        };

        let style = if self.input.is_empty() && !self.is_waiting {
            Style::default().fg(Color::Gray).add_modifier(Modifier::ITALIC)
        } else {
            Style::default().fg(Color::White)
        };

        let paragraph = Paragraph::new(display_text)
            .style(style)
            .wrap(Wrap { trim: false });

        paragraph.render(inner, buf);
    }
}

/// State for tracking cursor position in the prompt.
#[derive(Debug, Clone, Default)]
pub struct PromptState {
    /// The visual cursor X position.
    pub cursor_x: u16,
    /// The visual cursor Y position.
    pub cursor_y: u16,
}

impl PromptState {
    /// Create a new prompt state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Update cursor position from a prompt widget.
    pub fn update_from_prompt(&mut self, prompt: &PromptWidget, width: u16) {
        let (x, y) = prompt.cursor_coords(width);
        self.cursor_x = x;
        self.cursor_y = y;
    }
}

/// A widget for displaying keyboard shortcuts.
#[derive(Debug, Clone)]
pub struct ShortcutHelpWidget {
    /// Shortcuts to display.
    pub shortcuts: Vec<(String, String)>,
    /// Style for the shortcut key.
    pub key_style: Style,
    /// Style for the description.
    pub desc_style: Style,
}

impl Default for ShortcutHelpWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutHelpWidget {
    /// Create a new shortcut help widget with default shortcuts.
    #[must_use]
    pub fn new() -> Self {
        let shortcuts = vec![
            ("Enter".to_string(), "Submit".to_string()),
            ("Esc".to_string(), "Cancel".to_string()),
            ("↑/↓".to_string(), "History".to_string()),
            ("Ctrl+C".to_string(), "Quit".to_string()),
        ];

        Self {
            shortcuts,
            key_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            desc_style: Style::default().fg(Color::Gray),
        }
    }

    /// Create with custom shortcuts.
    #[must_use]
    pub fn with_shortcuts(shortcuts: Vec<(String, String)>) -> Self {
        Self {
            shortcuts,
            key_style: Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            desc_style: Style::default().fg(Color::Gray),
        }
    }

    /// Calculate the width needed.
    #[must_use]
    pub fn width(&self) -> u16 {
        let total: usize = self.shortcuts.iter().map(|(k, d)| k.len() + d.len() + 4).sum();
        total as u16
    }
}

impl Widget for ShortcutHelpWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut spans: Vec<Span<'_>> = Vec::new();

        for (i, (key, desc)) in self.shortcuts.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(" | "));
            }
            spans.push(Span::styled(key.clone(), self.key_style));
            spans.push(Span::raw(": "));
            spans.push(Span::styled(desc.clone(), self.desc_style));
        }

        let line = Line::from(spans);
        let text = Text::from(vec![line]);

        Paragraph::new(text).render(area, buf);
    }
}

/// A multiline text input widget.
#[derive(Debug, Clone)]
pub struct MultilineInputWidget {
    /// The prompt widget backing this input.
    prompt: PromptWidget,
    /// Maximum number of lines.
    max_lines: usize,
}

impl MultilineInputWidget {
    /// Create a new multiline input widget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            prompt: PromptWidget::new().with_multiline(true),
            max_lines: 10,
        }
    }

    /// Set max lines.
    #[must_use]
    pub fn with_max_lines(mut self, max: usize) -> Self {
        self.max_lines = max;
        self
    }

    /// Insert a newline if under max lines.
    pub fn insert_newline(&mut self) {
        let current_lines = self.prompt.input.matches('\n').count() + 1;
        if current_lines < self.max_lines {
            self.prompt.insert_char('\n');
        }
    }

    /// Delegate to inner prompt methods.
    pub fn insert_char(&mut self, c: char) {
        self.prompt.insert_char(c);
    }

    pub fn insert_str(&mut self, s: &str) {
        self.prompt.insert_str(s);
    }

    pub fn backspace(&mut self) {
        self.prompt.backspace();
    }

    pub fn delete(&mut self) {
        self.prompt.delete();
    }

    pub fn move_left(&mut self) {
        self.prompt.move_left();
    }

    pub fn move_right(&mut self) {
        self.prompt.move_right();
    }

    pub fn move_up(&mut self) {
        // In multiline, try to move to previous line
        if let Some(prev_newline) = self.prompt.input[..self.prompt.cursor_position].rfind('\n') {
            // Calculate column on current line
            let current_line_start = self.prompt.input[..self.prompt.cursor_position]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let column = self.prompt.cursor_position - current_line_start;

            // Move to previous line at same column (or end of line)
            let prev_line_start = self.prompt.input[..prev_newline]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let prev_line_length = prev_newline - prev_line_start;

            self.prompt.cursor_position = prev_line_start + column.min(prev_line_length);
        }
    }

    pub fn move_down(&mut self) {
        // Find next newline and move to that line
        let current_line_end = self.prompt.input[self.prompt.cursor_position..]
            .find('\n')
            .map(|i| self.prompt.cursor_position + i)
            .unwrap_or(self.prompt.input.len());

        if current_line_end < self.prompt.input.len() {
            // Calculate column on current line
            let current_line_start = self.prompt.input[..self.prompt.cursor_position]
                .rfind('\n')
                .map(|i| i + 1)
                .unwrap_or(0);
            let column = self.prompt.cursor_position - current_line_start;

            // Move to next line at same column
            let next_line_start = current_line_end + 1;
            let next_line_end = self.prompt.input[next_line_start..]
                .find('\n')
                .map(|i| next_line_start + i)
                .unwrap_or(self.prompt.input.len());
            let next_line_length = next_line_end - next_line_start;

            self.prompt.cursor_position = next_line_start + column.min(next_line_length);
        }
    }

    pub fn submit(&mut self) -> String {
        self.prompt.submit()
    }

    pub fn clear(&mut self) {
        self.prompt.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.prompt.is_empty()
    }

    pub fn input(&self) -> &str {
        self.prompt.input()
    }

    pub fn cursor_position(&self) -> usize {
        self.prompt.cursor_position
    }

    pub fn history_previous(&mut self) {
        self.prompt.history_previous();
    }

    pub fn history_next(&mut self) {
        self.prompt.history_next();
    }

    /// Calculate height needed.
    #[must_use]
    pub fn height(&self, width: u16) -> u16 {
        self.prompt.height(width)
    }

    /// Get cursor coordinates.
    #[must_use]
    pub fn cursor_coords(&self, width: u16) -> (u16, u16) {
        self.prompt.cursor_coords(width)
    }
}

impl Default for MultilineInputWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for MultilineInputWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.prompt.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_widget_new() {
        let prompt = PromptWidget::new();
        assert!(prompt.is_empty());
        assert_eq!(prompt.cursor_position, 0);
        assert!(prompt.history.is_empty());
    }

    #[test]
    fn test_prompt_insert_char() {
        let mut prompt = PromptWidget::new();
        prompt.insert_char('h');
        prompt.insert_char('i');
        assert_eq!(prompt.input, "hi");
        assert_eq!(prompt.cursor_position, 2);
    }

    #[test]
    fn test_prompt_backspace() {
        let mut prompt = PromptWidget::new();
        prompt.insert_str("hello");
        prompt.backspace();
        assert_eq!(prompt.input, "hell");
        assert_eq!(prompt.cursor_position, 4);
    }

    #[test]
    fn test_prompt_backspace_at_start() {
        let mut prompt = PromptWidget::new();
        prompt.backspace(); // Should not panic
        assert!(prompt.is_empty());
    }

    #[test]
    fn test_prompt_delete() {
        let mut prompt = PromptWidget::new();
        prompt.insert_str("hello");
        prompt.move_to_start();
        prompt.delete();
        assert_eq!(prompt.input, "ello");
    }

    #[test]
    fn test_prompt_cursor_movement() {
        let mut prompt = PromptWidget::new();
        prompt.insert_str("hello");

        prompt.move_to_home();
        assert_eq!(prompt.cursor_position, 0);

        prompt.move_to_end_of_input();
        assert_eq!(prompt.cursor_position, 5);

        prompt.move_left();
        assert_eq!(prompt.cursor_position, 4);

        prompt.move_right();
        assert_eq!(prompt.cursor_position, 5);
    }

    #[test]
    fn test_prompt_submit_adds_to_history() {
        let mut prompt = PromptWidget::new();
        prompt.insert_str("test message");
        let submitted = prompt.submit();

        assert_eq!(submitted, "test message");
        assert!(prompt.is_empty());
        assert_eq!(prompt.history.len(), 1);
        assert_eq!(prompt.history[0], "test message");
    }

    #[test]
    fn test_prompt_history_navigation() {
        let mut prompt = PromptWidget::new();

        // Add some history
        prompt.insert_str("first");
        prompt.submit();
        prompt.insert_str("second");
        prompt.submit();
        prompt.insert_str("third");
        prompt.submit();

        // Navigate back
        prompt.history_previous();
        assert_eq!(prompt.input, "third");

        prompt.history_previous();
        assert_eq!(prompt.input, "second");

        prompt.history_previous();
        assert_eq!(prompt.input, "first");

        // Navigate forward
        prompt.history_next();
        assert_eq!(prompt.input, "second");

        prompt.history_next();
        assert_eq!(prompt.input, "third");

        // Navigate past end should restore saved input
        prompt.history_next();
        assert!(prompt.input.is_empty());
    }

    #[test]
    fn test_prompt_history_dedup() {
        let mut prompt = PromptWidget::new();

        prompt.insert_str("duplicate");
        prompt.submit();
        prompt.insert_str("duplicate");
        prompt.submit();

        assert_eq!(prompt.history.len(), 1);
    }

    #[test]
    fn test_prompt_builder_methods() {
        let prompt = PromptWidget::new()
            .with_placeholder("Custom placeholder...")
            .with_title(" Custom ")
            .with_multiline(true)
            .with_waiting(true);

        assert_eq!(prompt.placeholder, "Custom placeholder...");
        assert_eq!(prompt.title, " Custom ");
        assert!(prompt.multiline);
        assert!(prompt.is_waiting);
    }

    #[test]
    fn test_multiline_input() {
        let mut input = MultilineInputWidget::new();
        input.insert_str("line 1");
        input.insert_newline();
        input.insert_str("line 2");

        assert_eq!(input.input(), "line 1\nline 2");
        assert_eq!(input.input().matches('\n').count(), 1);
    }

    #[test]
    fn test_multiline_max_lines() {
        let mut input = MultilineInputWidget::new().with_max_lines(2);
        input.insert_str("1");
        input.insert_newline();
        input.insert_str("2");
        input.insert_newline(); // Should not add third line
        input.insert_str("3");

        assert_eq!(input.input().matches('\n').count(), 1);
    }

    #[test]
    fn test_shortcut_help_widget() {
        let shortcuts = vec![
            ("Ctrl+X".to_string(), "Cut".to_string()),
            ("Ctrl+C".to_string(), "Copy".to_string()),
        ];
        let widget = ShortcutHelpWidget::with_shortcuts(shortcuts);

        assert!(!widget.shortcuts.is_empty());
        assert!(widget.width() > 0);
    }

    #[test]
    fn test_prompt_cursor_coords_single_line() {
        let mut prompt = PromptWidget::new();
        prompt.insert_str("hello");

        let (x, y) = prompt.cursor_coords(20);
        assert_eq!(x, 6); // cursor at position 5 + 1 for border
        assert_eq!(y, 1); // single line
    }

    #[test]
    fn test_prompt_cursor_coords_multiline() {
        let mut prompt = PromptWidget::new().with_multiline(true);
        prompt.insert_str("line1");
        prompt.insert_char('\n');
        prompt.insert_str("line2");

        let (x, y) = prompt.cursor_coords(20);
        assert_eq!(x, 7); // "line2" len + 1 for border
        assert_eq!(y, 1); // second line (0-indexed)
    }
}
