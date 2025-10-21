use anyhow::{anyhow, Result};
use serde::Deserialize;

#[swift_bridge::bridge]
mod ffi {
    extern "Swift" {
        fn fm_speech_transcribe_file(path: &str) -> String;
        fn fm_speech_preheat() -> String;
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
    use crate::version::{is_foundationmodels_supported, MacOSVersion};
    if !is_foundationmodels_supported() {
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
    use crate::version::{is_foundationmodels_supported, MacOSVersion};
    if !is_foundationmodels_supported() {
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

#[derive(Debug, Deserialize)]
struct InstallEnvelope {
    ok: Option<bool>,
    error: Option<String>,
}

pub async fn install_speech_assets() -> Result<()> {
    use crate::version::{is_foundationmodels_supported, MacOSVersion};
    if !is_foundationmodels_supported() {
        let ver = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".into());
        return Err(anyhow!(format!(
            "speech requires macos 26.0+, detected {}",
            ver
        )));
    }

    let json = tokio::task::spawn_blocking(|| ffi::fm_speech_install_assets()).await?;
    let env: InstallEnvelope = serde_json::from_str(&json)?;
    if let Some(err) = env.error {
        return Err(anyhow!(err));
    }
    if env.ok != Some(true) {
        return Err(anyhow!("speech assets install failed"));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct LocaleEnvelope<'a> {
    locale: Option<&'a str>,
    error: Option<String>,
}

pub async fn supported_locale() -> Result<String> {
    use crate::version::{is_foundationmodels_supported, MacOSVersion};
    if !is_foundationmodels_supported() {
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
