use anyhow::{anyhow, Result};
use serde::Deserialize;

#[swift_bridge::bridge]
mod ffi {
    extern "Swift" {
        fn fm_speech_transcribe_file(path: &str) -> String;
        fn fm_speech_preheat() -> String;
        fn fm_speech_assets_status() -> String;
        fn fm_speech_ensure_assets() -> String;
        fn fm_speech_install_assets() -> String;
        fn fm_speech_supported_locale() -> String;
    }
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

pub async fn transcribe_file(path: &str) -> Result<String> {
    // version check via existing helper in lib
    #[allow(unused_imports)]
    use crate::version::{is_macos_26_or_newer, MacOSVersion};
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }

    let path = path.to_string();
    let json = tokio::task::spawn_blocking(move || ffi::fm_speech_transcribe_file(&path)).await?;
    parse(&json)
}

#[derive(Debug, Deserialize)]
struct PreheatEnvelope {
    ok: Option<bool>,
    error: Option<String>,
}

pub async fn preheat_speech() -> Result<()> {
    use crate::version::{is_macos_26_or_newer, MacOSVersion};
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }

    let json = tokio::task::spawn_blocking(|| ffi::fm_speech_preheat()).await?;
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

pub async fn supported_locale() -> Result<String> {
    use crate::version::{is_macos_26_or_newer, MacOSVersion};
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }
    let json = tokio::task::spawn_blocking(|| ffi::fm_speech_supported_locale()).await?;
    let env: LocaleEnvelope = serde_json::from_str(&json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    Ok(env.locale.unwrap_or("").to_string())
}

pub async fn speech_asset_status() -> Result<SpeechAssetStatus> {
    use crate::version::{is_macos_26_or_newer, MacOSVersion};
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }

    let json = tokio::task::spawn_blocking(|| ffi::fm_speech_assets_status()).await?;
    let env = parse_status_envelope(&json)?;
    env.status
        .ok_or_else(|| anyhow!("speech assets status missing status field"))
}

pub async fn ensure_speech_assets_installed() -> Result<SpeechAssetEnsureResponse> {
    use crate::version::{is_macos_26_or_newer, MacOSVersion};
    if !is_macos_26_or_newer() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }

    let json = tokio::task::spawn_blocking(|| ffi::fm_speech_ensure_assets()).await?;
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
