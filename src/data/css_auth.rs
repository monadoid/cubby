use anyhow::Result;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as b64;
use reqwest_middleware::reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::data::dpop::{create_dpop_proof, DPoPProofParams};
use crate::models::pods;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub scope: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CssAuthenticatedClient {
    pub client: Client,
    pub access_token: String,
    pub expires_at: SystemTime,
    pub dpop_private_jwk: String,
    pub css_base_url: String,
}

#[derive(Debug, Clone)]
pub struct CssAuthService {
    client: Client,
    css_base_url: String,
}

impl CssAuthService {
    pub fn new(css_base_url: String) -> Self {
        Self {
            client: Client::new(),
            css_base_url,
        }
    }

    /// Get an authenticated client for a user's pod using stored credentials
    pub async fn get_authenticated_client(
        &self,
        db: &sea_orm::DatabaseConnection,
        user_id: Uuid,
    ) -> Result<CssAuthenticatedClient> {
        // Get user's pod with stored credentials and DPoP keys
        let pod = pods::Model::find_by_user(db, user_id).await?
            .ok_or_else(|| anyhow::anyhow!("User has no pod"))?;

        let css_client_id = pod.css_client_id
            .ok_or_else(|| anyhow::anyhow!("Pod missing CSS client ID"))?;
        let css_client_secret = pod.css_client_secret
            .ok_or_else(|| anyhow::anyhow!("Pod missing CSS client secret"))?;
        let dpop_private_jwk = pod.dpop_private_jwk
            .ok_or_else(|| anyhow::anyhow!("Pod missing DPoP private key"))?;

        // Request access token using client credentials flow with DPoP
        let access_token_response = self.request_access_token(
            &css_client_id,
            &css_client_secret,
            &dpop_private_jwk,
        ).await?;

        let expires_at = SystemTime::now() + Duration::from_secs(access_token_response.expires_in);

        Ok(CssAuthenticatedClient {
            client: self.client.clone(),
            access_token: access_token_response.access_token,
            expires_at,
            dpop_private_jwk,
            css_base_url: self.css_base_url.clone(),
        })
    }

    /// Get an authenticated client that operates on a user's pod-specific URL
    pub async fn get_pod_authenticated_client(
        &self,
        db: &sea_orm::DatabaseConnection,
        user_id: Uuid,
    ) -> Result<CssAuthenticatedClient> {
        // Get user's pod with stored credentials and DPoP keys
        let pod = pods::Model::find_by_user(db, user_id).await?
            .ok_or_else(|| anyhow::anyhow!("User has no pod"))?;

        let css_client_id = pod.css_client_id
            .ok_or_else(|| anyhow::anyhow!("Pod missing CSS client ID"))?;
        let css_client_secret = pod.css_client_secret
            .ok_or_else(|| anyhow::anyhow!("Pod missing CSS client secret"))?;
        let dpop_private_jwk = pod.dpop_private_jwk
            .ok_or_else(|| anyhow::anyhow!("Pod missing DPoP private key"))?;
        let pod_base_url = pod.link
            .ok_or_else(|| anyhow::anyhow!("Pod missing base URL"))?;

        // Request access token using client credentials flow with DPoP
        let access_token_response = self.request_access_token(
            &css_client_id,
            &css_client_secret,
            &dpop_private_jwk,
        ).await?;

        let expires_at = SystemTime::now() + Duration::from_secs(access_token_response.expires_in);

        Ok(CssAuthenticatedClient {
            client: self.client.clone(),
            access_token: access_token_response.access_token,
            expires_at,
            dpop_private_jwk,
            css_base_url: pod_base_url, // Use pod-specific URL instead of server base URL
        })
    }

