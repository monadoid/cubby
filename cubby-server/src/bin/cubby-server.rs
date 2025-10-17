use clap::Parser;
#[allow(unused_imports)]
use colored::Colorize;
use cubby_audio::{
    audio_manager::AudioManagerBuilder,
    core::device::{default_input_device, default_output_device, parse_audio_device},
};
use cubby_core::find_ffmpeg_path;
use cubby_db::DatabaseManager;
use cubby_server::mac_notifications;
use cubby_server::{
    cli::{
        Cli, CliApp, CliAudioTranscriptionEngine, CliCommand, CliOcrEngine, CliVadEngine,
        CliVadSensitivity,
    },
    permission_checker::{trigger_and_check_microphone, trigger_and_check_screen_recording},
    setup_state::SetupState,
    start_continuous_recording, ResourceMonitor, SCServer,
};
use cubby_vision::monitor::list_monitors;
#[cfg(target_os = "macos")]
use cubby_vision::run_ui;
use dirs::home_dir;
use futures::pin_mut;
use port_check::is_local_ipv4_port_free;
use std::{
    env, fs,
    net::SocketAddr,
    net::{IpAddr, Ipv4Addr},
    ops::Deref,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};
use tokio::{runtime::Runtime, signal, sync::broadcast};
use tracing::{debug, error, info, warn};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, Layer};

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

    // Build the final registry
    tracing_registry.init();

    Ok(guard)
}

#[tokio::main]
#[tracing::instrument]
async fn main() -> anyhow::Result<()> {
    debug!("starting cubby server");
    let app = CliApp::parse();

    match app.command {
        CliCommand::Setup(cli) => {
            let setup_state = SetupState::load().unwrap_or_default();
            run_setup_flow(&cli, setup_state).await
        }
        CliCommand::Service(cli) => {
            let mut setup_state = SetupState::load().unwrap_or_default();
            ensure_permissions_in_service(&cli, &mut setup_state).await?;
            run_service(&cli, setup_state).await
        }
        CliCommand::Uninstall => handle_uninstall().await,
    }
}

