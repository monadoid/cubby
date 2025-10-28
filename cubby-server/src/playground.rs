use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Once},
    time::Duration,
};

use anyhow::{Context, Result};
use cubby_audio::{
    audio_manager::{AudioManager, AudioManagerBuilder, RealtimeBackend},
    core::engine::AudioTranscriptionEngine as CoreAudioTranscriptionEngine,
    vad::{VadEngineEnum, VadSensitivity},
};
use cubby_core::Language;
use cubby_db::DatabaseManager;
use dirs::home_dir;

use crate::cli::{CliAudioTranscriptionEngine, CliVadEngine, CliVadSensitivity};

#[derive(Clone, Debug, Default)]
pub struct PlaygroundOptions {
    pub data_dir: Option<PathBuf>,
    pub languages: Vec<Language>,
    pub enable_audio: bool,
    pub audio: PlaygroundAudioOptions,
}

#[derive(Clone, Debug, Default)]
pub struct PlaygroundAudioOptions {
    pub devices: Vec<String>,
    pub transcription_engine: Option<CliAudioTranscriptionEngine>,
    pub vad_engine: Option<CliVadEngine>,
    pub vad_sensitivity: Option<CliVadSensitivity>,
    pub realtime: bool,
    pub deepgram_api_key: Option<String>,
    pub audio_chunk_seconds: Option<u64>,
}

pub struct PlaygroundContext {
    pub base_dir: PathBuf,
    pub data_dir: PathBuf,
    pub db: Arc<DatabaseManager>,
    pub audio_manager: Option<Arc<AudioManager>>,
    pub options: PlaygroundOptions,
}

impl PlaygroundContext {
    pub fn db(&self) -> Arc<DatabaseManager> {
        Arc::clone(&self.db)
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

pub async fn bootstrap(mut options: PlaygroundOptions) -> Result<PlaygroundContext> {
    ensure_logging();

    if options.languages.is_empty() {
        options.languages.push(Language::English);
    }

    let base_dir = resolve_data_dir(options.data_dir.as_deref())?;
    let data_dir = base_dir.join("data");
    let db_path = base_dir.join("db.sqlite");
    let db_path_str = db_path
        .to_str()
        .with_context(|| format!("invalid db path: {}", db_path.display()))?;
    let db = Arc::new(DatabaseManager::new(db_path_str).await?);

    let audio_manager = if options.enable_audio {
        Some(build_audio_manager(&options, data_dir.clone(), db.clone()).await?)
    } else {
        None
    };

    Ok(PlaygroundContext {
        base_dir,
        data_dir,
        db,
        audio_manager,
        options,
    })
}

pub fn resolve_data_dir(path: Option<&Path>) -> Result<PathBuf> {
    let base_dir = if let Some(dir) = path {
        dir.to_path_buf()
    } else {
        home_dir()
            .ok_or_else(|| anyhow::anyhow!("failed to find home directory"))?
            .join(".cubby")
    };

    let data_dir = base_dir.join("data");
    fs::create_dir_all(&data_dir)?;

    Ok(base_dir)
}

async fn build_audio_manager(
    options: &PlaygroundOptions,
    output_path: PathBuf,
    db: Arc<DatabaseManager>,
) -> Result<Arc<AudioManager>> {
    let audio = &options.audio;
    let mut builder = AudioManagerBuilder::new()
        .languages(options.languages.clone())
        .output_path(output_path)
        .enabled_devices(audio.devices.clone())
        .realtime(audio.realtime)
        .deepgram_api_key(audio.deepgram_api_key.clone());

    if let Some(seconds) = audio.audio_chunk_seconds {
        builder = builder.audio_chunk_duration(Duration::from_secs(seconds));
    }
    if let Some(vad) = audio.vad_engine.clone() {
        let vad_engine: VadEngineEnum = vad.into();
        builder = builder.vad_engine(vad_engine);
    }
    if let Some(sensitivity) = audio.vad_sensitivity.clone() {
        let sensitivity: VadSensitivity = sensitivity.into();
        builder = builder.vad_sensitivity(sensitivity);
    }
    if let Some(engine) = audio.transcription_engine.clone() {
        let core: CoreAudioTranscriptionEngine = engine.clone().into();
        builder = builder.transcription_engine(core);

        #[cfg(target_os = "macos")]
        if matches!(engine, CliAudioTranscriptionEngine::SpeechAnalyzer) {
            builder = builder.realtime_backend(RealtimeBackend::SpeechAnalyzer);
        }
    }

    Ok(Arc::new(builder.build(db).await?))
}

fn ensure_logging() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_target(false)
            .try_init();
    });
}

impl std::fmt::Debug for PlaygroundContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PlaygroundContext")
            .field("base_dir", &self.base_dir)
            .field("data_dir", &self.data_dir)
            .field("options", &self.options)
            .finish()
    }
}
