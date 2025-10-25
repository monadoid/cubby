use std::{
    collections::{HashSet, VecDeque},
    sync::Arc,
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use cubby_db::DatabaseManager;
use futures::StreamExt;
use serde_json::{json, Value};
use tokio::{sync::broadcast, task::JoinHandle};
use tracing::{debug, info, warn};

#[derive(Clone, Debug)]
pub struct SummarizerConfig {
    pub enabled: bool,
    pub tick_secs: u64,
    pub sampling_rate: f32,
    pub max_input_tokens: usize,
}

impl Default for SummarizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            tick_secs: 5,
            sampling_rate: 1.0,
            max_input_tokens: 1500,
        }
    }
}

pub fn spawn_live_summary_worker(
    db: Arc<DatabaseManager>,
    config: SummarizerConfig,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Option<JoinHandle<()>> {
    if !config.enabled {
        debug!("live summary worker disabled");
        return None;
    }

    Some(tokio::spawn(async move {
        if let Err(err) = run_event_loop(db, config, &mut shutdown_rx).await {
            warn!(error = ?err, "live summary worker exited with error");
        }
    }))
}

async fn run_event_loop(
    db: Arc<DatabaseManager>,
    config: SummarizerConfig,
    shutdown_rx: &mut broadcast::Receiver<()>,
) -> Result<()> {
    let mut ocr_events = cubby_events::subscribe_to_event::<Value>("ocr_result");
    let mut seen_frames: HashSet<i64> = HashSet::new();
    let mut order: VecDeque<i64> = VecDeque::new();

    loop {
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("live summary worker received shutdown");
                break;
            }
            maybe_event = ocr_events.next() => {
                match maybe_event {
                    Some(event) => {
                        if let Err(err) = process_ocr_event(db.as_ref(), &config, event.data, &mut seen_frames, &mut order).await {
                            warn!(error = ?err, "processing ocr event for live summary failed");
                        }
                    }
                    None => break,
                }
            }
        }
    }

    Ok(())
}

async fn process_ocr_event(
    db: &DatabaseManager,
    config: &SummarizerConfig,
    raw: Value,
    seen_frames: &mut HashSet<i64>,
    order: &mut VecDeque<i64>,
) -> Result<()> {
    let Some(event) = parse_ocr_event(raw) else {
        return Ok(());
    };

    if !event.focused {
        debug!("live summary skip: window not focused");
        return Ok(());
    }

    let frame_id = match event.frame_id {
        Some(id) if id > 0 => id,
        _ => return Ok(()),
    };

    debug!(frame_id, "live summary received OCR result");

    if seen_frames.contains(&frame_id) {
        debug!(frame_id, "live summary skip: already processed");
        return Ok(());
    }

    if fastrand::f32() > config.sampling_rate {
        debug!(frame_id, "live summary skip: sampling");
        return Ok(());
    }

    remember_frame(frame_id, seen_frames, order);
    summarize_window(db, frame_id, &event, config).await?;

    Ok(())
}

fn remember_frame(frame_id: i64, seen_frames: &mut HashSet<i64>, order: &mut VecDeque<i64>) {
    if seen_frames.insert(frame_id) {
        order.push_back(frame_id);
        if order.len() > 512 {
            if let Some(old) = order.pop_front() {
                seen_frames.remove(&old);
            }
        }
    }
}