async fn run_service(cli: &Cli, setup_state: SetupState) -> anyhow::Result<()> {

    let local_data_dir = get_base_dir(&cli.data_dir)?;
    let local_data_dir_clone = local_data_dir.clone();

    // Store the guard in a variable that lives for the entire main function
    let _log_guard = setup_logging(&local_data_dir, &cli)?;

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

    // Don't trigger permission prompts from service - only list monitors if we already have permission
    let all_monitors = if cli.disable_vision {
        Vec::new()
    } else {
        #[cfg(target_os = "macos")]
        {
            use objc2_core_graphics::CGPreflightScreenCaptureAccess;
            // Silently check permission without triggering dialog
            if CGPreflightScreenCaptureAccess() {
                list_monitors().await
            } else {
                // No permission - skip vision silently (onboarding should have handled this)
                warn!("screen recording permission not granted, skipping vision");
                Vec::new()
            }
        }
        #[cfg(not(target_os = "macos"))]
        list_monitors().await
    };

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
    let warning_audio_transcription_engine_clone = cli.audio_transcription_engine.as_ref();
    let monitor_ids = if cli.disable_vision || all_monitors.is_empty() {
        Vec::new()
    } else if cli.monitor_id.is_empty() {
        all_monitors.iter().map(|m| m.id()).collect::<Vec<_>>()
    } else {
        cli.monitor_id.clone()
    };

    let languages = cli.unique_languages().unwrap();
    let languages_clone = languages.clone();

    let ocr_engine_clone = cli.ocr_engine.clone();
    let vad_engine = cli.vad_engine.as_ref();
    let vad_engine_clone = cli.vad_engine.as_ref();
    let vad_sensitivity_clone = cli.vad_sensitivity.as_ref();
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let vision_runtime = Runtime::new().unwrap();
    let vision_handle = vision_runtime.handle().clone();

    let db_clone = Arc::clone(&db);
    let output_path_clone = Arc::new(local_data_dir.join("data").to_string_lossy().into_owned());
    let shutdown_tx_clone = shutdown_tx.clone();
    let monitor_ids_clone = monitor_ids.clone();
    let ignored_windows_clone = cli.ignored_windows.clone();
    let included_windows_clone = cli.included_windows.clone();
    let realtime_audio_devices_clone = realtime_audio_devices.clone();

    // Clone values needed in spawned task
    let video_chunk_duration = cli.video_chunk_duration;
    let ocr_engine_for_task = cli.ocr_engine.clone();
    let use_pii_removal = cli.use_pii_removal;
    let disable_vision_clone = cli.disable_vision;
    let capture_unfocused_windows = cli.capture_unfocused_windows;
    let enable_realtime_vision = cli.enable_realtime_vision;
    let realtime_vision_include_image = cli.realtime_vision_include_image;
    let ignored_windows_for_task = ignored_windows_clone.clone();
    let included_windows_for_task = included_windows_clone.clone();

    let fps = if cli.fps.is_finite() && cli.fps > 0.0 {
        cli.fps
    } else {
        eprintln!("invalid fps value: {}. using default of 1.0", cli.fps);
        1.0
    };

    let mut audio_manager_builder = AudioManagerBuilder::new()
        .realtime(cli.enable_realtime_audio_transcription)
        .enabled_devices(audio_devices)
        .deepgram_api_key(cli.deepgram_api_key.clone())
        .output_path(PathBuf::from(output_path_clone.clone().to_string()))
        .languages(languages.clone());

    // Only set values if explicitly provided by user, otherwise use crate defaults
    if let Some(duration) = cli.audio_chunk_duration {
        audio_manager_builder =
            audio_manager_builder.audio_chunk_duration(Duration::from_secs(duration));
    }
    if let Some(ref vad_engine_val) = cli.vad_engine {
        audio_manager_builder = audio_manager_builder.vad_engine(vad_engine_val.clone().into());
    }
    if let Some(ref vad_sensitivity) = cli.vad_sensitivity {
        audio_manager_builder =
            audio_manager_builder.vad_sensitivity(vad_sensitivity.clone().into());
    }
    if let Some(ref transcription_engine) = cli.audio_transcription_engine {
        audio_manager_builder =
            audio_manager_builder.transcription_engine(transcription_engine.clone().into());
    }

    let audio_manager = match audio_manager_builder.build(db.clone()).await {
        Ok(manager) => Arc::new(manager),
        Err(e) => {
            error!("failed to build audio manager: {e}");
            error!("continuing without audio functionality");
            // Create a dummy audio manager or handle this more gracefully
            // For now, we need to properly clean up and exit
            tokio::task::block_in_place(|| drop(vision_runtime));
            anyhow::bail!("audio manager initialization failed: {e}");
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
                    Duration::from_secs(video_chunk_duration),
                    Arc::new(ocr_engine_for_task.clone().into()),
                    monitor_ids_clone.clone(),
                    use_pii_removal,
                    disable_vision_clone,
                    &vision_handle,
                    &ignored_windows_for_task,
                    &included_windows_for_task,
                    languages_clone.clone(),
                    capture_unfocused_windows,
                    enable_realtime_vision,
                    realtime_vision_include_image,
                );

                let result = tokio::select! {
                    result = recording_future => result,
                    _ = shutdown_rx.recv() => {
                        info!("received shutdown signal for recording");
                        break;
                    }
                };

                match result {
                    Ok(_) => {
                        // Recording completed (likely no monitors available)
                        // Sleep before retrying to avoid tight loop
                        tokio::time::sleep(Duration::from_secs(30)).await;
                    }
                    Err(e) => {
                        error!("continuous recording error: {:?}", e);
                        // Also sleep on error to avoid tight loop
                        tokio::time::sleep(Duration::from_secs(5)).await;
                    }
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
            true => Some(cubby_core::LLM::new(cubby_core::ModelName::Llama)?),
            false => None,
        }
    };

    #[cfg(feature = "llm")]
    debug!("LLM initialized");

    let server = SCServer::new(
        db_server,
        SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), cli.port),
        local_data_dir_clone_2,
        cli.disable_vision,
        cli.disable_audio,
        cli.enable_ui_monitoring,
        audio_manager.clone(),
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
        cli.audio_chunk_duration
            .map(|d| format!("{} seconds", d))
            .unwrap_or_else(|| "default (30 seconds)".to_string())
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
        warning_audio_transcription_engine_clone
            .map(|e| format!("{:?}", e))
            .unwrap_or_else(|| "default (WhisperTinyQuantized)".to_string())
    );
    println!(
        "│ ocr engine             │ {:<34} │",
        format!("{:?}", ocr_engine_clone)
    );
    println!(
        "│ vad engine             │ {:<34} │",
        vad_engine_clone
            .map(|e| format!("{:?}", e))
            .unwrap_or_else(|| "default (Silero)".to_string())
    );
    println!(
        "│ vad sensitivity        │ {:<34} │",
        vad_sensitivity_clone
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "default (Low)".to_string())
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

    println!("└────────────────────────┴────────────────────────────────────┘");

    // Add warning for cloud arguments and telemetry
    if warning_audio_transcription_engine_clone == Some(&CliAudioTranscriptionEngine::Deepgram)
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

    // start recording after all this text
    if !cli.disable_audio {
        let audio_manager_clone = audio_manager.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            audio_manager_clone.start().await.unwrap();
        });
    }

    let server_future = server.start(cli.enable_frame_cache);
    pin_mut!(server_future);

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
        drop(vision_runtime);
        drop(audio_manager);
    });

    info!("shutdown complete");

    Ok(())
}

