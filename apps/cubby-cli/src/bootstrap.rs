use anyhow::{Context, Result};
use rust_embed::RustEmbed;
use std::path::PathBuf;
use std::{fs, io::Write};

// Embed the rust_embed_assets folder containing our binaries
#[derive(RustEmbed)]
#[folder = "rust_embed_assets"]
struct EmbeddedAssets;

// Binary names based on platform
pub const SCREENPIPE_BINARY: &str = if cfg!(target_os = "windows") {
    "screenpipe.exe"
} else {
    "screenpipe"
};
pub const CLOUDFLARED_BINARY: &str = if cfg!(target_os = "windows") {
    "cloudflared.exe"
} else {
    "cloudflared"
};

pub struct BinPaths {
    pub screenpipe: PathBuf,
    pub cloudflared: PathBuf,
}

/// Get a temporary directory for extracting embedded binaries
fn get_temp_bin_dir() -> Result<PathBuf> {
    let temp_dir = std::env::temp_dir().join("cubby-binaries");
    fs::create_dir_all(&temp_dir).context("Failed to create temp directory")?;
    Ok(temp_dir)
}

/// Extract an embedded binary to the temp directory and make it executable
fn extract_binary(name: &str, temp_dir: &PathBuf) -> Result<PathBuf> {
    // Get the embedded file data
    let data = EmbeddedAssets::get(name)
        .ok_or_else(|| anyhow::anyhow!("Embedded binary '{}' not found", name))?;

    // Determine the output filename based on platform
    let output_name = match name {
        "screenpipe" => SCREENPIPE_BINARY,
        "cloudflared" => CLOUDFLARED_BINARY,
        _ => name,
    };

    let output_path = temp_dir.join(output_name);

    // Write the binary data to disk
    let mut file = fs::File::create(&output_path).context("Failed to create binary file")?;
    file.write_all(&data.data)
        .context("Failed to write binary data")?;

    // Make the binary executable on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&output_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&output_path, perms)?;
    }

    Ok(output_path)
}

/// Extract both embedded binaries to temp directory and return their paths
pub fn ensure_binaries() -> Result<BinPaths> {
    let temp_dir = get_temp_bin_dir()?;

    println!("Extracting embedded binaries to: {}", temp_dir.display());

    // Extract both binaries
    let screenpipe =
        extract_binary("screenpipe", &temp_dir).context("Failed to extract screenpipe binary")?;
    let cloudflared =
        extract_binary("cloudflared", &temp_dir).context("Failed to extract cloudflared binary")?;

    println!("Extracted screenpipe to: {}", screenpipe.display());
    println!("Extracted cloudflared to: {}", cloudflared.display());

    Ok(BinPaths {
        screenpipe,
        cloudflared,
    })
}
