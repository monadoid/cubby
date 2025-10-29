use crate::{send_event, subscribe_to_event};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::debug;

pub const PIPELINE_TRACE_EVENT: &str = "pipeline_trace";

static PIPELINE_TRACE_ENABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineStage {
    Capture,
    Queue,
    Ocr,
    Database,
    Realtime,
    Summary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    Started,
    Progress,
    Completed,
    Errored,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineTraceEvent {
    #[serde(default)]
    pub frame_number: Option<u64>,
    #[serde(default)]
    pub frame_id: Option<i64>,
    #[serde(default)]
    pub window: Option<String>,
    #[serde(default)]
    pub app: Option<String>,
    pub stage: PipelineStage,
    pub status: StageStatus,
    pub started_at: DateTime<Utc>,
    #[serde(default)]
    pub finished_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub extra: Value,
}

impl PipelineTraceEvent {
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }
}

#[derive(Debug, Clone)]
pub struct FrameSnapshot {
    pub frame_number: Option<u64>,
    pub frame_id: Option<i64>,
    pub window: Option<String>,
    pub app: Option<String>,
    pub stages: HashMap<PipelineStage, Vec<PipelineTraceEvent>>,
    pub last_updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TraceKey {
    frame_number: Option<u64>,
    frame_id: Option<i64>,
    window: Option<String>,
}

#[derive(Debug)]
pub struct PipelineTraceAggregator {
    retention: ChronoDuration,
    frames: Arc<RwLock<HashMap<TraceKey, FrameSnapshot>>>,
}

impl PipelineTraceAggregator {
    pub fn start(retention: Duration) -> (Arc<Self>, JoinHandle<()>) {
        let retention =
            ChronoDuration::from_std(retention).unwrap_or_else(|_| ChronoDuration::minutes(10));
        let aggregator = Arc::new(Self {
            retention,
            frames: Arc::new(RwLock::new(HashMap::new())),
        });
        let task = PipelineTraceAggregator::spawn_listener(&aggregator);
        (aggregator, task)
    }

    fn spawn_listener(this: &Arc<Self>) -> JoinHandle<()> {
        let agg = Arc::clone(this);
        tokio::spawn(async move {
            agg.run().await;
        })
    }

    async fn run(self: Arc<Self>) {
        let mut subscription = subscribe_to_event::<PipelineTraceEvent>(PIPELINE_TRACE_EVENT);
        while let Some(event) = subscription.next().await {
            self.handle_event(event.data).await;
        }
    }

    async fn handle_event(&self, event: PipelineTraceEvent) {
        let mut frames = self.frames.write().await;
        let key = TraceKey {
            frame_number: event.frame_number,
            frame_id: event.frame_id,
            window: event.window.clone(),
        };

        let finished_at = event.finished_at.unwrap_or(event.started_at);

        let entry = frames.entry(key.clone()).or_insert_with(|| FrameSnapshot {
            frame_number: event.frame_number,
            frame_id: event.frame_id,
            window: event.window.clone(),
            app: event.app.clone(),
            stages: HashMap::new(),
            last_updated_at: finished_at,
        });

        if entry.frame_number.is_none() {
            entry.frame_number = event.frame_number;
        }
        if entry.frame_id.is_none() {
            entry.frame_id = event.frame_id;
        }
        if event.window.is_some() {
            entry.window = event.window.clone();
        }
        if event.app.is_some() {
            entry.app = event.app.clone();
        }

        entry.last_updated_at = finished_at;
        entry
            .stages
            .entry(event.stage)
            .or_insert_with(Vec::new)
            .push(event);

        self.evict_expired(&mut frames);
    }

    fn evict_expired(&self, frames: &mut HashMap<TraceKey, FrameSnapshot>) {
        if self.retention <= ChronoDuration::zero() {
            return;
        }

        let cutoff = Utc::now() - self.retention;
        frames.retain(|_, frame| frame.last_updated_at >= cutoff);
    }

    pub async fn snapshot(&self) -> Vec<FrameSnapshot> {
        let frames = self.frames.read().await;
        frames.values().cloned().collect()
    }

    pub async fn clear(&self) {
        let mut frames = self.frames.write().await;
        frames.clear();
    }
}

pub fn enable_pipeline_tracing() {
    PIPELINE_TRACE_ENABLED.store(true, Ordering::SeqCst);
}

pub fn disable_pipeline_tracing() {
    PIPELINE_TRACE_ENABLED.store(false, Ordering::SeqCst);
}

pub fn pipeline_tracing_enabled() -> bool {
    PIPELINE_TRACE_ENABLED.load(Ordering::SeqCst)
}

pub fn emit_pipeline_trace(event: PipelineTraceEvent) {
    if !pipeline_tracing_enabled() {
        return;
    }

    if let Err(err) = send_event(PIPELINE_TRACE_EVENT, event) {
        debug!(error = %err, "failed to emit pipeline trace event");
    }
}

pub fn approx_datetime_from_instant(instant: Instant) -> DateTime<Utc> {
    let now = Utc::now();
    match ChronoDuration::from_std(instant.elapsed()) {
        Ok(elapsed) => now - elapsed,
        Err(_) => now,
    }
}
