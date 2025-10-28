#[cfg(not(target_os = "macos"))]
use anyhow::anyhow;
use anyhow::Result;
use cubby_audio::apple_intelligence::GenerableSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSummaryEvent {
    pub label: String,
    pub detail: String,
    pub app: Option<String>,
    pub window: Option<String>,
    pub confidence: Option<f64>,
    pub time: String,
}

impl GenerableSchema for LiveSummaryEvent {
    fn schema_name() -> &'static str {
        "LiveSummaryEvent"
    }

    fn from_json(value: Value) -> Result<Self> {
        Ok(serde_json::from_value(value)?)
    }

    fn to_json(&self) -> Result<Value> {
        Ok(serde_json::to_value(self)?)
    }
}

#[cfg(target_os = "macos")]
pub async fn generate_summary(prompt: &str) -> Result<LiveSummaryEvent> {
    use cubby_audio::apple_intelligence::generate;
    generate::<LiveSummaryEvent>(prompt).await
}

#[cfg(not(target_os = "macos"))]
pub async fn generate_summary(_prompt: &str) -> Result<LiveSummaryEvent> {
    Err(anyhow!("live summaries require macOS"))
}
