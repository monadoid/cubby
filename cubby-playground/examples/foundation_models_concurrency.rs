use anyhow::Result;
use std::time::Instant;

#[cfg(target_os = "macos")]
use cubby_audio::apple_intelligence::{generate, version::is_macos_26_or_newer};
#[cfg(target_os = "macos")]
use cubby_server::apple_summary::LiveSummaryEvent;
#[cfg(target_os = "macos")]
use serde::Serialize;
#[cfg(target_os = "macos")]
use tokio::task::JoinSet;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        println!("This example requires macOS 15.0 (Sequoia) with Apple Intelligence enabled.");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        run_on_macos().await
    }
}

#[cfg(target_os = "macos")]
async fn run_on_macos() -> Result<()> {
    if !is_macos_26_or_newer() {
        println!("Apple Intelligence APIs require macOS 15.0 (26.0) or later. Skipping test.");
        return Ok(());
    }

    println!("Preparing long test prompt…");
    let prompt = build_long_prompt();

    println!("--- baseline single call ---");
    let single_start = Instant::now();
    let baseline = generate::<LiveSummaryEvent>(&prompt).await?;
    let single_elapsed = single_start.elapsed();
    println!(
        "baseline complete in {:?} (keys: {:?})",
        single_elapsed,
        top_level_keys(&baseline)
    );

    run_concurrent_batch("pair", 2, &prompt).await;
    run_concurrent_batch("quad", 4, &prompt).await;

    Ok(())
}

#[cfg(target_os = "macos")]
async fn timed_generate(label: String, prompt: String) -> Result<LiveSummaryEvent> {
    println!("{label}: starting (prompt chars: {})", prompt.len());
    let start = Instant::now();
    let result = generate::<LiveSummaryEvent>(&prompt).await;
    let elapsed = start.elapsed();

    match &result {
        Ok(value) => println!(
            "{label}: finished in {:?} (keys: {:?})",
            elapsed,
            top_level_keys(value)
        ),
        Err(err) => println!("{label}: error after {:?}: {err:#}", elapsed),
    }

    result
}

#[cfg(target_os = "macos")]
fn top_level_keys<T: Serialize>(value: &T) -> Vec<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| {
            v.as_object()
                .map(|obj| obj.keys().map(|k| k.to_string()).collect())
        })
        .unwrap_or_default()
}

#[cfg(target_os = "macos")]
fn build_long_prompt() -> String {
    const SAMPLE: &str = r#"
The quick brown fox jumps over the lazy dog. This sentence contains every letter of the alphabet,
and we're using it here as filler text to create a long OCR-style prompt that should take a moment
for the language model to process. Imagine this block representing captured on-screen content,
including multiple sentences, bullet points, and code snippets. The purpose of this sample is not
semantic meaning but rather token volume. "#;

    // Keep prompt under ~3k tokens (~12k chars) to avoid exceeding the default context window.
    const REPEAT_COUNT: usize = 28;
    SAMPLE.repeat(REPEAT_COUNT)
}

#[cfg(target_os = "macos")]
async fn run_concurrent_batch(
    batch_name: &str,
    count: usize,
    prompt: &str,
) -> Vec<Result<LiveSummaryEvent>> {
    println!("\n--- concurrent {batch_name} run ({count} tasks) ---");
    let start = Instant::now();
    let mut set = JoinSet::new();

    for index in 0..count {
        let label = format!("{batch_name}-{}", index + 1);
        let prompt_owned = prompt.to_string();
        set.spawn(async move { timed_generate(label, prompt_owned).await });
    }

    let mut results = Vec::with_capacity(count);
    while let Some(res) = set.join_next().await {
        match res {
            Ok(inner) => results.push(inner),
            Err(join_err) => results.push(Err(join_err.into())),
        }
    }

    let successes = results.iter().filter(|r| r.is_ok()).count();
    println!(
        "{batch_name} batch finished after {:?} (successes: {successes}/{count})",
        start.elapsed()
    );

    results
}
