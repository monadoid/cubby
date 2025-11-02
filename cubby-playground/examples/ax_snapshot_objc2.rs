use cubby_playground::{bootstrap, PlaygroundOptions};

use objc2_application_services::{
    AXIsProcessTrustedWithOptions, AXObserver, AXUIElement, AXError,
};
use objc2_core_foundation::{
    CFString as Objc2CFString, CFRunLoop, CFRunLoopSource, CFRetained, kCFRunLoopDefaultMode,
};
use objc2_app_kit::{NSWorkspace, NSRunningApplication};
use objc2_foundation::NSString;
use cubby_core::operator::platforms::{create_engine, AccessibilityEngine};
use cubby_core::operator::{AutomationError, UIElement};
use cubby_playground::ax_tree::{recorder::AxTreeRecorder, build_tree_snapshot};
use serde_json::Value;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};


use tokio::signal;

// Notification constants - these are the string values used by the AX API
const K_AX_APPLICATION_ACTIVATED_NOTIFICATION: &str = "AXApplicationActivated";
const K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION: &str = "AXFocusedWindowChanged";

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        println!("This example only runs on macOS.");
        return Ok(());
    }

    run_macos_snapshot().await?;

    Ok(())
}

async fn run_macos_snapshot() -> anyhow::Result<()> {
    use anyhow::Context;

    bootstrap(PlaygroundOptions::default())
        .await
        .context("failed to bootstrap playground context")?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let (ready_tx, ready_rx) = mpsc::channel();

    let worker_flag = Arc::clone(&stop_flag);
    let worker_task =
        tokio::task::spawn_blocking(move || run_accessibility_loop(worker_flag, ready_tx));

    ready_rx
        .recv()
        .context("failed to receive AX observer ready signal")?;

    let signal_flag = Arc::clone(&stop_flag);
    let ctrl_task = tokio::spawn(async move {
        if signal::ctrl_c().await.is_ok() {
            println!("Ctrl+C received, stopping accessibility observers...");
            signal_flag.store(true, Ordering::SeqCst);
        }
    });

    worker_task
        .await
        .context("accessibility loop task failed")??;

    ctrl_task.abort();

    Ok(())
}

fn run_accessibility_loop(
    stop_flag: Arc<AtomicBool>,
    ready_tx: mpsc::Sender<()>,
) -> anyhow::Result<()> {
    println!("Initializing accessibility engine (2 second delay to switch apps if needed)...");
    thread::sleep(Duration::from_secs(2));

    // Check if process is trusted using objc2-application-services
    let is_trusted = unsafe { AXIsProcessTrustedWithOptions(None) };
    if !is_trusted {
        println!("Accessibility access is required but not yet granted.");
        if let Ok(exe) = std::env::current_exe() {
            println!(
                "Open System Settings → Privacy & Security → Accessibility and enable access for:\n  {}",
                exe.display()
            );
            println!("Run `cargo run --example ax_snapshot_objc2` again after granting access.");
        } else {
            println!("Open System Settings → Privacy & Security → Accessibility and enable access for this binary, then rerun the example.");
        }
        return Ok(());
    }

    let engine = match create_engine(false, true) {
        Ok(engine) => engine,
        Err(AutomationError::PermissionDenied(msg)) => {
            println!("Accessibility access is required but not yet granted ({msg}).");
            if let Ok(exe) = std::env::current_exe() {
                println!(
                    "Open System Settings → Privacy & Security → Accessibility and enable access for:\n  {}",
                    exe.display()
                );
                println!("Run `cargo run --example ax_snapshot_objc2` again after granting access.");
            } else {
                println!("Open System Settings → Privacy & Security → Accessibility and enable access for this binary, then rerun the example.");
            }
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let engine: Arc<dyn AccessibilityEngine> = Arc::from(engine);

    OBSERVER_SHARED
        .set(ObserverShared::new(engine.clone()))
        .map_err(|_| anyhow::anyhow!("observer shared state already initialized"))?;

    let run_loop = CFRunLoop::current()
        .ok_or_else(|| anyhow::anyhow!("failed to get current run loop"))?;
    ready_tx
        .send(())
        .map_err(|_| anyhow::anyhow!("failed to notify main task that observers are ready"))?;

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
            .or_else(|| app.bundleIdentifier().as_ref().map(|id| ns_string_to_string(id)))
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
        println!("{message}");
    }

    println!(
        "Watching {} applications for AX notifications. Press Ctrl+C to exit.",
        handles.len()
    );

    while !stop_flag.load(Ordering::SeqCst) {
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, 0.25, false);
    }

    println!("Stopping accessibility observers...");
    drop(handles);

    Ok(())
}

