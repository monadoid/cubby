use crate::core::device::AudioDevice;
use crate::core::engine::AudioTranscriptionEngine;
use crate::speaker::embedding::EmbeddingExtractor;
use crate::speaker::embedding_manager::EmbeddingManager;
use crate::speaker::prepare_segments;
use crate::speaker::segment::SpeechSegment;
use crate::transcription::backend::TranscriptionBackend;
use crate::transcription::deepgram::batch::transcribe_with_deepgram;
use crate::transcription::whisper::batch::process_with_whisper;
use crate::utils::audio::resample;
use crate::utils::ffmpeg::{get_new_file_path, write_audio_to_file};
use crate::vad::VadEngine;
use anyhow::Result;
use chrono::Utc;
use cubby_core::Language;
#[cfg(target_os = "macos")]
use objc::rc::autoreleasepool;
use std::path::PathBuf;
use std::{
    sync::Arc,
    sync::Mutex as StdMutex,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;
use tracing::{error, trace};
use whisper_rs::WhisperContext;

use crate::{AudioInput, TranscriptionResult};

pub const SAMPLE_RATE: u32 = 16000;

#[allow(clippy::too_many_arguments)]
pub async fn stt_sync(
    audio: &[f32],
    sample_rate: u32,
    device: &str,
    audio_transcription_engine: Arc<AudioTranscriptionEngine>,
    deepgram_api_key: Option<String>,
    languages: Vec<Language>,
    whisper_context: Arc<WhisperContext>,
) -> Result<String> {
    let audio = audio.to_vec();

    let device = device.to_string();

    stt(
        &audio,
        sample_rate,
        &device,
        audio_transcription_engine,
        deepgram_api_key,
        languages,
        whisper_context,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn stt(
    audio: &[f32],
    sample_rate: u32,
    device: &str,
    audio_transcription_engine: Arc<AudioTranscriptionEngine>,
    deepgram_api_key: Option<String>,
    languages: Vec<Language>,
    whisper_context: Arc<WhisperContext>,
) -> Result<String> {
    let transcription: Result<String> =
        if audio_transcription_engine == AudioTranscriptionEngine::Deepgram.into() {
            // Deepgram implementation
            let api_key = deepgram_api_key.unwrap_or_default();

            match transcribe_with_deepgram(&api_key, audio, device, sample_rate, languages.clone())
                .await
            {
                Ok(transcription) => Ok(transcription),
                Err(e) => {
                    error!(
                        "device: {}, deepgram transcription failed, falling back to Whisper: {:?}",
                        device, e
                    );
                    // Fallback to Whisper
                    process_with_whisper(audio, languages.clone(), whisper_context).await
                }
            }
        } else {
            // Existing Whisper implementation
            process_with_whisper(audio, languages, whisper_context).await
        };

    transcription
}

#[allow(clippy::too_many_arguments)]
pub async fn process_audio_input(
    audio: AudioInput,
    vad_engine: Arc<Mutex<Box<dyn VadEngine + Send>>>,
    segmentation_model_path: PathBuf,
    embedding_manager: EmbeddingManager,
    embedding_extractor: Arc<StdMutex<EmbeddingExtractor>>,
    output_path: &PathBuf,
    audio_transcription_engine: Arc<AudioTranscriptionEngine>,
    deepgram_api_key: Option<String>,
    languages: Vec<Language>,
    output_sender: &crossbeam::channel::Sender<TranscriptionResult>,
    backend: TranscriptionBackend,
) -> Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    let audio_data = if audio.sample_rate != SAMPLE_RATE {
        resample(audio.data.as_ref(), audio.sample_rate, SAMPLE_RATE)?
    } else {
        audio.data.as_ref().to_vec()
    };

    let audio = AudioInput {
        data: Arc::new(audio_data.clone()),
        sample_rate: SAMPLE_RATE,
        ..audio
    };

    let (mut segments, speech_ratio_ok) = prepare_segments(
        &audio_data,
        vad_engine,
        &segmentation_model_path,
        embedding_manager,
        embedding_extractor,
        &audio.device.to_string(),
    )
    .await?;

    if !speech_ratio_ok {
        return Ok(());
    }

    let new_file_path = get_new_file_path(&audio.device.to_string(), output_path);

    if let Err(e) = write_audio_to_file(
        &audio.data.to_vec(),
        audio.sample_rate,
        &PathBuf::from(&new_file_path),
        false,
    ) {
        error!("Error writing audio to file: {:?}", e);
    }

    match backend {
        TranscriptionBackend::Whisper { context } => {
            while let Some(segment) = segments.recv().await {
                let path = new_file_path.clone();
                let transcription_result = if cfg!(target_os = "macos") {
                    #[cfg(target_os = "macos")]
                    {
                        let timestamp = timestamp + segment.start.round() as u64;
                        autoreleasepool(|| {
                            run_stt(
                                segment,
                                audio.device.clone(),
                                audio_transcription_engine.clone(),
                                deepgram_api_key.clone(),
                                languages.clone(),
                                path,
                                timestamp,
                                context.clone(),
                            )
                        })
                        .await?
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        unreachable!("This code should not be reached on non-macOS platforms")
                    }
                } else {
                    run_stt(
                        segment,
                        audio.device.clone(),
                        audio_transcription_engine.clone(),
                        deepgram_api_key.clone(),
                        languages.clone(),
                        path,
                        timestamp,
                        context.clone(),
                    )
                    .await?
                };

                if output_sender.send(transcription_result).is_err() {
                    break;
                }
            }
            Ok(())
        }
        TranscriptionBackend::SpeechAnalyzer { transcript_store } => {
            let mut start_time = None;
            let mut end_time = None;
            let mut speaker_embedding = Vec::new();

            while let Some(segment) = segments.recv().await {
                if start_time.is_none() {
                    start_time = Some(segment.start);
                    speaker_embedding = segment.embedding.clone();
                }
                end_time = Some(segment.end);
            }

            let device_name = audio.device.to_string();
            trace!(
                "speech analyzer backend draining transcripts for {}",
                device_name
            );
            let transcripts = transcript_store.drain(&device_name);
            if transcripts.is_empty() {
                trace!("speech analyzer drain empty for {}", device_name);
                return Ok(());
            }

            let non_empty: Vec<_> = transcripts
                .iter()
                .filter_map(|entry| {
                    let text = entry.text.trim();
                    if text.is_empty() {
                        None
                    } else {
                        Some((entry.timestamp, text.to_owned()))
                    }
                })
                .collect();

            if non_empty.is_empty() {
                trace!(
                    "speech analyzer transcripts all empty after trimming for {}",
                    device_name
                );
                return Ok(());
            }

            let combined = non_empty
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let first_timestamp = non_empty[0].0;
            trace!(
                "speech analyzer combined transcript for {} ({} parts)",
                device_name,
                non_empty.len()
            );

            let transcription_result = TranscriptionResult {
                input: audio.clone(),
                path: new_file_path,
                speaker_embedding,
                transcription: Some(combined),
                timestamp: first_timestamp.timestamp() as u64,
                error: None,
                start_time: start_time.unwrap_or(0.0),
                end_time: end_time.unwrap_or(audio.data.len() as f64 / SAMPLE_RATE as f64),
            };

            if output_sender.send(transcription_result).is_err() {
                trace!(
                    "speech analyzer failed to send transcription result for {}",
                    device_name
                );
            }

            Ok(())
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_stt(
    segment: SpeechSegment,
    device: Arc<AudioDevice>,
    audio_transcription_engine: Arc<AudioTranscriptionEngine>,
    deepgram_api_key: Option<String>,
    languages: Vec<Language>,
    path: String,
    timestamp: u64,
    whisper_context: Arc<WhisperContext>,
) -> Result<TranscriptionResult> {
    let audio = segment.samples.clone();
    let sample_rate = segment.sample_rate;
    match stt_sync(
        &audio,
        sample_rate,
        &device.to_string(),
        audio_transcription_engine.clone(),
        deepgram_api_key.clone(),
        languages.clone(),
        whisper_context,
    )
    .await
    {
        Ok(transcription) => Ok(TranscriptionResult {
            input: AudioInput {
                data: Arc::new(audio),
                sample_rate,
                channels: 1,
                device: device.clone(),
            },
            transcription: Some(transcription),
            path,
            timestamp,
            error: None,
            speaker_embedding: segment.embedding.clone(),
            start_time: segment.start,
            end_time: segment.end,
        }),
        Err(e) => {
            error!("STT error for input {}: {:?}", device, e);
            Ok(TranscriptionResult {
                input: AudioInput {
                    data: Arc::new(segment.samples),
                    sample_rate: segment.sample_rate,
                    channels: 1,
                    device: device.clone(),
                },
                transcription: None,
                path,
                timestamp,
                error: Some(e.to_string()),
                speaker_embedding: Vec::new(),
                start_time: segment.start,
                end_time: segment.end,
            })
        }
    }
}
