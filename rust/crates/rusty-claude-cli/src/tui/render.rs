//! Message list rendering for the TUI.
//!
//! This module provides widgets for rendering a scrollable list of messages
//! with proper layout and styling.

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget, Widget, Wrap},
};

use runtime::messages::Message;

use crate::tui::messages::MessageWidget;

/// A scrollable list of messages.
#[derive(Debug, Clone)]
pub struct MessageList {
    /// The messages to display.
    messages: Vec<Message>,
    /// The scroll offset (line number at top of view).
    scroll_offset: usize,
    /// Whether to show the scrollbar.
    show_scrollbar: bool,
    /// The title for the message area.
    title: Option<String>,
}

impl MessageList {
    /// Create a new message list.
    #[must_use]
    pub fn new(messages: Vec<Message>) -> Self {
        Self {
            messages,
            scroll_offset: 0,
            show_scrollbar: true,
            title: None,
        }
    }

    /// Create an empty message list.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    /// Set the scroll offset.
    #[must_use]
    pub fn with_scroll_offset(mut self, offset: usize) -> Self {
        self.scroll_offset = offset;
        self
    }

    /// Set whether to show the scrollbar.
    #[must_use]
    pub fn with_scrollbar(mut self, show: bool) -> Self {
        self.show_scrollbar = show;
        self
    }

    /// Set the title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Get the number of messages.
    #[must_use]
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the list is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scroll up by the given number of lines.
    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
    }

    /// Scroll down by the given number of lines.
    pub fn scroll_down(&mut self, lines: usize) {
        // Max scroll is handled during render
        self.scroll_offset = self.scroll_offset.saturating_add(lines);
    }

    /// Scroll to the top.
    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    /// Scroll to the bottom.
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = usize::MAX;
    }

    /// Add a message to the list.
    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Extend the list with multiple messages.
    pub fn extend(&mut self, messages: impl IntoIterator<Item = Message>) {
        self.messages.extend(messages);
    }

    /// Clear all messages.
    pub fn clear(&mut self) {
        self.messages.clear();
        self.scroll_offset = 0;
    }

    /// Get the last message if any.
    #[must_use]
    pub fn last(&self) -> Option<&Message> {
        self.messages.last()
    }

    /// Calculate the total height of all messages.
    fn total_height(&self, width: u16) -> usize {
        self.messages
            .iter()
            .map(|msg| {
                MessageWidget::from_message(msg)
                    .map(|widget| widget.height(width) as usize)
                    .unwrap_or(0)
            })
            .sum()
    }

    /// Render the messages to the buffer.
    fn render_messages(&self, area: Rect, buf: &mut Buffer) {
        let mut current_y = area.y;
        let mut remaining_height = area.height;
        let mut current_line = 0;

        for message in &self.messages {
            // Check if we've reached the scroll offset
            let Some(widget) = MessageWidget::from_message(message) else {
                continue;
            };
            let message_height = widget.height(area.width);

            if current_line + message_height as usize <= self.scroll_offset {
                // Message is above the visible area
                current_line += message_height as usize;
                continue;
            }

            // Check if message fits in remaining space
            if remaining_height == 0 {
                break;
            }

            // Calculate how much of the message to show
            let visible_height = std::cmp::min(message_height, remaining_height);
            let message_area = Rect {
                x: area.x,
                y: current_y,
                width: area.width,
                height: visible_height,
            };

            // Render the message
            widget.render(message_area, buf);

            current_y += visible_height;
            remaining_height -= visible_height;
            current_line += message_height as usize;
        }
    }
}

impl StatefulWidget for MessageList {
    type State = MessageListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Update state with current message count
        state.message_count = self.messages.len();

        // Calculate total content height
        let content_height = self.total_height(area.width.saturating_sub(2));

        // Clamp scroll offset
        let max_scroll = content_height.saturating_sub(area.height as usize);
        let scroll_offset = std::cmp::min(self.scroll_offset, max_scroll);
        state.scroll_offset = scroll_offset;

        // Create the block
        let block = if let Some(title) = &self.title {
            Block::default()
                .title(title.as_str())
                .title_style(Style::default().add_modifier(Modifier::BOLD))
                .borders(Borders::ALL)
        } else {
            Block::default().borders(Borders::ALL)
        };

        let inner = block.inner(area);
        block.render(area, buf);

        // Render messages in the inner area
        self.render_messages(inner, buf);

