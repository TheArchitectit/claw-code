//! Terminal UI module for the `claw` CLI.
//!
//! This module is gated behind the `tui` Cargo feature. When the feature is not
//! enabled, `claw tui` prints a help message and exits cleanly.

#[cfg(feature = "tui")]
mod app;
#[cfg(feature = "tui")]
mod event;

use std::io;

pub fn run() -> io::Result<()> {
    #[cfg(feature = "tui")]
    {
        run_tui()
    }
    #[cfg(not(feature = "tui"))]
    {
        eprintln!("The TUI is not available in this build.");
        eprintln!("Rebuild with: cargo build --features tui");
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "tui feature not enabled",
        ))
    }
}

#[cfg(feature = "tui")]
fn run_tui() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let result = run_app(&mut terminal);
    ratatui::restore();
    result
}

#[cfg(feature = "tui")]
fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
) -> io::Result<()> {
    let mut app = app::App::new();

    loop {
        terminal.draw(|f| app.draw(f))?;

        let event = event::next_event()?;
        app.handle_event(event);

        if app.should_quit {
            return Ok(());
        }
    }
}

#[cfg(all(test, feature = "tui"))]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use super::app::App;

    #[test]
    fn test_tui_app_renders_to_buffer() {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let app = App::new();
        terminal.draw(|f| app.draw(f)).unwrap();

        let buffer = terminal.backend().buffer();
        let content: String = buffer.content.iter().map(|c| c.symbol()).collect();
        assert!(content.contains("Welcome to claw TUI"));
    }

    #[test]
    fn test_tui_app_quits_on_q() {
        let mut app = App::new();
        assert!(!app.should_quit);
        app.handle_event(super::event::AppEvent::Quit);
        assert!(app.should_quit);
    }
}