async fn run_setup_flow(cli: &Cli, setup_state: SetupState) -> anyhow::Result<()> {
    use cubby_server::service_manager::cubbyServiceManager;

    let mut setup_state = setup_state;

    ensure_account(cli, &mut setup_state).await?;
    ensure_audio_preference(cli, &mut setup_state)?;
    ensure_cloudflared_installation(&mut setup_state).await?;

    let current_exe = std::env::current_exe()?;
    let service_manager =
        cubbyServiceManager::new(current_exe, build_service_args(cli, &setup_state), cli.port)?;

    ensure_launch_agent(&service_manager, &mut setup_state)?;

    let _ = log::step("Starting cubby service...");
    service_manager.start()?;

    #[cfg(debug_assertions)]
    println!("⏳ waiting for service to become healthy...");

    if !service_manager
        .wait_for_healthy(std::time::Duration::from_secs(30))
        .await?
    {
        #[cfg(debug_assertions)]
        {
            use std::fs;
            let home_dir = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("~"))
                .to_string_lossy()
                .to_string();

            eprintln!("\n❌ Service failed to start. Recent logs:\n");

            // Show service-error.log (stderr - most important for crashes)
            let error_log = format!("{}/.cubby/logs/service-error.log", home_dir);
            if let Ok(contents) = fs::read_to_string(&error_log) {
                let lines: Vec<&str> = contents.lines().collect();
                let start = if lines.len() > 30 {
                    lines.len() - 30
                } else {
                    0
                };
                eprintln!("📋 Last 30 lines of service-error.log:");
                for line in &lines[start..] {
                    eprintln!("  {}", line);
                }
                eprintln!();
            }

            // Show service.log (stdout)
            let service_log = format!("{}/.cubby/logs/service.log", home_dir);
            if let Ok(contents) = fs::read_to_string(&service_log) {
                let lines: Vec<&str> = contents.lines().collect();
                let start = if lines.len() > 30 {
                    lines.len() - 30
                } else {
                    0
                };
                eprintln!("📋 Last 30 lines of service.log:");
                for line in &lines[start..] {
                    eprintln!("  {}", line);
                }
                eprintln!();
            }

            eprintln!("💡 Full logs at: {}/.cubby/logs/\n", home_dir);
        }

        anyhow::bail!("cubby service did not become healthy");
    }

    use cliclack::log;
    let home_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .to_string_lossy()
        .to_string();
    log::success("cubby is running in the background!")?;
    log::success(&format!("logs: {}/.cubby/cubby-*.log", home_dir))?;
    cliclack::outro("to uninstall: cubby uninstall\n")?;
    Ok(())
}

