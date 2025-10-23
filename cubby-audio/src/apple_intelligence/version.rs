//! macOS version detection for FoundationModels availability.

use std::process::Command;
use std::sync::OnceLock;

/// Parsed macOS version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MacOSVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl MacOSVersion {
    /// Minimum version required for FoundationModels (26.0.0).
    pub const MINIMUM_REQUIRED: MacOSVersion = MacOSVersion {
        major: 26,
        minor: 0,
        patch: 0,
    };

    /// Parse from version string like "26.0.1" or "27.1.0".
    pub fn parse(version_str: &str) -> Option<Self> {
        let parts: Vec<&str> = version_str.trim().split('.').collect();

        if parts.is_empty() {
            return None;
        }

        let major = parts[0].parse().ok()?;
        let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

        Some(MacOSVersion {
            major,
            minor,
            patch,
        })
    }

    /// Get current macOS version by calling sw_vers.
    pub fn current() -> Option<Self> {
        static CACHED_VERSION: OnceLock<Option<MacOSVersion>> = OnceLock::new();

        *CACHED_VERSION.get_or_init(|| {
            let output = Command::new("sw_vers")
                .arg("-productVersion")
                .output()
                .ok()?;

            if !output.status.success() {
                return None;
            }

            let version_str = String::from_utf8(output.stdout).ok()?;
            Self::parse(&version_str)
        })
    }

    /// Check if FoundationModels should be available on this system.
    pub fn supports_foundationmodels(&self) -> bool {
        *self >= Self::MINIMUM_REQUIRED
    }
}

impl std::fmt::Display for MacOSVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Check if the current system is running macOS 26.0.0 or newer.
pub fn is_macos_26_or_newer() -> bool {
    MacOSVersion::current()
        .map(|v| v.supports_foundationmodels())
        .unwrap_or(false)
}

/// Backward-compatibility alias; prefer [`is_macos_26_or_newer`].
#[inline]
#[allow(dead_code)]
pub fn is_foundationmodels_supported() -> bool {
    is_macos_26_or_newer()
}
