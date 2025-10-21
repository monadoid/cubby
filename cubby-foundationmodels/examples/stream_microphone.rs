use std::sync::Arc;

use anyhow::{bail, Context, Result};
use cubby_audio::core::device::{list_audio_devices, AudioDevice, DeviceType};
use cubby_audio::core::stream::AudioStream;
use cubby_foundationmodels::{
    language_model_availability, start_streaming_session, LanguageModelAvailabilityStatus,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🎤 Starting live microphone transcription...");
    println!("Press Ctrl+C to stop\n");

    // Get available input devices
    let devices = list_audio_devices().await?;
    let input_devices: Vec<&AudioDevice> = devices
        .iter()
        .filter(|d| d.device_type == DeviceType::Input)
        .collect();

    if input_devices.is_empty() {
        anyhow::bail!("No input devices found");
    }

    // Select the first input device (could be enhanced to select default)
    let device = input_devices[0].clone();
    println!("Using microphone: {}", device.name);

    // Create audio stream
    let is_running = Arc::new(AtomicBool::new(true));
    let audio_stream = AudioStream::from_device(Arc::new(device.clone()), is_running.clone()).await?;
    let sample_rate = audio_stream.device_config.sample_rate().0;

    println!("\nchecking SystemLanguageModel availability...");
    let availability = language_model_availability().context(
        "failed to query model availability (macOS 26.0+ with Apple Intelligence is required)",
    )?;
    println!("status: {:?}", availability.status);
    if let Some(reason) = availability.reason.as_deref() {
        println!("reason: {}", reason);
    }
    if let Some(code) = availability.reason_code.as_deref() {
        println!("reason_code: {}", code);
    }
    match availability.status {
        LanguageModelAvailabilityStatus::Available => {
            println!("✓ model available, starting session\n");
        }
        LanguageModelAvailabilityStatus::Unavailable => {
            bail!("SystemLanguageModel unavailable; cannot start streaming session");
        }
        LanguageModelAvailabilityStatus::Unknown => {
            println!("⚠️  availability unknown; attempting to start session\n");
        }
    }

    // Create Foundation Models streaming session
    let (session, mut rx) = start_streaming_session().await?;
    println!("Foundation Models session created\n");

    // Spawn task to capture audio and send to streaming session
    let audio_sender = session.clone();
    let is_running_audio = is_running.clone();
    let audio_stream_for_task = audio_stream.clone();
    let audio_handle = tokio::spawn(async move {
        let mut audio_receiver = audio_stream_for_task.subscribe().await;
        
        while is_running_audio.load(Ordering::Relaxed) {
            match audio_receiver.recv().await {
                Ok(chunk) => {
                    if let Err(e) = audio_sender.push_samples_f32(&chunk, sample_rate).await {
                        eprintln!("Error sending audio: {}", e);
                        break;
                    }
                }
                Err(_) => {
                    println!("Audio stream disconnected");
                    break;
                }
            }
        }
        println!("Audio capture task stopped");
    });

    // Spawn task to handle Ctrl+C gracefully
    let is_running_clone = is_running.clone();
    let session_clone = session.clone();
    let audio_stream_clone = audio_stream.clone();
    tokio::spawn(async move {
        if let Ok(_) = signal::ctrl_c().await {
            println!("\n\n🛑 Stopping transcription...");
            
            // Signal all tasks to stop
            is_running_clone.store(false, Ordering::Relaxed);
            
            // Cancel the Foundation Models session
            if let Err(e) = session_clone.cancel().await {
                eprintln!("Error cancelling session: {}", e);
            } else {
                println!("Session cancelled successfully");
            }
            
            // Stop the audio stream
            if let Err(e) = audio_stream_clone.stop().await {
                eprintln!("Error stopping audio stream: {}", e);
            } else {
                println!("Audio stream stopped successfully");
            }
            
            println!("Cleanup complete, exiting...");
            std::process::exit(0); // Force exit
        }
    });

    // Display results in real-time with deduplication
    let mut partial_count = 0;
    let mut final_count = 0;
    let mut last_partial_text = String::new();

    while let Some(item) = rx.recv().await {
        // Check if we should stop
        if !is_running.load(Ordering::Relaxed) {
            println!("\nShutdown signal received, stopping result processing...");
            break;
        }
        
        match item {
            Ok(snapshot) => {
                if snapshot.is_final {
                    final_count += 1;
                    // Clear the line and print final result
                    print!("\r\x1b[K"); // Clear current line
                    println!("✓ [final] {}", snapshot.text);
                    // Reset partial text tracking after final result
                    last_partial_text.clear();
                } else {
                    // Normalize text for comparison (trim whitespace)
                    let normalized_text = snapshot.text.trim();
                    let normalized_last = last_partial_text.trim();
                    
                    // Only display partial results if they're different from the last one
                    if normalized_text != normalized_last {
                        partial_count += 1;
                        last_partial_text = snapshot.text.clone();
                        // Show partial result on same line (live updating)
                        print!("\r\x1b[K"); // Clear current line
                        print!("⏳ [partial] {}", snapshot.text);
                        std::io::Write::flush(&mut std::io::stdout()).unwrap();
                    }
                    // If it's identical, we silently skip it to prevent repetition
                }
            }
            Err(err) => {
                print!("\r\x1b[K"); // Clear current line
                eprintln!("❌ [error] {err:#}");
                break;
            }
        }
    }

    // Wait for audio task to finish gracefully with timeout
    println!("Waiting for audio task to finish...");
    tokio::select! {
        _ = audio_handle => {
            println!("Audio task finished");
        }
        _ = tokio::time::sleep(tokio::time::Duration::from_secs(2)) => {
            println!("Audio task timeout, forcing exit");
        }
    }
    
    println!("\n📊 Transcription complete: {} partial updates, {} final updates", 
             partial_count, final_count);
    println!("All tasks stopped successfully");

    Ok(())
}
