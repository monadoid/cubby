use service_manager::*;
use std::{ffi::OsString, path::PathBuf};

pub const SERVICE_LABEL_SCREENPIPE: &str = "com.example.cubby.screenpipe";

// Re-export types for use in main.rs
pub use service_manager::{ServiceLabel, ServiceLevel, ServiceStatus};

pub struct Service {
    pub(crate) label: ServiceLabel,
    pub(crate) level: ServiceLevel,
}

impl Service {
    pub fn new(label: ServiceLabel, level: ServiceLevel) -> Self {
        Self { label, level }
    }

    pub(crate) fn manager(&self) -> Result<Box<dyn ServiceManager>, Box<dyn std::error::Error>> {
        let mut m = <dyn ServiceManager>::native()?;
        m.set_level(self.level)?;
        Ok(m)
    }

    pub fn install_and_start(
        &self,
        program: PathBuf,
        args: Vec<OsString>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        m.install(ServiceInstallCtx {
            label: self.label.clone(),
            program,
            args,
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
            autostart: true,
            disable_restart_on_failure: false,
        })?;
        m.start(ServiceStartCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn uninstall(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx {
            label: self.label.clone(),
        });
        m.uninstall(ServiceUninstallCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx {
            label: self.label.clone(),
        });
        m.start(ServiceStartCtx {
            label: self.label.clone(),
        })?;
        Ok(())
    }

    pub fn status(&self) -> Result<ServiceStatus, Box<dyn std::error::Error>> {
        let m = self.manager()?;
        m.status(ServiceStatusCtx {
            label: self.label.clone(),
        })
        .map_err(Into::into)
    }
}

pub fn install_both(
    screenpipe: PathBuf,
    cloudflared: PathBuf,
    tunnel_token: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    // Install and start screenpipe (listening on 127.0.0.1:3030)
    svc_screen.install_and_start(screenpipe, vec![])?;

    // Install cloudflared service using its native service install command
    let output = std::process::Command::new(cloudflared)
        .args(&["service", "install", tunnel_token])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to install cloudflared service: {}", stderr).into());
    }

    Ok(())
}

pub struct DualServiceStatus {
    pub screenpipe_running: bool,
    pub cloudflared_running: bool,
}

impl DualServiceStatus {
    pub fn both_running(&self) -> bool {
        self.screenpipe_running && self.cloudflared_running
    }
}

