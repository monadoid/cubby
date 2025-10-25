use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{self, Span},
    widgets::{Block, Gauge, LineGauge, Paragraph, Row, Sparkline, Table, Tabs, Wrap},
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
        0 => draw_first_tab(frame, app, layout[1]),
        1 => draw_second_tab(frame, app, layout[1]),
        _ => {}
    }
}

fn draw_first_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(9), Constraint::Min(0)])
        .split(area);

    draw_gauges(frame, app, chunks[0]);
    draw_charts(frame, app, chunks[1]);
}

fn draw_gauges(frame: &mut Frame, app: &mut App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .margin(1)
        .split(area);

    frame.render_widget(Block::bordered().title("Graphs"), area);

    let gauge = Gauge::default()
        .block(Block::new().title("Gauge:"))
        .gauge_style(
            Style::default()
                .fg(Color::Magenta)
                .bg(Color::Black)
                .add_modifier(Modifier::ITALIC | Modifier::BOLD),
        )
        .use_unicode(app.enhanced_graphics)
        .label(format!("{:.2}%", app.progress * 100.0))
        .ratio(app.progress);
    frame.render_widget(gauge, layout[0]);

    let sparkline = Sparkline::default()
        .block(Block::new().title("Sparkline:"))
        .style(Style::default().fg(Color::Green))
        .data(&app.sparkline.points)
        .bar_set(if app.enhanced_graphics {
            symbols::bar::NINE_LEVELS
        } else {
            symbols::bar::THREE_LEVELS
        });
    frame.render_widget(sparkline, layout[1]);

    let line_gauge = LineGauge::default()
        .block(Block::new().title("LineGauge:"))
        .filled_style(Style::default().fg(Color::Magenta))
        .line_set(if app.enhanced_graphics {
            symbols::line::THICK
        } else {
            symbols::line::NORMAL
        })
        .ratio(app.progress);
    frame.render_widget(line_gauge, layout[2]);
}

fn draw_charts(frame: &mut Frame, app: &mut App, area: Rect) {
    draw_logs(frame, app, area);
}

fn draw_logs(frame: &mut Frame, app: &mut App, area: Rect) {
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

fn draw_second_tab(frame: &mut Frame, app: &mut App, area: Rect) {
    let layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    let up_style = Style::default().fg(Color::Green);
    let failure_style = Style::default()
        .fg(Color::Red)
        .add_modifier(Modifier::RAPID_BLINK | Modifier::CROSSED_OUT);

    let rows = app.servers.iter().map(|server| {
        let style = if server.status == "Up" {
            up_style
        } else {
            failure_style
        };
        Row::new(vec![
            server.name.as_str(),
            server.location.as_str(),
            server.status.as_str(),
        ])
        .style(style)
    });

    let table = Table::new(
        rows,
        [
            Constraint::Length(15),
            Constraint::Length(15),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Server", "Location", "Status"])
            .style(Style::default().fg(Color::Yellow))
            .bottom_margin(1),
    )
    .block(Block::bordered().title("Servers"));
    frame.render_widget(table, layout[0]);

    let info = Paragraph::new("Secondary view placeholder")
        .block(Block::bordered().title("Overview"))
        .wrap(Wrap { trim: true });
    frame.render_widget(info, layout[1]);
}
