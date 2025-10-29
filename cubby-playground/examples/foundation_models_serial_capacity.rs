use anyhow::{Context, Result};
use std::env;
use std::time::Instant;

#[cfg(target_os = "macos")]
use cubby_audio::apple_intelligence::{generate, version::is_macos_26_or_newer};
#[cfg(target_os = "macos")]
use cubby_server::apple_summary::LiveSummaryEvent;
#[cfg(target_os = "macos")]
use serde::Serialize;

#[cfg(target_os = "macos")]
use sqlx::Row;

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

    let sample_size = env::args()
        .nth(1)
        .as_deref()
        .and_then(|arg| arg.parse::<usize>().ok())
        .unwrap_or(20);

    if sample_size == 0 {
        println!("Sample size must be greater than zero.");
        return Ok(());
    }

    println!("Loading {sample_size} OCR samples from ~/.cubby/db.sqlite …");
    let samples = load_random_ocr_samples(sample_size).await?;
    if samples.is_empty() {
        println!("No OCR text rows available to sample.");
        return Ok(());
    }

    println!("Running sequential Apple Intelligence calls …");
    let mut stats = Vec::with_capacity(samples.len());
    for (index, sample) in samples.iter().enumerate() {
        println!(
            "\n[{}/{}] chars: {}, est tokens: {:.0}",
            index + 1,
            samples.len(),
            sample.char_len,
            sample.estimated_tokens
        );

        let prompt = build_prompt(&sample.text, sample.char_len);
        let start = Instant::now();
        let result = generate::<LiveSummaryEvent>(&prompt).await;
        let duration = start.elapsed();

        match result {
            Ok(response) => {
                println!(
                    "Completed in {:?} (top-level keys: {:?})",
                    duration,
                    top_level_keys(&response)
                );
                stats.push(SampleStat {
                    char_len: sample.char_len as f64,
                    estimated_tokens: sample.estimated_tokens,
                    duration_secs: duration.as_secs_f64(),
                });
            }
            Err(err) => {
                println!("Call failed after {:?}: {err:#}", duration);
            }
        }
    }

    if stats.is_empty() {
        println!("No successful responses recorded; aborting analysis.");
        return Ok(());
    }

    println!("\n=== Summary ===");
    print_summary(&stats);

    Ok(())
}

#[cfg(target_os = "macos")]
struct OcrSample {
    text: String,
    char_len: usize,
    estimated_tokens: f64,
}

#[cfg(target_os = "macos")]
struct SampleStat {
    char_len: f64,
    estimated_tokens: f64,
    duration_secs: f64,
}

#[cfg(target_os = "macos")]
async fn load_random_ocr_samples(limit: usize) -> Result<Vec<OcrSample>> {
    use sqlx::sqlite::SqlitePoolOptions;

    let mut db_path = dirs::home_dir().context("Could not resolve home directory")?;
    db_path.push(".cubby");
    db_path.push("db.sqlite");

    if !db_path.exists() {
        anyhow::bail!("Expected database at {:?}, but it was not found.", db_path);
    }

    let conn_str = format!("sqlite://{}", db_path.to_string_lossy());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&conn_str)
        .await
        .with_context(|| format!("Failed to connect to {}", conn_str))?;

    let rows = sqlx::query(
        "SELECT text, LENGTH(text) as char_len \
         FROM ocr_text \
         WHERE text IS NOT NULL AND text != '' \
         ORDER BY RANDOM() \
         LIMIT ?1",
    )
    .bind(limit as i64)
    .fetch_all(&pool)
    .await
    .context("Failed to fetch OCR sample rows")?;

    let samples = rows
        .into_iter()
        .filter_map(|row| {
            let text: Option<String> = row.try_get("text").ok();
            let char_len: Option<i64> = row.try_get("char_len").ok();
            match (text, char_len) {
                (Some(text), Some(chars)) if chars > 0 => {
                    let char_len = chars as usize;
                    let estimated_tokens = char_len as f64 / 4.0;
                    Some(OcrSample {
                        text,
                        char_len,
                        estimated_tokens,
                    })
                }
                _ => None,
            }
        })
        .collect();

    Ok(samples)
}

#[cfg(target_os = "macos")]
fn build_prompt(text: &str, char_len: usize) -> String {
    format!(
        "You are the on-screen summarizer for Cubby. The following OCR capture contains approximately {} characters. \
Provide a concise summary (≤ 5 bullet points) that highlights the most important actions or tasks. \
If the content is code, focus on explaining functional changes. If the content is empty or noisy, respond with \"No actionable content.\" \
\n\n--- OCR START ---\n{}\n--- OCR END ---",
        char_len, text
    )
}

#[cfg(target_os = "macos")]
fn percentile(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty());
    let rank = (p * (sorted.len() as f64 - 1.0)).round() as usize;
    sorted[rank]
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
fn print_summary(stats: &[SampleStat]) {
    let mut durations: Vec<f64> = stats.iter().map(|s| s.duration_secs).collect();
    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let total_duration: f64 = stats.iter().map(|s| s.duration_secs).sum();
    let total_chars: f64 = stats.iter().map(|s| s.char_len).sum();
    let total_tokens: f64 = stats.iter().map(|s| s.estimated_tokens).sum();

    let avg_duration = total_duration / stats.len() as f64;
    let avg_chars = total_chars / stats.len() as f64;
    let avg_tokens = total_tokens / stats.len() as f64;
    let tokens_per_sec = total_tokens / total_duration.max(f64::EPSILON);

    let median_duration = percentile(&durations, 0.5);
    let p90_duration = percentile(&durations, 0.9);
    let p95_duration = percentile(&durations, 0.95);

    println!("Successful samples: {}", stats.len());
    println!("Average chars: {:.0}", avg_chars);
    println!("Average tokens (chars ÷ 4): {:.0}", avg_tokens);
    println!("Average duration: {:.2}s", avg_duration);
    println!("Median duration: {:.2}s", median_duration);
    println!("P90 duration: {:.2}s", p90_duration);
    println!("P95 duration: {:.2}s", p95_duration);
    println!("Aggregate tokens/sec: {:.1}", tokens_per_sec);

    let fps_avg = 1.0 / avg_duration;
    let fps_p90 = 1.0 / p90_duration;
    let fps_p95 = 1.0 / p95_duration;

    println!("\nRecommended FPS (frames processed per second):");
    println!("  Based on average latency : {:.2} FPS", fps_avg);
    println!("  Based on P90 latency     : {:.2} FPS", fps_p90);
    println!("  Based on P95 latency     : {:.2} FPS", fps_p95);

    let sustained_fps = fps_p95.min(fps_p90);
    println!(
        "\nSuggested safe FPS (no backlog, P95 target): {:.2} FPS (≈ one frame every {:.2}s)",
        sustained_fps,
        1.0 / sustained_fps
    );
}
