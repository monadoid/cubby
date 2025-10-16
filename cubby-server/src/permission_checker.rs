use anyhow::Result;

/// Check screen recording permission using hybrid trigger + poll approach
/// 1. Attempt actual screen capture to trigger dialog
/// 2. Poll preflight API until granted or timeout
#[cfg(target_os = "macos")]
pub async fn trigger_and_check_screen_recording() -> Result<bool> {
    use cubby_vision::monitor::list_monitors;
    use objc2_core_graphics::CGPreflightScreenCaptureAccess;
    use tokio::time::{sleep, Duration};

    // First check if already granted
    if CGPreflightScreenCaptureAccess() {
        tracing::debug!("Screen recording permission already granted");
        return Ok(true);
    }

    tracing::info!("Screen recording permission needed - triggering dialog...");

    // Trigger the dialog by attempting to actually capture from a monitor
    let monitors = list_monitors().await;
    if let Some(monitor) = monitors.first() {
        let _ = monitor.capture_image().await; // This triggers the dialog
    }

    // Now poll until permission is granted or we timeout
    const MAX_ATTEMPTS: u32 = 60; // 60 seconds timeout
    const POLL_INTERVAL: Duration = Duration::from_secs(1);

    for attempt in 1..=MAX_ATTEMPTS {
        if CGPreflightScreenCaptureAccess() {
            tracing::info!(
                "Screen recording permission granted after {} seconds",
                attempt
            );
            return Ok(true);
        }

        if attempt < MAX_ATTEMPTS {
            sleep(POLL_INTERVAL).await;
        }
    }

    tracing::warn!(
        "Screen recording permission not granted after {} seconds",
        MAX_ATTEMPTS
    );
    Ok(false)
}

#[cfg(not(target_os = "macos"))]
pub async fn trigger_and_check_screen_recording() -> Result<bool> {
    Ok(true)
}

/// Check microphone permission using hybrid trigger + poll approach
/// 1. Attempt listing devices to trigger dialog
/// 2. Poll until devices are accessible
#[cfg(target_os = "macos")]
pub async fn trigger_and_check_microphone() -> Result<bool> {
    use cubby_audio::core::device::list_audio_devices;
    use tokio::time::{sleep, Duration};

    tracing::info!("Microphone permission needed - triggering dialog...");

    // Trigger the dialog by attempting to list audio devices
    let _ = list_audio_devices().await;

    // Poll until we can successfully list devices or timeout
    const MAX_ATTEMPTS: u32 = 60; // 60 seconds timeout
    const POLL_INTERVAL: Duration = Duration::from_secs(1);

    for attempt in 1..=MAX_ATTEMPTS {
        match list_audio_devices().await {
            Ok(devices) if !devices.is_empty() => {
                tracing::info!("Microphone permission granted after {} seconds", attempt);
                return Ok(true);
            }
            _ => {
                if attempt < MAX_ATTEMPTS {
                    sleep(POLL_INTERVAL).await;
                }
            }
        }
    }

    tracing::warn!(
        "Microphone permission not granted after {} seconds",
        MAX_ATTEMPTS
    );
    Ok(false)
}

#[cfg(not(target_os = "macos"))]
pub async fn trigger_and_check_microphone() -> Result<bool> {
    Ok(true)
}

/// Check if accessibility permission is granted (optional, for UI monitoring)
#[cfg(target_os = "macos")]
pub fn check_accessibility_permission() -> bool {
    // Skip for now - optional feature
    true
}

#[cfg(not(target_os = "macos"))]
pub fn check_accessibility_permission() -> bool {
    true
}
