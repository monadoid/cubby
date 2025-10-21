use std::{env, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use cubby_foundationmodels::{
    language_model_availability, start_streaming_session, LanguageModelAvailabilityStatus,
    SpeechStreamSnapshot, SpeechStreamingSession,
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
    let chunk_size = (sample_rate as usize / 200).max(1); // ~5ms chunks for faster streaming

    println!("checking SystemLanguageModel availability...");
    let availability = language_model_availability().map_err(|err| {
        anyhow!(
            "failed to query model availability: {err}. \
             ensure macOS 26.0+ with Apple Intelligence enabled."
        )
    })?;
    println!("status: {:?}", availability.status);
    if let Some(reason) = availability.reason.as_deref() {
        println!("reason: {}", reason);
    }
    if let Some(code) = availability.reason_code.as_deref() {
        println!("reason_code: {}", code);
    }

    match availability.status {
        LanguageModelAvailabilityStatus::Available => {
            println!("✓ model available, starting streaming session\n");
        }
        LanguageModelAvailabilityStatus::Unavailable => {
            println!("✗ model unavailable; aborting");
            return Ok(());
        }
        LanguageModelAvailabilityStatus::Unknown => {
            println!("⚠️  availability unknown; attempting to stream\n");
        }
    }

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
    let mut final_transcript = String::new();
    let start_time = Instant::now();
    let mut partial_count = 0;
    let mut final_count = 0;
    
    println!("🎤 Starting real-time speech transcription...\n");
    
    while let Some(item) = rx.recv().await {
        match item {
            Ok(snapshot) => {
                let elapsed = start_time.elapsed();
                let result_arrival_time = elapsed.as_secs_f64();
                
                if snapshot.is_final {
                    final_count += 1;
                    // Clear the line and print final result
                    print!("\r\x1b[K"); // Clear current line
                    println!("✓ [final] ({:.2}s) {}", 
                             result_arrival_time, snapshot.text);
                    final_transcript.push_str(&snapshot.text);
                    if !snapshot.text.ends_with(' ') {
                        final_transcript.push(' ');
                    }
                } else {
                    partial_count += 1;
                    // Show partial result on same line (live updating)
                    print!("\r\x1b[K"); // Clear current line
                    print!("⏳ [partial] ({:.2}s) {}", 
                           result_arrival_time, snapshot.text);
                    std::io::Write::flush(&mut std::io::stdout()).unwrap();
                }
            }
            Err(err) => {
                print!("\r\x1b[K"); // Clear current line
                eprintln!("❌ [error] {err:#}");
                break;
            }
        }
    }
    
    // Print final transcript summary
    let total_time = start_time.elapsed();
    if !final_transcript.trim().is_empty() {
        println!("\n📝 Final transcript: {}", final_transcript.trim());
    }
    println!("📊 Stats: {} partial updates, {} final updates in {:.2}s", 
             partial_count, final_count, total_time.as_secs_f64());
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
