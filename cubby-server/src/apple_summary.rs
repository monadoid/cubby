use anyhow::Result;
use serde_json::Value;

#[cfg(not(target_os = "macos"))]
use anyhow::anyhow;

#[cfg(target_os = "macos")]
pub async fn generate_summary(prompt: &str) -> Result<Value> {
    use cubby_audio::apple_intelligence::generation::generate_structured;
    generate_structured(prompt, "LiveSummaryEvent").await
}

#[cfg(not(target_os = "macos"))]
pub async fn generate_summary(_prompt: &str) -> Result<Value> {
    Err(anyhow!("live summaries require macOS"))
}
