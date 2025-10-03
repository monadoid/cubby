use anyhow::{anyhow, bail, Result};
use directories::ProjectDirs;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
pub enum Dep {
    Screenpipe,
    Cloudflared,
}

#[derive(Clone, Debug)]
pub enum VersionSpec {
    /// e.g., "v0.2.74" (screenpipe) or "2025.9.1" (cloudflared)
    Pinned(&'static str),
    Latest,
}

/// manager for resolving, downloading, and caching tool binaries.
#[derive(Clone, Debug)]
pub struct ToolManager {
    pub screenpipe: VersionSpec,
    pub cloudflared: VersionSpec,
    /// If true, allow local overrides and skip-download behaviors in dev.
    pub dev_mode: bool,
    /// Force redownload/extract even if present.
    pub force: bool,
}

impl ToolManager {
    pub fn pinned_defaults() -> Self {
        Self {
            screenpipe: VersionSpec::Pinned("v0.2.74"),
            cloudflared: VersionSpec::Pinned("2025.9.1"),
            dev_mode: cfg!(debug_assertions),
            force: false,
        }
    }

    pub fn latest_for_dev() -> Self {
        Self {
            screenpipe: VersionSpec::Latest,
            cloudflared: VersionSpec::Latest,
            dev_mode: true,
            force: false,
        }
    }

    /// Ensure a single tool; returns absolute path to the binary to execute.
    pub fn ensure(&self, dep: Dep) -> Result<PathBuf> {
        // Dev override: use a local dir if provided.
        if self.dev_mode {
            if let Ok(dir) = env::var("CUBBY_DEV_BIN_DIR") {
                let (os, _arch) = detect_platform()?;
                let p = PathBuf::from(dir).join(binary_filename(dep, os));
                if p.exists() {
                    return Ok(p.canonicalize().unwrap_or(p));
                }
            }
            if env::var("CUBBY_SKIP_DOWNLOAD").ok().as_deref() == Some("1") {
                bail!("CUBBY_SKIP_DOWNLOAD=1 set and no dev binary found");
            }
        }

        let (os, arch) = detect_platform()?;
        let proj = ProjectDirs::from("com", "tabsandtabs", "cubby")
            .ok_or_else(|| anyhow!("cannot resolve ProjectDirs"))?;
        let bindir = proj.data_dir().join("bin");
        fs::create_dir_all(&bindir)?;

        // Decide version policy for this dep.
        let version = match dep {
            Dep::Screenpipe => self.screenpipe.clone(),
            Dep::Cloudflared => self.cloudflared.clone(),
        };

        // Resolve release tag + URL + archive layout.
        let (tag, url, archive) = resolve_url(dep, &version, os, arch)?;

        // Versioned folder so multiple versions can coexist.
        let folder = bindir.join(format!(
            "{}-{}-{}-{}",
            dep_name(dep),
            tag,
            os_label(os),
            arch_label(arch)
        ));
        let bin_name = binary_filename(dep, os);
        let target = folder.join(&bin_name);

        if !self.force && target.exists() {
            return Ok(target);
        }

        // Download to a temp file and extract if needed.
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
            Archive::Zip(inner) => {
                extract_zip_single(&bytes, &tmp, inner)?;
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

        // Write a small metadata file (handy for debugging/upgrades).
        let meta = Meta {
            tag: tag.to_string(),
            source: url.clone(),
            downloaded_at_epoch_s: now_s(),
            sha256: sha256_file(&target).ok(),
        };
        fs::write(folder.join(".meta.json"), serde_json::to_vec_pretty(&meta)?)?;

        Ok(target)
    }

    pub fn ensure_both(&self) -> Result<(PathBuf, PathBuf)> {
        let screenpipe = self.ensure(Dep::Screenpipe)?;
        let cloudflared = self.ensure(Dep::Cloudflared)?;
        Ok((screenpipe, cloudflared))
    }
}

// ----- internals below -----

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
    Zip(&'static str), // zip; inner binary to extract
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(serde::Serialize)]
struct Meta {
    tag: String,
    source: String,
    downloaded_at_epoch_s: u64,
    sha256: Option<String>,
}

fn resolve_url(
    dep: Dep,
    ver: &VersionSpec,
    os: Os,
    arch: Arch,
) -> Result<(String, String, Archive)> {
    match dep {
        Dep::Cloudflared => {
            // Cloudflared uses stable, versionless asset names → /latest/download works.
            let (filename, archive) = match (os, arch) {
                (Os::Mac, Arch::Aarch64) => {
                    ("cloudflared-darwin-arm64.tgz", Archive::Tgz("cloudflared"))
                }
                (Os::Mac, Arch::X86_64) => {
                    ("cloudflared-darwin-amd64.tgz", Archive::Tgz("cloudflared"))
                }
                (Os::Linux, Arch::X86_64) => ("cloudflared-linux-amd64", Archive::Plain),
                (Os::Linux, Arch::Aarch64) => ("cloudflared-linux-arm64", Archive::Plain),
                (Os::Windows, Arch::X86_64) => ("cloudflared-windows-amd64.exe", Archive::Plain),
                _ => bail!("unsupported platform for cloudflared"),
            };
            match ver {
                VersionSpec::Latest => {
                    let url = format!(
                        "https://github.com/cloudflare/cloudflared/releases/latest/download/{}",
                        filename
                    );
                    // Fetch tag for cache folder naming
                    let rel = github_latest("cloudflare", "cloudflared")?;
                    Ok((rel.tag_name, url, archive))
                }
                VersionSpec::Pinned(tag) => {
                    let url = format!(
                        "https://github.com/cloudflare/cloudflared/releases/download/{}/{}",
                        tag, filename
                    );
                    Ok((tag.to_string(), url, archive))
                }
            }
        }
        Dep::Screenpipe => {
            // screenpipe assets include the version in their filenames → use the API.
            let rel = match ver {
                VersionSpec::Latest => github_latest("mediar-ai", "screenpipe")?,
                VersionSpec::Pinned(tag) => github_by_tag("mediar-ai", "screenpipe", tag)?,
            };
            let ver_no_v = rel.tag_name.trim_start_matches('v');
            let (needle, archive) = match (os, arch) {
                (Os::Mac, Arch::Aarch64) => (
                    format!("screenpipe-{}-aarch64-apple-darwin.tar.gz", ver_no_v),
                    Archive::Tgz("screenpipe"),
                ),
                (Os::Mac, Arch::X86_64) => (
                    format!("screenpipe-{}-x86_64-apple-darwin.tar.gz", ver_no_v),
                    Archive::Tgz("screenpipe"),
                ),
                (Os::Linux, Arch::X86_64) => (
                    format!("screenpipe-{}-x86_64-unknown-linux-gnu.tar.gz", ver_no_v),
                    Archive::Tgz("screenpipe"),
                ),
                (Os::Windows, Arch::X86_64) => (
                    format!("screenpipe-{}-x86_64-pc-windows-msvc.zip", ver_no_v),
                    Archive::Zip("screenpipe.exe"),
                ),
                _ => bail!("unsupported platform for screenpipe (tag {})", rel.tag_name),
            };
            let asset = rel
                .assets
                .into_iter()
                .find(|a| a.name == needle)
                .ok_or_else(|| anyhow!("no asset named {} in release {}", needle, rel.tag_name))?;
            Ok((rel.tag_name, asset.browser_download_url, archive))
        }
    }
}

fn github_latest(owner: &str, repo: &str) -> Result<Release> {
    github_get_release(&format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    ))
}
fn github_by_tag(owner: &str, repo: &str, tag: &str) -> Result<Release> {
    github_get_release(&format!(
        "https://api.github.com/repos/{}/{}/releases/tags/{}",
        owner, repo, tag
    ))
}

fn github_get_release(url: &str) -> Result<Release> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cubby/0.1 (+https://tabsandtabs)")
        .build()?;
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if let Ok(tok) = env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {}", tok));
    }
    Ok(req.send()?.error_for_status()?.json::<Release>()?)
}

