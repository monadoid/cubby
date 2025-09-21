use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use loco_rs::{config::Config, Error, Result};
use reqwest_middleware::reqwest::{Error as ReqwestError, Method, StatusCode};
use reqwest_middleware::Error as MiddlewareError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    error::Error as StdError,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

use crate::data::http::HttpClient;

fn default_base_url() -> String {
    "https://test.stytch.com/v1/".to_string()
}

fn default_jwks_path() -> String {
    String::new()
}

#[derive(Debug, Clone, Deserialize)]
pub struct StytchSettings {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    pub project_id: String,
    pub secret: String,
    #[serde(default = "default_jwks_path")]
    pub jwks_path: String,
    #[serde(default)]
    pub issuer: Option<String>,
    #[serde(default)]
    pub audience: Vec<String>,
    #[serde(default)]
    pub required_scopes: Vec<String>,
    #[serde(default)]
    pub jwks_ttl_seconds: Option<u64>,
}

impl StytchSettings {
    pub fn from_config(config: &Config) -> Result<Self> {
        let settings = config
            .settings
            .as_ref()
            .and_then(|value| value.get("stytch"))
            .ok_or_else(|| Error::Message("missing Stytch settings".to_string()))?;

        let parsed: StytchSettings =
            serde_json::from_value(settings.clone()).map_err(Error::from)?;

        if parsed.project_id.is_empty() {
            return Err(Error::Message("stytch.project_id is required".to_string()));
        }

        Ok(parsed)
    }
}

#[derive(Debug, Clone)]
pub struct PasswordAuthParams<'a> {
    pub email: &'a str,
    pub password: &'a str,
    pub session_duration_minutes: Option<u32>,
    pub trusted_metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CreateM2mClientParams<'a> {
    pub scopes: &'a [String],
    pub trusted_metadata: Value,
    pub client_name: Option<&'a str>,
    pub client_description: Option<&'a str>,
}

#[derive(Debug, Clone, Serialize)]
struct PasswordAuthRequest<'a> {
    email: &'a str,
    password: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_duration_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trusted_metadata: Option<&'a Value>,
}

#[derive(Clone)]
pub struct StytchClient {
    http: HttpClient,
    jwks_path: String,
    required_scopes: Vec<String>,
    expected_issuer: String,
    expected_audience: Vec<String>,
    jwks_cache: Arc<RwLock<Option<CachedJwks>>>,
    jwks_ttl: Duration,
}

impl StytchClient {
    pub fn new(settings: StytchSettings) -> Result<Self> {
        let http = HttpClient::from_base_url(
            &settings.base_url,
            Some((settings.project_id.clone(), settings.secret.clone())),
        )
        .map_err(|err| Error::Message(err.to_string()))?;

        let expected_issuer = settings
            .issuer
            .unwrap_or_else(|| format!("stytch.com/{}", settings.project_id));

        let expected_audience = if settings.audience.is_empty() {
            vec![settings.project_id.clone()]
        } else {
            settings.audience
        };

        let ttl = Duration::from_secs(settings.jwks_ttl_seconds.unwrap_or(300));

        let jwks_path = if settings.jwks_path.is_empty() {
            format!("sessions/jwks/{}", settings.project_id)
        } else {
            settings.jwks_path
        };

        Ok(Self {
            http,
            jwks_path,
            required_scopes: settings.required_scopes,
            expected_issuer,
            expected_audience,
            jwks_cache: Arc::new(RwLock::new(None)),
            jwks_ttl: ttl,
        })
    }

    pub fn required_scopes(&self) -> &[String] {
        &self.required_scopes
    }

    pub async fn login_or_create_password(
        &self,
        params: PasswordAuthParams<'_>,
    ) -> Result<PasswordAuthResponse> {
        let trusted_metadata = params.trusted_metadata.as_ref();
        
        let body = PasswordAuthRequest {
            email: params.email,
            password: params.password,
            session_duration_minutes: params.session_duration_minutes,
            trusted_metadata,
        };

        self.http
            .send(Method::POST, "passwords", Some(&body))
            .await
            .map_err(|err| map_stytch_error(err, "failed to create Stytch user"))
    }

