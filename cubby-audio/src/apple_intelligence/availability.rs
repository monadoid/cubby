use super::version::{is_macos_26_or_newer, MacOSVersion};
use anyhow::{anyhow, Result};
use serde::Deserialize;

mod ffi {
    use swift_rs::{swift, SRString};

    swift!(pub fn fm_check_model_availability() -> SRString);
}

/// Status values returned when checking language model availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanguageModelAvailabilityStatus {
    Available,
    Unavailable,
    Unknown,
}

/// Availability details for the system language model.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LanguageModelAvailability {
    pub status: LanguageModelAvailabilityStatus,
    pub reason: Option<String>,
    #[serde(rename = "reason_code")]
    pub reason_code: Option<String>,
}

impl LanguageModelAvailability {
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        matches!(self.status, LanguageModelAvailabilityStatus::Available)
    }
}

/// Check whether the default SystemLanguageModel is available.
pub fn language_model_availability() -> Result<LanguageModelAvailability> {
    ensure_supported()?;

    let availability_json = unsafe { ffi::fm_check_model_availability() }.to_string();
    parse_availability(&availability_json)
}

fn ensure_supported() -> Result<()> {
    if !is_macos_26_or_newer() {
        let current = MacOSVersion::current()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        return Err(anyhow!(
            "foundationmodels requires macOS 26.0+, but detected version {}. \
             please upgrade to macOS sequoia 26.0 or later (with apple intelligence).",
            current
        ));
    }
    Ok(())
}

fn parse_availability(json_str: &str) -> Result<LanguageModelAvailability> {
    let availability: LanguageModelAvailability = serde_json::from_str(json_str)?;
    Ok(availability)
}
