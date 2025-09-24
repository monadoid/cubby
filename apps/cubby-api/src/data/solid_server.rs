use loco_rs::{config::Config, Error, Result};
use reqwest_middleware::reqwest::Method;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::data::http::HttpClient;

fn default_base_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_account_index() -> String {
    "http://localhost:3000/.account/".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct SolidServerSettings {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default = "default_account_index")]
    pub account_index: String,
}

impl SolidServerSettings {
    pub fn from_config(config: &Config) -> Result<Self> {
        let settings = config
            .settings
            .as_ref()
            .and_then(|value| value.get("solid_server"))
            .ok_or_else(|| Error::Message("missing Solid Server settings".to_string()))?;

        let parsed: SolidServerSettings =
            serde_json::from_value(settings.clone()).map_err(Error::from)?;

        Ok(parsed)
    }
}

#[derive(Debug, Clone)]
pub struct CreateUserPodParams<'a> {
    pub email: &'a str,
    pub password: &'a str,
    pub pod_name: &'a str,
}

#[derive(Clone)]
pub struct SolidServerClient {
    http: HttpClient,
    settings: SolidServerSettings,
}

impl SolidServerClient {
    pub fn new(settings: SolidServerSettings) -> Result<Self> {
        // Create HTTP client without authentication - we'll authenticate per-request
        let http = HttpClient::from_base_url(&settings.account_index, None)
            .map_err(|err| Error::Message(err.to_string()))?;

        Ok(Self { http, settings })
    }

    /// Complete CSS account and pod creation flow
    /// Following: https://communitysolidserver.github.io/CommunitySolidServer/7.x/usage/account/json-api/
    pub async fn create_user_and_pod(
        &self,
        params: CreateUserPodParams<'_>,
    ) -> Result<CssProvisioningResult> {
        tracing::info!(
            email = %params.email,
            pod_name = %params.pod_name,
            "Starting CSS account and pod creation"
        );

        // Step 1: Discover controls (not needed for account creation, but good for validation)
        let _controls = self.discover_controls().await?;

        // Step 2: Create new account (unauthenticated)
        let account_result = self.create_account().await?;

        // Step 3: Get authenticated controls with account-specific URLs
        let auth_controls = self
            .get_authenticated_controls(&account_result.account_token)
            .await?;

        // Step 4: Add email/password login to the account
        self.add_password_login(
            &account_result.account_token,
            &auth_controls,
            params.email,
            params.password,
        )
        .await?;

        // Step 5: Create pod for the account
        let pod_result = self
            .create_pod(
                &account_result.account_token,
                &auth_controls,
                params.pod_name,
            )
            .await?;

        // Step 6: Create client credentials for server-to-server access
        let client_creds = self
            .create_client_credentials(
                &account_result.account_token,
                &auth_controls,
                &pod_result.web_id,
                params.pod_name,
            )
            .await?;

        Ok(CssProvisioningResult {
            account_token: account_result.account_token,
            pod_base_url: pod_result.base_url,
            web_id: pod_result.web_id,
            client_id: client_creds.id,
            client_secret: client_creds.secret,
            client_resource_url: client_creds.resource,
            css_email: params.email.to_string(),
        })
    }

