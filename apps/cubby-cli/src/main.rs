use clap::{Parser, Subcommand};
use anyhow::Result;
use rmcp::transport::streamable_http_server::{
    StreamableHttpService, session::local::LocalSessionManager,
};
use tracing_subscriber::{
    layer::SubscriberExt,
    util::SubscriberInitExt,
    {self},
};
use std::{env, ffi::OsString};
use tokio::time::{sleep, Duration};

mod counter;
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
    #[command(about = "Run as daemon")]
    Daemon,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(Commands::Start) => handle_start().await?,
        Some(Commands::Status) => handle_status()?,
        Some(Commands::Restart) => handle_restart()?,
        Some(Commands::Uninstall) => handle_uninstall()?,
        Some(Commands::Daemon) => handle_daemon().await?,
        None => println!("Hello world"),
    }
    Ok(())
}

async fn handle_start() -> Result<()> {
    let label: service_manager::ServiceLabel = service_manager::SERVICE_LABEL.parse().unwrap();
    let service = service_manager::Service::new(label, service_manager::ServiceLevel::User);
    
    let current_exe = env::current_exe()?;
    let args = vec![OsString::from("daemon")];
    
    service.install_and_start(current_exe, args)
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    
    println!("Service created, waiting for service to start...");
    
    // Wait for service to be running with 30 second timeout
    wait_for_service_status(&service, service_manager::ServiceStatus::Running, 30).await?;
    
    println!("Service started successfully");
    Ok(())
}

fn handle_status() -> Result<()> {
    let label: service_manager::ServiceLabel = service_manager::SERVICE_LABEL.parse().unwrap();
    let service = service_manager::Service::new(label, service_manager::ServiceLevel::User);
    
    match service.status() {
        Ok(service_manager::ServiceStatus::Running) => println!("Service is running"),
        Ok(service_manager::ServiceStatus::Stopped(_)) => println!("Service is not running"),
        Ok(status) => println!("Service status: {:?}", status),
        Err(e) => println!("Service is not running ({})", e),
    }
    Ok(())
}

fn handle_restart() -> Result<()> {
    let label: service_manager::ServiceLabel = service_manager::SERVICE_LABEL.parse().unwrap();
    let service = service_manager::Service::new(label, service_manager::ServiceLevel::User);
    
    service.restart().map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Service restarted successfully");
    Ok(())
}

fn handle_uninstall() -> Result<()> {
    let label: service_manager::ServiceLabel = service_manager::SERVICE_LABEL.parse().unwrap();
    let service = service_manager::Service::new(label, service_manager::ServiceLevel::User);
    
    service.uninstall().map_err(|e| anyhow::anyhow!("{}", e))?;
    println!("Service uninstalled successfully");
    Ok(())
}

async fn wait_for_service_status(
    service: &service_manager::Service,
    expected_status: service_manager::ServiceStatus,
    timeout_seconds: u64,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let timeout_duration = Duration::from_secs(timeout_seconds);
    
    loop {
        if start_time.elapsed() > timeout_duration {
            return Err(anyhow::anyhow!("Timeout waiting for service status"));
        }
        
        match service.status() {
            Ok(status) if status == expected_status => return Ok(()),
            _ => sleep(Duration::from_millis(500)).await,
        }
    }
}

async fn handle_daemon() -> Result<()> {
    const BIND_ADDRESS: &str = "127.0.0.1:8000";

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting MCP HTTP server on {}", BIND_ADDRESS);

    let service = StreamableHttpService::new(
        || Ok(counter::Counter::new()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(BIND_ADDRESS).await?;
    let _ = axum::serve(tcp_listener, router)
        .with_graceful_shutdown(async { tokio::signal::ctrl_c().await.unwrap() })
        .await;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use service_manager::{ServiceLabel, ServiceLevel};
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
        #[command(about = "Run as daemon")]
        Daemon,
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
}

