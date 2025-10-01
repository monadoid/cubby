use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn install_signal_flag() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);

    // Handler runs on a dedicated thread; keep it tiny & non-blocking.
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
        .expect("failed to install Ctrl-C handler");

    running
}