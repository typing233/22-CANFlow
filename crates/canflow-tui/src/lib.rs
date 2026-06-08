pub mod app;
pub mod input;
pub mod render;
pub mod panels;

pub use app::App;

use canflow_analysis::Alert;
use canflow_bus::{LiveStats, LiveStatsSnapshot};
use canflow_types::CanFrame;
use crossterm::event::{self, Event};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::prelude::*;
use std::io::stdout;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

pub async fn run_tui(
    mut frame_rx: broadcast::Receiver<Arc<CanFrame>>,
    mut alert_rx: mpsc::Receiver<Alert>,
    live_stats: Arc<LiveStats>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) -> std::io::Result<()> {
    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;

    let mut app = App::new();
    let tick_rate = Duration::from_millis(50);

    loop {
        // Draw
        terminal.draw(|frame| render::draw(frame, &app))?;

        // Handle events
        if crossterm::event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                input::handle_key_event(&mut app, key);
            }
        }

        if app.should_quit {
            break;
        }

        // Drain frames
        loop {
            match frame_rx.try_recv() {
                Ok(frame) => app.push_frame(frame),
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!(lagged = n, "TUI lagged");
                }
                Err(_) => break,
            }
        }

        // Drain alerts
        while let Ok(alert) = alert_rx.try_recv() {
            app.push_alert(alert);
        }

        // Update stats
        app.update_stats(live_stats.snapshot());

        // Check shutdown
        if shutdown.has_changed().unwrap_or(false) {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    Ok(())
}
