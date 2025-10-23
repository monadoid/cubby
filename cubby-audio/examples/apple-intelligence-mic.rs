#![cfg(target_os = "macos")]

use anyhow::Result;
use cubby_audio::apple_intelligence::run_live_microphone_demo;

#[tokio::main]
async fn main() -> Result<()> {
    run_live_microphone_demo().await
}
