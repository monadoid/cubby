use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::version::{is_foundationmodels_supported, MacOSVersion};

#[swift_bridge::bridge]
mod ffi {
    extern "Swift" {
        fn fm_speech_stream_create() -> String;
        fn fm_speech_stream_push_f32(
            session_id: &str,
            samples: Vec<f32>,
            sample_rate: i32,
        ) -> String;
        fn fm_speech_stream_finish(session_id: &str) -> String;
        fn fm_speech_stream_cancel(session_id: &str) -> String;
        fn fm_speech_stream_next_result(session_id: &str, timeout_ms: i32) -> String;
    }
}

#[derive(Debug, Clone)]
pub struct SpeechStreamSnapshot {
    pub text: String,
    pub is_final: bool,
    pub timestamp: Option<f64>,
}

#[derive(Clone)]
pub struct SpeechStreamingSession {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    id: String,
    closed: AtomicBool,
}

#[derive(Debug, Deserialize)]
struct CreateEnvelope {
    session_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OkEnvelope {
    ok: Option<bool>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamEventEnvelope {
    event: String,
    text: Option<String>,
    is_final: Option<bool>,
    timestamp: Option<f64>,
    error: Option<String>,
}

const DEFAULT_POLL_TIMEOUT_MS: i32 = 250; // Balanced timeout to reduce race conditions while maintaining responsiveness

impl SpeechStreamingSession {
    pub async fn create() -> Result<Self> {
        ensure_supported()?;

        let json = tokio::task::spawn_blocking(ffi::fm_speech_stream_create).await?;

        let env: CreateEnvelope = serde_json::from_str(&json)?;
        if let Some(err) = env.error {
            return Err(anyhow!(err));
        }
        let session_id = env
            .session_id
            .ok_or_else(|| anyhow!("missing session id from foundationmodels"))?;

        Ok(Self {
            inner: Arc::new(SessionInner {
                id: session_id,
                closed: AtomicBool::new(false),
            }),
        })
    }

    pub fn id(&self) -> &str {
        &self.inner.id
    }


    pub fn subscribe(&self) -> mpsc::UnboundedReceiver<Result<SpeechStreamSnapshot>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let session = self.clone();

        tokio::spawn(async move {
            loop {
                match session.poll_next(DEFAULT_POLL_TIMEOUT_MS).await {
                    Ok(StreamEvent::Timeout) => continue,
                    Ok(StreamEvent::Done) => break,
                    Ok(StreamEvent::Update(snapshot)) => {
                        if tx.send(Ok(snapshot)).is_err() {
                            break;
                        }
                    }
                    Err(err) => {
                        let _ = tx.send(Err(err));
                        break;
                    }
                }
            }
        });

        rx
    }

    pub async fn push_samples_f32(&self, samples: &[f32], sample_rate: u32) -> Result<()> {
        if samples.is_empty() {
            return Ok(());
        }
        ensure_supported()?;
        ensure_open(&self.inner)?;

        let id = self.inner.id.clone();
        let vec = samples.to_vec();
        let rate = i32::try_from(sample_rate).map_err(|_| anyhow!("sample rate out of range"))?;

        let json =
            tokio::task::spawn_blocking(move || ffi::fm_speech_stream_push_f32(&id, vec, rate))
                .await?;

        parse_ok(json.as_str())
    }

    pub async fn finish(&self) -> Result<()> {
        ensure_supported()?;
        if self.inner.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let id = self.inner.id.clone();
        let json = tokio::task::spawn_blocking(move || ffi::fm_speech_stream_finish(&id)).await?;
        parse_ok(json.as_str())
    }

    pub async fn cancel(&self) -> Result<()> {
        ensure_supported()?;
        if self.inner.closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        let id = self.inner.id.clone();
        let json = tokio::task::spawn_blocking(move || ffi::fm_speech_stream_cancel(&id)).await?;
        parse_ok(json.as_str())
    }

    async fn poll_next(&self, timeout_ms: i32) -> Result<StreamEvent> {
        let id = self.inner.id.clone();
        let timeout = timeout_ms.max(0);
        let json =
            tokio::task::spawn_blocking(move || ffi::fm_speech_stream_next_result(&id, timeout))
                .await?;
        let env: StreamEventEnvelope = serde_json::from_str(&json)?;
        if let Some(err) = env.error {
            return Err(anyhow!(err));
        }
        Ok(match env.event.as_str() {
            "timeout" => StreamEvent::Timeout,
            "done" => StreamEvent::Done,
            "partial" | "final" => StreamEvent::Update(SpeechStreamSnapshot {
                text: env.text.unwrap_or_default(),
                is_final: env.is_final.unwrap_or(env.event == "final"),
                timestamp: env.timestamp,
            }),
            other => {
                return Err(anyhow!("unexpected streaming event: {}", other));
            }
        })
    }
}

enum StreamEvent {
    Timeout,
    Done,
    Update(SpeechStreamSnapshot),
}

pub async fn start_streaming_session() -> Result<(
    SpeechStreamingSession,
    mpsc::UnboundedReceiver<Result<SpeechStreamSnapshot>>,
)> {
    let session = SpeechStreamingSession::create().await?;
    let rx = session.subscribe();
    Ok((session, rx))
}

fn ensure_supported() -> Result<()> {
    if is_foundationmodels_supported() {
        return Ok(());
    }
    let ver = MacOSVersion::current()
        .map(|v| v.to_string())
        .unwrap_or_else(|| "unknown".into());
    Err(anyhow!(
        "speech streaming requires macos 26.0+, detected {}",
        ver
    ))
}

fn ensure_open(inner: &SessionInner) -> Result<()> {
    if inner.closed.load(Ordering::SeqCst) {
        Err(anyhow!("streaming session already finished"))
    } else {
        Ok(())
    }
}

fn parse_ok(json: &str) -> Result<()> {
    let env: OkEnvelope = serde_json::from_str(json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    if env.ok == Some(true) {
        Ok(())
    } else {
        Err(anyhow!("unexpected response from foundationmodels"))
    }
}

