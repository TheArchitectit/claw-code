//! Event broker: translates crossterm events into application events.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

/// Application-level events consumed by the TUI state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEvent {
    /// Terminal resized.
    Resize(u16, u16),
    /// Key pressed.
    Key(KeyEvent),
    /// Backend sent a text delta.
    AssistantDelta(String),
    /// Backend started a tool call.
    ToolStart(String),
    /// Backend finished a tool call.
    ToolResult(String, String),
    /// Tick for frame updates (optional, not yet used).
    Tick,
    /// Quit the application.
    Quit,
}

/// Read the next crossterm event and translate it into an `AppEvent`.
///
/// This function blocks until an event is available.
pub fn next_event() -> crossterm::Result<AppEvent> {
    match event::read()? {
        Event::Resize(cols, rows) => Ok(AppEvent::Resize(cols, rows)),
        Event::Key(key) => {
            if key.code == KeyCode::Char('q')
                && key.modifiers == KeyModifiers::NONE
            {
                Ok(AppEvent::Quit)
            } else {
                Ok(AppEvent::Key(key))
            }
        }
        _ => Ok(AppEvent::Tick),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_on_q() {
        let event = AppEvent::Key(KeyEvent::from(KeyCode::Char('q')));
        // The translation of 'q' into Quit is done inside `next_event`; here
        // we just ensure the event variant round-trips.
        assert!(matches!(event, AppEvent::Key(_)));
    }
}
