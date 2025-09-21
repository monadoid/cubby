use std::{collections::HashSet, sync::Arc};

use axum::{
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use loco_rs::{app::AppContext, controller::extractor::shared_store::SharedStore, Error};

use crate::data::stytch::{extract_user_id, StytchClient};

pub struct StytchAuth {
    pub client_id: String,
    pub scopes: Vec<String>,
    pub custom_claims: serde_json::Map<String, serde_json::Value>,
    pub auth_id: String,
    pub user_id: uuid::Uuid,
}

impl FromRequestParts<AppContext> for StytchAuth {
    type Rejection = Error;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppContext,
    ) -> impl std::future::Future<Output = Result<Self, Self::Rejection>> + Send {
        async move {
            let SharedStore(client) =
                SharedStore::<Arc<StytchClient>>::from_request_parts(parts, state).await?;

            let header = parts
                .headers
                .get(AUTHORIZATION)
                .ok_or_else(|| Error::Unauthorized("missing bearer token".to_string()))?;
            let header_str = header
                .to_str()
                .map_err(|_| Error::Unauthorized("invalid authorization header".to_string()))?;
            let token = header_str
                .strip_prefix("Bearer ")
                .or_else(|| header_str.strip_prefix("bearer "))
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| Error::Unauthorized("invalid bearer token".to_string()))?;

            let validated = match client.validate(token).await {
                Ok(resp) => resp,
                Err(err) => {
                    tracing::error!(error = ?err, "failed to authenticate token with Stytch");
                    return Err(err);
                }
            };

            if !client.required_scopes().is_empty() {
                let granted: HashSet<&str> = validated
                    .scopes
                    .iter()
                    .map(|scope| scope.as_str())
                    .collect();

                if client
                    .required_scopes()
                    .iter()
                    .any(|required| !granted.contains(required.as_str()))
                {
                    return Err(Error::Unauthorized(
                        "access token missing required scope".to_string(),
                    ));
                }
            }

            let auth_id_value = extract_user_id(&validated.claims).ok_or_else(|| {
                Error::Unauthorized("missing user binding in access token".to_string())
            })?;

            if auth_id_value.is_empty() {
                return Err(Error::Unauthorized("invalid user binding".to_string()));
            }

            // Extract database user_id from the token
            let user_id_str = validated.claims.user_id.as_deref().ok_or_else(|| {
                Error::Unauthorized("missing user_id in access token".to_string())
            })?;
            let user_id = uuid::Uuid::parse_str(user_id_str).map_err(|_| {
                Error::Unauthorized("invalid user_id format in access token".to_string())
            })?;

            Ok(Self {
                client_id: validated.claims.sub.clone().unwrap_or_default(),
                scopes: validated.scopes,
                custom_claims: validated.claims.custom_claims.clone(),
                auth_id: auth_id_value.to_string(),
                user_id,
            })
        }
    }
}
