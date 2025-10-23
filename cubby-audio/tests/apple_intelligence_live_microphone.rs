#![cfg(target_os = "macos")]

use anyhow::Result;
use cubby_audio::apple_intelligence::run_live_microphone_demo;

fn should_skip(message: &str) -> bool {
    message.contains("unsupported locale")
        || message.contains("requires macos 26.0+")
        || message.contains("assets")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires microphone access and Apple Intelligence assets"]
async fn live_microphone_streaming_smoke() -> Result<()> {
    // cargo test captures stdout; to see live updates run with:
    // cargo test -p cubby-audio --test apple_intelligence_live_microphone -- --ignored --nocapture
    match run_live_microphone_demo().await {
        Ok(()) => Ok(()),
        Err(err) => {
            if should_skip(&err.to_string()) {
                println!("skipping mic streaming test: {err}");
                Ok(())
            } else {
                Err(err)
            }
        }
    }
}
