use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::app::App;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["Time", "ID", "DLC", "Data", "Iface"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))
        .bottom_margin(1);

    let rows: Vec<Row> = app
        .frames
        .iter()
        .rev()
        .take(area.height as usize - 4)
        .map(|f| {
            let ts = format!("{:.6}", f.timestamp_ns as f64 / 1_000_000_000.0);
            let id = format!("{}", f.id);
            let dlc = format!("{}", f.dlc);
            let data: String = f.payload().iter().map(|b| format!("{:02X} ", b)).collect();
            let iface = format!("{}", f.interface.0);

            Row::new(vec![ts, id, dlc, data.trim().to_string(), iface])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(16),
            Constraint::Length(10),
            Constraint::Length(4),
            Constraint::Min(24),
            Constraint::Length(6),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" Live Traffic ({} frames) ", app.frames.len()))
            .borders(Borders::ALL),
    );

    frame.render_widget(table, area);
}
