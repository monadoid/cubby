use anyhow::{anyhow, Context, Result};
use directories::ProjectDirs;
use rust_embed::RustEmbed;
use std::path::PathBuf;
use std::{fs, io::Write};

#[derive(RustEmbed)]
#[folder = "embedded_assets/"]
struct Assets;

#[derive(Clone, Copy, Debug)]
pub enum EmbeddedBin {
    Cloudflared,
    Screenpipe,
}

impl EmbeddedBin {
    fn asset_name(self) -> &'static str {
        match self {
            EmbeddedBin::Cloudflared => "cloudflared",
            EmbeddedBin::Screenpipe => "screenpipe",
        }
    }

    // Final filename on disk (adds platform suffix + optional .exe + hash)
    fn target_filename(self) -> String {
        #[cfg(target_os = "windows")]
        let ext = ".exe";
        #[cfg(not(target_os = "windows"))]
        let ext = "";

        #[cfg(target_os = "linux")]
        let plat = "linux";
        #[cfg(target_os = "macos")]
        let plat = "macos";
        #[cfg(target_os = "windows")]
        let plat = "windows";

        format!("{}-{}-{}", self.asset_name(), plat, ext)
    }
}

pub fn ensure_embedded_bin(bin: EmbeddedBin) -> Result<PathBuf> {
    let bytes = Assets::get(bin.asset_name()).unwrap();

    let proj = ProjectDirs::from("com", "tabsandtabs", "cubby").unwrap();
    let bindir = proj.data_dir().join("bin");
    fs::create_dir_all(&bindir)?;

    let target = bindir.join(bin.target_filename());
    if target.exists() {
        return Err(anyhow!("{} already exists", target.display()));
    }
    // Write atomically
    let tmp = target.with_extension("tmp");
    fs::write(&tmp, &bytes.data)?;

    // chmod +x on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&tmp)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&tmp, perms)?;
    }
    // Atomic rename into place
    fs::rename(&tmp, &target)?;
    Ok(target)
}
