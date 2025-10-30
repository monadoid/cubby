use cubby_playground::{bootstrap, PlaygroundOptions};

#[cfg(target_os = "macos")]
use cubby_core::operator::platforms::create_engine;
#[cfg(target_os = "macos")]
use cubby_core::operator::{AutomationError, UIElement};
#[cfg(target_os = "macos")]
use serde_json::Value;
#[cfg(target_os = "macos")]
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    #[cfg(not(target_os = "macos"))]
    {
        println!("This example only runs on macOS.");
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        run_macos_snapshot().await?;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
async fn run_macos_snapshot() -> anyhow::Result<()> {
    use anyhow::Context;
    use chrono::Local;
    use tokio::time::sleep;

    bootstrap(PlaygroundOptions::default())
        .await
        .context("failed to bootstrap playground context")?;

    println!("Will snapshot the focused window in 3 seconds...");
    sleep(std::time::Duration::from_secs(3)).await;

    let engine = match create_engine(false, true) {
        Ok(engine) => engine,
        Err(AutomationError::PermissionDenied(msg)) => {
            println!("Accessibility access is required but not yet granted ({msg}).");
            if let Ok(exe) = env::current_exe() {
                println!(
                    "Open System Settings → Privacy & Security → Accessibility and enable access for:\n  {}",
                    exe.display()
                );
                println!("Run `cargo run --example ax_snapshot` again after granting access.");
            } else {
                println!("Open System Settings → Privacy & Security → Accessibility and enable access for this binary, then rerun the example.");
            }
            return Ok(());
        }
        Err(err) => return Err(err).context("failed to create accessibility engine"),
    };

    let applications = engine
        .get_applications()
        .context("failed to enumerate applications")?;
    println!(
        "[AX debug] Accessibility permissions verified ({} application elements visible)",
        applications.len()
    );

    let (focused_window, app_label) = find_focused_window(applications)
        .context("no focused window found via accessibility API")?;

    let window_attrs = focused_window.attributes();
    let window_label = window_attrs
        .label
        .clone()
        .unwrap_or_else(|| window_attrs.role.clone());

    let bounds = focused_window.bounds().ok();
    let snapshot_text = focused_window
        .text(8)
        .unwrap_or_else(|_| String::from("<failed to collect text>"));

    let timestamp = Local::now();
    println!("--- AX Snapshot @ {timestamp} ---");
    println!("Application : {app_label}");
    println!("Window      : {window_label}");
    if let Some((x, y, w, h)) = bounds {
        println!("Bounds      : x={x:.1}, y={y:.1}, width={w:.1}, height={h:.1}");
    } else {
        println!("Bounds      : <unavailable>");
    }
    println!("Content:");
    println!("{}", snapshot_text.trim());

    Ok(())
}

#[cfg(target_os = "macos")]
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

            if attrs.role != "window" {
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

#[cfg(target_os = "macos")]
fn property_truthy(attrs: &cubby_core::operator::UIElementAttributes, key: &str) -> bool {
    attrs
        .properties
        .get(key)
        .and_then(|opt| opt.as_ref())
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
