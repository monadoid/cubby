//! macOS version detection for FoundationModels availability

use std::process::Command;
use std::sync::OnceLock;

/// Parsed macOS version
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MacOSVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl MacOSVersion {
    /// Minimum version required for FoundationModels (26.0.0)
    /// Note: Apple changed versioning - this is macOS Sequoia with SDK 26.0
    pub const MINIMUM_REQUIRED: MacOSVersion = MacOSVersion {
        major: 26,
        minor: 0,
        patch: 0,
    };

    /// Parse from version string like "26.0.1" or "27.1.0"
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

    /// Get current macOS version by calling sw_vers
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

    /// Check if FoundationModels should be available on this system
    pub fn supports_foundationmodels(&self) -> bool {
        *self >= Self::MINIMUM_REQUIRED
    }
}

impl std::fmt::Display for MacOSVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Check if FoundationModels is available on the current system
///
/// Returns `false` if:
/// - Not running on macOS
/// - macOS version < 26.0.0
/// - Version detection fails
pub fn is_foundationmodels_supported() -> bool {
    MacOSVersion::current()
        .map(|v| v.supports_foundationmodels())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(
            MacOSVersion::parse("26.0.0"),
            Some(MacOSVersion {
                major: 26,
                minor: 0,
                patch: 0
            })
        );

        assert_eq!(
            MacOSVersion::parse("26.0.1"),
            Some(MacOSVersion {
                major: 26,
                minor: 0,
                patch: 1
            })
        );

        assert_eq!(
            MacOSVersion::parse("26.1"),
            Some(MacOSVersion {
                major: 26,
                minor: 1,
                patch: 0
            })
        );

        assert_eq!(
            MacOSVersion::parse("14"),
            Some(MacOSVersion {
                major: 14,
                minor: 0,
                patch: 0
            })
        );

        assert_eq!(MacOSVersion::parse(""), None);
        assert_eq!(MacOSVersion::parse("invalid"), None);
    }

    #[test]
    fn test_version_comparison() {
        let v26_0_0 = MacOSVersion {
            major: 26,
            minor: 0,
            patch: 0,
        };
        let v25_0_0 = MacOSVersion {
            major: 25,
            minor: 0,
            patch: 0,
        };
        let v26_0_1 = MacOSVersion {
            major: 26,
            minor: 0,
            patch: 1,
        };

        assert!(v26_0_0 > v25_0_0);
        assert!(v26_0_1 > v26_0_0);
        assert!(v26_0_0 >= MacOSVersion::MINIMUM_REQUIRED);
        assert!(v25_0_0 < MacOSVersion::MINIMUM_REQUIRED);
    }

    #[test]
    fn test_supports_foundationmodels() {
        let supported = MacOSVersion {
            major: 26,
            minor: 0,
            patch: 0,
        };
        let unsupported = MacOSVersion {
            major: 25,
            minor: 9,
            patch: 9,
        };
        let future = MacOSVersion {
            major: 27,
            minor: 0,
            patch: 0,
        };

        assert!(supported.supports_foundationmodels());
        assert!(!unsupported.supports_foundationmodels());
        assert!(future.supports_foundationmodels());
    }

    #[test]
    fn test_current_version() {
        // this test actually calls sw_vers
        if let Some(version) = MacOSVersion::current() {
            println!("detected macos version: {}", version);
            assert!(version.major > 0);
        } else {
            println!("could not detect macos version (expected if not on macos)");
        }
    }

    #[test]
    fn test_display() {
        let v = MacOSVersion {
            major: 26,
            minor: 0,
            patch: 1,
        };
        assert_eq!(format!("{}", v), "26.0.1");
    }
}
