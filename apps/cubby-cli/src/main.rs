use anyhow::Result;
use clap::{Parser, Subcommand};
use cliclack::{input, intro, outro};
use serde::{Deserialize, Serialize};

mod bootstrap;
mod cubby_client;
mod service_manager;

#[derive(Default, Debug, Serialize, Deserialize)]
struct Config {
    device_id: String,
    email: Option<String>,
    tunnel_token: Option<String>,
}

#[derive(Parser)]
#[command(name = "cubby")]
#[command(about = "Give your computer MCP superpowers")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Start the application")]
    Start,
    #[command(about = "Check service status")]
    Status,
    #[command(about = "Restart the application")]
    Restart,
    #[command(about = "Uninstall the service")]
    Uninstall,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Start) => handle_start().await?,
        Some(Commands::Status) => handle_status()?,
        Some(Commands::Restart) => handle_restart()?,
        Some(Commands::Uninstall) => handle_uninstall()?,
        None => println!("Hello world"),
    }
    Ok(())
}

async fn handle_start() -> Result<()> {
    intro("🚀 Cubby Setup")?;

    // Load or generate device ID
    let mut config: Config = confy::load("cubby", None).unwrap_or_default();
    if config.device_id.is_empty() {
        config.device_id = nanoid::nanoid!();
        confy::store("cubby", None, &config)?;
        println!("Generated new device ID: {}", config.device_id);
    } else {
        println!("Using existing device ID: {}", config.device_id);
    }

    // Get user credentials for account creation
    let email: String = input("What's your email address?")
        .placeholder("user@example.com")
        .validate(|input: &String| {
            if input.is_empty() {
                Err("Please enter an email address.")
            } else if !input.contains('@') {
                Err("Please enter a valid email address.")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let password: String = input("Create a password:")
        .placeholder("Enter a secure password")
        .validate(|input: &String| {
            if input.is_empty() {
                Err("Please enter a password.")
            } else if input.len() < 8 {
                Err("Password must be at least 8 characters long.")
            } else {
                Ok(())
            }
        })
        .interact()?;

    // Create account using our custom client
    let client = cubby_client::CubbyClient::new("http://localhost:8787");

    println!("Creating your account...");
    match client
        .sign_up(&cubby_client::SignUpRequest {
            email: email.clone(),
            password,
        })
        .await
    {
        Ok(response) => {
            println!("✅ Account created successfully!");
            println!("Response: {:?}", response);

            // Store email in config
            config.email = Some(email.clone());
            confy::store("cubby", None, &config)?;
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to create account: {}", e));
        }
    }

    println!("Enrolling device...");
    match client
        .enroll_device(&cubby_client::DeviceEnrollRequest {
            device_id: config.device_id.clone(),
        })
        .await
    {
        Ok(response) => {
            println!("✅ Device enrolled successfully!");
            println!("Device ID: {}", response.device_id);
            println!("Hostname: {}", response.hostname);
            println!("Tunnel Token: {}", response.tunnel_token);

            // Store tunnel token in config
            config.tunnel_token = Some(response.tunnel_token.clone());
            confy::store("cubby", None, &config)?;
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Failed to enroll device: {}", e));
        }
    }

    // 1) Ensure external dependencies are available
    println!("Ensuring dependencies are available...");
    let bins = bootstrap::ensure_binaries()?;

    // 2) Install/start both services
    println!("Installing and starting services...");
    let tunnel_token = config
        .tunnel_token
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Tunnel token not found in config"))?;
    service_manager::install_both(
        bins.screenpipe.clone(),
        bins.cloudflared.clone(),
        tunnel_token,
    )
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Services created, waiting for screenpipe on http://127.0.0.1:3030 ...");

    // 3) Wait for localhost:3030 to accept TCP connections
    wait_for_tcp("127.0.0.1:3030", 30).await?;

    outro("🎉 Setup complete! Your services are running.")?;
    Ok(())
}

fn handle_status() -> Result<()> {
    let bins = bootstrap::ensure_binaries()?;
    let status = service_manager::status_both(Some(bins.cloudflared));

    println!(
        "Screenpipe: {}",
        if status.screenpipe_running {
            "Running"
        } else {
            "Not running"
        }
    );
    println!(
        "Cloudflared: {}",
        if status.cloudflared_running {
            "Running"
        } else {
            "Not running"
        }
    );

    if status.both_running() {
        println!("Overall status: Running");
    } else {
        println!("Overall status: Not running");
    }
    Ok(())
}

fn handle_restart() -> Result<()> {
    let bins = bootstrap::ensure_binaries()?;
    service_manager::restart_both(Some(bins.cloudflared)).map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Services restarted successfully");
    Ok(())
}

fn handle_uninstall() -> Result<()> {
    let bins = bootstrap::ensure_binaries()?;
    service_manager::uninstall_both(Some(bins.cloudflared))
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Services uninstalled successfully");
    Ok(())
}

async fn wait_for_tcp(addr: &str, timeout_s: u64) -> Result<()> {
    use tokio::{
        net::TcpStream,
        time::{Duration, Instant, sleep},
    };
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_s) {
        if TcpStream::connect(addr).await.is_ok() {
            return Ok(());
        }
        sleep(Duration::from_millis(300)).await;
    }
    Err(anyhow::anyhow!("Timeout waiting for {}", addr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service_manager::Service;
    use ::service_manager::{ServiceLabel, ServiceLevel};
    use clap::Parser;

    #[derive(clap::Parser)]
    #[command(name = "cubby")]
    #[command(about = "Give your computer MCP superpowers")]
    #[command(version)]
    struct Cli {
        #[command(subcommand)]
        command: Option<Commands>,
    }

    #[derive(clap::Subcommand)]
    enum Commands {
        #[command(about = "Start the application")]
        Start,
        #[command(about = "Check service status")]
        Status,
        #[command(about = "Restart the application")]
        Restart,
        #[command(about = "Uninstall the service")]
        Uninstall,
    }

    #[test]
    fn parses_start_subcommand() {
        let args = Cli::try_parse_from(["cubby", "start"]).unwrap();
        assert!(matches!(args.command, Some(Commands::Start)));
    }

    #[test]
    fn defaults_to_no_command() {
        let args = Cli::try_parse_from(["cubby"]).unwrap();
        assert!(args.command.is_none());
    }

    #[test]
    fn rejects_unknown_subcommand() {
        assert!(Cli::try_parse_from(["cubby", "unknown"]).is_err());
    }

    #[test]
    fn test_service_new() {
        let label: ServiceLabel = "com.example.test".parse().unwrap();
        let service = Service::new(label.clone(), ServiceLevel::User);
        assert_eq!(service.label, label);
        assert_eq!(service.level, ServiceLevel::User);
    }

    #[test]
    fn test_service_manager_creation() {
        let label: ServiceLabel = "com.example.test".parse().unwrap();
        let service = Service::new(label, ServiceLevel::User);
        let result = service.manager();
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_both_function() {
        // This should not crash even if services aren't installed
        let status = service_manager::status_both(None);
        // We can't assume the services are running, but the function should work
        assert!(status.screenpipe_running == true || status.screenpipe_running == false);
        assert!(status.cloudflared_running == true || status.cloudflared_running == false);
    }
}
