#![allow(unused)]
use anyhow::anyhow;
use reqwest_middleware::reqwest;
use reqwest_middleware::reqwest::header::{HeaderMap, CONTENT_TYPE, AUTHORIZATION};
use reqwest_middleware::reqwest::Method;
use reqwest_middleware::ClientBuilder;
use reqwest_middleware::ClientWithMiddleware;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use url::Url;

#[derive(Clone)]
pub struct HttpClient {
    client: ClientWithMiddleware,
    base: Url,
    auth: Option<(String, String)>,
    bearer_token: Option<String>,
}

impl HttpClient {
    pub fn with_client(
        base: Url,
        client: ClientWithMiddleware,
        auth: Option<(String, String)>,
    ) -> Self {
        Self { client, base, auth, bearer_token: None }
    }

    pub fn with_client_and_bearer(
        base: Url,
        client: ClientWithMiddleware,
        bearer_token: String,
    ) -> Self {
        Self { client, base, auth: None, bearer_token: Some(bearer_token) }
    }

    pub fn from_url(base: Url, auth: Option<(String, String)>) -> anyhow::Result<Self> {
        let inner = ClientBuilder::new(reqwest::Client::builder().build()?).build();
        Ok(Self::with_client(base, inner, auth))
    }

    pub fn from_base_url(base: &str, auth: Option<(String, String)>) -> anyhow::Result<Self> {
        let url = Url::parse(base)?;
        Self::from_url(url, auth)
    }

    pub fn from_base_url_with_bearer_auth(base: &str, bearer_token: &str) -> anyhow::Result<Self> {
        let url = Url::parse(base)?;
        let inner = ClientBuilder::new(reqwest::Client::builder().build()?).build();
        Ok(Self::with_client_and_bearer(url, inner, bearer_token.to_string()))
    }

    pub async fn send<Req, Res>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Req>,
    ) -> Result<Res, reqwest_middleware::Error>
    where
        Req: Serialize + ?Sized,
        Res: DeserializeOwned,
    {
        let url = self.base.join(path).expect("valid relative path");
        let mut builder = self.client.request(method.clone(), url.clone());
        if let Some((ref user, ref pass)) = self.auth {
            builder = builder.basic_auth(user, Some(pass));
        } else if let Some(ref token) = self.bearer_token {
            builder = builder.header(AUTHORIZATION, format!("Bearer {}", token));
        }

        // Prepare JSON body manually so we can log it before sending
        let mut logged_body: Option<String> = None;
        if let Some(b) = body {
            match serde_json::to_vec(b) {
                Ok(bytes) => {
                    logged_body = Some(String::from_utf8_lossy(&bytes).to_string());
                    builder = builder.header(CONTENT_TYPE, "application/json").body(bytes);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to serialize request body for logging");
                    // Proceed without logging the body and let the server handle missing/invalid body
                }
            }
        }
        // Build the request to inspect headers and log before sending
        let request = builder
            .build()
            .map_err(|e: reqwest::Error| reqwest_middleware::Error::from(e))?; // Log outgoing request (method, url, headers, body)
        let formatted_headers = format_headers(request.headers());
        if let Some(ref body_str) = logged_body {
            tracing::debug!( method = %method, url = %request.url(), headers = %formatted_headers, body = %truncate(body_str, 4096), "outgoing HTTP request" );
        } else {
            tracing::debug!( method = %method, url = %request.url(), headers = %formatted_headers, "outgoing HTTP request" );
        }
        let resp = self.client.execute(request).await?; // If status is an error, log body and return the original error
        if let Err(err) = resp.error_for_status_ref() {
            let status = resp.status();
            match resp.bytes().await {
                Ok(bytes) => {
                    let logged = match serde_json::from_slice::<Value>(&bytes) {
                        Ok(json) => json.to_string(),
                        Err(_) => String::from_utf8_lossy(&bytes).to_string(),
                    };
                    tracing::error!( status = %status, body = %truncate(&logged, 4096), "HTTP error response" );
                }
                Err(e) => {
                    tracing::error!( status = %status, source = %e, body = "<failed to read body>", "HTTP error response" );
                }
            }
            return Err(err.into());
        }
        let bytes = resp.bytes().await?;
        serde_json::from_slice::<Res>(&bytes)
            .map_err(|e| reqwest_middleware::Error::from(anyhow!(e)))
    }

    pub async fn send_with_auth_header<Req, Res>(
        &self,
        method: Method,
        path: &str,
        body: Option<&Req>,
        auth_header: &str,
    ) -> Result<Res, reqwest_middleware::Error>
    where
        Req: Serialize + ?Sized,
        Res: DeserializeOwned,
    {
        let url = self.base.join(path).expect("valid relative path");
        let mut builder = self.client.request(method.clone(), url.clone());
        
        // Set custom authorization header
        builder = builder.header(AUTHORIZATION, auth_header);

        // Prepare JSON body manually so we can log it before sending
        let mut logged_body: Option<String> = None;
        if let Some(b) = body {
            match serde_json::to_vec(b) {
                Ok(bytes) => {
                    logged_body = Some(String::from_utf8_lossy(&bytes).to_string());
                    builder = builder.header(CONTENT_TYPE, "application/json").body(bytes);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to serialize request body for logging");
                    // Proceed without logging the body and let the server handle missing/invalid body
                }
            }
        }
        // Build the request to inspect headers and log before sending
        let request = builder
            .build()
            .map_err(|e: reqwest::Error| reqwest_middleware::Error::from(e))?;
        let formatted_headers = format_headers(request.headers());
        if let Some(ref body_str) = logged_body {
            tracing::debug!( method = %method, url = %request.url(), headers = %formatted_headers, body = %truncate(body_str, 4096), "outgoing HTTP request with custom auth" );
        } else {
            tracing::debug!( method = %method, url = %request.url(), headers = %formatted_headers, "outgoing HTTP request with custom auth" );
        }
        let resp = self.client.execute(request).await?;
        if let Err(err) = resp.error_for_status_ref() {
            let status = resp.status();
            match resp.bytes().await {
                Ok(bytes) => {
                    let logged = match serde_json::from_slice::<Value>(&bytes) {
                        Ok(json) => json.to_string(),
                        Err(_) => String::from_utf8_lossy(&bytes).to_string(),
                    };
                    tracing::error!( status = %status, body = %truncate(&logged, 4096), "HTTP error response with custom auth" );
                }
                Err(e) => {
                    tracing::error!( status = %status, source = %e, body = "<failed to read body>", "HTTP error response with custom auth" );
                }
            }
            return Err(err.into());
        }
        let bytes = resp.bytes().await?;
        serde_json::from_slice::<Res>(&bytes)
            .map_err(|e| reqwest_middleware::Error::from(anyhow!(e)))
    }
}

fn format_headers(headers: &HeaderMap) -> String {
    headers
        .iter()
        .map(|(name, value)| format!("{}: {}", name, value.to_str().unwrap_or("<invalid>")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}
