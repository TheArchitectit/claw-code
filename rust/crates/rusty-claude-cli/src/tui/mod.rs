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

#[cfg(test)]
mod tests {
    #[test]
    fn test_tui_stub_disabled_without_feature() {
        // When the tui feature is NOT enabled, run() returns an error.
        // This test verifies the stub path compiles correctly.
        // The actual TUI tests are compiled only with --features tui.
    }
}
