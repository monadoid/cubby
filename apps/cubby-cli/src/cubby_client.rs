use anyhow::Result;
use reqwest;
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

pub struct CubbyClient {
    base_url: String,
    http_client: reqwest::Client,
}

impl CubbyClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn sign_up(&self, request: &SignUpRequest) -> Result<SignUpResponse> {
        let url = format!("{}/sign-up", self.base_url);

        let response = self.http_client.post(&url).json(request).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Sign up failed with status: {}",
                response.status()
            ));
        }

        let sign_up_response = response.json::<SignUpResponse>().await?;
        Ok(sign_up_response)
    }

    pub async fn enroll_device(
        &self,
        request: &DeviceEnrollRequest,
    ) -> Result<DeviceEnrollResponse> {
        let url = format!("{}/devices/enroll", self.base_url);

        let response = self.http_client.post(&url).json(request).send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Device enrollment failed with status: {}",
                response.status()
            ));
        }

        let enroll_response = response.json::<DeviceEnrollResponse>().await?;
        Ok(enroll_response)
    }
}