    /// Step 1: Discover CSS account API controls
    async fn discover_controls(&self) -> Result<CssControls> {
        tracing::debug!("Discovering CSS account API controls");

        let response: CssControlsResponse = self
            .http
            .send(Method::GET, "", None::<&()>)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to discover CSS controls");
                Error::BadRequest(format!("Failed to discover CSS controls: {}", err))
            })?;

        Ok(response.controls)
    }

    /// Step 2: Create new CSS account
    async fn create_account(&self) -> Result<AccountCreationResult> {
        tracing::debug!("Creating new CSS account");

        // POST to account creation endpoint
        let response: AccountCreationResponse = self
            .http
            .send(Method::POST, "account/", Some(&serde_json::json!({})))
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to create CSS account");
                Error::BadRequest(format!("Failed to create CSS account: {}", err))
            })?;

        Ok(AccountCreationResult {
            account_token: response.authorization,
        })
    }

    /// Create authenticated HTTP client for CSS requests  
    fn create_auth_client(&self) -> Result<HttpClient> {
        HttpClient::from_base_url(&self.settings.account_index, None)
            .map_err(|err| Error::Message(err.to_string()))
    }

    /// Step 3: Get authenticated controls with account-specific URLs
    async fn get_authenticated_controls(&self, account_token: &str) -> Result<CssControls> {
        tracing::debug!("Getting authenticated CSS controls");

        let auth_http = self.create_auth_client()?;

        let response: CssControlsResponse = auth_http
            .send_with_auth_header(
                Method::GET,
                "",
                None::<&()>,
                &format!("CSS-Account-Token {}", account_token),
            )
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to get authenticated controls");
                Error::BadRequest(format!("Failed to get authenticated controls: {}", err))
            })?;

        Ok(response.controls)
    }

    /// Step 4: Add email/password login to account
    async fn add_password_login(
        &self,
        account_token: &str,
        auth_controls: &CssControls,
        email: &str,
        password: &str,
    ) -> Result<()> {
        tracing::debug!(email = %email, "Adding password login to CSS account");

        // Extract the account-specific password creation URL
        let password_create_url = auth_controls.password.create.as_ref().ok_or_else(|| {
            Error::BadRequest("No password create URL in authenticated controls".to_string())
        })?;
        let path = password_create_url
            .strip_prefix(&self.settings.account_index)
            .unwrap_or(password_create_url);

        let body = serde_json::json!({
            "email": email,
            "password": password
        });

        // Create authenticated HTTP client for this request
        let auth_http = self.create_auth_client()?;

        let _response: Value = auth_http
            .send_with_auth_header(
                Method::POST,
                path,
                Some(&body),
                &format!("CSS-Account-Token {}", account_token),
            )
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to add password login");
                Error::BadRequest(format!("Failed to add password login: {}", err))
            })?;

        tracing::debug!("Password login added successfully");
        Ok(())
    }

    /// Step 5: Create pod for the account
    async fn create_pod(
        &self,
        account_token: &str,
        auth_controls: &CssControls,
        pod_name: &str,
    ) -> Result<PodCreationResult> {
        tracing::debug!(pod_name = %pod_name, "Creating pod for CSS account");

        // Extract the account-specific pod creation URL
        let pod_create_url = auth_controls.account.pod.as_ref().ok_or_else(|| {
            Error::BadRequest("No pod create URL in authenticated controls".to_string())
        })?;
        let path = pod_create_url
            .strip_prefix(&self.settings.account_index)
            .unwrap_or(pod_create_url);

        let body = serde_json::json!({
            "name": pod_name
        });

        let auth_http = self.create_auth_client()?;

        let response: PodCreationResponse = auth_http
            .send_with_auth_header(
                Method::POST,
                path,
                Some(&body),
                &format!("CSS-Account-Token {}", account_token),
            )
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to create pod");
                Error::BadRequest(format!("Failed to create pod: {}", err))
            })?;

        Ok(PodCreationResult {
            base_url: response.pod,
            web_id: response.web_id,
        })
    }

    /// Step 6: Create client credentials for server-to-server access
    async fn create_client_credentials(
        &self,
        account_token: &str,
        auth_controls: &CssControls,
        web_id: &str,
        name: &str,
    ) -> Result<ClientCredentialsResult> {
        tracing::debug!(web_id = %web_id, "Creating client credentials");

        // Extract the account-specific client credentials creation URL
        let cc_create_url = auth_controls
            .account
            .client_credentials
            .as_ref()
            .ok_or_else(|| {
                Error::BadRequest(
                    "No client credentials create URL in authenticated controls".to_string(),
                )
            })?;
        let path = cc_create_url
            .strip_prefix(&self.settings.account_index)
            .unwrap_or(cc_create_url);

        let body = serde_json::json!({
            "name": format!("cubby-{}", name),
            "webId": web_id
        });

        let auth_http = self.create_auth_client()?;

        let response: ClientCredentialsResponse = auth_http
            .send_with_auth_header(
                Method::POST,
                path,
                Some(&body),
                &format!("CSS-Account-Token {}", account_token),
            )
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "Failed to create client credentials");
                Error::BadRequest(format!("Failed to create client credentials: {}", err))
            })?;

        Ok(ClientCredentialsResult {
            id: response.id,
            secret: response.secret,
            resource: response.resource,
        })
    }

    /// Delete a pod and its associated CSS account resources
    pub async fn delete_user_pod(
        &self,
        _account_token: &str,
        client_resource_url: &str,
    ) -> Result<()> {
        tracing::info!(client_resource_url = %client_resource_url, "Deleting CSS client credentials");

        // For now, just log the action - actual implementation would delete the client credentials
        tracing::info!("CSS client credentials deleted (stubbed)");
        Ok(())
    }
}

// Response types for CSS API calls
#[derive(Debug, Clone, Deserialize)]
pub struct CssControlsResponse {
    pub controls: CssControls,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CssControls {
    pub account: AccountControls,
    pub password: PasswordControls,
    // Ignore additional fields that might be present
    #[serde(flatten)]
    pub _extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountControls {
    pub create: String,
    #[serde(default)]
    pub pod: Option<String>,
    #[serde(default)]
    pub client_credentials: Option<String>,
    #[serde(default)]
    pub web_id: Option<String>,
    #[serde(default)]
    pub logout: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasswordControls {
    pub login: String,
    pub forgot: String,
    pub reset: String,
    #[serde(default)]
    pub create: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AccountCreationResponse {
    pub authorization: String,
    // Ignore additional fields that might be present
    #[serde(flatten)]
    pub _extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PodCreationResponse {
    pub pod: String,
    pub web_id: String,
    #[serde(default)]
    pub pod_resource: Option<String>,
    // Ignore additional fields that might be present
    #[serde(flatten)]
    pub _extra: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientCredentialsResponse {
    pub id: String,
    pub secret: String,
    pub resource: String,
}

// Result types for internal use
pub struct AccountCreationResult {
    pub account_token: String,
}

pub struct PodCreationResult {
    pub base_url: String,
    pub web_id: String,
}

pub struct ClientCredentialsResult {
    pub id: String,
    pub secret: String,
    pub resource: String,
}

#[derive(Debug, Clone)]
pub struct CssProvisioningResult {
    pub account_token: String,
    pub pod_base_url: String,
    pub web_id: String,
    pub client_id: String,
    pub client_secret: String,
    pub client_resource_url: String,
    pub css_email: String,
}
