#[cfg(target_os = "macos")]
mod mac {
    use anyhow::{anyhow, Context, Result};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    #[cfg(target_arch = "aarch64")]
    const NOTIFY_HELPER_BYTES: &[u8] = include_bytes!("../sidecars/macos-aarch64/notify-helper");
    #[cfg(target_arch = "x86_64")]
    const NOTIFY_HELPER_BYTES: &[u8] = include_bytes!("../sidecars/macos-x86_64/notify-helper");

    fn helper_install_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("find home directory")?;
        Ok(home
            .join("Applications")
            .join("Cubby")
            .join("notify-helper"))
    }

    fn ensure_helper_installed() -> Result<PathBuf> {
        let helper_path = helper_install_path()?;

        if let Some(parent) = helper_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create helper directory at {:?}", parent))?;
        }

        let tmp_path = helper_path.with_extension("tmp");
        fs::write(&tmp_path, NOTIFY_HELPER_BYTES)
            .with_context(|| format!("write helper binary to {:?}", tmp_path))?;

        let metadata = fs::metadata(&tmp_path)?;
        let mut perms = metadata.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp_path, perms)?;

        fs::rename(&tmp_path, &helper_path)
            .with_context(|| format!("replace helper at {:?}", helper_path))?;

        Ok(helper_path)
    }

    pub fn send(title: &str, body: &str) -> Result<()> {
        let helper = ensure_helper_installed()?;

        let status = Command::new(helper)
            .arg("--title")
            .arg(title)
            .arg("--body")
            .arg(body)
            .status()
            .context("launch notify-helper sidecar")?;

        if status.success() {
            Ok(())
        } else {
            Err(anyhow!("notify-helper exited with status {:?}", status))
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::send;

#[cfg(not(target_os = "macos"))]
pub fn send(_title: &str, _body: &str) -> anyhow::Result<()> {
    Ok(())
}
