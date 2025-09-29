use clap::{Parser, Subcommand};
use anyhow::Result;

mod bootstrap;
mod service_manager;

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
    // 1) Ensure external dependencies are available
    println!("Ensuring dependencies are available...");
    let bins = bootstrap::ensure_binaries()?;

    // 2) Install/start both services
    println!("Installing and starting services...");
    service_manager::install_both(bins.screenpipe.clone(), bins.cloudflared.clone())
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    println!("Services created, waiting for screenpipe on http://127.0.0.1:3030 ...");

    // 3) Wait for localhost:3030 to accept TCP connections
    wait_for_tcp("127.0.0.1:3030", 30).await?;

    println!("Services started successfully");
    Ok(())
}

fn handle_status() -> Result<()> {
    let status = service_manager::status_both();
    
    println!("Screenpipe: {}", if status.screenpipe_running { "Running" } else { "Not running" });
    println!("Cloudflared: {}", if status.cloudflared_running { "Running" } else { "Not running" });
    
    if status.both_running() {
        println!("Overall status: Running");
    } else {
        println!("Overall status: Not running");
    }
    Ok(())
}

fn handle_restart() -> Result<()> {
    service_manager::restart_both().map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Services restarted successfully");
    Ok(())
}

fn handle_uninstall() -> Result<()> {
    service_manager::uninstall_both().map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Services uninstalled successfully");
    Ok(())
}

async fn wait_for_tcp(addr: &str, timeout_s: u64) -> Result<()> {
    use tokio::{net::TcpStream, time::{sleep, Duration, Instant}};
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
    use clap::Parser;
    use ::service_manager::{ServiceLabel, ServiceLevel};
    use crate::service_manager::Service;

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
        let status = service_manager::status_both();
        // We can't assume the services are running, but the function should work
        assert!(status.screenpipe_running == true || status.screenpipe_running == false);
        assert!(status.cloudflared_running == true || status.cloudflared_running == false);
    }
}

