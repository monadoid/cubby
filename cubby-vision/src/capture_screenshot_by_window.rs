use image::DynamicImage;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use tracing::{debug, error};

use xcap::{Window, XCapError};

use crate::monitor::SafeMonitor;

#[derive(Debug)]
enum CaptureError {
    NoWindows,
    XCapError(XCapError),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CaptureError::NoWindows => write!(f, "No windows found"),
            CaptureError::XCapError(e) => write!(f, "XCap error: {}", e),
        }
    }
}

impl Error for CaptureError {}

impl From<XCapError> for CaptureError {
    fn from(error: XCapError) -> Self {
        error!("XCap error occurred: {}", error);
        CaptureError::XCapError(error)
    }
}

// Platform specific skip lists
#[cfg(target_os = "macos")]
static SKIP_APPS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Window Server",
        "SystemUIServer",
        "ControlCenter",
        "Control Center",
        "Dock",
        "NotificationCenter",
        "loginwindow",
        "WindowManager",
        "Contexts",
        "Screenshot",
    ])
});

#[cfg(target_os = "windows")]
static SKIP_APPS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Windows Shell Experience Host",
        "Microsoft Text Input Application",
        "Windows Explorer",
        "Program Manager",
        "Microsoft Store",
        "Search",
        "TaskBar",
    ])
});

#[cfg(target_os = "linux")]
static SKIP_APPS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Gnome-shell",
        "Plasma",
        "Xfdesktop",
        "Polybar",
        "i3bar",
        "Plank",
        "Dock",
    ])
});

#[cfg(target_os = "macos")]
static SKIP_TITLES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Item-0",
        "App Icon Window",
        "Dock",
        "NowPlaying",
        "FocusModes",
        "Shortcuts",
        "AudioVideoModule",
        "Clock",
        "WiFi",
        "Battery",
        "BentoBox",
        "Menu Bar",
        "Notification Center",
        "Control Center",
        "Spotlight",
        "Mission Control",
        "Desktop",
        "Screen Sharing",
        "Touch Bar",
        "Status Bar",
        "Menu Extra",
        "System Settings",
    ])
});

#[cfg(target_os = "windows")]
static SKIP_TITLES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Program Manager",
        "Windows Input Experience",
        "Microsoft Text Input Application",
        "Task View",
        "Start",
        "System Tray",
        "Notification Area",
        "Action Center",
        "Task Bar",
        "Desktop",
    ])
});

#[cfg(target_os = "linux")]
static SKIP_TITLES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "Desktop",
        "Panel",
        "Top Bar",
        "Status Bar",
        "Dock",
        "Dashboard",
        "Activities",
        "System Tray",
        "Notification Area",
    ])
});

#[derive(Debug, Clone)]
pub struct CapturedWindow {
    pub image: DynamicImage,
    pub app_name: String,
    pub window_name: String,
    pub process_id: i32,
    pub is_focused: bool,
}

pub struct WindowFilters {
    ignore_set: HashSet<String>,
    include_set: HashSet<String>,
}

