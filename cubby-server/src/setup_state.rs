use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Persistent state that allows the installer/startup flow to be idempotent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupState {
    pub account_created: bool,
    pub cloudflared_installed: bool,
    pub service_installed: bool,
    pub microphone_granted: bool,
    pub screen_granted: bool,
    pub tunnel_token: Option<String>,
    pub session_jwt: Option<String>,
    pub audio_enabled: Option<bool>,
}

impl Default for SetupState {
    fn default() -> Self {
        Self {
            account_created: false,
            cloudflared_installed: false,
            service_installed: false,
            microphone_granted: false,
            screen_granted: false,
            tunnel_token: None,
            session_jwt: None,
            audio_enabled: None,
        }
    }
}

impl SetupState {
    const APP_NAME: &'static str = "cubby";
    const CONFIG_NAME: &'static str = "setup-state";

    pub fn load() -> Result<Self> {
        let state: Self = confy::load(Self::APP_NAME, Some(Self::CONFIG_NAME))?;
        Ok(state)
    }

    pub fn store(&self) -> Result<()> {
        confy::store(Self::APP_NAME, Some(Self::CONFIG_NAME), self)?;
        Ok(())
    }
}
