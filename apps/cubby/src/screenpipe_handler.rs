use anyhow::Result;
use reqwest::StatusCode;
use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceLevel, ServiceManager, ServiceStartCtx, ServiceStopCtx,
    ServiceUninstallCtx,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const SERVICE_LABEL_STR: &str = "com.example.screenpipe";
const HEALTH_URL: &str = "http://127.0.0.1:3030/health";

pub struct ScreenpipeService {
    label: ServiceLabel,
    binary_path: PathBuf,
    args: Vec<OsString>,
    autostart: bool,
    health_url: String,
    health_timeout: Duration,
}

impl ScreenpipeService {
    pub fn new_with_binary(binary_path: PathBuf) -> Result<Self> {
        Ok(Self {
            label: SERVICE_LABEL_STR.parse()?,
            binary_path,
            args: Vec::new(), // or inject if you need to parametrize
            autostart: true,
            health_url: HEALTH_URL.to_string(),
            health_timeout: Duration::from_secs(30),
        })
    }

    /// Install the service definition (LaunchAgent on macOS) without starting it.
    pub fn install(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        // todo: Add a check for absolute path
        manager.install(ServiceInstallCtx {
            label: self.label.clone(),
            program: self.binary_path.clone(),
            args: self.args.clone(),
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
            autostart: self.autostart,
            // When false, services restart on crash by default if the platform supports it.
            disable_restart_on_failure: false,
        })?;

        Ok(())
    }
    pub fn start_and_wait_healthy(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        // Start (idempotent-ish: if already running, platforms may return “already running”).
        let _ = manager.start(ServiceStartCtx {
            label: self.label.clone(),
        });

        self.wait_for_health()?;

        Ok(())
    }

    pub fn stop_and_uninstall(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        // Try to stop first to ensure the process is killed, even if uninstall would unload it.
        manager.stop(ServiceStopCtx {
            label: self.label.clone(),
        })?;

        // Then uninstall the definition.
        manager.uninstall(ServiceUninstallCtx {
            label: self.label.clone(),
        })?;

        Ok(())
    }

    pub fn wait_for_health(&self) -> Result<()> {
        let start = Instant::now();
        let mut delay = Duration::from_millis(200);

        loop {
            if start.elapsed() > self.health_timeout {
                anyhow::bail!("Timed out waiting for health at {}", self.health_url);
            }

            match reqwest::blocking::get(&self.health_url) {
                Ok(resp) if resp.status() == StatusCode::OK => {
                    return Ok(());
                }
                _ => {
                    thread::sleep(delay);
                    // simple linear-ish backoff capped at 1s
                    delay = std::cmp::min(
                        delay + Duration::from_millis(100),
                        Duration::from_millis(1000),
                    );
                }
            }
        }
    }
}