async fn ensure_account(cli: &Cli, state: &mut SetupState) -> anyhow::Result<()> {
    if state.account_created && state.tunnel_token.is_some() {
        return Ok(());
    }

    let onboarding_result = cubby_server::run_onboarding_flow(cli).await?;
    let tunnel_token = onboarding_result
        .tunnel_token
        .ok_or_else(|| anyhow::anyhow!("authentication succeeded but tunnel token missing"))?;

    state.account_created = true;
    state.tunnel_token = Some(tunnel_token);
    state.session_jwt = onboarding_result.session_jwt;
    state.store()?;
    Ok(())
}

async fn ensure_cloudflared_installation(state: &mut SetupState) -> anyhow::Result<()> {
    use cubby_server::cloudflared_downloader::ensure_cloudflared;
    use cubby_server::cloudflared_manager::CloudflaredManager;

    let token = state
        .tunnel_token
        .clone()
        .ok_or_else(|| anyhow::anyhow!("missing tunnel token; run cubby again to authenticate"))?;

    let cloudflared_path = tokio::task::spawn_blocking(|| ensure_cloudflared())
        .await
        .map_err(|e| anyhow::anyhow!("failed to download cloudflared: {}", e))??;

    let manager = CloudflaredManager::new(cloudflared_path.clone())?;
    if manager.is_installed() && state.cloudflared_installed {
        return Ok(());
    }

    let _ = cliclack::log::step("Setting up...");

    let path_clone = cloudflared_path.clone();
    let token_clone = token.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mgr = CloudflaredManager::new(path_clone)?;
        mgr.install_with_overwrite(&token_clone)?;
        Ok(())
    })
    .await
    .map_err(|e| anyhow::anyhow!("cloudflared install task failed: {}", e))??;

    state.cloudflared_installed = true;
    state.store()?;
    Ok(())
}

fn ensure_launch_agent(
    service_manager: &cubby_server::service_manager::cubbyServiceManager,
    state: &mut SetupState,
) -> anyhow::Result<()> {
    if service_manager.is_installed() && state.service_installed {
        return Ok(());
    }

    service_manager.install()?;
    state.service_installed = true;
    state.store()?;
    Ok(())
}

fn ensure_audio_preference(cli: &Cli, state: &mut SetupState) -> anyhow::Result<()> {
    // If user supplied flag explicitly, respect it and persist
    if cli.disable_audio {
        if state.audio_enabled != Some(false) {
            state.audio_enabled = Some(false);
            state.store()?
        }
        return Ok(());
    }

    if let Some(choice) = state.audio_enabled {
        // State already recorded - nothing to do
        if !choice && !cli.disable_audio {
            // user previously disabled audio but did not pass flag this time; keep state
        }
        return Ok(());
    }

    let enable_audio =
        cliclack::confirm("enable audio recording? (screen capture enabled by default)")
            .initial_value(false)
            .interact()?;

    state.audio_enabled = Some(enable_audio);
    state.store()?;
    if enable_audio {
        cliclack::log::info("audio recording will be enabled")?;
    } else {
        cliclack::log::info("audio recording will remain disabled")?;
    }
    Ok(())
}

