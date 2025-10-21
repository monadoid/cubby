use std::{env, path::PathBuf};

use anyhow::{Context, Result};
use cubby_foundationmodels::{
    start_streaming_session, SpeechStreamSnapshot, SpeechStreamingSession,
};
use hound::{SampleFormat, WavReader};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{sleep, Duration, Instant};

#[tokio::main]
async fn main() -> Result<()> {
    let path = env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/test_audio_2.wav")
    });

    println!("streaming file: {}", path.display());

    let (samples, sample_rate) = load_wav(&path)?;
    let chunk_size = (sample_rate as usize / 100).max(1); // ~10ms chunks

    let (session, mut rx) = start_streaming_session().await?;
    let sender = session.clone();

    let send_handle =
        tokio::spawn(async move { stream_audio(sender, samples, sample_rate, chunk_size).await });

    println!("awaiting streaming results...");
    drain_results(&mut rx).await;

    send_handle.await.map_err(|err| anyhow::anyhow!(err))??;

    Ok(())
}

async fn drain_results(rx: &mut UnboundedReceiver<Result<SpeechStreamSnapshot>>) {
    while let Some(item) = rx.recv().await {
        match item {
            Ok(snapshot) => {
                if snapshot.is_final {
                    println!("[final] {}", snapshot.text);
                } else {
                    println!("[partial] {}", snapshot.text);
                }
            }
            Err(err) => {
                eprintln!("[error] {err:#}");
                break;
            }
        }
    }
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

async fn stream_audio(
    session: SpeechStreamingSession,
    samples: Vec<f32>,
    sample_rate: u32,
    chunk_size: usize,
) -> Result<()> {
    let mut sent_samples: usize = 0;
    let start = Instant::now();

    for chunk in samples.chunks(chunk_size) {
        session.push_samples_f32(chunk, sample_rate).await?;
        sent_samples += chunk.len();

        // Pace delivery to approximate real-time streaming. This keeps the channel
        // open while results arrive and avoids dumping the entire buffer at once.
        let elapsed_target = Duration::from_secs_f64(sent_samples as f64 / sample_rate as f64);
        let elapsed_now = start.elapsed();
        if elapsed_target > elapsed_now {
            sleep(elapsed_target - elapsed_now).await;
        }
    }

    session.finish().await?;
    Ok(())
}
