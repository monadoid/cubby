use std::sync::Arc;

use clap::ValueEnum;
use clap::{Args, Parser, Subcommand, ValueHint};
use cubby_audio::{
    core::engine::AudioTranscriptionEngine as CoreAudioTranscriptionEngine,
    vad::{VadEngineEnum, VadSensitivity},
};
use cubby_core::Language;
use cubby_db::CustomOcrConfig as DBCustomOcrConfig;
use cubby_db::OcrEngine as DBOcrEngine;
use cubby_vision::{custom_ocr::CustomOcrConfig, utils::OcrEngine as CoreOcrEngine};
#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum CliAudioTranscriptionEngine {
    #[clap(name = "deepgram")]
    Deepgram,
    #[clap(name = "whisper-tiny")]
    WhisperTiny,
    #[clap(name = "whisper-tiny-quantized")]
    WhisperTinyQuantized,
    #[clap(name = "whisper-large")]
    WhisperLargeV3,
    #[clap(name = "whisper-large-quantized")]
    WhisperLargeV3Quantized,
    #[clap(name = "whisper-large-v3-turbo")]
    WhisperLargeV3Turbo,
    #[clap(name = "whisper-large-v3-turbo-quantized")]
    WhisperLargeV3TurboQuantized,
    #[cfg(target_os = "macos")]
    #[clap(name = "speech-analyzer")]
    SpeechAnalyzer,
}

