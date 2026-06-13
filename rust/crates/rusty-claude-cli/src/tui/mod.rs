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
    let result = tokio::runtime::Handle::try_current()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
        .block_on(run_app_async(&mut terminal));
    ratatui::restore();
    result
}

#[cfg(feature = "tui")]
async fn run_app_async<B: ratatui::backend::Backend>(
    terminal: &mut ratatui::Terminal<B>,
) -> io::Result<()> {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<event::AppEvent>(100);
    let mut app = app::App::new();

    loop {
        terminal.draw(|f| app.draw(f))?;

        tokio::select! {
            crossterm_event = read_crossterm_event_async() => {
                let app_event = crossterm_event?;
                app.handle_event(app_event.clone());

                if matches!(app_event, event::AppEvent::Quit) {
                    return Ok(());
                }

                // On Enter with non-empty input, start streaming
                if let event::AppEvent::Key(key) = &app_event {
                    use crossterm::event::KeyCode;
                    if key.code == KeyCode::Enter && !app.input_text.trim().is_empty() {
                        let tx_clone = tx.clone();
                        let input = app.input_text.clone();
                        tokio::spawn(async move {
                            stream_mock_response(input, tx_clone).await;
                        });
                    }
                }
            }
            Some(channel_event) = rx.recv() => {
                app.handle_event(channel_event);
            }
        }
    }
}

#[cfg(feature = "tui")]
async fn read_crossterm_event_async() -> io::Result<event::AppEvent> {
    tokio::task::spawn_blocking(|| event::next_event())
        .await
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?
}

#[cfg(feature = "tui")]
async fn stream_mock_response(_input: String, tx: tokio::sync::mpsc::Sender<event::AppEvent>) {
    use event::AppEvent;

    // Simulate typing a response word by word
    let words = ["Hello", "from", "the", "claw", "TUI!", "This", "is", "a", "streaming", "response."];

    // Small delay before starting
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    // Send a start signal (add assistant message cell)
    let _ = tx.send(AppEvent::AssistantDelta("".to_string())).await;

    for word in words {
        let _ = tx.send(AppEvent::AssistantDelta(format!(" {word}"))).await;
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Final newline to complete
    let _ = tx.send(AppEvent::AssistantDelta("\n".to_string())).await;
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
