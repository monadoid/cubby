mod cloudflared_handler;
mod cubby_api_client;
mod screenpipe_handler;
mod signals;
mod deps_manager;

use crate::cubby_api_client::{CubbyApiClient, SignUpRequest};
use crate::screenpipe_handler::ScreenpipeService;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use cliclack::{input, password};
use std::{thread, time::Duration};
use crate::cloudflared_handler::CloudflaredService;
use crate::deps_manager::{Dep, ToolManager};

/// A tiny wrapper around cloudflared service lifecycle.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install (or reinstall) and start the cloudflared service
    Start,
    /// Uninstall the cloudflared service (future: remove other services too)
    Uninstall,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Start => start_command(),
        Commands::Uninstall => uninstall_command(),
    }
}

fn start_command() -> Result<()> {
    let running = signals::install_signal_flag();

    let email: String = input("What's your email?")
        .validate(|value: &String| -> std::result::Result<(), &'static str> {
            if value.trim().is_empty() {
                Err("Email is required")
            } else {
                Ok(())
            }
        })
        .interact()
        .context("Failed to read email input")?;

    let password: String = password("Choose a password")
        .mask('▪')
        .validate(|value: &String| -> std::result::Result<(), &'static str> {
            if value.trim().is_empty() {
                Err("Password is required")
            } else {
                Ok(())
            }
        })
        .interact()
        .context("Failed to read password input")?;

    let client = CubbyApiClient::new("http://localhost:8787".to_string());
    let sign_up_response = client
        .sign_up(SignUpRequest {
            email: email.clone(),
            password,
        })
        .context("Failed to sign up user")?;

    let session_jwt = sign_up_response.session_jwt.trim().to_owned();
    if session_jwt.is_empty() {
        bail!("Sign-up response missing session JWT");
    }

    // TODO: Persist onboarding state (email, session token/JWT, issued device ids) with `confy`
    // so future runs can reuse existing sessions or trigger refresh flows instead of prompting
    // every time.

    let tm = ToolManager::pinned_defaults();
    let screenpipe_path = tm.ensure(Dep::Screenpipe)?;
    let cloudflared_path = tm.ensure(Dep::Cloudflared)?;

    let screenpipe = ScreenpipeService::new_with_binary(screenpipe_path)?;
    let cloudflared = CloudflaredService::new_with_binary(cloudflared_path)?;

    // Enroll device to get tunnel token
    let enroll_response = client
        .enroll_device(&session_jwt)
        .context("Failed to enroll device")?;

    println!("Device enrolled successfully!");
    println!("Device ID: {}", enroll_response.device_id);
    println!("Hostname: {}", enroll_response.hostname);

    cloudflared.run_install_flow(&enroll_response.tunnel_token)?;
    screenpipe.install()?;
    screenpipe.start_and_wait_healthy()?;

    println!("✅ Services are up. Press Ctrl-C to stop...");
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    println!("🛑 Signal received. Stopping services...");
    screenpipe.stop_and_uninstall()?;

    Ok(())
}

fn uninstall_command() -> Result<()> {
    let tm = ToolManager::pinned_defaults();
    let screenpipe_path = tm.ensure(Dep::Screenpipe)?;
    let cloudflared_path = tm.ensure(Dep::Cloudflared)?;

    let screenpipe = ScreenpipeService::new_with_binary(screenpipe_path)?;
    let cloudflared = CloudflaredService::new_with_binary(cloudflared_path)?;

    cloudflared.uninstall()?;
    screenpipe.stop_and_uninstall()?;

    Ok(())
}
