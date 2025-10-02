use anyhow::{anyhow, bail, Context, Result};
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

#[derive(Debug, Serialize)]
pub struct DeviceEnrollRequest {
    pub device_id: String,
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

        if !response.status().is_success() {
            return bail!(
                "Sign-up request failed with status: {}",
                response.status()
            );
        }

        let sign_up_response: SignUpResponse = response
            .json()
            .context("Failed to parse sign-up response")?;

        Ok(sign_up_response)
    }

    pub fn enroll_device(&self, request: DeviceEnrollRequest) -> Result<DeviceEnrollResponse> {
        let url = format!("{}/devices/enroll", self.base_url);

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .context("Failed to send device enrollment request")?;

        if !response.status().is_success() {
            return bail!(
                "Device enrollment request failed with status: {}",
                response.status()
            );
        }

        let response_text = response.text().context("Failed to get response text")?;
        println!("Raw API response: {}", response_text);

        let enroll_response: DeviceEnrollResponse = serde_json::from_str(&response_text)
            .context("Failed to parse device enrollment response")?;

        Ok(enroll_response)
    }
}