unsafe fn register_accessibility_observer(
    pid: i32,
    run_loop: CFRetained<CFRunLoop>,
    handles: &mut Vec<ObserverHandle>,
    label: &str,
    skipped: &mut HashMap<i32, String>,
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

    let observer_ptr = NonNull::new(observer_ptr)
        .ok_or_else(|| anyhow::anyhow!("observer creation returned success but observer is null"))?;
    let observer = unsafe { observer_ptr.as_ref() };

    // Create AXUIElement for application using objc2-application-services
    let ax_app = AXUIElement::new_application(pid as libc::pid_t);

    // Create CFString instances for notifications - need to use objc2_core_foundation types
    let activation_cf = Objc2CFString::from_str(K_AX_APPLICATION_ACTIVATED_NOTIFICATION);
    let focus_cf = Objc2CFString::from_str(K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION);
    
    let refcon = Box::into_raw(Box::new(CallbackRefcon { pid })) as *mut c_void;

    let mut activation_registered = false;
    let mut focus_registered = false;

    // Add activation notification
    let add_activation = unsafe {
        observer.add_notification(
            &ax_app,
            &activation_cf,
            refcon,
        )
    };
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
    let add_focus = unsafe {
        observer.add_notification(
            &ax_app,
            &focus_cf,
            refcon,
        )
    };
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
                        label,
                        pid,
                        status
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
        observer: observer_ptr,
        source,
        refcon: refcon as *mut CallbackRefcon,
        run_loop,
    });

    Ok(())
}

unsafe extern "C-unwind" fn ax_observer_callback(
    _observer: NonNull<AXObserver>,
    _element: NonNull<AXUIElement>,
    notification: NonNull<Objc2CFString>,
    refcon: *mut c_void,
) {
    if refcon.is_null() {
        return;
    }

    let Some(shared) = OBSERVER_SHARED.get() else {
        return;
    };

    let pid = (*refcon.cast::<CallbackRefcon>()).pid;
    
    // Convert CFString to Rust String using objc2_core_foundation
    let cf_string = notification.as_ref();
    let notification_str = cf_string.to_string();
    
    shared.handle_notification(pid, &notification_str);
}

fn snapshot_by_pid(
    engine: &dyn AccessibilityEngine,
    pid: i32,
    notification: &str,
    recorder: &mut AxTreeRecorder,
) -> anyhow::Result<()> {
    use anyhow::Context;
    use chrono::Local;

    let running_app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid);

    let app_label = running_app
        .as_ref()
        .and_then(|app| app.localizedName().as_ref().map(|name| ns_string_to_string(name)))
        .or_else(|| {
            running_app
                .as_ref()
                .and_then(|app| app.bundleIdentifier().as_ref().map(|id| ns_string_to_string(id)))
        })
        .unwrap_or_else(|| format!("pid {}", pid));

    println!(
        "[AX debug] {} for '{app_label}' (pid {pid})",
        match notification {
            n if n == K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION => "Focused window changed",
            n if n == K_AX_APPLICATION_ACTIVATED_NOTIFICATION => "Application activated",
            other => other,
        }
    );

    // Get root element instead of just focused window
    let root_element = engine.get_root_element();

    // Build tree snapshot from root
    let snapshot = build_tree_snapshot(&root_element, pid, notification)
        .context("failed to build tree snapshot")?;

    // Capture snapshot and get diff
    let diff = recorder.capture(snapshot);

    let timestamp = Local::now();
    println!(
        "--- AX Tree Snapshot ({}) @ {timestamp} ---",
        match notification {
            n if n == K_AX_FOCUSED_WINDOW_CHANGED_NOTIFICATION => "focus change",
            n if n == K_AX_APPLICATION_ACTIVATED_NOTIFICATION => "activation",
            other => other,
        }
    );
    println!("Application : {app_label}");
    println!("Root nodes  : {}", recorder.current().map(|s| s.nodes.len()).unwrap_or(0));
    
    if let Some(ref current) = recorder.current() {
        if let Some(focused) = current.focused_node() {
            println!("Focused node: {} ({})", 
                focused.label.as_ref().unwrap_or(&focused.role),
                focused.role
            );
        }
    }

    // Display diff if available
    if let Some(diff) = diff {
        if diff.has_changes() {
            println!("Changes     : {}", diff.summary());
        } else {
            println!("Changes     : none");
        }
    } else {
        println!("Changes     : initial snapshot");
    }

    Ok(())
}

