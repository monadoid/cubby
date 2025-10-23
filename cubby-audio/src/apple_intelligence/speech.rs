use anyhow::{anyhow, Result};
use serde::Deserialize;
use swift_rs::SRString;

use super::version::{is_macos_26_or_newer, MacOSVersion};

mod ffi {
    use swift_rs::{swift, SRString};

    swift!(pub fn fm_speech_transcribe_file(path: &SRString) -> SRString);
    swift!(pub fn fm_speech_preheat() -> SRString);
    swift!(pub fn fm_speech_assets_status() -> SRString);
    swift!(pub fn fm_speech_ensure_assets() -> SRString);
    swift!(pub fn fm_speech_install_assets() -> SRString);
    swift!(pub fn fm_speech_supported_locale() -> SRString);
}

#[derive(Debug, Deserialize)]
struct SpeechEnvelope<'a> {
    #[serde(borrow)]
    transcript: Option<&'a str>,
    error: Option<String>,
}

fn parse(json: &str) -> Result<String> {
    let env: SpeechEnvelope = serde_json::from_str(json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    Ok(env.transcript.unwrap_or_default().to_string())
}

#[allow(dead_code)]
pub async fn transcribe_file(path: &str) -> Result<String> {
    ensure_supported()?;

    let path = path.to_string();
    let json = tokio::task::spawn_blocking(move || {
        let path_sr = SRString::from(path.as_str());
        let result = unsafe { ffi::fm_speech_transcribe_file(&path_sr) };
        result.to_string()
    })
    .await?;
    parse(&json)
}

#[derive(Debug, Deserialize)]
struct PreheatEnvelope {
    ok: Option<bool>,
    error: Option<String>,
}

#[allow(dead_code)]
pub async fn preheat_speech() -> Result<()> {
    ensure_supported()?;

    let json =
        tokio::task::spawn_blocking(|| unsafe { ffi::fm_speech_preheat() }.to_string()).await?;
    let env: PreheatEnvelope = serde_json::from_str(&json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    if env.ok != Some(true) {
        return Err(anyhow!("speech preheat failed"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechAssetStatus {
    Unsupported,
    Supported,
    Downloading,
    Installed,
}

#[derive(Debug, Deserialize)]
struct AssetStatusEnvelope {
    status: Option<SpeechAssetStatus>,
    #[serde(default)]
    installed_now: Option<bool>,
    error: Option<String>,
}

#[derive(Debug)]
pub struct SpeechAssetEnsureResponse {
    pub status: SpeechAssetStatus,
    pub installed_now: bool,
}

#[allow(dead_code)]
pub async fn install_speech_assets() -> Result<()> {
    let response = ensure_speech_assets_installed().await?;
    match response.status {
        SpeechAssetStatus::Installed | SpeechAssetStatus::Downloading => Ok(()),
        SpeechAssetStatus::Supported => Err(anyhow!(
            "speech assets download did not complete; status remained supported"
        )),
        SpeechAssetStatus::Unsupported => Err(anyhow!(
            "speech assets unsupported for current locale/configuration"
        )),
    }
}

#[derive(Debug, Deserialize)]
struct LocaleEnvelope<'a> {
    locale: Option<&'a str>,
    error: Option<String>,
}

#[allow(dead_code)]
pub async fn supported_locale() -> Result<String> {
    ensure_supported()?;

    let json =
        tokio::task::spawn_blocking(|| unsafe { ffi::fm_speech_supported_locale() }.to_string())
            .await?;
    let env: LocaleEnvelope = serde_json::from_str(&json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    Ok(env.locale.unwrap_or("").to_string())
}

#[allow(dead_code)]
pub async fn speech_asset_status() -> Result<SpeechAssetStatus> {
    ensure_supported()?;

    let json =
        tokio::task::spawn_blocking(|| unsafe { ffi::fm_speech_assets_status() }.to_string())
            .await?;
    let env = parse_status_envelope(&json)?;
    env.status
        .ok_or_else(|| anyhow!("speech assets status missing status field"))
}

pub async fn ensure_speech_assets_installed() -> Result<SpeechAssetEnsureResponse> {
    ensure_supported()?;

    let json =
        tokio::task::spawn_blocking(|| unsafe { ffi::fm_speech_ensure_assets() }.to_string())
            .await?;
    let env = parse_status_envelope(&json)?;
    let status = env
        .status
        .ok_or_else(|| anyhow!("speech assets ensure response missing status"))?;
    Ok(SpeechAssetEnsureResponse {
        status,
        installed_now: env.installed_now.unwrap_or(false),
    })
}

fn parse_status_envelope(json: &str) -> Result<AssetStatusEnvelope> {
    let env: AssetStatusEnvelope = serde_json::from_str(json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    Ok(env)
}

fn ensure_supported() -> Result<()> {
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }
    Ok(())
}
