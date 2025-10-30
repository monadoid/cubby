use anyhow::{anyhow, bail, Result};
use directories::BaseDirs;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const CLOUDFLARED_VERSION: &str = "2025.9.1";

#[derive(Clone, Copy, Debug)]
enum Os {
    Mac,
    Linux,
    Windows,
}

#[derive(Clone, Copy, Debug)]
enum Arch {
    X86_64,
    Aarch64,
}

#[derive(Clone, Copy, Debug)]
enum Archive {
    Plain,             // a single binary file
    Tgz(&'static str), // tar.gz; inner binary to extract
}

#[derive(Serialize)]
struct Meta {
    tag: String,
    source: String,
    downloaded_at_epoch_s: u64,
    sha256: Option<String>,
}

/// Download and cache cloudflared binary, returning the path to execute
pub fn ensure_cloudflared() -> Result<PathBuf> {
    let (os, arch) = detect_platform()?;
    let base_dirs = BaseDirs::new().ok_or_else(|| anyhow!("cannot resolve base directories"))?;
    let bindir = base_dirs.home_dir().join(".cubby").join("bin");
    fs::create_dir_all(&bindir)?;

    let (filename, archive) = get_cloudflared_asset(os, arch)?;
    let tag = CLOUDFLARED_VERSION;
    let url = format!(
        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
        tag, filename
    );

    // Versioned folder so multiple versions can coexist
    let folder = bindir.join(format!(
        "cloudflared-{}-{}-{}",
        tag,
        os_label(os),
        arch_label(arch)
    ));
    let bin_name = binary_filename(os);
    let target = folder.join(&bin_name);

    if target.exists() {
        return Ok(target);
    }

    // Download and extract
    fs::create_dir_all(&folder)?;
    let tmp = target.with_extension("tmp");
    let bytes = http_get_octets(&url)?;

    match archive {
        Archive::Plain => {
            fs::write(&tmp, &bytes)?;
        }
        Archive::Tgz(inner) => {
            extract_tgz_single(&bytes, &tmp, inner)?;
        }
    }

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

    // Write metadata file
    let meta = Meta {
        tag: tag.to_string(),
        source: url.clone(),
        downloaded_at_epoch_s: now_s(),
        sha256: sha256_file(&target).ok(),
    };
    fs::write(folder.join(".meta.json"), serde_json::to_vec_pretty(&meta)?)?;

    Ok(target)
}

/// Remove all downloaded cloudflared binaries
pub fn cleanup_cloudflared() -> Result<()> {
    let base_dirs = BaseDirs::new().ok_or_else(|| anyhow!("cannot resolve base directories"))?;
    let bindir = base_dirs.home_dir().join(".cubby").join("bin");

    if bindir.exists() {
        // Remove only cloudflared directories
        for entry in fs::read_dir(&bindir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("cloudflared-") {
                        fs::remove_dir_all(&path)?;
                    }
                }
            }
        }
    }

    Ok(())
}

fn detect_platform() -> Result<(Os, Arch)> {
    let os = match std::env::consts::OS {
        "macos" => Os::Mac,
        "linux" => Os::Linux,
        "windows" => Os::Windows,
        other => bail!("unsupported OS: {}", other),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => Arch::X86_64,
        "aarch64" => Arch::Aarch64,
        other => bail!("unsupported ARCH: {}", other),
    };
    Ok((os, arch))
}

fn get_cloudflared_asset(os: Os, arch: Arch) -> Result<(&'static str, Archive)> {
    match (os, arch) {
        (Os::Mac, Arch::Aarch64) => {
            Ok(("cloudflared-darwin-arm64.tgz", Archive::Tgz("cloudflared")))
        }
        (Os::Mac, Arch::X86_64) => {
            Ok(("cloudflared-darwin-amd64.tgz", Archive::Tgz("cloudflared")))
        }
        (Os::Linux, Arch::X86_64) => Ok(("cloudflared-linux-amd64", Archive::Plain)),
        (Os::Linux, Arch::Aarch64) => Ok(("cloudflared-linux-arm64", Archive::Plain)),
        (Os::Windows, Arch::X86_64) => Ok(("cloudflared-windows-amd64.exe", Archive::Plain)),
        _ => bail!("unsupported platform for cloudflared"),
    }
}

fn binary_filename(os: Os) -> String {
    match os {
        Os::Windows => "cloudflared.exe".to_string(),
        _ => "cloudflared".to_string(),
    }
}

fn os_label(o: Os) -> &'static str {
    match o {
        Os::Mac => "macos",
        Os::Linux => "linux",
        Os::Windows => "windows",
    }
}

fn arch_label(a: Arch) -> &'static str {
    match a {
        Arch::X86_64 => "x86_64",
        Arch::Aarch64 => "aarch64",
    }
}

fn http_get_octets(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cubby/0.1 (+https://cubby.sh)")
        .build()?;
    let mut req = client.get(url);
    if let Ok(tok) = std::env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {}", tok));
    }
    let mut resp = req.send()?.error_for_status()?;
    let total = resp.content_length().unwrap_or(0);

    let mut buf = Vec::with_capacity(total as usize);
    let mut chunk = [0u8; 8192];

    loop {
        let n = resp.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    Ok(buf)
}

fn extract_tgz_single(bytes: &[u8], out_path: &Path, inner_name: &str) -> Result<()> {
    let gz = flate2::read::GzDecoder::new(bytes);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut e = entry?;
        let path = e.path()?;
        if path.file_name().and_then(|s| s.to_str()) == Some(inner_name) {
            let mut f = fs::File::create(out_path)?;
            std::io::copy(&mut e, &mut f)?;
            return Ok(());
        }
    }
    bail!("did not find {} in archive", inner_name)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex::encode(h.finalize()))
}

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
