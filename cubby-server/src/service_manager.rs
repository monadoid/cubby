use anyhow::Result;
use reqwest::StatusCode;
use service_manager::{
    ServiceInstallCtx, ServiceLabel, ServiceLevel, ServiceManager, ServiceStartCtx, ServiceStopCtx,
    ServiceUninstallCtx,
};
use std::ffi::OsString;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

const SERVICE_LABEL_STR: &str = "com.tabsandtabs.cubby";

#[derive(Clone)]
pub struct cubbyServiceManager {
    label: ServiceLabel,
    binary_path: PathBuf,
    args: Vec<OsString>,
    port: u16,
    health_timeout: Duration,
}

impl cubbyServiceManager {
    pub fn new(binary_path: PathBuf, args: Vec<String>, port: u16) -> Result<Self> {
        let os_args: Vec<OsString> = args.into_iter().map(OsString::from).collect();
        Ok(Self {
            label: SERVICE_LABEL_STR.parse()?,
            binary_path,
            args: os_args,
            port,
            health_timeout: Duration::from_secs(30),
        })
    }

    fn health_url(&self) -> String {
        format!("http://127.0.0.1:{}/health", self.port)
    }

    pub fn is_installed(&self) -> bool {
        let manager_result = <dyn ServiceManager>::native();
        if let Ok(mut manager) = manager_result {
            if manager.set_level(ServiceLevel::User).is_err() {
                return false;
            }
            // Try to query the service status - if it exists, we consider it installed
            // The service-manager crate doesn't have a direct "is_installed" method,
            // so we check by attempting to get the service status
            #[cfg(target_os = "macos")]
            {
                // On macOS, check if the plist file exists
                let home = std::env::var("HOME").unwrap_or_default();
                let plist_path = format!(
                    "{}/Library/LaunchAgents/{}.plist",
                    home, SERVICE_LABEL_STR
                );
                std::path::Path::new(&plist_path).exists()
            }
            #[cfg(target_os = "linux")]
            {
                // On Linux, check if the systemd service file exists
                let home = std::env::var("HOME").unwrap_or_default();
                let service_path = format!(
                    "{}/.config/systemd/user/{}.service",
                    home, SERVICE_LABEL_STR
                );
                std::path::Path::new(&service_path).exists()
            }
            #[cfg(target_os = "windows")]
            {
                // For Windows, we'd need to check the service registry
                // For now, return false and let install handle it
                false
            }
        } else {
            false
        }
    }

    pub fn install(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        manager.install(ServiceInstallCtx {
            label: self.label.clone(),
            program: self.binary_path.clone(),
            args: self.args.clone(),
            contents: None,
            username: None,
            working_directory: None,
            environment: None,
            autostart: true,
            disable_restart_on_failure: false,
        })?;

        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        // Start the service (if already running, this may return an error which we ignore)
        let _ = manager.start(ServiceStartCtx {
            label: self.label.clone(),
        });

        Ok(())
    }

    pub fn stop_and_uninstall(&self) -> Result<()> {
        let mut manager = <dyn ServiceManager>::native()?;
        manager.set_level(ServiceLevel::User)?;

        // Try to stop first to ensure the process is killed, even if uninstall would unload it.
        let _ = manager.stop(ServiceStopCtx {
            label: self.label.clone(),
        });

        // Wait a moment for the process to stop
        std::thread::sleep(Duration::from_millis(500));

        // Then uninstall the definition.
        manager.uninstall(ServiceUninstallCtx {
            label: self.label.clone(),
        })?;

        // Additional cleanup for macOS LaunchAgent
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let plist_path = format!(
                "{}/Library/LaunchAgents/{}.plist",
                home, SERVICE_LABEL_STR
            );
            let _ = std::fs::remove_file(&plist_path);
        }

        // Additional cleanup for Linux systemd
        #[cfg(target_os = "linux")]
        {
            let home = std::env::var("HOME").unwrap_or_default();
            let service_path = format!(
                "{}/.config/systemd/user/{}.service",
                home, SERVICE_LABEL_STR
            );
            let _ = std::fs::remove_file(&service_path);
        }

        Ok(())
    }
}

