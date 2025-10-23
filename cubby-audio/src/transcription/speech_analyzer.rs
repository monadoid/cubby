#[cfg(target_os = "macos")]
mod inner {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use anyhow::{anyhow, Result};
    use chrono::Utc;
    use cubby_events::send_event;
    use tracing::{debug, error, info, warn};

    use crate::apple_intelligence::{
        ensure_speech_assets_installed, start_streaming_session, SpeechAssetStatus,
    };

    use crate::core::device::DeviceType;
    use crate::core::stream::AudioStream;
    use crate::transcription::deepgram::streaming::RealtimeTranscriptionEvent;

    pub async fn stream_transcription_speech_analyzer(
        stream: Arc<AudioStream>,
        is_running: Arc<AtomicBool>,
    ) -> Result<()> {
        let ensure_result = ensure_speech_assets_installed().await?;
        match ensure_result.status {
            SpeechAssetStatus::Installed => {
                if ensure_result.installed_now {
                    info!("speech analyzer assets installed successfully");
                } else {
                    debug!("speech analyzer assets already available");
                }
            }
            SpeechAssetStatus::Downloading => {
                return Err(anyhow!(
                    "speech analyzer assets still downloading; retry after installation completes"
                ));
            }
            SpeechAssetStatus::Supported => {
                return Err(anyhow!(
                    "speech analyzer assets not installed; status remained supported"
                ));
            }
            SpeechAssetStatus::Unsupported => {
                return Err(anyhow!(
                    "speech analyzer assets unsupported for current locale/configuration"
                ));
            }
        }

        let device = stream.device.clone();
        let device_name = device.to_string();
        let is_input = matches!(device.device_type, DeviceType::Input);
        let sample_rate = stream.device_config.sample_rate().0;

        let mut audio_rx = stream.subscribe().await;

        let (session, mut results_rx) = start_streaming_session().await?;

        let session_for_audio = session.clone();
        let is_running_audio = is_running.clone();
        let device_name_audio = device_name.clone();

        let audio_task = tokio::spawn(async move {
            while is_running_audio.load(Ordering::Relaxed) {
                match audio_rx.recv().await {
                    Ok(chunk) => {
                        if chunk.is_empty() {
                            continue;
                        }
                        if let Err(err) = session_for_audio
                            .push_samples_f32(&chunk, sample_rate)
                            .await
                        {
                            error!(
                                "speech analyzer failed to push samples for device {}: {}",
                                device_name_audio, err
                            );
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                        warn!(
                            "speech analyzer audio stream lagged ({} frames dropped) for {}",
                            count, device_name_audio
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        debug!(
                            "speech analyzer audio stream closed for device {}",
                            device_name_audio
                        );
                        break;
                    }
                }
            }
        });

        while is_running.load(Ordering::Relaxed) {
            match results_rx.recv().await {
                Some(Ok(snapshot)) => {
                    let text = snapshot.text.trim();
                    if text.is_empty() {
                        continue;
                    }
                    let event = RealtimeTranscriptionEvent {
                        timestamp: Utc::now(),
                        device: device_name.clone(),
                        transcription: snapshot.text.clone(),
                        is_final: snapshot.is_final,
                        is_input,
                        speaker: None,
                    };

                    if let Err(err) = send_event("transcription", event) {
                        warn!("failed to emit speech analyzer transcript: {}", err);
                    }
                }
                Some(Err(err)) => {
                    error!(
                        "speech analyzer streaming error for device {}: {}",
                        device_name, err
                    );
                    break;
                }
                None => {
                    debug!(
                        "speech analyzer session closed result channel for device {}",
                        device_name
                    );
                    break;
                }
            }
        }

        if let Err(err) = session.finish().await {
            warn!(
                "speech analyzer session finish failed for device {}: {}",
                device_name, err
            );
        }

        if let Err(err) = audio_task.await {
            warn!(
                "speech analyzer audio task ended with error for {}: {}",
                device_name, err
            );
        }

        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod inner {
    use std::sync::{atomic::AtomicBool, Arc};

    use anyhow::{anyhow, Result};

    use crate::core::device::DeviceType;
    use crate::core::stream::AudioStream;

    pub async fn stream_transcription_speech_analyzer(
        _stream: Arc<AudioStream>,
        _is_running: Arc<AtomicBool>,
    ) -> Result<()> {
        Err(anyhow!(
            "speech analyzer transcription is only supported on macOS 26.0+"
        ))
    }
}

pub use inner::stream_transcription_speech_analyzer;
