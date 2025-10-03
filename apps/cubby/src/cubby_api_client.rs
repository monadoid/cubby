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

pub struct CubbyApiClient {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl CubbyApiClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn sign_up(&self, request: SignUpRequest) -> Result<SignUpResponse> {
        let url = format!("{}/sign-up", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send sign-up request")?;

        let status = response.status();
        if !status.is_success() {
            // Get the response body as text first
            let body_text = response
                .text()
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
            .context("Failed to parse sign-up response")?;

        Ok(sign_up_response)
    }

    pub fn enroll_device(&self, session_jwt: &str) -> Result<DeviceEnrollResponse> {
        let url = format!("{}/devices/enroll", self.base_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(session_jwt)
            .json(&serde_json::json!({}))
            .send()
            .context("Failed to send device enrollment request")?;

        let status = response.status();
        if !status.is_success() {
            // Get the response body as text first
            let body_text = response
                .text()
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
            .context("Failed to parse device enrollment response")?;

        Ok(enroll_response)
    }
}
