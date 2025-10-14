use clap::Parser;
#[allow(unused_imports)]
use colored::Colorize;
use dirs::home_dir;
use reqwest::Client;
use futures::pin_mut;
use port_check::is_local_ipv4_port_free;
use cubby_audio::{
    audio_manager::AudioManagerBuilder,
    core::device::{
        default_input_device, default_output_device, list_audio_devices, parse_audio_device,
    },
};
use cubby_core::find_ffmpeg_path;
use cubby_db::{
    create_migration_worker, DatabaseManager, MigrationCommand, MigrationConfig, MigrationStatus,
};
use cubby_server::{
    cli::{
        AudioCommand, Cli, CliAudioTranscriptionEngine, CliOcrEngine, CliVadEngine, 
        CliVadSensitivity, Command, MigrationSubCommand,
        OutputFormat, PipeCommand, VisionCommand, McpCommand,
    },
    handle_index_command,
    pipe_manager::PipeInfo,
    start_continuous_recording, watch_pid, PipeManager, ResourceMonitor, SCServer,
};
use cubby_vision::monitor::list_monitors;
#[cfg(target_os = "macos")]
use cubby_vision::run_ui;
use serde_json::{json, Value};
use std::{
    env, fs, io::Write, net::SocketAddr, ops::Deref, path::PathBuf, sync::Arc, time::Duration,
    net::{IpAddr, Ipv4Addr},
};
use tokio::{runtime::Runtime, signal, sync::broadcast};
use tracing::{debug, error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Layer};
use serde::Deserialize;
use std::path::Path;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};

