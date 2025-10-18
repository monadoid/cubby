use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct SignUpRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct SignUpResponse {
    pub user_id: String,
    pub session_token: String,
    pub session_jwt: String,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
pub struct DeviceEnrollResponse {
    pub device_id: String,
    pub hostname: String,
    pub tunnel_token: String,
}

#[derive(Debug, Serialize)]
pub struct CreateM2MClientRequest {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateM2MClientResponse {
    pub client_id: String,
    pub client_secret: String,
    pub name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct TokenExchangeResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: Option<String>,
}

pub struct CubbyApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl CubbyApiClient {
    pub fn new() -> Self {
        let base_url = Self::get_api_base_url();
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    fn get_api_base_url() -> String {
        #[cfg(debug_assertions)]
        {
            "http://localhost:8787".to_string()
        }

        #[cfg(not(debug_assertions))]
        {
            "https://api.cubby.sh".to_string()
        }
    }

    pub async fn sign_up(&self, email: String, password: String) -> Result<SignUpResponse> {
        let url = format!("{}/sign-up", self.base_url);

        let request = SignUpRequest { email, password };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send sign-up request")?;

        let status = response.status();
        if !status.is_success() {
            // Get the response body as text first
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());

            // Try to parse as JSON error response
            let error_msg = serde_json::from_str::<ApiErrorResponse>(&body_text)
                .map(|err| err.error)
                .unwrap_or_else(|_| {
                    // If not valid JSON, show the raw body
                    format!("HTTP {}: {}", status, body_text)
                });

            bail!("{}", error_msg);
        }

        let sign_up_response: SignUpResponse = response
            .json()
            .await
            .context("Failed to parse sign-up response")?;

        Ok(sign_up_response)
    }

    pub async fn enroll_device(&self, session_jwt: &str) -> Result<DeviceEnrollResponse> {
        let url = format!("{}/devices/enroll", self.base_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(session_jwt)
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("Failed to send device enrollment request")?;

        let status = response.status();
        if !status.is_success() {
            // Get the response body as text first
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());

            // Try to parse as JSON error response
            let error_msg = serde_json::from_str::<ApiErrorResponse>(&body_text)
                .map(|err| err.error)
                .unwrap_or_else(|_| {
                    // If not valid JSON, show the raw body
                    format!("HTTP {}: {}", status, body_text)
                });

            bail!("{}", error_msg);
        }

        let enroll_response: DeviceEnrollResponse = response
            .json()
            .await
            .context("Failed to parse device enrollment response")?;

        Ok(enroll_response)
    }

    pub async fn create_m2m_client(&self, session_jwt: &str) -> Result<CreateM2MClientResponse> {
        let url = format!("{}/m2m/clients", self.base_url);
        let name = format!("cubby-cli-{}", chrono::Utc::now().timestamp());

        let response = self
            .client
            .post(&url)
            .bearer_auth(session_jwt)
            .json(&CreateM2MClientRequest { name: Some(name) })
            .send()
            .await
            .context("failed to create m2m client")?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());

            let error_msg = serde_json::from_str::<ApiErrorResponse>(&body_text)
                .map(|err| err.error)
                .unwrap_or_else(|_| format!("HTTP {}: {}", status, body_text));

            bail!("{}", error_msg);
        }

        let m2m_response: CreateM2MClientResponse = response
            .json()
            .await
            .context("failed to parse m2m client response")?;

        Ok(m2m_response)
    }

    pub async fn exchange_for_token(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<TokenExchangeResponse> {
        // Get the OAuth token endpoint
        let stytch_domain = if cfg!(debug_assertions) {
            // In dev, this would be the test environment
            self.base_url.clone()
        } else {
            "https://login.cubby.sh".to_string()
        };

        let url = format!("{}/v1/oauth2/token", stytch_domain);

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("scope", "read:cubby"),
        ];

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .context("failed to exchange token")?;

        let status = response.status();
        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_else(|_| "<failed to read body>".to_string());
            bail!("token exchange failed: HTTP {}: {}", status, body_text);
        }

        let token_response: TokenExchangeResponse = response
            .json()
            .await
            .context("failed to parse token response")?;

        Ok(token_response)
    }
}

impl Default for CubbyApiClient {
    fn default() -> Self {
        Self::new()
    }
}
