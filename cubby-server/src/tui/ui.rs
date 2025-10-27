use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{self, Span},
    widgets::{Block, Paragraph, Row, Table, Tabs, Wrap},
    Frame,
};

use super::app::App;

pub fn draw(frame: &mut Frame, app: &mut App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(frame.area());

    let tabs = app
        .tabs
        .titles
        .iter()
        .map(|t| text::Line::from(Span::styled(*t, Style::default().fg(Color::Green))))
        .collect::<Tabs>()
        .block(Block::bordered().title(app.title.as_str()))
        .highlight_style(Style::default().fg(Color::Yellow))
        .select(app.tabs.index);
    frame.render_widget(tabs, layout[0]);

    match app.tabs.index {
        0 => draw_summary_tab(frame, app, layout[1]),
        1 => draw_log_tab(frame, app, layout[1]),
        _ => {}
    }
}

fn draw_summary_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let rows: Vec<Row> = app
        .logs
        .items
        .iter()
        .map(|entry| {
            let confidence = entry
                .confidence
                .map(|c| format!("{:.2}", c))
                .unwrap_or_else(|| "-".to_string());
            Row::new(vec![
                entry.time.clone(),
                entry.event.clone(),
                entry.detail.clone(),
                entry.app.clone().unwrap_or_else(|| "-".to_string()),
                entry.window.clone().unwrap_or_else(|| "-".to_string()),
                confidence,
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Length(18),
            Constraint::Percentage(50),
            Constraint::Length(18),
            Constraint::Length(24),
            Constraint::Length(7),
        ],
    )
    .header(
        Row::new(vec!["Time", "Event", "Detail", "App", "Window", "Conf"])
            .style(Style::default().fg(Color::Yellow))
            .bottom_margin(1),
    )
    .block(Block::bordered().title("Live summaries"));

    frame.render_widget(table, area);
}

fn draw_log_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let mut text = String::new();
    for line in app.log_lines.iter().rev() {
        text.push_str(line);
        if !line.ends_with('\n') {
            text.push('\n');
        }
    }

    let paragraph = Paragraph::new(text)
        .block(Block::bordered().title("Recent logs"))
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