impl WindowFilters {
    pub fn new(ignore_list: &[String], include_list: &[String]) -> Self {
        Self {
            ignore_set: ignore_list.iter().map(|s| s.to_lowercase()).collect(),
            include_set: include_list.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    // O(n) - we could figure out a better way to do this
    pub fn is_valid(&self, app_name: &str, title: &str) -> bool {
        let app_name_lower = app_name.to_lowercase();
        let title_lower = title.to_lowercase();

        // If include list is empty, we're done
        if self.include_set.is_empty() {
            return true;
        }

        // Check include list
        if self
            .include_set
            .iter()
            .any(|include| app_name_lower.contains(include) || title_lower.contains(include))
        {
            return true;
        }

        // Check ignore list first (usually smaller)
        if !self.ignore_set.is_empty()
            && self
                .ignore_set
                .iter()
                .any(|ignore| app_name_lower.contains(ignore) || title_lower.contains(ignore))
        {
            return false;
        }

        false
    }
}

pub async fn capture_all_visible_windows(
    monitor: &SafeMonitor,
    window_filters: &WindowFilters,
    capture_unfocused_windows: bool,
) -> Result<Vec<CapturedWindow>, Box<dyn Error>> {
    let mut all_captured_images = Vec::new();

    // Get windows and immediately extract the data we need
    struct WindowCapture {
        app_name: String,
        window_name: String,
        buffer: image::RgbaImage,
        process_id: i32,
        is_focused: bool,
        monitor_id: Option<u32>,
    }

    let mut windows_data: Vec<WindowCapture> = Vec::new();

    for window in Window::all()? {
        let app_name = match window.app_name() {
            Ok(name) => name.to_string(),
            Err(e) => {
                // Log warning and skip this window
                // mostly noise
                debug!("Failed to get app_name for window: {}", e);
                continue;
            }
        };

        let title = match window.title() {
            Ok(title) => title.to_string(),
            Err(e) => {
                error!("Failed to get title for window {}: {}", app_name, e);
                continue;
            }
        };

        match window.is_minimized() {
            Ok(is_minimized) => {
                if is_minimized {
                    debug!("Window {} ({}) is_minimized", app_name, title);
                    continue;
                }
            }
            Err(e) => {
                // Log warning and skip this window
                // mostly noise
                error!("Failed to get is_minimized for window {}: {}", app_name, e);
            }
        };

        let process_id = match window.pid() {
            Ok(pid) => pid as i32,
            Err(e) => {
                error!(
                    "Failed to get process ID for window {} ({}): {}",
                    app_name, title, e
                );
                -1
            }
        };

        let is_focused = window.is_focused().unwrap_or(false);

        let monitor_id = window.current_monitor().ok().and_then(|m| m.id().ok());

        match window.capture_image() {
            Ok(buffer) => windows_data.push(WindowCapture {
                app_name,
                window_name: title,
                buffer,
                process_id,
                is_focused,
                monitor_id,
            }),
            Err(e) => {
                error!(
                    "Failed to capture image for window {} ({}): {}",
                    app_name, title, e
                );
            }
        }
    }

    if windows_data.is_empty() {
        return Err(Box::new(CaptureError::NoWindows));
    }

    // Filter windows that belong to this monitor and pass skip/include filters.
    let candidates: Vec<WindowCapture> = windows_data
        .into_iter()
        .filter(|capture| {
            let monitor_matches = capture
                .monitor_id
                .map(|id| id == monitor.id())
                .unwrap_or(true);

            let passes_filters = !SKIP_APPS.contains(capture.app_name.as_str())
                && !SKIP_TITLES.contains(capture.window_name.as_str())
                && window_filters.is_valid(&capture.app_name, &capture.window_name);

            monitor_matches && passes_filters
        })
        .collect();

    if candidates.is_empty() {
        return Ok(all_captured_images);
    }

    // Prefer the highest priority candidate that xcap marks as focused.
    let focused_index = candidates
        .iter()
        .position(|capture| capture.is_focused)
        .or(Some(0));

    // Process candidates in order, keeping only the frontmost focused window unless unfocused capture is enabled.
    for (idx, capture) in candidates.into_iter().enumerate() {
        let is_frontmost = Some(idx) == focused_index;
        if !is_frontmost && !capture_unfocused_windows {
            continue;
        }

        // Convert to DynamicImage
        let image = DynamicImage::ImageRgba8(
            image::ImageBuffer::from_raw(
                capture.buffer.width(),
                capture.buffer.height(),
                capture.buffer.into_raw(),
            )
            .unwrap(),
        );

        all_captured_images.push(CapturedWindow {
            image,
            app_name: capture.app_name,
            window_name: capture.window_name,
            process_id: capture.process_id,
            is_focused: is_frontmost,
        });
    }

    Ok(all_captured_images)
}