async fn ensure_permissions_in_service(cli: &Cli, state: &mut SetupState) -> anyhow::Result<()> {
    let mut updated = false;

    if !cli.disable_audio && !state.microphone_granted {
        info!("requesting microphone permission from cubby service...");
        let granted = trigger_and_check_microphone().await?;
        if granted {
            info!("microphone permission granted");
            state.microphone_granted = true;
            updated = true;
        } else {
            warn!("microphone permission not granted");
            anyhow::bail!("microphone permission required");
        }
    }

    if !cli.disable_vision && !state.screen_granted {
        info!("requesting screen recording permission from cubby service...");
        let granted = trigger_and_check_screen_recording().await?;
        if granted {
            info!("screen recording permission granted");
            state.screen_granted = true;
            updated = true;
        } else {
            warn!("screen recording permission not granted");
            anyhow::bail!("screen recording permission required");
        }
    }

    if updated {
        state.store()?;
    }

    Ok(())
}

async fn handle_uninstall() -> anyhow::Result<()> {
    use cubby_server::cloudflared_downloader::{cleanup_cloudflared, ensure_cloudflared};
    use cubby_server::cloudflared_manager::CloudflaredManager;
    use cubby_server::service_manager::cubbyServiceManager;

    #[cfg(debug_assertions)]
    cliclack::log::info("Stopping and uninstalling cubby service...")?;

    // Try to uninstall cloudflared service if it exists
    if let Some(cloudflared_path) = tokio::task::spawn_blocking(|| ensure_cloudflared())
        .await
        .ok()
        .and_then(|r| r.ok())
    {
        let cloudflared_manager = CloudflaredManager::new(cloudflared_path)?;
        if cloudflared_manager.is_installed() {
            #[cfg(debug_assertions)]
            println!("🛑 Uninstalling cloudflared tunnel service...");

            let uninstall_result =
                tokio::task::spawn_blocking(move || cloudflared_manager.uninstall())
                    .await
                    .map_err(|e| {
                        anyhow::anyhow!("Failed to spawn cloudflared uninstall task: {}", e)
                    })?;

            if let Err(e) = uninstall_result {
                #[cfg(debug_assertions)]
                println!(
                    "⚠️  Warning: Failed to uninstall cloudflared service: {}",
                    e
                );

                #[cfg(not(debug_assertions))]
                {
                    use cliclack::log;
                    log::warning(format!("Failed to uninstall cloudflared service: {}", e))?;
                }
            }
        }

        // Clean up cloudflared binaries
        if let Some(Err(e)) = tokio::task::spawn_blocking(|| cleanup_cloudflared())
            .await
            .ok()
        {
            #[cfg(debug_assertions)]
            println!(
                "⚠️  Warning: Failed to clean up cloudflared binaries: {}",
                e
            );
        }
    }

    let current_exe = std::env::current_exe()?;
    let service_manager = cubbyServiceManager::new(current_exe, vec![], 3030)?;

    // Check if service exists first
    if !service_manager.is_installed() {
        cliclack::outro("No cubby service found to uninstall")?;
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

    // Remove CLI binary/symlink so `cubby` is no longer available
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::fs;
        use std::path::PathBuf;
        // remove ~/.local/bin/cubby (symlink or binary)
        if let Some(home) = dirs::home_dir() {
            let bin_link: PathBuf = home.join(".local").join("bin").join("cubby");
            let _ = fs::remove_file(&bin_link);

            // remove install dir ~/.local/cubby if present (used by install.sh)
            let install_dir: PathBuf = home.join(".local").join("cubby");
            let _ = fs::remove_dir_all(&install_dir);
        }
    }

    #[cfg(target_os = "windows")]
    {
        use std::fs;
        use std::io::Write;
        use std::path::PathBuf;
        use std::process::Command;

        // current_exe typically is: %USERPROFILE%\cubby\bin\cubby.exe
        let exe_path = std::env::current_exe()?;
        let install_root: Option<PathBuf> = exe_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_path_buf());

        // Create a temporary batch file to delete after this process exits
        let mut bat_path = std::env::temp_dir();
        bat_path.push("cubby_self_delete.bat");

        let mut script = String::new();
        script.push_str("@echo off\r\n");
        // short delay to ensure current process has exited
        script.push_str("timeout /t 2 /nobreak > NUL\r\n");
        script.push_str(&format!(
            "del /f /q \"{}\" > NUL 2>&1\r\n",
            exe_path.to_string_lossy()
        ));
        if let Some(root) = install_root {
            script.push_str(&format!(
                "rmdir /s /q \"{}\" > NUL 2>&1\r\n",
                root.to_string_lossy()
            ));
        }
        // delete the bat itself
        script.push_str("del /f /q \"%~f0\" > NUL 2>&1\r\n");

        // Write batch file
        if let Ok(mut f) = fs::File::create(&bat_path) {
            let _ = f.write_all(script.as_bytes());
        }

        // Spawn detached cmd to run the batch (use 'start' to detach)
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", &bat_path.to_string_lossy()])
            .spawn();
    }

    #[cfg(not(debug_assertions))]
    {
        use cliclack::log;
        cliclack::outro("Uninstall complete!")?;
    }

    if let Err(e) = SetupState::delete() {
        warn!("failed to delete setup state: {e}");
    }

    Ok(())
}

