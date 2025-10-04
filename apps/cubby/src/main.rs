mod cloudflared_handler;
mod config;
mod cubby_api_client;
mod deps_manager;
mod screenpipe_handler;
mod signals;

use crate::cloudflared_handler::CloudflaredService;
use crate::config::Config;
use crate::cubby_api_client::{CubbyApiClient, SignUpRequest};
use crate::deps_manager::{Dep, ToolManager};
use crate::screenpipe_handler::ScreenpipeService;
use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use cliclack::{input, password};
use std::{thread, time::Duration};

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
    Start {
        /// Force redownload of binaries even if they exist
        #[arg(long)]
        force: bool,
    },
    /// Uninstall the cloudflared service (future: remove other services too)
    Uninstall,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Commands::Start { force } => start_command(force),
        Commands::Uninstall => uninstall_command(),
    }
}

fn start_command(force: bool) -> Result<()> {
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

    let config = Config::from_build_profile();
    let client = CubbyApiClient::new(config.api_base_url);
    #[cfg(debug_assertions)]
    println!("Created account for {email}...");

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

    let mut tm = ToolManager::pinned_defaults();
    tm.force = force;
    let (screenpipe_path, cloudflared_path) = tm.ensure_both_parallel()?;

    let screenpipe = ScreenpipeService::new_with_binary(screenpipe_path)?;
    let cloudflared = CloudflaredService::new_with_binary(cloudflared_path)?;

    // Enroll device to get tunnel token
    let enroll_response = client
        .enroll_device(&session_jwt)
        .context("Failed to enroll device")?;

    cliclack::log::info("Device enrolled successfully!")?;
    cliclack::log::info(format!("Hostname: {}", enroll_response.hostname))?;

    cloudflared.run_install_flow(&enroll_response.tunnel_token)?;
    screenpipe.install()?;
    screenpipe.start_and_wait_healthy()?;

    cliclack::log::info("✅ Services are up. Press Ctrl-C to stop...")?;
    while running.load(std::sync::atomic::Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(100));
    }
    cliclack::log::info("🛑 Signal received. Stopping services...")?;
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
