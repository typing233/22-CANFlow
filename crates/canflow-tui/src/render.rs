use ratatui::prelude::*;
use ratatui::widgets::*;
use crate::app::App;
use crate::panels;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // tabs
            Constraint::Min(10),   // main content
            Constraint::Length(3), // status bar
        ])
        .split(frame.area());

    // Tab bar
    let tabs = Tabs::new(vec!["Traffic", "Stats", "Alerts"])
        .select(app.selected_tab)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .divider("|");

    let tab_block = Block::default()
        .title(" CANFlow ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    frame.render_widget(tabs.block(tab_block), chunks[0]);

    // Main content
    match app.selected_tab {
        0 => panels::live_traffic::render(frame, app, chunks[1]),
        1 => panels::stats::render(frame, app, chunks[1]),
        2 => panels::alerts::render(frame, app, chunks[1]),
        _ => {}
    }

    // Status bar
    let status = if app.paused {
        " PAUSED | [Space] Resume | [Tab] Switch | [Q] Quit "
    } else {
        " LIVE | [Space] Pause | [Tab] Switch | [Q] Quit "
    };

    let status_bar = Paragraph::new(status)
        .style(Style::default().fg(Color::Black).bg(if app.paused {
            Color::Yellow
        } else {
            Color::Green
        }));

    frame.render_widget(status_bar, chunks[2]);
}