    /// Request an access token using client credentials + DPoP
    async fn request_access_token(
        &self,
        client_id: &str,
        client_secret: &str,
        dpop_private_jwk: &str,
    ) -> Result<AccessTokenResponse> {
        let token_url = format!("{}/.oidc/token", self.css_base_url);
        
        // Create basic auth header
        let auth_string = format!("{}:{}", 
            urlencoding::encode(client_id), 
            urlencoding::encode(client_secret)
        );
        let auth_header = format!("Basic {}", b64.encode(auth_string.as_bytes()));

        // Create DPoP proof for token request
        let dpop_params = DPoPProofParams {
            method: "POST".to_string(),
            url: token_url.clone(),
            access_token: None,
            nonce: None,
        };
        
        let dpop_proof = create_dpop_proof(dpop_private_jwk, &dpop_params).await?;

        // Make token request
        let response = self.client
            .post(&token_url)
            .header("Authorization", auth_header.clone())
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("DPoP", dpop_proof)
            .body("grant_type=client_credentials&scope=webid")
            .send()
            .await?;

        if response.status().is_success() {
            let token_response: AccessTokenResponse = response.json().await?;
            Ok(token_response)
        } else {
            // Check for DPoP nonce requirement
            if let Some(nonce) = response.headers().get("DPoP-Nonce") {
                let nonce_str = nonce.to_str()?;
                
                // Retry with nonce
                let dpop_params_with_nonce = DPoPProofParams {
                    method: "POST".to_string(),
                    url: token_url.clone(),
                    access_token: None,
                    nonce: Some(nonce_str.to_string()),
                };
                
                let dpop_proof_with_nonce = create_dpop_proof(dpop_private_jwk, &dpop_params_with_nonce).await?;

                let retry_response = self.client
                    .post(&token_url)
                    .header("Authorization", auth_header)
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .header("DPoP", dpop_proof_with_nonce)
                    .body("grant_type=client_credentials&scope=webid")
                    .send()
                    .await?;

                if retry_response.status().is_success() {
                    let token_response: AccessTokenResponse = retry_response.json().await?;
                    Ok(token_response)
                } else {
                    let error_body = retry_response.text().await?;
                    Err(anyhow::anyhow!("Token request failed after nonce retry: {}", error_body))
                }
            } else {
                let error_body = response.text().await?;
                Err(anyhow::anyhow!("Token request failed: {}", error_body))
            }
        }
    }
}