fn http_get_octets(url: &str) -> Result<Vec<u8>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("cubby/0.1 (+https://tabsandtabs)")
        .build()?;
    let mut req = client.get(url);
    if let Ok(tok) = env::var("GITHUB_TOKEN") {
        req = req.header("Authorization", format!("Bearer {}", tok));
    }
    let mut resp = req.send()?.error_for_status()?;
    let mut buf = Vec::with_capacity(resp.content_length().unwrap_or(0) as usize);
    resp.copy_to(&mut buf)?;
    Ok(buf)
}

// ----- extraction helpers -----

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

fn extract_zip_single(bytes: &[u8], out_path: &Path, inner_name: &str) -> Result<()> {
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)?;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        if f.name().ends_with(inner_name) {
            let mut out = fs::File::create(out_path)?;
            std::io::copy(&mut f, &mut out)?;
            return Ok(());
        }
    }
    bail!("did not find {} in zip", inner_name)
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

// ----- platform helpers -----

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

fn dep_name(dep: Dep) -> &'static str {
    match dep {
        Dep::Screenpipe => "screenpipe",
        Dep::Cloudflared => "cloudflared",
    }
}

fn binary_filename(dep: Dep, os: Os) -> String {
    let base = match dep {
        Dep::Screenpipe => "screenpipe",
        Dep::Cloudflared => "cloudflared",
    };
    match os {
        Os::Windows => format!("{}.exe", base),
        _ => base.to_string(),
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

fn now_s() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
