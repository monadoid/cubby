use anyhow::{bail, Result};
use cliclack::confirm;
use duct::cmd;
use std::path::{Path, PathBuf};

/// Manages install / uninstall flow for the `cloudflared` system service.
pub struct CloudflaredService {
    binary_path: PathBuf,
}

impl CloudflaredService {
    pub fn new_with_binary(binary_path: PathBuf) -> Result<Self> {
        Ok(Self { binary_path })
    }

    #[cfg(target_os = "macos")]
    fn does_cloudflared_service_exist(&self) -> bool {
        let home = std::env::var("HOME").unwrap();
        Path::new("/Library/LaunchDaemons/com.cloudflare.cloudflared.plist").exists()
            || Path::new(&format!(
                "{home}/Library/LaunchAgents/com.cloudflare.cloudflared.plist"
            ))
            .exists()
    }

    #[cfg(target_os = "linux")]
    fn does_cloudflared_service_exist(&self) -> bool {
        // Common locations for systemd unit files; adjust if you manage it differently.
        Path::new("/etc/systemd/system/cloudflared.service").exists()
            || Path::new("/lib/systemd/system/cloudflared.service").exists()
    }

    #[cfg(target_os = "windows")]
    fn does_cloudflared_service_exist(&self) -> bool {
        // `sc query` returns success (0) if the service exists.
        cmd!("sc", "query", "Cloudflared")
            .unchecked()
            .run()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Uninstall the existing `cloudflared` service.
    pub fn uninstall(&self) -> Result<()> {
        let bin = self.binary_path.clone();
        cmd(bin, ["service", "uninstall"])
            .stderr_to_stdout()
            .run()?;

        println!("cloudflared service uninstalled successfully.");
        Ok(())
    }

    pub fn install(&self, token: &str) -> Result<()> {
        let bin = self.binary_path.clone();
        cmd(bin, ["service", "install", token])
            .stderr_to_stdout()
            .run()?;
        println!("cloudflared service command completed successfully");
        Ok(())
    }

    pub fn run_install_flow(&self, token: &str) -> Result<()> {
        println!("Installing cloudflared service...");

        if self.does_cloudflared_service_exist() {
            println!("cloudflared service already exists.");

            let overwrite = confirm(
                "You already have a cloudflared service installed. Do you want to overwrite it?",
            )
            .initial_value(false)
            .interact()
            .unwrap_or(false);

            if !overwrite {
                println!(
                    "Exiting as user does not want to overwrite existing cloudflared service."
                );
                bail!("User chose not to overwrite existing cloudflared service.");
            }

            // If uninstall fails, abort early.
            self.uninstall()?;
        } else {
            println!("cloudflared service does not exist yet");
        }

        // If install fails, propagate the error.
        self.install(token)?;
        Ok(())
    }
}
