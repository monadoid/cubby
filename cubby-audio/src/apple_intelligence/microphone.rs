#![cfg(target_os = "macos")]

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::core::device::{list_audio_devices, AudioDevice, DeviceType};
use crate::core::stream::AudioStream;
use anyhow::Result;
use tokio::signal;

use super::{
    language_model_availability, start_streaming_session, LanguageModelAvailabilityStatus,
};

fn first_input_device(devices: &[AudioDevice]) -> Option<AudioDevice> {
    devices
        .iter()
        .find(|d| d.device_type == DeviceType::Input)
        .cloned()
}

fn should_skip(message: &str) -> bool {
    message.contains("unsupported locale")
        || message.contains("requires macos 26.0+")
        || message.contains("assets")
}

/// Interactive Speech Analyzer demo that streams microphone audio to Apple
/// Intelligence and prints partial/final transcripts in real time.
pub async fn run_live_microphone_demo() -> Result<()> {
    println!("🎤 Starting live microphone transcription...");
    println!("Press Ctrl+C to stop\n");

    let devices = list_audio_devices().await?;
    let Some(device) = first_input_device(&devices) else {
        println!("skipping mic streaming demo: no input devices found");
        return Ok(());
    };

    println!("Using microphone: {}", device.name);

    let is_running = Arc::new(AtomicBool::new(true));
    let audio_stream =
        match AudioStream::from_device(Arc::new(device.clone()), is_running.clone()).await {
            Ok(stream) => stream,
            Err(err) => {
                println!("skipping mic streaming demo: failed to open device: {err:#}");
                return Ok(());
            }
        };
    let sample_rate = audio_stream.device_config.sample_rate().0;

    println!("\nchecking SystemLanguageModel availability...");
    let availability = match language_model_availability() {
        Ok(avail) => avail,
        Err(err) => {
            let msg = err.to_string();
            if should_skip(&msg) {
                println!("skipping mic streaming demo: {msg}");
                return Ok(());
            }
            return Err(err);
        }
    };
    println!("status: {:?}", availability.status);
    if let Some(reason) = availability.reason.as_deref() {
        println!("reason: {}", reason);
    }
    if let Some(code) = availability.reason_code.as_deref() {
        println!("reason_code: {}", code);
    }

    match availability.status {
        LanguageModelAvailabilityStatus::Unavailable => {
            println!("skipping mic streaming demo: SystemLanguageModel unavailable");
            return Ok(());
        }
        LanguageModelAvailabilityStatus::Unknown => {
            println!("⚠️  availability unknown; attempting to start session\n");
        }
        LanguageModelAvailabilityStatus::Available => {
            println!("✓ model available, starting session\n");
        }
    }

    let (session, mut rx) = match start_streaming_session().await {
        Ok(pair) => pair,
        Err(err) => {
            let msg = err.to_string();
            if should_skip(&msg) {
                println!("skipping mic streaming demo: {msg}");
                return Ok(());
            }
            return Err(err);
        }
    };
    println!("Foundation Models session created\n");

    // Spawn task to capture audio and send to streaming session
    let audio_sender = session.clone();
    let is_running_audio = is_running.clone();
    let audio_stream_for_task = audio_stream.clone();
    let audio_handle = tokio::spawn(async move {
        let mut audio_receiver = audio_stream_for_task.subscribe().await;

        while is_running_audio.load(Ordering::Relaxed) {
            match audio_receiver.recv().await {
                Ok(chunk) if !chunk.is_empty() => {
                    if let Err(e) = audio_sender.push_samples_f32(&chunk, sample_rate).await {
                        let msg = e.to_string();
                        if should_skip(&msg) {
                            println!("audio push skip: {msg}");
                            break;
                        }
                        println!("audio push error: {}", e);
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        println!("Audio capture task stopped");
    });

    // Handle Ctrl+C to stop cleanly.
    let is_running_clone = is_running.clone();
    let session_clone = session.clone();
    let audio_stream_clone = audio_stream.clone();
    tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            println!("\n\n🛑 Stopping transcription...");

            is_running_clone.store(false, Ordering::Relaxed);

            if let Err(e) = session_clone.cancel().await {
                eprintln!("Error cancelling session: {}", e);
            } else {
                println!("Session cancelled successfully");
            }

            if let Err(e) = audio_stream_clone.stop().await {
                eprintln!("Error stopping audio stream: {}", e);
            } else {
                println!("Audio stream stopped successfully");
            }

            println!("Cleanup complete, exiting...");
            std::process::exit(0);
        }
    });

    // Display results in real-time with deduplication
    let mut partial_count = 0;
    let mut final_count = 0;
    let mut last_partial_text = String::new();

    while let Some(item) = rx.recv().await {
        if !is_running.load(Ordering::Relaxed) {
            println!("\nShutdown signal received, stopping result processing...");
            break;
        }

        match item {
            Ok(snapshot) => {
                if snapshot.is_final {
                    final_count += 1;
                    print!("\r\x1b[K");
                    println!("✓ [final] {}", snapshot.text);
                    last_partial_text.clear();
                } else {
                    let normalized_text = snapshot.text.trim();
                    let normalized_last = last_partial_text.trim();

                    if normalized_text != normalized_last {
                        partial_count += 1;
                        last_partial_text = snapshot.text.clone();
                        print!("\r\x1b[K");
                        print!("⏳ [partial] {}", snapshot.text);
                        let _ = std::io::Write::flush(&mut std::io::stdout());
                    }
                }
            }
            Err(err) => {
                let msg = err.to_string();
                if should_skip(&msg) {
                    println!("runtime skip: {msg}");
                    break;
                }
                print!("\r\x1b[K");
                eprintln!("❌ [error] {err:#}");
                break;
            }
        }
    }

    println!("Waiting for audio task to finish...");
    if tokio::time::timeout(tokio::time::Duration::from_secs(2), audio_handle)
        .await
        .is_err()
    {
        println!("Audio task timeout, forcing exit");
    }

    println!(
        "\n📊 Transcription complete: {} partial updates, {} final updates",
        partial_count, final_count
    );
    println!("All tasks stopped successfully");

    Ok(())
}