const DISPLAY: &str = r"
                                            _          
   __________________  ___  ____     ____  (_____  ___ 
  / ___/ ___/ ___/ _ \/ _ \/ __ \   / __ \/ / __ \/ _ \
 (__  / /__/ /  /  __/  __/ / / /  / /_/ / / /_/ /  __/
/____/\___/_/   \___/\___/_/ /_/  / .___/_/ .___/\___/ 
                                 /_/     /_/           

";

// Add the struct definition with proper derive attributes
#[derive(Deserialize, Debug)]
struct GitHubContent {
    name: String,
    path: String,
    download_url: Option<String>,
    #[serde(rename = "type")]
    content_type: String,
}

fn get_base_dir(custom_path: &Option<String>) -> anyhow::Result<PathBuf> {
    let default_path = home_dir()
        .ok_or_else(|| anyhow::anyhow!("failed to get home directory"))?
        .join(".cubby");

    let base_dir = custom_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or(default_path);
    let data_dir = base_dir.join("data");

    fs::create_dir_all(&data_dir)?;
    Ok(base_dir)
}

fn setup_logging(local_data_dir: &PathBuf, cli: &Cli) -> anyhow::Result<WorkerGuard> {
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("cubby")
        .filename_suffix("log")
        .max_log_files(5)
        .build(local_data_dir)?;

    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);

    let make_env_filter = || {
        let filter = EnvFilter::from_default_env()
            .add_directive("tokio=debug".parse().unwrap())
            .add_directive("runtime=debug".parse().unwrap())
            .add_directive("info".parse().unwrap())
            .add_directive("tokenizers=error".parse().unwrap())
            .add_directive("rusty_tesseract=error".parse().unwrap())
            .add_directive("symphonia=error".parse().unwrap())
            .add_directive("hf_hub=error".parse().unwrap())
            .add_directive("whisper_rs=error".parse().unwrap());

        #[cfg(target_os = "windows")]
        let filter = filter
            .add_directive("xcap::platform::impl_window=off".parse().unwrap())
            .add_directive("xcap::platform::impl_monitor=off".parse().unwrap())
            .add_directive("xcap::platform::utils=off".parse().unwrap());

        let filter = env::var("CUBBY_LOG")
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .fold(filter, |filter, module_directive| {
                match module_directive.parse() {
                    Ok(directive) => filter.add_directive(directive),
                    Err(e) => {
                        eprintln!(
                            "warning: invalid log directive '{}': {}",
                            module_directive, e
                        );
                        filter
                    }
                }
            });

        if cli.debug {
            filter.add_directive("cubby=debug".parse().unwrap())
        } else {
            filter
        }
    };

    let timer =
        tracing_subscriber::fmt::time::ChronoLocal::new("%Y-%m-%dT%H:%M:%S%.6fZ".to_string());

    let tracing_registry = tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(std::io::stdout)
                .with_timer(timer.clone())
                .with_filter(make_env_filter()),
        )
        .with(
            fmt::layer()
                .with_writer(file_writer)
                .with_timer(timer)
                .with_filter(make_env_filter()),
        );

    #[cfg(feature = "debug-console")]
    let tracing_registry = tracing_registry.with(
        console_subscriber::spawn().with_filter(
            EnvFilter::from_default_env()
                .add_directive("tokio=trace".parse().unwrap())
                .add_directive("runtime=trace".parse().unwrap()),
        ),
    );

    // Build the final registry with conditional Sentry layer
    if !cli.disable_telemetry {
        tracing_registry
            .with(sentry::integrations::tracing::layer())
            .init();
    } else {
        tracing_registry.init();
    };

    Ok(guard)
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    debug!("starting cubby server");
    let cli = Cli::parse();

    // Handle uninstall command first
    if cli.uninstall {
        return handle_uninstall().await;
    }

    // If not running with --no-service, handle service mode
    if !cli.no_service && cli.command.is_none() {
        return handle_service_mode(&cli).await;
    }

    // Initialize Sentry only if telemetry is enabled
    let _sentry_guard = if !cli.disable_telemetry {
        let sentry_release_name_append = env::var("SENTRY_RELEASE_NAME_APPEND").unwrap_or_default();
        let release_name = format!(
            "{}{}",
            sentry::release_name!().unwrap_or_default(),
            sentry_release_name_append
        );
        Some(sentry::init((
            "https://cf682877173997afc8463e5ca2fbe3c7@o4507617161314304.ingest.us.sentry.io/4507617170161664",
            sentry::ClientOptions {
                release: Some(release_name.into()),
                traces_sample_rate: 0.1,
                ..Default::default()
            }
        )))
    } else {
        None
    };

    let local_data_dir = get_base_dir(&cli.data_dir)?;
    let local_data_dir_clone = local_data_dir.clone();

    // Only set up logging if we're not running a pipe command with JSON output
    let should_log = match &cli.command {
        Some(Command::Pipe { subcommand }) => {
            matches!(
                subcommand,
                PipeCommand::List {
                    output: OutputFormat::Text,
                    ..
                } | PipeCommand::Install {
                    output: OutputFormat::Text,
                    ..
                } | PipeCommand::Info {
                    output: OutputFormat::Text,
                    ..
                } | PipeCommand::Enable { .. }
                    | PipeCommand::Disable { .. }
                    | PipeCommand::Update { .. }
                    | PipeCommand::Purge { .. }
                    | PipeCommand::Delete { .. }
            )
        }
        Some(Command::Add {
            output: OutputFormat::Text,
            ..
        }) => true,
        Some(Command::Migrate {
            output: OutputFormat::Text,
            ..
        }) => true,
        _ => true,
    };

    // Store the guard in a variable that lives for the entire main function
    let _log_guard = if should_log {
        Some(setup_logging(&local_data_dir, &cli)?)
    } else {
        None
    };

    let pipe_manager = if cli.enable_pipe_manager {
        Arc::new(PipeManager::new(local_data_dir_clone.clone()))
    } else {
        Arc::new(PipeManager::new(PathBuf::from("")))
    };

    if let Some(ref command) = cli.command {
        match command {
            Command::Audio { subcommand } => match subcommand {
                AudioCommand::List { output } => {
                    let default_input = default_input_device().unwrap();
                    let default_output = default_output_device().await.unwrap();
                    let devices = list_audio_devices().await?;
                    match output {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "data": devices.iter().map(|d| {
                                    json!({
                                        "name": d.to_string(),
                                        "is_default": d.name == default_input.name || d.name == default_output.name
                                    })
                                }).collect::<Vec<_>>(),
                                "success": true
                            }))?
                        ),
                        OutputFormat::Text => {
                            println!("available audio devices:");
                            for device in devices.iter() {
                                println!("  {}", device);
                            }
                            #[cfg(target_os = "macos")]
                            println!("note: on macos, output devices are your displays");
                        }
                    }
                    return Ok(());
                }
            },
            Command::Vision { subcommand } => match subcommand {
                VisionCommand::List { output } => {
                    let monitors = list_monitors().await;
                    match output {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "data": monitors.iter().map(|m| {
                                    json!({
                                        "id": m.id(),
                                        "name": m.name(),
                                        "width": m.width(),
                                        "height": m.height(),
                                        "is_default": m.is_primary(),
                                    })
                                }).collect::<Vec<_>>(),
                                "success": true
                            }))?
                        ),
                        OutputFormat::Text => {
                            println!("available monitors:");
                            for monitor in monitors.iter() {
                                println!("  {}. {:?}", monitor.id(), monitor.name());
                            }
                        }
                    }
                    return Ok(());
                }
            },
            Command::Completions { shell } => {
                cli.handle_completions(*shell)?;
                return Ok(());
            }
            Command::Pipe { subcommand } => {
                handle_pipe_command(subcommand, &pipe_manager, cli.enable_pipe_manager).await?;
                return Ok(());
            }
            Command::Migrate {
                migration_name,
                data_dir,
                subcommand,
                output,
                batch_size,
                batch_delay_ms,
                continue_on_error,
            } => {
                // Initialize the database
                let local_data_dir = get_base_dir(data_dir)?;
                let db = Arc::new(
                    DatabaseManager::new(&format!(
                        "{}/db.sqlite",
                        local_data_dir.to_string_lossy()
                    ))
                    .await
                    .map_err(|e| {
                        error!("failed to initialize database: {:?}", e);
                        e
                    })?,
                );

                // Create a migration worker config
                let config = MigrationConfig::new(*batch_size, *batch_delay_ms, *continue_on_error);

                // Start the migration worker
                let (cmd_tx, mut status_rx, worker_handle) =
                    create_migration_worker(db, Some(config));

                // Process the specified subcommand or default to status
                let cmd = match subcommand {
                    Some(MigrationSubCommand::Start) => MigrationCommand::Start,
                    Some(MigrationSubCommand::Pause) => MigrationCommand::Pause,
                    Some(MigrationSubCommand::Stop) => MigrationCommand::Stop,
                    Some(MigrationSubCommand::Status) | None => MigrationCommand::Status,
                };

                // Send the command to the worker
                if let Err(e) = cmd_tx.send(cmd.clone()).await {
                    error!("failed to send command to migration worker: {}", e);
                    return Err(anyhow::anyhow!(
                        "Failed to send command to migration worker"
                    ));
                }

                // If the command is start, we need to track the progress
                if matches!(cmd, MigrationCommand::Start) {
                    // Send the start command and wait for the worker to acknowledge
                    if let Some(response) = status_rx.recv().await {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&response.status)?);
                            }
                            OutputFormat::Text => {
                                info!("Started migration: {}", migration_name);
                                match response.status {
                                    MigrationStatus::Running {
                                        total_records,
                                        processed_records,
                                    } => {
                                        info!(
                                            "Processing records: {}/{} ({:.2}%)",
                                            processed_records,
                                            total_records,
                                            if total_records > 0 {
                                                (processed_records as f64 / total_records as f64)
                                                    * 100.0
                                            } else {
                                                0.0
                                            }
                                        );
                                    }
                                    _ => {
                                        info!("Migration status: {:?}", response.status);
                                    }
                                }
                            }
                        }
                    }

                    // Keep checking status periodically until migration completes, fails, or is stopped
                    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
                    loop {
                        interval.tick().await;

                        // Send status command
                        if let Err(e) = cmd_tx.send(MigrationCommand::Status).await {
                            error!("failed to send status command: {}", e);
                            break;
                        }

                        // Wait for response
                        if let Some(response) = status_rx.recv().await {
                            match output {
                                OutputFormat::Json => {
                                    println!("{}", serde_json::to_string_pretty(&response.status)?);
                                }
                                OutputFormat::Text => match &response.status {
                                    MigrationStatus::Running {
                                        total_records,
                                        processed_records,
                                    } => {
                                        info!(
                                            "Processing records: {}/{} ({:.2}%)",
                                            processed_records,
                                            total_records,
                                            if *total_records > 0 {
                                                (*processed_records as f64 / *total_records as f64)
                                                    * 100.0
                                            } else {
                                                0.0
                                            }
                                        );
                                    }
                                    MigrationStatus::Completed {
                                        total_records,
                                        duration_secs,
                                    } => {
                                        info!(
                                            "Migration completed: {} records processed in {} seconds",
                                            total_records, duration_secs
                                        );
                                        break;
                                    }
                                    MigrationStatus::Paused {
                                        total_records,
                                        processed_records,
                                    } => {
                                        info!(
                                            "Migration paused: {}/{} ({:.2}%)",
                                            processed_records,
                                            total_records,
                                            if *total_records > 0 {
                                                (*processed_records as f64 / *total_records as f64)
                                                    * 100.0
                                            } else {
                                                0.0
                                            }
                                        );
                                    }
                                    MigrationStatus::Failed {
                                        total_records,
                                        processed_records,
                                        error,
                                    } => {
                                        error!(
                                            "Migration failed: {}/{} records processed. Error: {}",
                                            processed_records, total_records, error
                                        );
                                        break;
                                    }
                                    _ => {
                                        info!("Migration status: {:?}", response.status);
                                    }
                                },
                            }
                        } else {
                            break;
                        }
                    }
                } else {
                    // For non-start commands, just get the status once
                    if let Some(response) = status_rx.recv().await {
                        match output {
                            OutputFormat::Json => {
                                println!("{}", serde_json::to_string_pretty(&response.status)?);
                            }
                            OutputFormat::Text => {
                                info!("Migration status: {:?}", response.status);
                            }
                        }
                    }
                }

                // If we explicitly stopped, wait for the worker to finish
                if matches!(cmd, MigrationCommand::Stop) {
                    if let Err(e) = worker_handle.await {
                        error!("error waiting for worker to finish: {}", e);
                    }
                }

                return Ok(());
            }
            Command::Add {
                path,
                output,
                data_dir,
                pattern,
                ocr_engine,
                metadata_override,
                copy_videos,
                debug,
                use_embedding,
            } => {
                let local_data_dir = get_base_dir(data_dir)?;

                // Update logging filter if debug is enabled
                if *debug {
                    tracing::subscriber::set_global_default(
                        tracing_subscriber::registry()
                            .with(
                                EnvFilter::from_default_env()
                                    .add_directive("cubby=debug".parse().unwrap()),
                            )
                            .with(fmt::layer().with_writer(std::io::stdout)),
                    )
                    .ok();
                    debug!("debug logging enabled");
                }

                let db = Arc::new(
                    DatabaseManager::new(&format!(
                        "{}/db.sqlite",
                        local_data_dir.to_string_lossy()
                    ))
                    .await
                    .map_err(|e| {
                        error!("failed to initialize database: {:?}", e);
                        e
                    })?,
                );
                handle_index_command(
                    local_data_dir,
                    path.to_string(),
                    pattern.clone(),
                    db,
                    output.clone(),
                    ocr_engine.clone(),
                    metadata_override.clone(),
                    *copy_videos,
                    *use_embedding,
                )
                .await?;
                return Ok(());
            }
            Command::Mcp { subcommand } => {
                handle_mcp_command(subcommand, &local_data_dir_clone).await?;
                return Ok(());
            }
        }
    }

    // Replace the current conditional check with:
    let ffmpeg_path = find_ffmpeg_path();
    if ffmpeg_path.is_none() {
        // Try one more time, which might trigger the installation
        let ffmpeg_path = find_ffmpeg_path();
        if ffmpeg_path.is_none() {
            eprintln!("ffmpeg not found and installation failed. please install ffmpeg manually.");
            std::process::exit(1);
        }
    }

    if !is_local_ipv4_port_free(cli.port) {
        error!(
            "you're likely already running cubby instance in a different environment, e.g. terminal/ide, close it and restart or use different port"
        );
        return Err(anyhow::anyhow!("port already in use"));
    }

    let all_monitors = list_monitors().await;

    let mut audio_devices = Vec::new();

    let mut realtime_audio_devices = Vec::new();

    if !cli.disable_audio {
        if cli.audio_device.is_empty() {
            // Use default devices
            if let Ok(input_device) = default_input_device() {
                audio_devices.push(input_device.to_string());
            }
            if let Ok(output_device) = default_output_device().await {
                audio_devices.push(output_device.to_string());
            }
        } else {
            // Use specified devices
            for d in &cli.audio_device {
                let device = parse_audio_device(d).expect("failed to parse audio device");
                audio_devices.push(device.to_string());
            }
        }

        if audio_devices.is_empty() {
            warn!("no audio devices available.");
        }

        if cli.enable_realtime_audio_transcription {
            if cli.realtime_audio_device.is_empty() {
                // Use default devices
                if let Ok(input_device) = default_input_device() {
                    realtime_audio_devices.push(Arc::new(input_device.clone()));
                }
                if let Ok(output_device) = default_output_device().await {
                    realtime_audio_devices.push(Arc::new(output_device.clone()));
                }
            } else {
                for d in &cli.realtime_audio_device {
                    let device = parse_audio_device(d).expect("failed to parse audio device");
                    realtime_audio_devices.push(Arc::new(device.clone()));
                }
            }

            if realtime_audio_devices.is_empty() {
                eprintln!("no realtime audio devices available. realtime audio transcription will be disabled.");
            }
        }
    }

    let audio_devices_clone = audio_devices.clone();
    let resource_monitor = ResourceMonitor::new(!cli.disable_telemetry);
    resource_monitor.start_monitoring(Duration::from_secs(30), Some(Duration::from_secs(60)));

    let db = Arc::new(
        DatabaseManager::new(&format!("{}/db.sqlite", local_data_dir.to_string_lossy()))
            .await
            .map_err(|e| {
                eprintln!("failed to initialize database: {:?}", e);
                e
            })?,
    );

    let db_server = db.clone();

    let warning_ocr_engine_clone = cli.ocr_engine.clone();
    let warning_audio_transcription_engine_clone = cli.audio_transcription_engine.clone();
    let monitor_ids = if cli.monitor_id.is_empty() {
        all_monitors.iter().map(|m| m.id()).collect::<Vec<_>>()
    } else {
        cli.monitor_id.clone()
    };

    let languages = cli.unique_languages().unwrap();
    let languages_clone = languages.clone();

    let ocr_engine_clone = cli.ocr_engine.clone();
    let vad_engine = cli.vad_engine.clone();
    let vad_engine_clone = vad_engine.clone();
    let vad_sensitivity_clone = cli.vad_sensitivity.clone();
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let vision_runtime = Runtime::new().unwrap();
    let pipes_runtime = Runtime::new().unwrap();

    let vision_handle = vision_runtime.handle().clone();
    let pipes_handle = pipes_runtime.handle().clone();

    let db_clone = Arc::clone(&db);
    let output_path_clone = Arc::new(local_data_dir.join("data").to_string_lossy().into_owned());
    let shutdown_tx_clone = shutdown_tx.clone();
    let monitor_ids_clone = monitor_ids.clone();
    let ignored_windows_clone = cli.ignored_windows.clone();
    let included_windows_clone = cli.included_windows.clone();
    let realtime_audio_devices_clone = realtime_audio_devices.clone();

    let fps = if cli.fps.is_finite() && cli.fps > 0.0 {
        cli.fps
    } else {
        eprintln!("invalid fps value: {}. using default of 1.0", cli.fps);
        1.0
    };

    let audio_chunk_duration = Duration::from_secs(cli.audio_chunk_duration);

    let mut audio_manager_builder = AudioManagerBuilder::new()
        .audio_chunk_duration(audio_chunk_duration)
        .vad_engine(vad_engine.into())
        .vad_sensitivity(cli.vad_sensitivity.into())
        .languages(languages.clone())
        .transcription_engine(cli.audio_transcription_engine.into())
        .realtime(cli.enable_realtime_audio_transcription)
        .enabled_devices(audio_devices)
        .deepgram_api_key(cli.deepgram_api_key.clone())
        .output_path(PathBuf::from(output_path_clone.clone().to_string()));

    let audio_manager = match audio_manager_builder.build(db.clone()).await {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            error!("{e}");
            return Ok(());
        }
    };

    let handle = {
        let runtime = &tokio::runtime::Handle::current();
        runtime.spawn(async move {
            loop {
                let mut shutdown_rx = shutdown_tx_clone.subscribe();
                let recording_future = start_continuous_recording(
                    db_clone.clone(),
                    output_path_clone.clone(),
                    fps,
                    Duration::from_secs(cli.video_chunk_duration),
                    Arc::new(cli.ocr_engine.clone().into()),
                    monitor_ids_clone.clone(),
                    cli.use_pii_removal,
                    cli.disable_vision,
                    &vision_handle,
                    &cli.ignored_windows,
                    &cli.included_windows,
                    languages_clone.clone(),
                    cli.capture_unfocused_windows,
                    cli.enable_realtime_audio_transcription,
                );

                let result = tokio::select! {
                    result = recording_future => result,
                    _ = shutdown_rx.recv() => {
                        info!("received shutdown signal for recording");
                        break;
                    }
                };

                if let Err(e) = result {
                    error!("continuous recording error: {:?}", e);
                }
            }
        })
    };

    let local_data_dir_clone_2 = local_data_dir_clone.clone();
    #[cfg(feature = "llm")]
    debug!("LLM initializing");

    #[cfg(feature = "llm")]
    let _llm = {
        match cli.enable_llm {
            true => Some(cubby_core::LLM::new(
                cubby_core::ModelName::Llama,
            )?),
            false => None,
        }
    };

    #[cfg(feature = "llm")]
    debug!("LLM initialized");

    let server = SCServer::new(
        db_server,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), cli.port),
        local_data_dir_clone_2,
        pipe_manager.clone(),
        cli.disable_vision,
        cli.disable_audio,
        cli.enable_ui_monitoring,
        audio_manager.clone(),
        cli.enable_pipe_manager,
    );

    // print cubby in gradient
    println!("\n\n{}", DISPLAY.truecolor(147, 112, 219).bold());
    println!(
        "\n{}",
        "build ai apps that have the full context"
            .bright_yellow()
            .italic()
    );
    println!(
        "{}\n\n",
        "open source | runs locally | developer friendly".bright_green()
    );

    println!("┌────────────────────────┬────────────────────────────────────┐");
    println!("│ setting                │ value                              │");
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ fps                    │ {:<34} │", cli.fps);
    println!(
        "│ audio chunk duration   │ {:<34} │",
        format!("{} seconds", cli.audio_chunk_duration)
    );
    println!(
        "│ video chunk duration   │ {:<34} │",
        format!("{} seconds", cli.video_chunk_duration)
    );
    println!("│ port                   │ {:<34} │", cli.port);
    println!(
        "│ realtime audio enabled │ {:<34} │",
        cli.enable_realtime_audio_transcription
    );
    println!("│ audio disabled         │ {:<34} │", cli.disable_audio);
    println!("│ vision disabled        │ {:<34} │", cli.disable_vision);
    println!(
        "│ audio engine           │ {:<34} │",
        format!("{:?}", warning_audio_transcription_engine_clone)
    );
    println!(
        "│ ocr engine             │ {:<34} │",
        format!("{:?}", ocr_engine_clone)
    );
    println!(
        "│ vad engine             │ {:<34} │",
        format!("{:?}", vad_engine_clone)
    );
    println!(
        "│ vad sensitivity        │ {:<34} │",
        format!("{:?}", vad_sensitivity_clone)
    );
    println!(
        "│ data directory         │ {:<34} │",
        local_data_dir_clone.display()
    );
    println!("│ debug mode             │ {:<34} │", cli.debug);
    println!(
        "│ telemetry              │ {:<34} │",
        !cli.disable_telemetry
    );
    println!("│ local llm              │ {:<34} │", cli.enable_llm);

    println!("│ use pii removal        │ {:<34} │", cli.use_pii_removal);
    println!(
        "│ ignored windows        │ {:<34} │",
        format_cell(&format!("{:?}", &ignored_windows_clone), VALUE_WIDTH)
    );
    println!(
        "│ included windows       │ {:<34} │",
        format_cell(&format!("{:?}", &included_windows_clone), VALUE_WIDTH)
    );
    println!(
        "│ ui monitoring          │ {:<34} │",
        cli.enable_ui_monitoring
    );
    println!(
        "│ frame cache            │ {:<34} │",
        cli.enable_frame_cache
    );
    println!(
        "│ capture unfocused wins │ {:<34} │",
        cli.capture_unfocused_windows
    );
    println!(
        "│ auto-destruct pid      │ {:<34} │",
        cli.auto_destruct_pid.unwrap_or(0)
    );
    // For security reasons, you might want to mask the API key if displayed
    println!(
        "│ deepgram key           │ {:<34} │",
        if cli.deepgram_api_key.is_some() {
            "set (masked)"
        } else {
            "not set"
        }
    );

    const VALUE_WIDTH: usize = 34;

    // Function to truncate and pad strings
    fn format_cell(s: &str, width: usize) -> String {
        if s.len() > width {
            let mut max_pos = 0;
            for (i, c) in s.char_indices() {
                if i + c.len_utf8() > width - 3 {
                    break;
                }
                max_pos = i + c.len_utf8();
            }

            format!("{}...", &s[..max_pos])
        } else {
            format!("{:<width$}", s, width = width)
        }
    }

    // Add languages section
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ languages              │                                    │");
    const MAX_ITEMS_TO_DISPLAY: usize = 5;

    if cli.language.is_empty() {
        println!("│ {:<22} │ {:<34} │", "", "all languages");
    } else {
        let total_languages = cli.language.len();
        for (_, language) in languages.iter().enumerate().take(MAX_ITEMS_TO_DISPLAY) {
            let language_str = format!("id: {}", language);
            let formatted_language = format_cell(&language_str, VALUE_WIDTH);
            println!("│ {:<22} │ {:<34} │", "", formatted_language);
        }
        if total_languages > MAX_ITEMS_TO_DISPLAY {
            println!(
                "│ {:<22} │ {:<34} │",
                "",
                format!("... and {} more", total_languages - MAX_ITEMS_TO_DISPLAY)
            );
        }
    }

    // Add monitors section
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ monitors               │                                    │");

    if cli.disable_vision {
        println!("│ {:<22} │ {:<34} │", "", "vision disabled");
    } else if monitor_ids.is_empty() {
        println!("│ {:<22} │ {:<34} │", "", "no monitors available");
    } else {
        let total_monitors = monitor_ids.len();
        for (_, monitor) in monitor_ids.iter().enumerate().take(MAX_ITEMS_TO_DISPLAY) {
            let monitor_str = format!("id: {}", monitor);
            let formatted_monitor = format_cell(&monitor_str, VALUE_WIDTH);
            println!("│ {:<22} │ {:<34} │", "", formatted_monitor);
        }
        if total_monitors > MAX_ITEMS_TO_DISPLAY {
            println!(
                "│ {:<22} │ {:<34} │",
                "",
                format!("... and {} more", total_monitors - MAX_ITEMS_TO_DISPLAY)
            );
        }
    }

    // Audio devices section
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ audio devices          │                                    │");

    if cli.disable_audio {
        println!("│ {:<22} │ {:<34} │", "", "disabled");
    } else if audio_devices_clone.is_empty() {
        println!("│ {:<22} │ {:<34} │", "", "no devices available");
    } else {
        let total_devices = audio_devices_clone.len();
        for (_, device) in audio_devices_clone
            .iter()
            .enumerate()
            .take(MAX_ITEMS_TO_DISPLAY)
        {
            let device_str = device.deref().to_string();
            let formatted_device = format_cell(&device_str, VALUE_WIDTH);

            println!("│ {:<22} │ {:<34} │", "", formatted_device);
        }
        if total_devices > MAX_ITEMS_TO_DISPLAY {
            println!(
                "│ {:<22} │ {:<34} │",
                "",
                format!("... and {} more", total_devices - MAX_ITEMS_TO_DISPLAY)
            );
        }
    }
    // Realtime Audio devices section
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ realtime audio devices │                                    │");

    if cli.disable_audio || !cli.enable_realtime_audio_transcription {
        println!("│ {:<22} │ {:<34} │", "", "disabled");
    } else if realtime_audio_devices_clone.is_empty() {
        println!("│ {:<22} │ {:<34} │", "", "no devices available");
    } else {
        let total_devices = realtime_audio_devices_clone.len();
        for (_, device) in realtime_audio_devices_clone
            .iter()
            .enumerate()
            .take(MAX_ITEMS_TO_DISPLAY)
        {
            let device_str = device.deref().to_string();
            let formatted_device = format_cell(&device_str, VALUE_WIDTH);

            println!("│ {:<22} │ {:<34} │", "", formatted_device);
        }
        if total_devices > MAX_ITEMS_TO_DISPLAY {
            println!(
                "│ {:<22} │ {:<34} │",
                "",
                format!("... and {} more", total_devices - MAX_ITEMS_TO_DISPLAY)
            );
        }
    }

    // Pipes section
    println!("├────────────────────────┼────────────────────────────────────┤");
    println!("│ pipes                  │                                    │");
    let pipes = pipe_manager.list_pipes().await;
    if pipes.is_empty() {
        println!("│ {:<22} │ {:<34} │", "", "no pipes available");
    } else {
        let total_pipes = pipes.len();
        for (_, pipe) in pipes.iter().enumerate().take(MAX_ITEMS_TO_DISPLAY) {
            let pipe_str = format!(
                "({}) {}",
                if pipe.enabled { "enabled" } else { "disabled" },
                pipe.id,
            );
            let formatted_pipe = format_cell(&pipe_str, VALUE_WIDTH);
            println!("│ {:<22} │ {:<34} │", "", formatted_pipe);
        }
        if total_pipes > MAX_ITEMS_TO_DISPLAY {
            println!(
                "│ {:<22} │ {:<34} │",
                "",
                format!("... and {} more", total_pipes - MAX_ITEMS_TO_DISPLAY)
            );
        }
    }

    println!("└────────────────────────┴────────────────────────────────────┘");

    // Add warning for cloud arguments and telemetry
    if warning_audio_transcription_engine_clone == CliAudioTranscriptionEngine::Deepgram
        || warning_ocr_engine_clone == CliOcrEngine::Unstructured
    {
        println!(
            "{}",
            "warning: you are using cloud now. make sure to understand the data privacy risks."
                .bright_yellow()
        );
    } else {
        println!(
            "{}",
            "you are using local processing. all your data stays on your computer.\n"
                .bright_green()
        );
    }

    // Add warning for telemetry
    if !cli.disable_telemetry {
        println!(
            "{}",
            "warning: telemetry is enabled. only error-level data will be sent.\n\
            to disable, use the --disable-telemetry flag."
                .bright_yellow()
        );
    } else {
        println!(
            "{}",
            "telemetry is disabled. no data will be sent to external services.".bright_green()
        );
    }

    // Add changelog link
    println!(
        "\n{}",
        "check latest changes here: https://github.com/monadoid/cubby-sp/releases"
            .bright_blue()
            .italic()
    );

    // start recording after all this text
    if !cli.disable_audio {
        let audio_manager_clone = audio_manager.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            audio_manager_clone.start().await.unwrap();
        });
    }

    // Start pipes
    info!("starting pipes");
    let pipes = pipe_manager.list_pipes().await;
    for pipe in pipes {
        debug!("pipe: {:?}", pipe.id);
        if !pipe.enabled {
            debug!("pipe {} is disabled, skipping", pipe.id);
            continue;
        }
        match pipe_manager.start_pipe_task(pipe.id.clone()).await {
            Ok(future) => {
                pipes_handle.spawn(future);
            }
            Err(e) => {
                error!("failed to start pipe {}: {}", pipe.id, e);
            }
        }
    }

    let server_future = server.start(cli.enable_frame_cache);
    pin_mut!(server_future);

    // Add auto-destruct watcher
    if let Some(pid) = cli.auto_destruct_pid {
        info!("watching pid {} for auto-destruction", pid);
        let shutdown_tx_clone = shutdown_tx.clone();
        tokio::spawn(async move {
            // sleep for 1 seconds
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            if watch_pid(pid).await {
                info!("Watched pid ({}) has stopped, initiating shutdown", pid);

                // Get list of enabled pipes
                let pipes = pipe_manager.list_pipes().await;
                let enabled_pipes: Vec<_> = pipes.into_iter().filter(|p| p.enabled).collect();
                // Stop all enabled pipes in parallel
                let stop_futures = enabled_pipes.iter().map(|pipe| {
                    let pipe_manager = pipe_manager.clone();
                    let pipe_id = pipe.id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = pipe_manager.stop_pipe(&pipe_id).await {
                            error!("failed to stop pipe {}: {}", pipe_id, e);
                        }
                    })
                });
                // Wait for all pipes to stop with timeout
                let timeout = tokio::time::sleep(Duration::from_secs(10));
                tokio::pin!(timeout);
                tokio::select! {
                    _ = futures::future::join_all(stop_futures) => {
                        info!("all pipes stopped successfully");
                    }
                    _ = &mut timeout => {
                        warn!("timeout waiting for pipes to stop");
                    }
                }
                let _ = shutdown_tx_clone.send(());
            }
        });
    }

    let ctrl_c_future = signal::ctrl_c();
    pin_mut!(ctrl_c_future);

    // Start the UI monitoring task
    #[cfg(target_os = "macos")]
    if cli.enable_ui_monitoring {
        let shutdown_tx_clone = shutdown_tx.clone();
        tokio::spawn(async move {
            let mut shutdown_rx = shutdown_tx_clone.subscribe();

            loop {
                tokio::select! {
                    result = run_ui() => {
                        match result {
                            Ok(_) => break,
                            Err(e) => {
                                error!("ui monitoring error: {}", e);
                                tokio::time::sleep(Duration::from_secs(5)).await;
                                continue;
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("received shutdown signal, stopping ui monitoring");
                        break;
                    }
                }
            }
        });
    }

    tokio::select! {
        _ = handle => info!("recording completed"),
        result = &mut server_future => {
            match result {
                Ok(_) => info!("server stopped normally"),
                Err(e) => error!("server stopped with error: {:?}", e),
            }
        }
        _ = ctrl_c_future => {
            info!("received ctrl+c, initiating shutdown");
            audio_manager.shutdown().await?;
            let _ = shutdown_tx.send(());
        }
    }

    tokio::task::block_in_place(|| {
        drop(pipes_runtime);
        drop(vision_runtime);
        drop(audio_manager);
    });

    info!("shutdown complete");

    Ok(())
}

async fn handle_pipe_command(
    command: &PipeCommand,
    pipe_manager: &Arc<PipeManager>,
    enable_pipe_manager: bool,
) -> anyhow::Result<()> {

    if !enable_pipe_manager {
        println!("note: pipe functionality is disabled");
        return Ok(());
    }

    let client = reqwest::Client::new();
    let server_url = "http://localhost";

    match command {
        PipeCommand::List { output, port } => {
            let server_url = format!("{}:{}", server_url, port);
            let pipes = match client
                .get(format!("{}/pipes/list", server_url))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    // The server returns { data: [...] }, so we need to extract the data field
                    let response: Value = response.json().await?;
                    response
                        .get("data")
                        .and_then(|d| d.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| serde_json::from_value::<PipeInfo>(v.clone()).ok())
                                .collect()
                        })
                        .ok_or_else(|| anyhow::anyhow!("invalid response format"))?
                }
                _ => {
                    println!("note: server not running, showing pipe configurations");
                    pipe_manager.list_pipes().await
                }
            };

            match output {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "data": pipes,
                        "success": true
                    }))?
                ),
                OutputFormat::Text => {
                    println!("available pipes:");
                    for pipe in pipes {
                        let id = pipe.id;
                        let enabled = pipe.enabled;
                        println!("  id: {}, enabled: {}", id, enabled);
                    }
                }
            }
        }

        #[allow(deprecated)]
        PipeCommand::Download { url, output, port }
        | PipeCommand::Install { url, output, port } => {
            match client
                .post(format!("{}:{}/pipes/download", server_url, port))
                .json(&json!({ "url": url }))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let data: Value = response.json().await?;
                    match output {
                        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&data)?),
                        OutputFormat::Text => println!(
                            "pipe downloaded successfully. id: {}",
                            data["pipe_id"].as_str().unwrap_or("unknown")
                        ),
                    }
                }
                _ => match pipe_manager.download_pipe(url).await {
                    Ok(pipe_id) => match output {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "data": {
                                    "pipe_id": pipe_id,
                                    "message": "pipe downloaded successfully"
                                },
                                "success": true
                            }))?
                        ),
                        OutputFormat::Text => {
                            println!("pipe downloaded successfully. id: {}", pipe_id)
                        }
                    },
                    Err(e) => {
                        let error_msg = format!("failed to download pipe: {}", e);
                        match output {
                            OutputFormat::Json => println!(
                                "{}",
                                serde_json::to_string_pretty(&json!({
                                    "error": error_msg,
                                    "success": false
                                }))?
                            ),
                            OutputFormat::Text => eprintln!("{}", error_msg),
                        }
                    }
                },
            }
        }

        PipeCommand::Info { id, output, port } => {
            let info = match client
                .get(format!("{}:{}/pipes/info/{}", server_url, port, id))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => response.json().await?,
                _ => {
                    println!("note: server not running, showing pipe configuration");
                    pipe_manager
                        .get_pipe_info(id)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("pipe not found"))?
                }
            };

            match output {
                OutputFormat::Json => println!("{}", serde_json::to_string_pretty(&info)?),
                OutputFormat::Text => println!("pipe info: {:?}", info),
            }
        }
        PipeCommand::Enable { id, port } => {
            match client
                .post(format!("{}:{}/pipes/enable", server_url, port))
                .json(&json!({ "pipe_id": id }))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("pipe {} enabled in running server", id);
                }
                _ => {
                    pipe_manager
                        .update_config(id, json!({"enabled": true}))
                        .await?;
                    println!("note: server not running, updated config only. pipe will start on next server launch");
                }
            }
        }

        PipeCommand::Disable { id, port } => {
            match client
                .post(format!("{}:{}/pipes/disable", server_url, port))
                .json(&json!({ "pipe_id": id }))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("pipe {} disabled in running server", id);
                }
                _ => {
                    pipe_manager
                        .update_config(id, json!({"enabled": false}))
                        .await?;
                    println!("note: server not running, updated config only");
                }
            }
        }

        PipeCommand::Update { id, config, port } => {
            let config: Value =
                serde_json::from_str(config).map_err(|e| anyhow::anyhow!("invalid json: {}", e))?;

            match client
                .post(format!("{}:{}/pipes/update", server_url, port))
                .json(&json!({
                    "pipe_id": id,
                    "config": config
                }))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("pipe {} config updated in running server", id);
                }
                _ => {
                    pipe_manager.update_config(id, config).await?;
                    println!("note: server not running, updated config only");
                }
            }
        }

        PipeCommand::Delete { id, yes, port } => {
            if !yes {
                print!("are you sure you want to delete pipe '{}'? [y/N] ", id);
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("pipe deletion cancelled");
                    return Ok(());
                }
            }

            match client
                .delete(format!("{}:{}/pipes/delete/{}", server_url, port, id))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("pipe '{}' deleted from running server", id);
                }
                _ => match pipe_manager.delete_pipe(id).await {
                    Ok(_) => println!("pipe '{}' deleted from local files", id),
                    Err(e) => println!("failed to delete pipe: {}", e),
                },
            }
        }

        PipeCommand::Purge { yes, port } => {
            if !yes {
                print!("are you sure you want to purge all pipes? this action cannot be undone. (y/N): ");
                std::io::stdout().flush()?;
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("pipe purge cancelled");
                    return Ok(());
                }
            }

            match client
                .post(format!("{}:{}/pipes/purge", server_url, port))
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    println!("all pipes purged from running server");
                }
                _ => match pipe_manager.purge_pipes().await {
                    Ok(_) => println!("all pipes purged from local files"),
                    Err(e) => println!("failed to purge pipes: {}", e),
                },
            }
        }
    }
    Ok(())
}