async fn summarize_window(
    db: &DatabaseManager,
    frame_id: i64,
    ocr: &OcrEvent,
    config: &SummarizerConfig,
) -> Result<()> {
    let text = ocr.text.trim();
    let approx_tokens = (text.len() + 3) / 4;
    if approx_tokens == 0 || approx_tokens > config.max_input_tokens {
        return Ok(());
    }

    if text.is_empty() {
        return Ok(());
    }

    let event_time = ocr.timestamp;
    let app_name = ocr.app_name.as_deref();
    let window_name = ocr.window_name.as_deref();

    let context = PromptContext {
        timestamp: event_time,
        app_name,
        window_name,
    };

    let prompt = build_prompt(&context, text);
    let provider = "apple-foundation-models";
    let model = "ocr_activity_summary_v1";

    match crate::apple_summary::generate_summary(&prompt).await {
        Ok(value) => {
            if let Some(event) = parse_primary_event(&value, &context) {
                debug!(frame_id, "live summary generated event");
                db.insert_live_summary(
                    frame_id,
                    provider,
                    model,
                    &event.label,
                    &event.detail,
                    event.app.as_deref(),
                    event.window.as_deref(),
                    event.confidence,
                    event.time,
                    None,
                )
                .await?;

                let _ = cubby_events::send_event(
                    "live_summary",
                    json!({
                        "frame_id": frame_id,
                        "time": event.time,
                        "label": event.label,
                        "detail": event.detail,
                        "app": event.app,
                        "window": event.window,
                        "confidence": event.confidence,
                    }),
                );
            } else {
                warn!("live summary response missing primary event");
            }
        }
        Err(err) => {
            warn!(error = %err, "live summary call failed");
            db.insert_live_summary(
                frame_id,
                provider,
                model,
                "error",
                &format!("generation failed: {}", err),
                app_name,
                window_name,
                None,
                event_time,
                Some(&err.to_string()),
            )
            .await?;
        }
    }

    Ok(())
}

fn build_prompt(context: &PromptContext<'_>, text: &str) -> String {
    let app = context.app_name.unwrap_or("Unknown App");
    let window = context.window_name.unwrap_or("Unknown Window");
    format!(
        "System:\nYou are an online activity summarizer. Work ONLY from OCR text plus metadata. Produce a single actionable event describing what the user is doing.\n\nContext:\n- Timestamp: {}\n- Application: {}\n- Window: {}\n- OCR Text:\n{}\n\nRespond with JSON matching exactly:\n{{\n  \"label\": \"<verb_noun>\",\n  \"detail\": \"<specific detail>\",\n  \"app\": \"<app or omit>\",\n  \"window\": \"<window or omit>\",\n  \"confidence\": <0.0-1.0 or omit>,\n  \"time\": \"<ISO-8601 timestamp>\"\n}}\n- Keep \"detail\" detailed and precise.\n- Prefer the provided app/window when they fit; otherwise omit the field.\n- Use the supplied timestamp when unsure.",
        context.timestamp.to_rfc3339(),
        app,
        window,
        text,
    )
}

struct ParsedEvent {
    label: String,
    detail: String,
    app: Option<String>,
    window: Option<String>,
    confidence: Option<f32>,
    time: DateTime<Utc>,
}

fn parse_primary_event(
    value: &serde_json::Value,
    context: &PromptContext<'_>,
) -> Option<ParsedEvent> {
    let label = value
        .get("label")
        .and_then(|v| v.as_str())?
        .trim()
        .to_string();
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
    let time = value
        .get("time")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or(context.timestamp);

    Some(ParsedEvent {
        label,
        detail,
        app: value
            .get("app")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| context.app_name.map(|s| s.to_string())),
        window: value
            .get("window")
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| context.window_name.map(|s| s.to_string())),
        confidence: value
            .get("confidence")
            .and_then(|v| v.as_f64())
            .map(|c| c as f32),
        time,
    })
}

struct PromptContext<'a> {
    timestamp: DateTime<Utc>,
    app_name: Option<&'a str>,
    window_name: Option<&'a str>,
}

struct OcrEvent {
    text: String,
    app_name: Option<String>,
    window_name: Option<String>,
    focused: bool,
    frame_id: Option<i64>,
    timestamp: DateTime<Utc>,
}

fn parse_ocr_event(value: Value) -> Option<OcrEvent> {
    let obj = value.as_object()?;
    let text = obj.get("text")?.as_str()?.to_owned();
    let focused = obj
        .get("focused")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let frame_id = obj.get("frame_id").and_then(|v| v.as_i64());
    let timestamp = parse_timestamp(obj.get("timestamp"));

    Some(OcrEvent {
        text,
        app_name: obj
            .get("app_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
        window_name: obj
            .get("window_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned()),
        focused,
        frame_id,
        timestamp,
    })
}

fn parse_timestamp(value: Option<&Value>) -> DateTime<Utc> {
    if let Some(Value::Number(num)) = value {
        if let Some(ms) = num.as_i64() {
            if let Some(dt) = DateTime::<Utc>::from_timestamp_millis(ms) {
                return dt;
            }
        }
    }

    if let Some(Value::String(s)) = value {
        if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
            return dt.with_timezone(&Utc);
        }
    }

    Utc::now()
}