    pub async fn authenticate_password(
        &self,
        params: PasswordAuthParams<'_>,
    ) -> Result<PasswordAuthResponse> {
        let trusted_metadata = params.trusted_metadata.as_ref();
        
        let body = PasswordAuthRequest {
            email: params.email,
            password: params.password,
            session_duration_minutes: params.session_duration_minutes,
            trusted_metadata,
        };

        self.http
            .send(Method::POST, "passwords/authenticate", Some(&body))
            .await
            .map_err(|err| map_stytch_error(err, "failed to authenticate Stytch password"))
    }

    pub async fn create_m2m_client(
        &self,
        params: CreateM2mClientParams<'_>,
    ) -> Result<M2mClientEnvelope> {
        #[derive(Serialize)]
        struct CreateRequest<'a> {
            scopes: &'a [String],
            #[serde(skip_serializing_if = "Option::is_none")]
            client_name: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            client_description: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            trusted_metadata: Option<&'a Value>,
        }

        let trusted_metadata = if params.trusted_metadata.is_null() {
            None
        } else {
            Some(&params.trusted_metadata)
        };

        let body = CreateRequest {
            scopes: params.scopes,
            client_name: params.client_name,
            client_description: params.client_description,
            trusted_metadata,
        };

        self.http
            .send(Method::POST, "m2m/clients", Some(&body))
            .await
            .map_err(|err| map_stytch_error(err, "failed to create Stytch client"))
    }

    pub async fn rotate_m2m_client_secret(&self, client_id: &str) -> Result<M2mClientEnvelope> {
        let path = format!("m2m/clients/{client_id}/secret/rotate");
        self.http
            .send::<(), M2mClientEnvelope>(Method::POST, &path, None::<&()>)
            .await
            .map_err(|err| map_stytch_error(err, "failed to rotate client secret"))
    }

    pub async fn delete_m2m_client(&self, client_id: &str) -> Result<()> {
        let path = format!("m2m/clients/{client_id}");
        let _: BasicResponse = self
            .http
            .send(Method::DELETE, &path, None::<&()>)
            .await
            .map_err(|err| map_stytch_error(err, "failed to delete client"))?;
        Ok(())
    }

    pub async fn validate(&self, token: &str) -> Result<ValidatedToken> {
        let header = decode_header(token).map_err(|err| {
            tracing::warn!(error = %err, "failed to parse JWT header");
            Error::Unauthorized("invalid access token".to_string())
        })?;

        let kid = header.kid.ok_or_else(|| {
            tracing::warn!("JWT missing kid header");
            Error::Unauthorized("invalid access token".to_string())
        })?;

        let jwk = self.get_key(&kid).await?;
        let decoding_key = jwk
            .to_decoding_key()
            .map_err(|err| Error::Message(format!("invalid JWK components: {err}")))?;

        let mut validation = Validation::new(Algorithm::RS256);
        let issuer = [self.expected_issuer.as_str()];
        validation.set_issuer(&issuer);

        if !self.expected_audience.is_empty() {
            let audience_refs: Vec<&str> = self
                .expected_audience
                .iter()
                .map(|value| value.as_str())
                .collect();
            validation.set_audience(&audience_refs);
        }

        let token_data =
            decode::<TokenClaims>(token, &decoding_key, &validation).map_err(|err| {
                tracing::warn!(error = %err, "failed to validate JWT");
                Error::Unauthorized("invalid access token".to_string())
            })?;

        let claims = token_data.claims;
        let scopes = claims
            .scope
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        Ok(ValidatedToken { claims, scopes })
    }

    async fn get_key(&self, kid: &str) -> Result<Jwk> {
        let mut cache_guard = self.jwks_cache.write().await;

        let refresh_needed = match &*cache_guard {
            Some(cache) => cache.is_expired(self.jwks_ttl),
            None => true,
        };

        if refresh_needed {
            let jwks = self.fetch_jwks().await?;
            *cache_guard = Some(CachedJwks::new(jwks));
        }

        if let Some(cache) = &*cache_guard {
            if let Some(key) = cache.jwks.find(kid) {
                return Ok(key.clone());
            }
        }

        // Key not found – refresh once to handle rotations
        let jwks = self.fetch_jwks().await?;
        *cache_guard = Some(CachedJwks::new(jwks));

        if let Some(cache) = cache_guard.as_ref() {
            if let Some(key) = cache.jwks.find(kid) {
                return Ok(key.clone());
            }
        }

        Err(Error::Unauthorized("invalid access token".to_string()))
    }

    async fn fetch_jwks(&self) -> Result<Jwks> {
        tracing::debug!("refreshing Stytch JWKS");
        self.http
            .send::<(), Jwks>(Method::GET, &self.jwks_path, None::<&()>)
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "failed to fetch Stytch JWKS");
                Error::InternalServerError
            })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PasswordAuthResponse {
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub session_token: Option<String>,
    #[serde(default)]
    pub session_jwt: Option<String>,
    pub user_id: String,
    #[serde(default)]
    pub session: Option<SessionResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SessionResponse {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct M2mClientEnvelope {
    #[serde(rename = "m2m_client")]
    pub client: M2mClient,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub status_code: Option<u16>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct M2mClient {
    pub client_id: String,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub client_description: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
    #[serde(default)]
    pub client_secret_last_four: Option<String>,
    #[serde(default)]
    pub next_client_secret_last_four: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub status: String,
    #[serde(default)]
    pub trusted_metadata: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BasicResponse {
    #[serde(default)]
    pub status_code: Option<u16>,
    #[serde(default)]
    pub request_id: Option<String>,
}

fn map_stytch_error(err: MiddlewareError, context: &'static str) -> Error {
    let status = extract_status(&err);
    tracing::error!(error = %err, ?status, context, "stytch request failed");
    
    // Try to extract more detailed error information
    let error_details = format!("{}: {} (status: {:?})", context, err, status);
    
    match status {
        Some(StatusCode::UNAUTHORIZED) => {
            Error::Unauthorized("unauthorized credentials".to_string())
        }
        Some(StatusCode::NOT_FOUND) => Error::NotFound,
        _ => Error::BadRequest(error_details),
    }
}

fn extract_status(err: &MiddlewareError) -> Option<StatusCode> {
    err.source()
        .and_then(|source| source.downcast_ref::<ReqwestError>())
        .and_then(|req_err| req_err.status())
}

#[derive(Debug, Clone)]
struct CachedJwks {
    jwks: Jwks,
    fetched_at: Instant,
}

impl CachedJwks {
    fn new(jwks: Jwks) -> Self {
        Self {
            jwks,
            fetched_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.fetched_at.elapsed() >= ttl
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

impl Jwks {
    fn find(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|key| key.kid == kid)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Jwk {
    kid: String,
    kty: String,
    #[serde(rename = "n")]
    n: String,
    #[serde(rename = "e")]
    e: String,
}

impl Jwk {
    fn to_decoding_key(&self) -> Result<DecodingKey> {
        if self.kty != "RSA" {
            return Err(Error::Message(format!("unsupported kty: {}", self.kty)));
        }

        DecodingKey::from_rsa_components(&self.n, &self.e)
            .map_err(|err| Error::Message(format!("failed to build decoding key: {err}")))
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenClaims {
    pub iss: String,
    #[serde(default, deserialize_with = "deserialize_audience")]
    pub aud: Vec<String>,
    pub exp: u64,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub sub: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub custom_claims: Map<String, Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

pub struct ValidatedToken {
    pub claims: TokenClaims,
    pub scopes: Vec<String>,
}

fn deserialize_audience<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Audience {
        Single(String),
        Multiple(Vec<String>),
    }

    match Audience::deserialize(deserializer)? {
        Audience::Single(value) => Ok(vec![value]),
        Audience::Multiple(values) => Ok(values),
    }
}

pub fn extract_user_id(claims: &TokenClaims) -> Option<&str> {
    // M2M tokens should have user_id field from trusted_metadata template
    claims.user_id.as_deref()
}
