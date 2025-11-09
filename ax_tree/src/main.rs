#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    init_tracing();

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    let list_mode = args.iter().any(|arg| arg == "--list-mode");
    let dump_tree = args.iter().any(|arg| arg == "--dump-tree");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Accessibility Tree Viewer")
            .with_icon(
                // NOTE: Adding an icon is optional
                eframe::icon_data::from_png_bytes(&include_bytes!("../assets/icon-256.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };
    eframe::run_native(
        "ax_tree",
        native_options,
        Box::new(move |cc| Ok(Box::new(ax_tree::AxTreeApp::new(cc, list_mode, dump_tree)))),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn init_tracing() {
    use once_cell::sync::OnceCell;
    use std::path::PathBuf;
    use tracing_appender::{non_blocking, rolling};
    use tracing_log::LogTracer;
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

    static FILE_GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();

    let _ = LogTracer::init();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn"));

    let log_dir = PathBuf::from("logs");
    let log_dir = if log_dir.exists() {
        log_dir
    } else {
        PathBuf::from("ax_tree/logs")
    };

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("failed to create log directory {:?}: {e}", log_dir);
    }

    let file_appender = rolling::never(&log_dir, "ax_observer.log");
    let (file_writer, guard) = non_blocking(file_appender);
    let _ = FILE_GUARD.set(guard);

    let stdout_layer = fmt::layer().with_target(false).with_ansi(true);
    let file_layer = fmt::layer()
        .with_writer(file_writer)
        .with_ansi(false)
        .with_target(true);

    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer);

    let _ = subscriber.try_init();
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("No window")
            .document()
            .expect("No document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("Failed to find the_canvas_id")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id was not a HtmlCanvasElement");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(ax_tree::AxTreeApp::new(cc, false, false)))),
            )
            .await;

        // Remove the loading text and spinner:
        if let Some(loading_text) = document.get_element_by_id("loading_text") {
            match start_result {
                Ok(_) => {
                    loading_text.remove();
                }
                Err(e) => {
                    loading_text.set_inner_html(
                        "<p> The app has crashed. See the developer console for details. </p>",
                    );
                    panic!("Failed to start eframe: {e:?}");
                }
            }
        }
    });
}