pub async fn handle_mcp_command(command: &McpCommand, local_data_dir: &PathBuf) -> Result<(), anyhow::Error> {
    let client = Client::new();

    // Check if Python is installed
    if !is_command_available("python") || !is_command_available("python3") {
        warn!("note: python is not installed. please install it from the official website: https://www.python.org/");
    }

    // Check if uv is installed
    if !is_command_available("uv") {
        warn!("note: uv is not installed. please install it using the instructions at: https://docs.astral.sh/uv/#installation");
    }

    match command {
        McpCommand::Setup { directory, output, port, update, purge } => {
            let mcp_dir = directory
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| local_data_dir.join("mcp"));

            // If purge flag is set, just remove the directory and return
            if *purge {
                if mcp_dir.exists() {
                    info!("Purging MCP directory: {}", mcp_dir.display());
                    tokio::fs::remove_dir_all(&mcp_dir).await?;
                    
                    match output {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "data": {
                                    "message": "MCP directory purged successfully",
                                    "directory": mcp_dir.to_string_lossy(),
                                },
                                "success": true
                            }))?
                        ),
                        OutputFormat::Text => {
                            println!("MCP directory purged successfully");
                            println!("Directory: {}", mcp_dir.display());
                        }
                    }
                } else {
                    match output {
                        OutputFormat::Json => println!(
                            "{}",
                            serde_json::to_string_pretty(&json!({
                                "data": {
                                    "message": "MCP directory does not exist",
                                    "directory": mcp_dir.to_string_lossy(),
                                },
                                "success": true
                            }))?
                        ),
                        OutputFormat::Text => {
                            println!("MCP directory does not exist: {}", mcp_dir.display());
                        }
                    }
                }
                return Ok(());
            }

            let should_download = if mcp_dir.exists() {
                if *update {
                    tokio::fs::remove_dir_all(&mcp_dir).await?;
                    true
                } else {
                    let mut entries = tokio::fs::read_dir(&mcp_dir).await?;
                    entries.next_entry().await?.is_none()
                }
            } else {
                true
            };

            // Create config regardless of download status
            let config = json!({
                "mcpServers": {
                    "cubby": {
                        "command": "uv",
                        "args": [
                            "--directory",
                            mcp_dir.to_string_lossy().to_string(),
                            "run",
                            "cubby-mcp",
                            "--port",
                            port.to_string()
                        ]
                    }
                }
            });

            let run_command = format!(
                "uv --directory {} run cubby-mcp --port {}",
                mcp_dir.to_string_lossy(),
                port
            );

            let config_path = mcp_dir.join("config.json");

            if should_download {
                tokio::fs::create_dir_all(&mcp_dir).await?;
                
                // Log the start of the download process
                info!("starting download process for MCP directory");

                let owner = "monadoid";
                let repo = "cubby";
                let branch = "main";
                let target_dir = "cubby-integrations/cubby-mcp";

                let api_url = format!(
                    "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
                    owner, repo, target_dir, branch
                );

                // Setup ctrl+c handler
                let (tx, mut rx) = tokio::sync::mpsc::channel(1);
                let cancel_handle = tokio::spawn(async move {
                    if signal::ctrl_c().await.is_ok() {
                        let _ = tx.send(()).await;
                    }
                });

                // Download with cancellation support
                let download_result = tokio::select! {
                    result = download_mcp_directory(&client, &api_url, &mcp_dir) => result,
                    _ = rx.recv() => {
                        info!("Received ctrl+c, canceling download...");
                        Err(anyhow::anyhow!("Download cancelled by user"))
                    }
                };

                // Clean up cancel handler
                cancel_handle.abort();

                // Handle download result
                match download_result {
                    Ok(_) => {
                        tokio::fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;
                    }
                    Err(e) => {
                        // Clean up on failure
                        if mcp_dir.exists() {
                            let _ = tokio::fs::remove_dir_all(&mcp_dir).await;
                        }
                        return Err(e);
                    }
                }
            }

            // Always create/update config.json regardless of download
            tokio::fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;

            match output {
                OutputFormat::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({
                        "data": {
                            "message": if should_download { "MCP setup completed successfully" } else { "MCP files already exist" },
                            "config": config,
                            "config_path": config_path.to_string_lossy(),
                            "directory": mcp_dir.to_string_lossy(),
                            "port": port
                        },
                        "success": true
                    }))?
                ),
                OutputFormat::Text => {
                    if should_download {
                        println!("MCP setup completed successfully");
                    } else {
                        println!("MCP files already exist at: {}", mcp_dir.display());
                        println!("Use --update flag to force update or --purge to start fresh");
                    }
                    println!("Directory: {}", mcp_dir.display());
                    println!("Config file: {}", config_path.display());
                    println!("\nTo run the MCP server, use this command:");
                    println!("$ {}", run_command);
                }
            }
        }
    }

    Ok(())
}