pub fn status_both(cloudflared_path: Option<PathBuf>) -> DualServiceStatus {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse().unwrap();
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    let screenpipe_running = matches!(
        svc_screen.status(),
        Ok(service_manager::ServiceStatus::Running)
    );

    // Check cloudflared service status using its native command
    let cloudflared_running = if let Some(cloudflared) = cloudflared_path {
        std::process::Command::new(cloudflared)
            .args(&["service", "status"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    } else {
        // Try to find cloudflared in PATH
        std::process::Command::new("cloudflared")
            .args(&["service", "status"])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    };

    DualServiceStatus {
        screenpipe_running,
        cloudflared_running,
    }
}

pub fn restart_both(cloudflared_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    // Restart screenpipe service
    let _ = svc_screen.restart();

    // Restart cloudflared service using its native command
    if let Some(cloudflared) = cloudflared_path {
        let _ = std::process::Command::new(cloudflared)
            .args(&["service", "restart"])
            .output();
    } else {
        // Try to find cloudflared in PATH
        let _ = std::process::Command::new("cloudflared")
            .args(&["service", "restart"])
            .output();
    }

    Ok(())
}

pub fn uninstall_both(cloudflared_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse()?;
    let svc_screen = Service::new(label_screen, ServiceLevel::User);

    // Uninstall screenpipe service (ignore errors if service doesn't exist)
    let _ = svc_screen.uninstall();

    // Uninstall cloudflared service using its native command
    if let Some(cloudflared) = cloudflared_path {
        let _ = std::process::Command::new(cloudflared)
            .args(&["service", "uninstall"])
            .output();
    } else {
        // Try to find cloudflared in PATH
        let _ = std::process::Command::new("cloudflared")
            .args(&["service", "uninstall"])
            .output();
    }

    Ok(())
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use serial_test::serial;
    use std::process::Command;
    use std::time::Duration;

    const SERVER_URL: &str = "http://127.0.0.1:3030";

    fn get_services() -> Service {
        let label_screen: ServiceLabel = SERVICE_LABEL_SCREENPIPE.parse().unwrap();
        Service::new(label_screen, ServiceLevel::User)
    }

    #[tokio::test]
    async fn test_cli_help() {
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains(
                "Give your computer MCP superpowers",
            ));
    }

    #[tokio::test]
    async fn test_cli_version() {
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("--version")
            .assert()
            .success()
            .stdout(predicate::str::contains("cubby"));
    }

    #[tokio::test]
    #[serial]
    async fn test_service_lifecycle() {
        // Ensure service is not running initially
        cleanup_service().await;

        // Test start command
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("start")
            .assert()
            .success()
            .stdout(predicate::str::contains("Service started successfully"));

        // Test status command - should show running
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("status")
            .assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Running"));

        // Test server is actually responding on port 3030
        tokio::time::timeout(Duration::from_secs(30), test_server_health())
            .await
            .expect("Server health check timed out")
            .expect("Server should be healthy");

        // Test restart command
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("restart");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Service restarted successfully"));

        // Verify service is still running after restart
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("status");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Running"));

        // Test uninstall command
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("uninstall");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Service uninstalled successfully"));

        // Test status command - should show not running
        let mut cmd = Command::cargo_bin("cubby").unwrap();
        cmd.arg("status");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Overall status: Not running"));
    }

    #[tokio::test]
    #[serial]
    async fn test_direct_service_start_and_health() {
        cleanup_service().await;

        // Test starting services directly via start command
        let mut start_cmd = Command::cargo_bin("cubby").unwrap();
        start_cmd.arg("start");
        let _start_result = start_cmd.assert().success();

        // Wait a bit for services to fully start
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Test server health
        match test_server_health().await {
            Ok(_) => println!("Server health check passed"),
            Err(e) => {
                cleanup_service().await;
                panic!("Server health check failed: {}", e);
            }
        }

        // Clean up
        cleanup_service().await;
    }

    #[tokio::test]
    #[serial]
    async fn test_service_installation_edge_cases() {
        cleanup_service().await;

        // Test starting when already installed
        let mut cmd1 = Command::cargo_bin("cubby").unwrap();
        cmd1.arg("start");
        cmd1.assert().success();

        // Starting again should handle gracefully
        let mut cmd2 = Command::cargo_bin("cubby").unwrap();
        cmd2.arg("start");
        // This might fail or succeed depending on implementation, but shouldn't crash
        cmd2.assert().code(predicate::in_iter([0, 1]));

        // Test uninstalling when not installed
        cleanup_service().await;
        let mut cmd3 = Command::cargo_bin("cubby").unwrap();
        cmd3.arg("uninstall");
        // Should handle gracefully
        cmd3.assert().code(predicate::in_iter([0, 1]));
    }

    async fn test_server_health() -> Result<(), Box<dyn std::error::Error>> {
        let client = reqwest::Client::new();

        // Try to connect to the MCP server
        let response = client
            .get(SERVER_URL)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => {
                println!("Server responded with status: {}", resp.status());
                Ok(())
            }
            Err(e) => {
                println!("Server health check failed: {}", e);
                Err(e.into())
            }
        }
    }

    async fn cleanup_service() {
        // Use our service manager to clean up both services
        let _ = uninstall_both(None);

        // Kill any processes using port 3030
        let _ = Command::new("bash")
            .args(&["-c", "lsof -ti:3030 | xargs kill -9 2>/dev/null || true"])
            .output();
    }
}
