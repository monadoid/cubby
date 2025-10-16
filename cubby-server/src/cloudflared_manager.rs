use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Manages cloudflared system service lifecycle
pub struct CloudflaredManager {
    binary_path: PathBuf,
}

impl CloudflaredManager {
    pub fn new(binary_path: PathBuf) -> Result<Self> {
        Ok(Self { binary_path })
    }

    /// Check if cloudflared service exists by checking platform-specific paths
    #[cfg(target_os = "macos")]
    pub fn is_installed(&self) -> bool {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new("/Library/LaunchDaemons/com.cloudflare.cloudflared.plist").exists()
            || Path::new(&format!(
                "{}/Library/LaunchAgents/com.cloudflare.cloudflared.plist",
                home
            ))
            .exists()
    }

    #[cfg(target_os = "linux")]
    pub fn is_installed(&self) -> bool {
        let home = std::env::var("HOME").unwrap_or_default();
        Path::new("/etc/systemd/system/cloudflared.service").exists()
            || Path::new("/lib/systemd/system/cloudflared.service").exists()
            || Path::new(&format!(
                "{}/.config/systemd/user/cloudflared.service",
                home
            ))
            .exists()
    }

    #[cfg(target_os = "windows")]
    pub fn is_installed(&self) -> bool {
        // `sc query` returns success (0) if the service exists
        Command::new("sc")
            .args(["query", "Cloudflared"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Uninstall the cloudflared service
    pub fn uninstall(&self) -> Result<()> {
        let output = Command::new(&self.binary_path)
            .args(["service", "uninstall"])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("cloudflared service uninstall failed: {}", stderr);
        }

        tracing::info!("cloudflared service uninstalled successfully");
        Ok(())
    }

    /// Install cloudflared service with the provided tunnel token
    pub fn install(&self, token: &str) -> Result<()> {
        let output = Command::new(&self.binary_path)
            .args(["service", "install", token])
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("cloudflared service install failed: {}", stderr);
        }

        tracing::info!("cloudflared service installed successfully");
        Ok(())
    }

    /// Install cloudflared service, handling existing installations
    pub fn install_with_overwrite(&self, token: &str) -> Result<()> {
        if self.is_installed() {
            tracing::info!("cloudflared service already exists, uninstalling first");
            // If uninstall fails, continue anyway
            let _ = self.uninstall();
        }

        self.install(token)?;
        Ok(())
    }
}
