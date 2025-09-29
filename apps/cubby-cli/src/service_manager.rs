use service_manager::*;
use std::{ffi::OsString, path::PathBuf};

pub const SERVICE_LABEL: &str = "com.example.cubby";

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

    pub fn install_and_start(&self, program: PathBuf, args: Vec<OsString>) -> Result<(), Box<dyn std::error::Error>> {
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
        m.start(ServiceStartCtx { label: self.label.clone() })?;
        Ok(())
    }

    pub fn uninstall(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx { label: self.label.clone() });
        m.uninstall(ServiceUninstallCtx { label: self.label.clone() })?;
        Ok(())
    }

    pub fn restart(&self) -> Result<(), Box<dyn std::error::Error>> {
        let m = self.manager()?;
        let _ = m.stop(ServiceStopCtx { label: self.label.clone() });
        m.start(ServiceStartCtx { label: self.label.clone() })?;
        Ok(())
    }

    pub fn status(&self) -> Result<ServiceStatus, Box<dyn std::error::Error>> {
        let m = self.manager()?;
        m.status(ServiceStatusCtx { label: self.label.clone() }).map_err(Into::into)
    }
}



#[cfg(test)]
mod integration_tests {
    use super::*;
    use assert_cmd::prelude::*;
    use predicates::prelude::*;
    use serial_test::serial;
    use std::process::Command;
    use std::time::Duration;

    const SERVER_URL: &str = "http://127.0.0.1:8000/mcp";

    fn get_service() -> Service {
        let label: ServiceLabel = SERVICE_LABEL.parse().unwrap();
        Service::new(label, ServiceLevel::User)
    }

    #[tokio::test]
    async fn test_cli_help() {
        Command::cargo_bin("cubby")
            .unwrap()
            .arg("--help")
            .assert()
            .success()
            .stdout(predicate::str::contains("Give your computer MCP superpowers"));
    }

    #[tokio::test]
    async fn test_cli_version() {
        Command::cargo_bin("cubby").unwrap()
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
            .stdout(predicate::str::contains("Service is running"));

        // Test server is actually responding
        tokio::time::timeout(Duration::from_secs(10), test_server_health()).await
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
            .stdout(predicate::str::contains("Service is running"));

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
            .stdout(predicate::str::contains("Service is not running"));
    }

    #[tokio::test]
    #[serial]
    async fn test_daemon_mode_direct() {
        cleanup_service().await;

        // Start daemon in background
        let mut daemon_cmd = Command::cargo_bin("cubby").unwrap();
        daemon_cmd.arg("daemon");
        let mut daemon_process = daemon_cmd.spawn().expect("Failed to start daemon");

        // Wait for server to start using health check with retry
        let mut server_ready = false;
        for _ in 0..50 { // 5 seconds max (100ms * 50)
            if test_server_health().await.is_ok() {
                server_ready = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if !server_ready {
            daemon_process.kill().ok();
            panic!("Server failed to start within timeout");
        }

        // Test server health
        match test_server_health().await {
            Ok(_) => println!("Server health check passed"),
            Err(e) => {
                daemon_process.kill().ok();
                panic!("Server health check failed: {}", e);
            }
        }

        // Clean up
        daemon_process.kill().expect("Failed to kill daemon process");
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
        let service = get_service();
        
        // Use our service manager to clean up
        let _ = service.uninstall();

        // Kill any processes using port 8000
        let _ = Command::new("bash")
            .args(&["-c", "lsof -ti:8000 | xargs kill -9 2>/dev/null || true"])
            .output();
    }
}