async fn download_mcp_directory(
    client: &Client,
    api_url: &str,
    target_dir: &Path,
) -> Result<(), anyhow::Error> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("cubby-cli"));

    let response = client
        .get(api_url)
        .headers(headers)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to send request: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!("GitHub API error (status {}): {}", status, error_text));
    }

    let contents: Vec<GitHubContent> = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse GitHub API response: {}", e))?;

    for item in contents {
        let target_path = target_dir.join(&item.name);
        
        match item.content_type.as_str() {
            "file" => {
                if let Some(download_url) = item.download_url {
                    let file_response = client
                        .get(&download_url)
                        .send()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to download file {}: {}", download_url, e))?;

                    let content = file_response
                        .bytes()
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to get file content: {}", e))?;

                    tokio::fs::write(&target_path, content)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to write file {}: {}", target_path.display(), e))?;

                    debug!("Downloaded file: {}", target_path.display());
                }
            }
            "dir" => {
                tokio::fs::create_dir_all(&target_path)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to create directory {}: {}", target_path.display(), e))?;

                let subdir_api_url = format!(
                    "https://api.github.com/repos/{}/{}/contents/{}?ref={}",
                    "monadoid", "cubby-sp", item.path, "main"
                );
                
                // Fix recursion with Box::pin
                let future = Box::pin(download_mcp_directory(client, &subdir_api_url, &target_path));
                future.await?;
            }
            _ => {
                warn!("Skipping unsupported content type: {}", item.content_type);
            }
        }
    }

    Ok(())
}

