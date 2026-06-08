use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Top: general stats
    let stats_text = if let Some(ref stats) = app.stats {
        vec![
            Line::from(format!("  Uptime:       {:.1}s", stats.uptime_secs)),
            Line::from(format!("  Current FPS:  {:.0}", stats.current_fps)),
            Line::from(format!("  Total Frames: {}", stats.total_frames)),
            Line::from(format!("  Unique IDs:   {}", stats.unique_ids)),
            Line::from(""),
        ]
    } else {
        vec![Line::from("  Waiting for data...")]
    };

    let stats_widget = Paragraph::new(stats_text)
        .block(Block::default().title(" Statistics ").borders(Borders::ALL));
    frame.render_widget(stats_widget, chunks[0]);

    // Bottom: top IDs
    let top_ids_text = if let Some(ref stats) = app.stats {
        stats
            .top_ids
            .iter()
            .enumerate()
            .map(|(i, (id, count))| {
                Line::from(format!("  {:2}. 0x{:03X}  {:>8} frames", i + 1, id, count))
            })
            .collect()
    } else {
        vec![Line::from("  No data yet")]
    };

    let top_ids_widget = Paragraph::new(top_ids_text)
        .block(Block::default().title(" Top Frame IDs ").borders(Borders::ALL));
    frame.render_widget(top_ids_widget, chunks[1]);
}
