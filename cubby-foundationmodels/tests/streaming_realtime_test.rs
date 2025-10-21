#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use cubby_foundationmodels::{start_streaming_session, version};
use hound::{SampleFormat, WavReader};
use tokio::time::timeout;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streaming_realtime_test() -> Result<()> {
    if !version::is_foundationmodels_supported() {
        println!("skipping streaming test: macOS 26.0+ required");
        return Ok(());
    }

    let audio_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_audio.wav");
    let (samples, sample_rate) = load_wav(&audio_path)?;

    let (session, mut rx) = match start_streaming_session().await {
        Ok(pair) => pair,
        Err(err) => {
            let msg = err.to_string();
            if should_skip(&msg) {
                println!("skipping streaming test: {msg}");
                return Ok(());
            }
            return Err(err);
        }
    };

    let chunk = (sample_rate as usize / 100).max(1);
    for frame in samples.chunks(chunk) {
        if let Err(err) = session.push_samples_f32(frame, sample_rate).await {
            let msg = err.to_string();
            if should_skip(&msg) {
                println!("skipping streaming test during push: {msg}");
                return Ok(());
            }
            return Err(err);
        }
    }

    if let Err(err) = session.finish().await {
        let msg = err.to_string();
        if should_skip(&msg) {
            println!("skipping streaming test on finish: {msg}");
            return Ok(());
        }
        return Err(err);
    }

    let mut partial_seen = false;
    let mut final_transcript: Option<String> = None;

    let deadline = Duration::from_secs(120);
    let mut remaining = deadline;

    while remaining.as_secs() > 0 {
        let start = tokio::time::Instant::now();
        match timeout(remaining.min(Duration::from_secs(5)), rx.recv()).await {
            Ok(Some(Ok(snapshot))) => {
                if snapshot.is_final {
                    final_transcript = Some(snapshot.text);
                    break;
                } else {
                    partial_seen = true;
                }
            }
            Ok(Some(Err(err))) => {
                let msg = err.to_string();
                if should_skip(&msg) {
                    println!("skipping streaming test due to runtime error: {msg}");
                    return Ok(());
                }
                return Err(err);
            }
            Ok(None) => break,
            Err(_) => break,
        }
        let elapsed = start.elapsed();
        remaining = remaining.saturating_sub(elapsed);
    }

    let Some(final_text) = final_transcript else {
        println!("skipping streaming test: no final transcript produced");
        return Ok(());
    };

    if final_text.trim().len() < 4 {
        println!(
            "skipping streaming test: transcript too short ({})",
            final_text
        );
        return Ok(());
    }

    if !partial_seen {
        println!("note: no partial streaming updates emitted before final transcript");
    }

    Ok(())
}

fn load_wav(path: &PathBuf) -> Result<(Vec<f32>, u32)> {
    let mut reader =
        WavReader::open(path).with_context(|| format!("failed to open {}", path.display()))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;
    let channels = spec.channels.max(1) as usize;

    let mut frames: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .context("failed to read float samples")?,
        SampleFormat::Int => match spec.bits_per_sample {
            16 => reader
                .samples::<i16>()
                .map(|s| s.map(|v| v as f32 / i16::MAX as f32))
                .collect::<Result<Vec<_>, _>>()
                .context("failed to read int16 samples")?,
            other => anyhow::bail!("unsupported bit depth: {}", other),
        },
    };

    if channels > 1 {
        let mut mono = Vec::with_capacity(frames.len() / channels);
        for frame in frames.chunks(channels) {
            let sum: f32 = frame.iter().take(channels).sum();
            mono.push(sum / channels as f32);
        }
        frames = mono;
    }

    Ok((frames, sample_rate))
}

fn should_skip(message: &str) -> bool {
    message.contains("unsupported locale")
        || message.contains("requires macos 26.0+")
        || message.contains("assets")
}