impl From<CliAudioTranscriptionEngine> for CoreAudioTranscriptionEngine {
    fn from(cli_engine: CliAudioTranscriptionEngine) -> Self {
        match cli_engine {
            CliAudioTranscriptionEngine::Deepgram => CoreAudioTranscriptionEngine::Deepgram,
            CliAudioTranscriptionEngine::WhisperTiny => CoreAudioTranscriptionEngine::WhisperTiny,
            CliAudioTranscriptionEngine::WhisperTinyQuantized => {
                CoreAudioTranscriptionEngine::WhisperTinyQuantized
            }
            CliAudioTranscriptionEngine::WhisperLargeV3 => {
                CoreAudioTranscriptionEngine::WhisperLargeV3
            }
            CliAudioTranscriptionEngine::WhisperLargeV3Quantized => {
                CoreAudioTranscriptionEngine::WhisperLargeV3Quantized
            }
            CliAudioTranscriptionEngine::WhisperLargeV3Turbo => {
                CoreAudioTranscriptionEngine::WhisperLargeV3Turbo
            }
            CliAudioTranscriptionEngine::WhisperLargeV3TurboQuantized => {
                CoreAudioTranscriptionEngine::WhisperLargeV3TurboQuantized
            }
            #[cfg(target_os = "macos")]
            CliAudioTranscriptionEngine::SpeechAnalyzer => {
                CoreAudioTranscriptionEngine::SpeechAnalyzer
            }
        }
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum CliOcrEngine {
    Unstructured,
    #[cfg(target_os = "linux")]
    Tesseract,
    #[cfg(target_os = "windows")]
    WindowsNative,
    #[cfg(target_os = "macos")]
    AppleNative,
    Custom,
}

impl From<CliOcrEngine> for Arc<DBOcrEngine> {
    fn from(cli_engine: CliOcrEngine) -> Self {
        match cli_engine {
            CliOcrEngine::Unstructured => Arc::new(DBOcrEngine::Unstructured),
            #[cfg(target_os = "macos")]
            CliOcrEngine::AppleNative => Arc::new(DBOcrEngine::AppleNative),
            #[cfg(target_os = "linux")]
            CliOcrEngine::Tesseract => Arc::new(DBOcrEngine::Tesseract),
            #[cfg(target_os = "windows")]
            CliOcrEngine::WindowsNative => Arc::new(DBOcrEngine::WindowsNative),
            CliOcrEngine::Custom => Arc::new(DBOcrEngine::Custom(DBCustomOcrConfig::default())),
        }
    }
}

impl From<CliOcrEngine> for CoreOcrEngine {
    fn from(cli_engine: CliOcrEngine) -> Self {
        match cli_engine {
            CliOcrEngine::Unstructured => CoreOcrEngine::Unstructured,
            #[cfg(target_os = "linux")]
            CliOcrEngine::Tesseract => CoreOcrEngine::Tesseract,
            #[cfg(target_os = "windows")]
            CliOcrEngine::WindowsNative => CoreOcrEngine::WindowsNative,
            #[cfg(target_os = "macos")]
            CliOcrEngine::AppleNative => CoreOcrEngine::AppleNative,
            CliOcrEngine::Custom => {
                // Try to read config from environment variable
                if let Ok(config_str) = std::env::var("CUBBY_CUSTOM_OCR_CONFIG") {
                    match serde_json::from_str(&config_str) {
                        Ok(config) => CoreOcrEngine::Custom(config),
                        Err(e) => {
                            tracing::warn!("failed to parse custom ocr config from env: {}", e);
                            CoreOcrEngine::Custom(CustomOcrConfig::default())
                        }
                    }
                } else {
                    CoreOcrEngine::Custom(CustomOcrConfig::default())
                }
            }
        }
    }
}
#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum CliVadEngine {
    #[clap(name = "webrtc")]
    WebRtc,
    #[clap(name = "silero")]
    Silero,
}

impl From<CliVadEngine> for VadEngineEnum {
    fn from(cli_engine: CliVadEngine) -> Self {
        match cli_engine {
            CliVadEngine::WebRtc => VadEngineEnum::WebRtc,
            CliVadEngine::Silero => VadEngineEnum::Silero,
        }
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum CliVadSensitivity {
    Low,
    Medium,
    High,
}

impl From<CliVadSensitivity> for VadSensitivity {
    fn from(cli_sensitivity: CliVadSensitivity) -> Self {
        match cli_sensitivity {
            CliVadSensitivity::Low => VadSensitivity::Low,
            CliVadSensitivity::Medium => VadSensitivity::Medium,
            CliVadSensitivity::High => VadSensitivity::High,
        }
    }
}

#[derive(Args, Clone, Debug)]
#[command(author, version, about, long_about = None, name = "cubby")]
pub struct Cli {
    /// FPS for continuous recording
    /// 1 FPS = 30 GB / month
    /// 5 FPS = 150 GB / month
    /// Optimise based on your needs.
    /// Your screen rarely change more than 1 times within a second, right?
    #[cfg_attr(not(target_os = "macos"), arg(short, long, default_value_t = 1.0))]
    #[cfg_attr(target_os = "macos", arg(short, long, default_value_t = 0.5))]
    pub fps: f64, // ! not crazy about this (inconsistent behaviour across platforms) see https://github.com/mediar-ai/screenpipe/issues/173

    /// Audio chunk duration in seconds
    #[arg(short = 'd', long)]
    pub audio_chunk_duration: Option<u64>,

    /// Port to run the server on
    #[arg(short = 'p', long, default_value_t = 3030)]
    pub port: u16,

    /// Disable audio recording
    #[arg(long, default_value_t = false)]
    pub disable_audio: bool,

    /// Audio devices to use (can be specified multiple times)
    #[arg(short = 'i', long)]
    pub audio_device: Vec<String>,

    // Audio devices to use for realtime audio transcription
    #[arg(short = 'r', long)]
    pub realtime_audio_device: Vec<String>,

    /// Data directory. Default to $HOME/.cubby
    #[arg(long, value_hint = ValueHint::DirPath)]
    pub data_dir: Option<String>,

    /// Enable debug logging for cubby modules
    #[arg(long)]
    pub debug: bool,

    /// Audio transcription engine to use.
    /// Deepgram is a very high quality cloud-based transcription service (free of charge on us for now), recommended for high quality audio.
    /// WhisperTiny is a local, lightweight transcription model, recommended for high data privacy.
    /// WhisperDistilLargeV3 is a local, lightweight transcription model (-a whisper-large), recommended for higher quality audio than tiny.
    /// WhisperLargeV3Turbo is a local, lightweight transcription model (-a whisper-large-v3-turbo), recommended for higher quality audio than tiny.
    /// SpeechAnalyzer (macOS 26+) streams via Apple's on-device Speech Analyzer for low-latency realtime transcripts.
    #[arg(short = 'a', long, value_enum)]
    pub audio_transcription_engine: Option<CliAudioTranscriptionEngine>,

    /// Enable realtime audio transcription
    #[arg(long, default_value_t = false)]
    pub enable_realtime_audio_transcription: bool,

    // HN: called fusion mode because it helps you fuse with the AI faster.
    /// Enable live OCR summarizer
    #[cfg(target_os = "macos")]
    #[arg(long = "live-summary-enabled", default_value_t = false)]
    pub live_summary_enabled: bool,

    /// Interval between live summaries in seconds
    #[cfg(target_os = "macos")]
    #[arg(long = "live-summary-interval-secs", default_value_t = 10)]
    pub live_summary_interval_secs: u64,

    /// Sliding window size for live summaries in seconds
    #[cfg(target_os = "macos")]
    #[arg(long = "live-summary-window-secs", default_value_t = 60)]
    pub live_summary_window_secs: u64,

    /// Maximum input tokens passed to the live summary model
    #[cfg(target_os = "macos")]
    #[arg(long = "live-summary-max-input-tokens", default_value_t = 1500)]
    pub live_summary_max_input_tokens: usize,

    /// Disable the fusion mode TUI (show logs on stdout instead)
    #[arg(long = "no-tui", default_value_t = false)]
    pub no_tui: bool,

    /// Enable realtime vision
    #[arg(long, default_value_t = true)]
    pub enable_realtime_vision: bool,

    /// Include base64-encoded screenshots in realtime vision events
    #[arg(long, default_value_t = false)]
    pub realtime_vision_include_image: bool,

    /// OCR engine to use.
    /// AppleNative is the default local OCR engine for macOS.
    /// WindowsNative is a local OCR engine for Windows.
    /// Unstructured is a cloud OCR engine (free of charge on us for now), recommended for high quality OCR.
    /// Tesseract is a local OCR engine (not supported on macOS)
    #[cfg_attr(
        target_os = "macos",
        arg(short = 'o', long, value_enum, default_value_t = CliOcrEngine::AppleNative)
    )]
    #[cfg_attr(
        target_os = "windows",
        arg(short = 'o', long, value_enum, default_value_t = CliOcrEngine::WindowsNative)
    )]
    #[cfg_attr(
        not(any(target_os = "macos", target_os = "windows")),
        arg(short = 'o', long, value_enum, default_value_t = CliOcrEngine::Tesseract)
    )]
    pub ocr_engine: CliOcrEngine,

    /// Monitor IDs to use, these will be used to select the monitors to record
    #[arg(short = 'm', long)]
    pub monitor_id: Vec<u32>,

    #[arg(short = 'l', long, value_enum)]
    pub language: Vec<Language>,

    /// Enable PII removal from OCR text property that is saved to db and returned in search results
    #[arg(long, default_value_t = false)]
    pub use_pii_removal: bool,

    /// Disable vision recording
    #[arg(long, default_value_t = false)]
    pub disable_vision: bool,

    /// VAD engine to use for speech detection
    #[arg(long, value_enum)]
    pub vad_engine: Option<CliVadEngine>,

    /// List of windows to ignore (by title) for screen recording - we use contains to match, example:
    /// --ignored-windows "Spotify" --ignored-windows "Bit" will ignore both "Bitwarden" and "Bittorrent"
    /// --ignored-windows "x" will ignore "Home / X" and "SpaceX"
    #[arg(long)]
    pub ignored_windows: Vec<String>,

    /// List of windows to include (by title) for screen recording - we use contains to match, example:
    /// --included-windows "Chrome" will include "Google Chrome"
    /// --included-windows "WhatsApp" will include "WhatsApp"
    #[arg(long)]
    pub included_windows: Vec<String>,

    /// Video chunk duration in seconds
    #[arg(long, default_value_t = 60)]
    pub video_chunk_duration: u64,

    /// Deepgram API Key for audio transcription
    #[arg(long = "deepgram-api-key")]
    pub deepgram_api_key: Option<String>,

    /// PID to watch for auto-destruction. If provided, cubby will stop when this PID is no longer running.
    #[arg(long)]
    pub auto_destruct_pid: Option<u32>,

    /// Voice activity detection sensitivity level
    #[arg(long, value_enum)]
    pub vad_sensitivity: Option<CliVadSensitivity>,

    /// Disable telemetry
    #[arg(long, default_value_t = false)]
    pub disable_telemetry: bool,

    /// Enable Local LLM API
    #[arg(long, default_value_t = false)]
    pub enable_llm: bool,

    /// Enable UI monitoring (macOS only)
    #[arg(long, default_value_t = false)]
    pub enable_ui_monitoring: bool,

    /// Enable experimental video frame cache (may increase CPU usage) - makes timeline UI available, frame streaming, etc.
    #[arg(long, default_value_t = true)]
    pub enable_frame_cache: bool,

    /// Capture windows that are not focused (default: false)
    #[arg(long, default_value_t = false)]
    pub capture_unfocused_windows: bool,
}

impl Cli {
    pub fn unique_languages(&self) -> Result<Vec<Language>, String> {
        let mut unique_langs = std::collections::HashSet::new();
        for lang in &self.language {
            if !unique_langs.insert(lang.clone()) {
                // continue don't care
            }
        }
        Ok(unique_langs.into_iter().collect())
    }
}

#[derive(Clone, Debug, ValueEnum, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
}

#[derive(Parser)]
#[command(
    author,
    version,
    about,
    long_about = None,
    name = "cubby",
    disable_help_subcommand = true,
    subcommand_required = true,
    arg_required_else_help = true
)]
pub struct CliApp {
    #[command(subcommand)]
    pub command: CliCommand,
}

#[derive(Subcommand, Debug)]
pub enum CliCommand {
    /// Run interactive setup (authentication, install services, etc.)
    Setup(Cli),
    /// Launch the long-running background service
    Service(Cli),
    /// Uninstall cubby service and clean up all data
    Uninstall,
}
