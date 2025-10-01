mod cloudflared_handler;
mod cubby_api_client;
mod embedded_assets;
mod screenpipe_handler;
mod signals;

use crate::cloudflared_handler::CloudflaredService;
use crate::cubby_api_client::{CubbyApiClient, DeviceEnrollRequest};
use crate::screenpipe_handler::ScreenpipeService;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::thread;
use std::time::Duration;

const HOSTNAME_ALPHABET: [char; 37] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '-',
];

// = nanoid!(21, &HOSTNAME_ALPHABET);
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
    let cli = Cli::parse();
    let cloudflared_service = CloudflaredService::new();
    let screenpipe_service = ScreenpipeService::new()?;

    match cli.command {
        Commands::Start => {
            let running = signals::install_signal_flag();

            // Generate random device ID using nanoid
            let device_id = nanoid::nanoid!(21, &HOSTNAME_ALPHABET);
            println!("Generated device ID: {}", device_id);

            // Enroll device to get tunnel token
            let client = CubbyApiClient::new("http://localhost:8787".to_string());
            let enroll_request = DeviceEnrollRequest { device_id };
            let enroll_response = client
                .enroll_device(enroll_request)
                .context("Failed to enroll device")?;

            println!("Device enrolled successfully!");
            println!("Hostname: {}", enroll_response.hostname);

            cloudflared_service.run_install_flow(&enroll_response.tunnel_token)?;
            screenpipe_service.install()?;
            screenpipe_service.start_and_wait_healthy()?;

            println!("✅ Services are up. Press Ctrl-C to stop...");
            while running.load(std::sync::atomic::Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
            }
            println!("🛑 Signal received. Stopping services...");
            screenpipe_service.stop_and_uninstall()?;
        }
        Commands::Uninstall => {
            cloudflared_service.uninstall()?;
        }
    }

    Ok(())
}