impl CssAuthenticatedClient {
    /// Check if the access token is expired
    pub fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }

    /// Make an authenticated HTTP request to the CSS server
    pub async fn authenticated_request(
        &self,
        method: &str,
        path: &str,
        headers: Option<&reqwest_middleware::reqwest::header::HeaderMap>,
        body: Option<String>,
    ) -> Result<reqwest_middleware::reqwest::Response> {
        if self.is_expired() {
            return Err(anyhow::anyhow!("Access token has expired"));
        }

        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{}", self.css_base_url, path)
        };

        // Create DPoP proof for this request
        let dpop_params = DPoPProofParams {
            method: method.to_uppercase(),
            url: url.clone(),
            access_token: Some(self.access_token.clone()),
            nonce: None,
        };
        
        let dpop_proof = create_dpop_proof(&self.dpop_private_jwk, &dpop_params).await?;

        // Build request
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            "OPTIONS" => self.client.request(reqwest_middleware::reqwest::Method::OPTIONS, &url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
        };

        // Add authentication headers
        request_builder = request_builder
            .header("Authorization", format!("DPoP {}", self.access_token))
            .header("DPoP", dpop_proof);

        // Add custom headers if provided
        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                request_builder = request_builder.header(key, value);
            }
        }

        // Add body if provided
        if let Some(ref body_content) = body {
            request_builder = request_builder
                .header("Content-Length", body_content.len().to_string())
                .body(body_content.clone());
        }

        let response = request_builder.send().await?;

        // Handle DPoP nonce challenges
        if response.status() == 400 || response.status() == 401 {
            if let Some(nonce) = response.headers().get("DPoP-Nonce") {
                let nonce_str = nonce.to_str()?;
                
                // Retry with nonce
                let dpop_params_with_nonce = DPoPProofParams {
                    method: method.to_uppercase(),
                    url: url.clone(),
                    access_token: Some(self.access_token.clone()),
                    nonce: Some(nonce_str.to_string()),
                };
                
                let dpop_proof_with_nonce = create_dpop_proof(&self.dpop_private_jwk, &dpop_params_with_nonce).await?;

                let mut retry_builder = match method.to_uppercase().as_str() {
                    "GET" => self.client.get(&url),
                    "POST" => self.client.post(&url),
                    "PUT" => self.client.put(&url),
                    "DELETE" => self.client.delete(&url),
                    "PATCH" => self.client.patch(&url),
                    "HEAD" => self.client.head(&url),
                    "OPTIONS" => self.client.request(reqwest_middleware::reqwest::Method::OPTIONS, &url),
                    _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
                };

                retry_builder = retry_builder
                    .header("Authorization", format!("DPoP {}", self.access_token))
                    .header("DPoP", dpop_proof_with_nonce);

                if let Some(custom_headers) = headers {
                    for (key, value) in custom_headers {
                        retry_builder = retry_builder.header(key, value);
                    }
                }

                if let Some(ref body_content) = body {
                    retry_builder = retry_builder
                        .header("Content-Length", body_content.len().to_string())
                        .body(body_content.clone());
                }

                return Ok(retry_builder.send().await?);
            }
        }

        Ok(response)
    }

    /// Make an authenticated HTTP request with binary data to the CSS server
    pub async fn authenticated_binary_request(
        &self,
        method: &str,
        path: &str,
        headers: Option<&reqwest_middleware::reqwest::header::HeaderMap>,
        body: Option<Vec<u8>>,
    ) -> Result<reqwest_middleware::reqwest::Response> {
        if self.is_expired() {
            return Err(anyhow::anyhow!("Access token has expired"));
        }

        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{}", self.css_base_url, path)
        };

        // Create DPoP proof for this request
        let dpop_params = DPoPProofParams {
            method: method.to_uppercase(),
            url: url.clone(),
            access_token: Some(self.access_token.clone()),
            nonce: None,
        };
        
        let dpop_proof = create_dpop_proof(&self.dpop_private_jwk, &dpop_params).await?;

        // Build request
        let mut request_builder = match method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            "OPTIONS" => self.client.request(reqwest_middleware::reqwest::Method::OPTIONS, &url),
            _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
        };

        // Add authentication headers
        request_builder = request_builder
            .header("Authorization", format!("DPoP {}", self.access_token))
            .header("DPoP", dpop_proof);

        // Add custom headers if provided
        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                request_builder = request_builder.header(key, value);
            }
        }

        // Add body if provided
        if let Some(ref body_content) = body {
            request_builder = request_builder
                .header("Content-Length", body_content.len().to_string())
                .body(body_content.clone());
        }

        let response = request_builder.send().await?;

        // Handle DPoP nonce challenges
        if response.status() == 400 || response.status() == 401 {
            if let Some(nonce) = response.headers().get("DPoP-Nonce") {
                let nonce_str = nonce.to_str()?;
                
                // Retry with nonce
                let dpop_params_with_nonce = DPoPProofParams {
                    method: method.to_uppercase(),
                    url: url.clone(),
                    access_token: Some(self.access_token.clone()),
                    nonce: Some(nonce_str.to_string()),
                };
                
                let dpop_proof_with_nonce = create_dpop_proof(&self.dpop_private_jwk, &dpop_params_with_nonce).await?;

                let mut retry_builder = match method.to_uppercase().as_str() {
                    "GET" => self.client.get(&url),
                    "POST" => self.client.post(&url),
                    "PUT" => self.client.put(&url),
                    "DELETE" => self.client.delete(&url),
                    "PATCH" => self.client.patch(&url),
                    "HEAD" => self.client.head(&url),
                    "OPTIONS" => self.client.request(reqwest_middleware::reqwest::Method::OPTIONS, &url),
                    _ => return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method)),
                };

                retry_builder = retry_builder
                    .header("Authorization", format!("DPoP {}", self.access_token))
                    .header("DPoP", dpop_proof_with_nonce);

                if let Some(custom_headers) = headers {
                    for (key, value) in custom_headers {
                        retry_builder = retry_builder.header(key, value);
                    }
                }

                if let Some(ref body_content) = body {
                    retry_builder = retry_builder
                        .header("Content-Length", body_content.len().to_string())
                        .body(body_content.clone());
                }

                return Ok(retry_builder.send().await?);
            }
        }

        Ok(response)
    }
}