// Helper function to check if a command is available
fn is_command_available(command: &str) -> bool {
    std::process::Command::new(command)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

async fn handle_service_mode(cli: &Cli) -> anyhow::Result<()> {
    use cubby_server::service_manager::cubbyServiceManager;
    use cubby_server::cloudflared_downloader::ensure_cloudflared;
    use cubby_server::cloudflared_manager::CloudflaredManager;
    use cubby_server::run_onboarding_flow;
    
    let current_exe = std::env::current_exe()?;
    let service_manager = cubbyServiceManager::new(
        current_exe,
        build_service_args(cli),
        cli.port
    )?;
    
    // Check if this is first run (service not installed)
    let is_first_run = !service_manager.is_installed();
    
    if is_first_run {
        // Run full onboarding flow
        let onboarding_result = run_onboarding_flow(cli).await?;
        
        // Always install cloudflared with token from onboarding
        let token = onboarding_result.tunnel_token
            .ok_or_else(|| anyhow::anyhow!("No tunnel token received from authentication"))?;
        
        #[cfg(debug_assertions)]
        println!("📦 Setting up cloudflared tunnel...");
        
        // Download cloudflared binary
        let cloudflared_path = ensure_cloudflared()?;
        
        #[cfg(debug_assertions)]
        println!("✅ Cloudflared binary ready");
        
        // Install cloudflared service
        let cloudflared_manager = CloudflaredManager::new(cloudflared_path)?;
        cloudflared_manager.install_with_overwrite(&token)?;
        
        #[cfg(not(debug_assertions))]
        use cliclack::log;
        
        #[cfg(not(debug_assertions))]
        log::success("Cloudflared tunnel service installed")?;
        
        #[cfg(debug_assertions)]
        println!("✅ Cloudflared tunnel service installed\n");
        
        // Install cubby service
        #[cfg(debug_assertions)]
        println!("📦 Installing cubby-sp as a background service...");
        
        service_manager.install()?;
        
        #[cfg(not(debug_assertions))]
        log::success("Service installed successfully")?;
        
        #[cfg(debug_assertions)]
        println!("✅ Service installed successfully\n");
    } else {
        // Service already installed, just ensure it's running
        #[cfg(debug_assertions)]
        println!("📦 Service already installed");
    }
    
    // Start the service
    println!("🚀 Starting cubby service...");
    
    service_manager.start()?;
    
    // Wait for health check
    #[cfg(debug_assertions)]
    println!("⏳ Waiting for service to become healthy...");
    
    service_manager.wait_for_health()?;
    
    #[cfg(not(debug_assertions))]
    {
        use cliclack::log;
        log::success("cubby-sp is running in the background!")?;
        let home_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .to_string_lossy()
            .to_string();
        log::info(format!("Logs: {}/.cubby/cubby-*.log", home_dir))?;
        log::info("Stop service: cubby --uninstall")?;
        cliclack::outro("Setup complete!")?;
    }
    
    #[cfg(debug_assertions)]
    {
        println!("\n✅ cubby-sp is running in the background!");
        let home_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("~"))
            .to_string_lossy()
            .to_string();
        println!("   Logs: {}/.cubby/cubby-*.log", home_dir);
        println!("   Stop service: cubby --uninstall\n");
    }
    
    Ok(())
}

