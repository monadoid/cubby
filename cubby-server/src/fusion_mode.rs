use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::tui::{app::App, ui};
use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Local, Utc};
use cubby_db::DatabaseManager;
use cubby_events::subscribe_to_event;
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{self, Event as CrosstermEvent, KeyCode},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use serde_json::Value;
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
};

pub struct FusionModeHandles {
    pub ui: JoinHandle<Result<()>>,
    pub forwarder: JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct FusionModeConfig {
    pub history: ChronoDuration,
    pub max_events: usize,
    pub initial_limit: i64,
    pub tick_rate: Duration,
}

impl Default for FusionModeConfig {
    fn default() -> Self {
        Self {
            history: ChronoDuration::hours(2),
            max_events: 512,
            initial_limit: 256,
            tick_rate: Duration::from_millis(250),
        }
    }
}

struct LiveSummaryEvent {
    label: String,
    detail: String,
    app: Option<String>,
    window: Option<String>,
    confidence: Option<f32>,
    timestamp: DateTime<Utc>,
}

pub async fn start_fusion_mode(
    _db: Arc<DatabaseManager>,
    config: FusionModeConfig,
    shutdown_rx: broadcast::Receiver<()>,
) -> Result<Option<FusionModeHandles>> {
    let (tx, rx) = mpsc::unbounded_channel::<LiveSummaryEvent>();

    let mut event_stream = subscribe_to_event::<Value>("live_summary");
    let mut forwarder_shutdown = shutdown_rx.resubscribe();
    let forwarder = tokio::spawn(async move {
        loop {
            tokio::select! {
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(event) => {
                            let payload = extract_events(&event.data, Utc::now());
                            if payload.is_empty() {
                                continue;
                            }
                            for item in payload {
                                if tx.send(item).is_err() {
                                    return;
                                }
                            }
                        }
                        None => return,
                    }
                }
                _ = forwarder_shutdown.recv() => {
                    return;
                }
            }
        }
    });

    let ui_handle = tokio::task::spawn_blocking(move || run_ui(rx, shutdown_rx, config.tick_rate));

    Ok(Some(FusionModeHandles {
        ui: ui_handle,
        forwarder,
    }))
}

fn run_ui(
    mut rx: mpsc::UnboundedReceiver<LiveSummaryEvent>,
    mut shutdown_rx: broadcast::Receiver<()>,
    tick_rate: Duration,
) -> Result<()> {
    enable_raw_mode().context("enabling raw mode for fusion mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("creating terminal for fusion mode")?;
    terminal.hide_cursor()?;

    let mut app = App::new("Fusion Mode – live summaries (q to exit)", true);
    let mut last_tick = Instant::now();

    loop {
        drain_events(&mut app, &mut rx);

        terminal
            .draw(|f| ui::draw(f, &mut app))
            .context("drawing fusion mode frame")?;

        if app.should_quit {
            break;
        }
        if shutdown_rx.try_recv().is_ok() {
            break;
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        let mut should_quit = false;
        if event::poll(timeout).context("polling terminal events")? {
            match event::read().context("reading terminal event")? {
                CrosstermEvent::Key(key) => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => should_quit = true,
                    KeyCode::Left => app.on_left(),
                    KeyCode::Right => app.on_right(),
                    KeyCode::Up => app.on_up(),
                    KeyCode::Down => app.on_down(),
                    KeyCode::Char('t') => app.on_key('t'),
                    _ => {}
                },
                CrosstermEvent::Resize(_, _) => {}
                _ => {}
            }
        }

        if should_quit {
            break;
        }

        if last_tick.elapsed() >= tick_rate {
            app.on_tick();
            last_tick = Instant::now();
        }
    }

    terminal.show_cursor()?;
    let mut stdout = io::stdout();
    execute!(stdout, LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

fn drain_events(app: &mut App, rx: &mut mpsc::UnboundedReceiver<LiveSummaryEvent>) {
    while let Ok(event) = rx.try_recv() {
        let local_time = event.timestamp.with_timezone(&Local);
        app.push_summary(
            local_time.format("%H:%M:%S").to_string(),
            event.label,
            event.detail,
            event.app,
            event.window,
            event.confidence,
        );
    }
}

fn extract_events(value: &Value, fallback: DateTime<Utc>) -> Vec<LiveSummaryEvent> {
    parse_event_object(value, fallback).into_iter().collect()
}

fn parse_event_object(value: &Value, fallback: DateTime<Utc>) -> Option<LiveSummaryEvent> {
    let label = value.get("label")?.as_str()?.trim().to_string();
    if label.is_empty() {
        return None;
    }
    let detail = value
        .get("detail")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if detail.is_empty() {
        return None;
    }

    let app = value
        .get("app")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let window = value
        .get("window")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let confidence = value
        .get("confidence")
        .and_then(|v| v.as_f64())
        .map(|c| c as f32);

    let ts = value
        .get("time")
        .and_then(|v| v.as_str())
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(fallback);

    Some(LiveSummaryEvent {
        label,
        detail,
        app,
        window,
        confidence,
        timestamp: ts,
    })
}
