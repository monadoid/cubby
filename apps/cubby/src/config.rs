/// Configuration that varies between development and production builds.
///
/// Debug builds (cargo run, cargo build) use localhost endpoints.
/// Release builds (cargo build --release) use production endpoints.

#[derive(Debug, Clone)]
pub struct Config {
    pub api_base_url: String,
}

impl Config {
    /// Returns the appropriate configuration based on the build profile.
    ///
    /// Debug builds: localhost endpoints
    /// Release builds: production endpoints
    pub fn from_build_profile() -> Self {
        #[cfg(debug_assertions)]
        {
            Self::development()
        }

        #[cfg(not(debug_assertions))]
        {
            Self::production()
        }
    }

    /// Development configuration (localhost)
    fn development() -> Self {
        Self {
            api_base_url: "http://localhost:8787".to_string(),
        }
    }

    /// Production configuration
    fn production() -> Self {
        Self {
            api_base_url: "https://api.cubby.sh".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loads() {
        let config = Config::from_build_profile();
        assert!(!config.api_base_url.is_empty());
    }
}