async fn handle_uninstall() -> anyhow::Result<()> {
    use cubby_server::service_manager::cubbyServiceManager;
    use cubby_server::cloudflared_manager::CloudflaredManager;
    use cubby_server::cloudflared_downloader::{ensure_cloudflared, cleanup_cloudflared};
    
    #[cfg(not(debug_assertions))]
    cliclack::intro("Uninstalling cubby-sp...")?;
    
    #[cfg(debug_assertions)]
    println!("🛑 Stopping and uninstalling cubby-sp service...");
    
    // Try to uninstall cloudflared service if it exists
    if let Ok(cloudflared_path) = ensure_cloudflared() {
        let cloudflared_manager = CloudflaredManager::new(cloudflared_path)?;
        if cloudflared_manager.is_installed() {
            #[cfg(debug_assertions)]
            println!("🛑 Uninstalling cloudflared tunnel service...");
            
            if let Err(e) = cloudflared_manager.uninstall() {
                #[cfg(debug_assertions)]
                println!("⚠️  Warning: Failed to uninstall cloudflared service: {}", e);
                
                #[cfg(not(debug_assertions))]
                {
                    use cliclack::log;
                    log::warning(format!("Failed to uninstall cloudflared service: {}", e))?;
                }
            } else {
                #[cfg(debug_assertions)]
                println!("✅ Cloudflared tunnel service uninstalled");
                
                #[cfg(not(debug_assertions))]
                {
                    use cliclack::log;
                    log::success("Cloudflared tunnel service uninstalled")?;
                }
            }
        }
        
        // Clean up cloudflared binaries
        if let Err(e) = cleanup_cloudflared() {
            #[cfg(debug_assertions)]
            println!("⚠️  Warning: Failed to clean up cloudflared binaries: {}", e);
        }
    }
    
    let current_exe = std::env::current_exe()?;
    let service_manager = cubbyServiceManager::new(current_exe, vec![], 3030)?;
    
    // Check if service exists first
    if !service_manager.is_installed() {
        #[cfg(not(debug_assertions))]
        {
            use cliclack::log;
            log::info("No cubby service found to uninstall")?;
        }
        
        #[cfg(debug_assertions)]
        println!("ℹ️  No cubby service found to uninstall");
        return Ok(());
    }
    
    // Additional cleanup: kill any stray cubby processes
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("pkill")
            .args(["-f", "cubby.*--no-service"])
            .output();
    }
    
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("pkill")
            .args(["-f", "cubby.*--no-service"])
            .output();
    }
    
    service_manager.stop_and_uninstall()?;
    
    #[cfg(not(debug_assertions))]
    {
        use cliclack::log;
        log::success("cubby service uninstalled successfully")?;
    }
    
    #[cfg(debug_assertions)]
    println!("✅ cubby service uninstalled successfully");
    
    #[cfg(all(target_os = "macos", debug_assertions))]
    {
        println!("\n💡 To reset permissions for testing:");
        println!("   tccutil reset ScreenCapture");
        println!("   tccutil reset Microphone");
        println!("   (Requires Terminal to have Full Disk Access in System Settings)\n");
    }
    
    #[cfg(not(debug_assertions))]
    cliclack::outro("Uninstall complete!")?;
    
    Ok(())
}