fn build_service_args(cli: &Cli, state: &SetupState) -> Vec<String> {
    let mut args = vec!["service".to_string()];

    // Port
    args.push("--port".to_string());
    args.push(cli.port.to_string());

    // FPS
    args.push("--fps".to_string());
    args.push(cli.fps.to_string());

    // Audio chunk duration (only if explicitly set)
    if let Some(duration) = cli.audio_chunk_duration {
        args.push("--audio-chunk-duration".to_string());
        args.push(duration.to_string());
    }

    // Video chunk duration
    args.push("--video-chunk-duration".to_string());
    args.push(cli.video_chunk_duration.to_string());

    // Boolean flags
    let audio_enabled = state.audio_enabled.unwrap_or(!cli.disable_audio);
    if !audio_enabled {
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

    if cli.realtime_vision_include_image {
        args.push("--realtime-vision-include-image".to_string());
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

    // Audio transcription engine (only if explicitly set)
    if let Some(engine) = &cli.audio_transcription_engine {
        args.push("--audio-transcription-engine".to_string());
        args.push(
            match engine {
                CliAudioTranscriptionEngine::Deepgram => "deepgram",
                CliAudioTranscriptionEngine::WhisperTiny => "whisper-tiny",
                CliAudioTranscriptionEngine::WhisperTinyQuantized => "whisper-tiny-quantized",
                CliAudioTranscriptionEngine::WhisperLargeV3 => "whisper-large",
                CliAudioTranscriptionEngine::WhisperLargeV3Quantized => "whisper-large-quantized",
                CliAudioTranscriptionEngine::WhisperLargeV3Turbo => "whisper-large-v3-turbo",
                CliAudioTranscriptionEngine::WhisperLargeV3TurboQuantized => {
                    "whisper-large-v3-turbo-quantized"
                }
            }
            .to_string(),
        );
    }

    // OCR engine
    args.push("--ocr-engine".to_string());
    args.push(
        match cli.ocr_engine {
            CliOcrEngine::Unstructured => "unstructured",
            #[cfg(target_os = "macos")]
            CliOcrEngine::AppleNative => "apple-native",
            #[cfg(target_os = "linux")]
            CliOcrEngine::Tesseract => "tesseract",
            #[cfg(target_os = "windows")]
            CliOcrEngine::WindowsNative => "windows-native",
            CliOcrEngine::Custom => "custom",
        }
        .to_string(),
    );

    // VAD engine (only if explicitly set)
    if let Some(engine) = &cli.vad_engine {
        args.push("--vad-engine".to_string());
        args.push(
            match engine {
                CliVadEngine::WebRtc => "webrtc",
                CliVadEngine::Silero => "silero",
            }
            .to_string(),
        );
    }

    // VAD sensitivity (only if explicitly set)
    if let Some(sensitivity) = &cli.vad_sensitivity {
        args.push("--vad-sensitivity".to_string());
        args.push(
            match sensitivity {
                CliVadSensitivity::Low => "low",
                CliVadSensitivity::Medium => "medium",
                CliVadSensitivity::High => "high",
            }
            .to_string(),
        );
    }

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
