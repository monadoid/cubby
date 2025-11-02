//! macOS AXObserver integration for event-driven tree updates
//!
//! This module watches for accessibility notifications and triggers tree snapshots

#[cfg(target_os = "macos")]
use crate::tree::{build_tree_snapshot, recorder::AxTreeRecorder, AxElement};
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSRunningApplication, NSWorkspace};
#[cfg(target_os = "macos")]
use objc2_application_services::{AXError, AXIsProcessTrustedWithOptions, AXObserver, AXUIElement};
#[cfg(target_os = "macos")]
use objc2_core_foundation::{
    kCFRunLoopDefaultMode, CFRetained, CFRunLoop, CFRunLoopSource, CFString as Objc2CFString,
};
#[cfg(target_os = "macos")]
use objc2_foundation::NSString;
#[cfg(target_os = "macos")]
use std::collections::HashMap;
#[cfg(target_os = "macos")]
use std::os::raw::c_void;
#[cfg(target_os = "macos")]
use std::ptr::NonNull;
#[cfg(target_os = "macos")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};

// Notification constants - these are the string values used by the AX API
#[cfg(target_os = "macos")]
const K_AX_APPLICATION_ACTIVATED_NOTIFICATION: &str = "AXApplicationActivated";
#[cfg(target_os = "macos")]
const K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION: &str = "AXFocusedWindowChanged";

#[cfg(target_os = "macos")]
#[derive(Debug)]
pub struct ObserverShared {
    recorder: Arc<Mutex<AxTreeRecorder>>,
    debounce: Arc<Mutex<HashMap<i32, Instant>>>,
    ignored: Arc<Mutex<HashMap<i32, Instant>>>,
}

#[cfg(target_os = "macos")]
impl ObserverShared {
    pub fn new() -> Self {
        Self {
            recorder: Arc::new(Mutex::new(AxTreeRecorder::new(100))),
            debounce: Arc::new(Mutex::new(HashMap::new())),
            ignored: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn recorder(&self) -> Arc<Mutex<AxTreeRecorder>> {
        self.recorder.clone()
    }

    pub fn handle_notification(&self, pid: i32, notification: &str) {
        {
            let ignored = match self.ignored.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            if ignored.contains_key(&pid) {
                return;
            }
        }

        const DEBOUNCE: Duration = Duration::from_millis(350);
        let now = Instant::now();

        {
            let mut last = match self.debounce.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };

            if let Some(previous) = last.get(&pid) {
                if now.duration_since(*previous) < DEBOUNCE {
                    return;
                }
            }

            last.insert(pid, now);
        }

        let mut recorder = match self.recorder.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Err(err) = snapshot_by_pid(pid, notification, &mut *recorder) {
            tracing::warn!(error = ?err, pid, "snapshot error");
            let mut ignored = match self.ignored.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            ignored.insert(pid, Instant::now());
        }
    }
}

#[cfg(target_os = "macos")]
fn snapshot_by_pid(
    pid: i32,
    notification: &str,
    recorder: &mut AxTreeRecorder,
) -> anyhow::Result<()> {
    use anyhow::Context;

    let running_app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid);

    let app_label = running_app
        .as_ref()
        .and_then(|app| {
            app.localizedName()
                .as_ref()
                .map(|name| ns_string_to_string(name))
        })
        .or_else(|| {
            running_app.as_ref().and_then(|app| {
                app.bundleIdentifier()
                    .as_ref()
                    .map(|id| ns_string_to_string(id))
            })
        })
        .unwrap_or_else(|| format!("pid {}", pid));

    tracing::debug!(
        "[AX] {} for '{app_label}' (pid {pid})",
        match notification {
            n if n == K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION => "Focused window changed",
            n if n == K_AX_APPLICATION_ACTIVATED_NOTIFICATION => "Application activated",
            other => other,
        }
    );

    // Get application-specific root element (like cubby-core does)
    // The playground uses engine.get_root_element() which returns system-wide,
    // but we need to use the application element to get proper children
    // Actually, looking at cubby-core, it uses system-wide but the issue might be
    // in how we get children. Let's try using the application element first.
    let root_element =
        AxElement::for_application(pid).context("failed to get application element")?;

    // Build tree snapshot from root
    let snapshot = build_tree_snapshot(&root_element, pid, notification)
        .context("failed to build tree snapshot")?;

    let is_active = running_app
        .as_ref()
        .map(|app| app.isActive())
        .unwrap_or(false);
    let notification_indicates_focus = matches!(
        notification,
        K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION | K_AX_APPLICATION_ACTIVATED_NOTIFICATION
    );

    // Get the actual frontmost application PID to determine which snapshot should be promoted
    let frontmost_pid = {
        let workspace = NSWorkspace::sharedWorkspace();
        workspace
            .frontmostApplication()
            .as_ref()
            .map(|app| app.processIdentifier())
    };

    // Capture snapshot and get diff
    let diff = recorder.capture(
        snapshot.clone(),
        is_active || notification_indicates_focus,
        frontmost_pid,
    );