fn build_service_args(cli: &Cli) -> Vec<String> {
    let mut args = vec!["--no-service".to_string()];
    
    // Port
    args.push("--port".to_string());
    args.push(cli.port.to_string());
    
    // FPS
    args.push("--fps".to_string());
    args.push(cli.fps.to_string());
    
    // Audio chunk duration
    args.push("--audio-chunk-duration".to_string());
    args.push(cli.audio_chunk_duration.to_string());
    
    // Video chunk duration
    args.push("--video-chunk-duration".to_string());
    args.push(cli.video_chunk_duration.to_string());
    
    // Boolean flags
    if cli.disable_audio {
        args.push("--disable-audio".to_string());
    }
    
    if cli.disable_vision {
        args.push("--disable-vision".to_string());
    }
    
    if cli.debug {
        args.push("--debug".to_string());
    }
    
    if cli.use_pii_removal {
        args.push("--use-pii-removal".to_string());
    }
    
    if cli.disable_telemetry {
        args.push("--disable-telemetry".to_string());
    }
    
    if cli.enable_llm {
        args.push("--enable-llm".to_string());
    }
    
    if cli.enable_ui_monitoring {
        args.push("--enable-ui-monitoring".to_string());
    }
    
    if cli.enable_frame_cache {
        args.push("--enable-frame-cache".to_string());
    }
    
    if cli.capture_unfocused_windows {
        args.push("--capture-unfocused-windows".to_string());
    }
    
    if cli.enable_realtime_audio_transcription {
        args.push("--enable-realtime-audio-transcription".to_string());
    }
    
    if cli.enable_realtime_vision {
        args.push("--enable-realtime-vision".to_string());
    }
    
    if cli.enable_pipe_manager {
        args.push("--enable-pipe-manager".to_string());
    }
    
    // Audio devices
    for device in &cli.audio_device {
        args.push("--audio-device".to_string());
        args.push(device.clone());
    }
    
    // Realtime audio devices
    for device in &cli.realtime_audio_device {
        args.push("--realtime-audio-device".to_string());
        args.push(device.clone());
    }
    
    // Monitor IDs
    for monitor in &cli.monitor_id {
        args.push("--monitor-id".to_string());
        args.push(monitor.to_string());
    }
    
    // Languages
    for language in &cli.language {
        args.push("--language".to_string());
        args.push(format!("{:?}", language));
    }
    
    // Ignored windows
    for window in &cli.ignored_windows {
        args.push("--ignored-windows".to_string());
        args.push(window.clone());
    }
    
    // Included windows
    for window in &cli.included_windows {
        args.push("--included-windows".to_string());
        args.push(window.clone());
    }
    
    // Audio transcription engine
    args.push("--audio-transcription-engine".to_string());
    args.push(match cli.audio_transcription_engine {
        CliAudioTranscriptionEngine::Deepgram => "deepgram",
        CliAudioTranscriptionEngine::WhisperTiny => "whisper-tiny",
        CliAudioTranscriptionEngine::WhisperTinyQuantized => "whisper-tiny-quantized",
        CliAudioTranscriptionEngine::WhisperLargeV3 => "whisper-large",
        CliAudioTranscriptionEngine::WhisperLargeV3Quantized => "whisper-large-quantized",
        CliAudioTranscriptionEngine::WhisperLargeV3Turbo => "whisper-large-v3-turbo",
        CliAudioTranscriptionEngine::WhisperLargeV3TurboQuantized => "whisper-large-v3-turbo-quantized",
    }.to_string());
    
    // OCR engine
    args.push("--ocr-engine".to_string());
    args.push(match cli.ocr_engine {
        CliOcrEngine::Unstructured => "unstructured",
        #[cfg(target_os = "macos")]
        CliOcrEngine::AppleNative => "apple-native",
        #[cfg(target_os = "linux")]
        CliOcrEngine::Tesseract => "tesseract",
        #[cfg(target_os = "windows")]
        CliOcrEngine::WindowsNative => "windows-native",
        CliOcrEngine::Custom => "custom",
    }.to_string());
    
    // VAD engine
    args.push("--vad-engine".to_string());
    args.push(match cli.vad_engine {
        CliVadEngine::WebRtc => "webrtc",
        CliVadEngine::Silero => "silero",
    }.to_string());
    
    // VAD sensitivity
    args.push("--vad-sensitivity".to_string());
    args.push(match cli.vad_sensitivity {
        CliVadSensitivity::Low => "low",
        CliVadSensitivity::Medium => "medium",
        CliVadSensitivity::High => "high",
    }.to_string());
    
    // Data directory
    if let Some(ref data_dir) = cli.data_dir {
        args.push("--data-dir".to_string());
        args.push(data_dir.clone());
    }
    
    // Deepgram API key
    if let Some(ref api_key) = cli.deepgram_api_key {
        args.push("--deepgram-api-key".to_string());
        args.push(api_key.clone());
    }
    
    // Auto destruct PID
    if let Some(pid) = cli.auto_destruct_pid {
        args.push("--auto-destruct-pid".to_string());
        args.push(pid.to_string());
    }
    
    args
}