fn ns_string_to_string(ns_string: &NSString) -> String {
    ns_string.to_string()
}

fn find_focused_window(
    applications: Vec<UIElement>,
) -> Result<(UIElement, String), AutomationError> {
    for app in applications {
        let app_attrs = app.attributes();
        let app_label = app_attrs
            .label
            .clone()
            .unwrap_or_else(|| app_attrs.role.clone());

        println!(
            "[AX debug] Application: '{}' role='{}'",
            app_label, app_attrs.role
        );

        let Ok(children) = app.children() else {
            println!("  [AX debug] Failed to fetch children for '{}'", app_label);
            continue;
        };

        println!(
            "  [AX debug] '{}' returned {} children",
            app_label,
            children.len()
        );

        for window in children {
            let attrs = window.attributes();

            println!(
                "    [AX debug] child role='{}' label='{}'",
                attrs.role,
                attrs.label.clone().unwrap_or_default()
            );

            if !attrs.properties.is_empty() {
                println!("      [AX debug] properties:");
                for (key, value) in &attrs.properties {
                    println!("        {} => {:?}", key, value);
                }
            } else {
                println!("      [AX debug] no AX properties recorded");
            }

            if attrs.role != "AXWindow" && attrs.role != "window" {
                continue;
            }

            if property_truthy(&attrs, "AXFocused") || property_truthy(&attrs, "AXMain") {
                println!("    [AX debug] '{}' flagged as focused/main", app_label);
                return Ok((window, app_label));
            }
        }
    }

    Err(AutomationError::ElementNotFound(
        "Could not find a focused window via AX".to_string(),
    ))
}

fn property_truthy(attrs: &cubby_core::operator::UIElementAttributes, key: &str) -> bool {
    let Some(value) = attrs
        .properties
        .get(key)
        .and_then(|opt| opt.as_ref())
    else {
        return false;
    };

    match value {
        Value::Bool(b) => *b,
        Value::Number(num) => num.as_f64().map(|n| n != 0.0).unwrap_or(false),
        Value::String(s) => {
            let trimmed = s.trim_matches(|c: char| c == '"' || c.is_whitespace());
            let lowered = trimmed.to_ascii_lowercase();
            lowered == "true"
                || lowered == "yes"
                || lowered == "1"
                || lowered.contains("value = true")
        }
        _ => false,
    }
}

struct ObserverHandle {
    observer: NonNull<AXObserver>,
    source: CFRetained<CFRunLoopSource>,
    refcon: *mut CallbackRefcon,
    run_loop: CFRetained<CFRunLoop>,
}

impl Drop for ObserverHandle {
    fn drop(&mut self) {
        self.run_loop.remove_source(Some(&self.source), unsafe { kCFRunLoopDefaultMode });
        // observer and source are automatically dropped via CFRetained
        unsafe {
            drop(Box::from_raw(self.refcon));
        }
    }
}

#[derive(Debug)]
struct CallbackRefcon {
    pid: i32,
}

struct ObserverShared {
    engine: Arc<dyn AccessibilityEngine>,
    debounce: Mutex<HashMap<i32, Instant>>,
    ignored: Mutex<HashMap<i32, Instant>>,
    recorder: Mutex<AxTreeRecorder>,
}

impl ObserverShared {
    fn new(engine: Arc<dyn AccessibilityEngine>) -> Self {
        Self {
            engine,
            debounce: Mutex::new(HashMap::new()),
            ignored: Mutex::new(HashMap::new()),
            recorder: Mutex::new(AxTreeRecorder::new(100)),
        }
    }

    fn handle_notification(&self, pid: i32, notification: &str) {
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
        
        if let Err(err) = snapshot_by_pid(self.engine.as_ref(), pid, notification, &mut *recorder) {
            println!("Snapshot error for pid {pid}: {err:#}");
            let mut ignored = match self.ignored.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            ignored.insert(pid, Instant::now());
        }
    }
}

static OBSERVER_SHARED: OnceLock<ObserverShared> = OnceLock::new();

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
        .or_else(|| app.bundleIdentifier().as_ref().map(|id| ns_string_to_string(id)));

    if let Some(name) = name {
        let lower = name.to_ascii_lowercase();
        if EXCLUDE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
            || lower.contains("firefoxcp")
        {
            return false;
        }
    }

    true
}