    tracing::debug!(
        "[AX DEBUG] '{}' (pid {}): {} nodes captured, root has {} children, frontmost_pid={:?}",
        app_label,
        pid,
        snapshot.nodes.len(),
        snapshot.root().map(|r| r.children.len()).unwrap_or(0),
        frontmost_pid
    );

    if let Some(ref diff) = diff {
        if diff.has_changes() {
            tracing::info!("[AX] Changes for '{app_label}': {}", diff.summary());
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn ns_string_to_string(ns_string: &NSString) -> String {
    ns_string.to_string()
}

#[cfg(target_os = "macos")]
pub fn start_observer_background(
    shared: Arc<ObserverShared>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    use std::sync::atomic::AtomicBool;
    use std::thread;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let shared_clone = Arc::clone(&shared);
    let stop_flag_clone = Arc::clone(&stop_flag);

    let handle = thread::spawn(move || {
        if let Err(e) = run_accessibility_loop(shared_clone, stop_flag_clone) {
            tracing::error!(error = ?e, "accessibility loop error");
        }
    });

    Ok(handle)
}

#[cfg(target_os = "macos")]
fn run_accessibility_loop(
    shared: Arc<ObserverShared>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
) -> anyhow::Result<()> {
    // Check if process is trusted
    let is_trusted = unsafe { AXIsProcessTrustedWithOptions(None) };
    if !is_trusted {
        tracing::warn!(
            "Accessibility access is required but not yet granted. Open System Settings → Privacy & Security → Accessibility and enable access for this app."
        );
        return Ok(());
    }

    let run_loop =
        CFRunLoop::current().ok_or_else(|| anyhow::anyhow!("failed to get current run loop"))?;

    let workspace = NSWorkspace::sharedWorkspace();
    let running_apps = workspace.runningApplications();

    let mut handles = Vec::new();
    let mut skipped = HashMap::new();

    for app in running_apps.iter() {
        let pid = app.processIdentifier();
        let label = app
            .localizedName()
            .as_ref()
            .map(|n| ns_string_to_string(n))
            .or_else(|| {
                app.bundleIdentifier()
                    .as_ref()
                    .map(|id| ns_string_to_string(id))
            })
            .unwrap_or_else(|| format!("pid {pid}"));

        if !should_observe_app(&app) {
            skipped.insert(
                pid,
                format!("Skipping observer for '{label}' (pid {pid}) — filtered out"),
            );
            continue;
        }

        unsafe {
            match register_accessibility_observer(
                pid as i32,
                run_loop.clone(),
                &mut handles,
                &label,
                &mut skipped,
                Arc::clone(&shared),
            ) {
                Ok(_) => {}
                Err(err) => {
                    skipped.insert(
                        pid,
                        format!("Skipping observer for '{label}' (pid {pid}): {err:#}"),
                    );
                }
            }
        }
    }

    for message in skipped.values() {
        tracing::debug!("{message}");
    }

    tracing::info!(
        "Watching {} applications for AX notifications",
        handles.len()
    );

    while !stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, 0.25, false);
    }

    tracing::info!("Stopping accessibility observers...");
    drop(handles);

    Ok(())
}

#[cfg(target_os = "macos")]
unsafe fn register_accessibility_observer(
    pid: i32,
    run_loop: CFRetained<CFRunLoop>,
    handles: &mut Vec<ObserverHandle>,
    label: &str,
    skipped: &mut HashMap<i32, String>,
    shared: Arc<ObserverShared>,
) -> anyhow::Result<()> {
    // Create observer using objc2-application-services
    let mut observer_ptr: *mut AXObserver = std::ptr::null_mut();
    let observer_out = NonNull::new(&mut observer_ptr)
        .ok_or_else(|| anyhow::anyhow!("failed to create non-null pointer"))?;

    let status = AXObserver::create(
        pid as libc::pid_t,
        Some(ax_observer_callback as unsafe extern "C-unwind" fn(_, _, _, _)),
        observer_out,
    );

    if status != AXError::Success {
        anyhow::bail!(
            "AXObserverCreate failed for '{}' (pid {}): {:?}",
            label,
            pid,
            status
        );
    }

    let observer_ptr = NonNull::new(observer_ptr).ok_or_else(|| {
        anyhow::anyhow!("observer creation returned success but observer is null")
    })?;
    let observer = unsafe { observer_ptr.as_ref() };

    // Create AXUIElement for application using objc2-application-services
    let ax_app = AXUIElement::new_application(pid as libc::pid_t);

    // Create CFString instances for notifications
    let activation_cf = Objc2CFString::from_str(K_AX_APPLICATION_ACTIVATED_NOTIFICATION);
    let focus_cf = Objc2CFString::from_str(K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION);

    let refcon = Box::into_raw(Box::new(CallbackRefcon { pid, shared })) as *mut c_void;

    let mut activation_registered = false;
    let mut focus_registered = false;

    // Add activation notification
    let add_activation = unsafe { observer.add_notification(&ax_app, &activation_cf, refcon) };
    match add_activation {
        AXError::Success => activation_registered = true,
        AXError::NotificationUnsupported => {
            skipped.insert(
                pid,
                format!(
                    "Activation notification unsupported for '{}' (pid {})",
                    label, pid
                ),
            );
        }
        status => {
            drop(ax_app);
            drop(Box::from_raw(refcon as *mut CallbackRefcon));
            anyhow::bail!(
                "AXObserverAddNotification (activation) failed for '{}' (pid {}): {:?}",
                label,
                pid,
                status
            );
        }
    }

    // Add focus notification
    let add_focus = unsafe { observer.add_notification(&ax_app, &focus_cf, refcon) };
    match add_focus {
        AXError::Success => focus_registered = true,
        AXError::NotificationUnsupported => {
            skipped.insert(
                pid,
                format!(
                    "Focused window notification unsupported for '{}' (pid {})",
                    label, pid
                ),
            );
        }
        status => {
            if !activation_registered {
                drop(ax_app);
                drop(Box::from_raw(refcon as *mut CallbackRefcon));
                anyhow::bail!(
                    "AXObserverAddNotification (focus) failed for '{}' (pid {}): {:?}",
                    label,
                    pid,
                    status
                );
            } else {
                skipped.insert(
                    pid,
                    format!(
                        "Focused window notification failed for '{}' (pid {}): {:?}",
                        label, pid, status
                    ),
                );
            }
        }
    }

    if !activation_registered && !focus_registered {
        drop(ax_app);
        drop(Box::from_raw(refcon as *mut CallbackRefcon));
        return Ok(());
    }

    // Get run loop source using the observer method
    let source = observer.run_loop_source();
    run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

    drop(ax_app);

    handles.push(ObserverHandle {
        _observer: observer_ptr,
        source,
        refcon: refcon as *mut CallbackRefcon,
        run_loop,
    });

    Ok(())
}

#[cfg(target_os = "macos")]
unsafe extern "C-unwind" fn ax_observer_callback(
    _observer: NonNull<AXObserver>,
    _element: NonNull<AXUIElement>,
    notification: NonNull<Objc2CFString>,
    refcon: *mut c_void,
) {
    if refcon.is_null() {
        return;
    }

    let refcon_box = &*(refcon.cast::<CallbackRefcon>());
    let pid = refcon_box.pid;
    let shared = &refcon_box.shared;

    // Convert CFString to Rust String using objc2_core_foundation
    let cf_string = notification.as_ref();
    let notification_str = cf_string.to_string();

    shared.handle_notification(pid, &notification_str);
}

#[cfg(target_os = "macos")]
struct ObserverHandle {
    _observer: NonNull<AXObserver>,
    source: CFRetained<CFRunLoopSource>,
    refcon: *mut CallbackRefcon,
    run_loop: CFRetained<CFRunLoop>,
}

#[cfg(target_os = "macos")]
impl Drop for ObserverHandle {
    fn drop(&mut self) {
        self.run_loop
            .remove_source(Some(&self.source), unsafe { kCFRunLoopDefaultMode });
        // observer and source are automatically dropped via CFRetained
        unsafe {
            drop(Box::from_raw(self.refcon));
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct CallbackRefcon {
    pid: i32,
    shared: Arc<ObserverShared>,
}

#[cfg(target_os = "macos")]
const EXCLUDE_PREFIXES: &[&str] = &[
    "com.apple.dock",
    "com.apple.windowmanager",
    "com.apple.viewbridge",
    "com.apple.universalaccessd",
    "com.apple.familycircled",
    "com.apple.chronod",
    "com.apple.uikit",
    "com.apple.cursorui",
    "cursoruiviewservice",
    "viewbridgeauxiliary",
];

#[cfg(target_os = "macos")]
fn should_observe_app(app: &NSRunningApplication) -> bool {
    if app.isTerminated() {
        return false;
    }

    if !app.isFinishedLaunching() {
        return false;
    }

    if app.ownsMenuBar() {
        return false;
    }

    let name = app
        .localizedName()
        .as_ref()
        .map(|n| ns_string_to_string(n))
        .or_else(|| {
            app.bundleIdentifier()
                .as_ref()
                .map(|id| ns_string_to_string(id))
        });

    if let Some(name) = name {
        let lower = name.to_ascii_lowercase();
        if EXCLUDE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
        {
            return false;
        }
    }

    true
}

// Non-macOS stub
#[cfg(not(target_os = "macos"))]
pub struct ObserverShared;

#[cfg(not(target_os = "macos"))]
impl ObserverShared {
    pub fn new() -> Self {
        Self
    }

    pub fn recorder(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<crate::tree::recorder::AxTreeRecorder>> {
        std::sync::Arc::new(std::sync::Mutex::new(
            crate::tree::recorder::AxTreeRecorder::new(100),
        ))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn start_observer_background(
    _shared: std::sync::Arc<ObserverShared>,
) -> anyhow::Result<std::thread::JoinHandle<()>> {
    Ok(std::thread::spawn(|| {}))
}
