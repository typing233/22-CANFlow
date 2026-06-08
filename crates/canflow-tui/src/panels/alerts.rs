use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::app::App;
use canflow_analysis::Severity;

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .alerts
        .iter()
        .rev()
        .take(area.height as usize - 4)
        .map(|alert| {
            let style = match alert.severity {
                Severity::Info => Style::default().fg(Color::Blue),
                Severity::Warning => Style::default().fg(Color::Yellow),
                Severity::Critical => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            };

            let id_str = alert.frame_id.map_or("-".to_string(), |id| format!("0x{:03X}", id));

            Row::new(vec![
                format!("{}", alert.severity),
                alert.analyzer.clone(),
                id_str,
                alert.message.clone(),
            ])
            .style(style)
        })
        .collect();

    let header = Row::new(vec!["Severity", "Analyzer", "Frame ID", "Message"])
        .style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan))
        .bottom_margin(1);

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(" Alerts ({}) ", app.alerts.len()))
            .borders(Borders::ALL),
    );

    frame.render_widget(table, area);
}