        // Render scrollbar if needed
        if self.show_scrollbar && content_height > inner.height as usize {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::new(content_height)
                .position(scroll_offset)
                .content_length(content_height.saturating_sub(inner.height as usize));

            scrollbar.render(inner, buf, &mut scrollbar_state);
        }
    }
}

/// State for the message list widget.
#[derive(Debug, Clone, Default)]
pub struct MessageListState {
    /// The number of messages in the list.
    pub message_count: usize,
    /// The current scroll offset.
    pub scroll_offset: usize,
}

impl MessageListState {
    /// Create a new state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the message count.
    #[must_use]
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Get the scroll offset.
    #[must_use]
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }
}

/// Render a list of messages to the given area.
///
/// This is a convenience function for rendering messages without
/// managing the MessageList widget directly.
pub fn render_messages(
    messages: &[Message],
    area: Rect,
    buf: &mut Buffer,
    scroll_offset: usize,
) -> usize {
    let mut current_y = area.y;
    let mut remaining_height = area.height;
    let mut current_line = 0;
    let mut last_visible_line = scroll_offset;

    for message in messages {
        let Some(widget) = MessageWidget::from_message(message) else {
            continue;
        };
        let message_height = widget.height(area.width);

        if current_line + message_height as usize <= scroll_offset {
            // Message is above the visible area
            current_line += message_height as usize;
            continue;
        }

        if remaining_height == 0 {
            break;
        }

        let visible_height = std::cmp::min(message_height, remaining_height);
        let message_area = Rect {
            x: area.x,
            y: current_y,
            width: area.width,
            height: visible_height,
        };

        widget.render(message_area, buf);

        current_y += visible_height;
        remaining_height -= visible_height;
        current_line += message_height as usize;
        last_visible_line = current_line;
    }

    last_visible_line
}

/// Calculate the total height needed to render all messages.
#[must_use]
pub fn calculate_total_height(messages: &[Message], width: u16) -> u16 {
    messages
        .iter()
        .filter_map(|msg| MessageWidget::from_message(msg))
        .map(|widget| widget.height(width))
        .sum()
}

/// A widget for displaying an empty state.
#[derive(Debug, Clone)]
pub struct EmptyState {
    /// The message to display.
    pub message: String,
}

impl EmptyState {
    /// Create a new empty state widget.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Widget for EmptyState {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let text = Text::from(vec![
            Line::from(""),
            Line::from(Span::styled(
                self.message.clone(),
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::ITALIC),
            )),
        ]);

        let paragraph = Paragraph::new(text)
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: true });

        paragraph.render(area, buf);
    }
}

/// A widget for displaying a typing indicator.
#[derive(Debug, Clone)]
pub struct TypingIndicator {
    /// The current frame of the animation.
    pub frame: usize,
}

impl TypingIndicator {
    /// Create a new typing indicator.
    #[must_use]
    pub fn new(frame: usize) -> Self {
        Self { frame }
    }

    /// Get the current animation frame.
    #[must_use]
    pub fn dots(&self) -> &'static str {
        const FRAMES: &[&str] = &["  ", ". ", "..", "..."];
        FRAMES[self.frame % FRAMES.len()]
    }
}

impl Widget for TypingIndicator {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let dots = self.dots();
        let text = Text::from(vec![Line::from(vec![
            Span::styled("Claude is thinking", Style::default().fg(Color::Green)),
            Span::styled(dots, Style::default().fg(Color::Green)),
        ])]);

        Paragraph::new(text).render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_message_list_empty() {
        let list = MessageList::empty();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    // TODO: Fix this test - SystemMessage API has changed
    // #[test]
    // fn test_message_list_push() {
    //     let mut list = MessageList::empty();
    //     // Need to update for new SystemMessage API
    //     list.push(message);
    //     assert_eq!(list.len(), 1);
    //     assert!(!list.is_empty());
    // }

    #[test]
    fn test_message_list_scroll() {
        let mut list = MessageList::empty();
        list.scroll_down(10);
        assert_eq!(list.scroll_offset(), 10);
        list.scroll_up(5);
        assert_eq!(list.scroll_offset(), 5);
    }

    #[test]
    fn test_typing_indicator() {
        let indicator = TypingIndicator::new(0);
        assert_eq!(indicator.dots(), "  ");
        let indicator = TypingIndicator::new(2);
        assert_eq!(indicator.dots(), "..");
    }

    #[test]
    fn test_empty_state() {
        let empty = EmptyState::new("No messages");
        assert_eq!(empty.message, "No messages");
    }
}
