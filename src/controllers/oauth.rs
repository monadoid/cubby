use serde::{Deserialize, Serialize};

use axum::extract::State;
use loco_rs::prelude::*;

use crate::{
    controllers::oauth_helpers::stytch_client,
    data::stytch::{StytchTokenExchangeRequest, GrantType},
};

#[derive(Debug, Deserialize)]
pub struct TokenParams {
    pub grant_type: String,
    pub code: String,
    pub redirect_uri: String,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub code_verifier: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u32,
    pub scope: Option<String>,
}

async fn token_post(
    State(ctx): State<AppContext>,
    Json(params): Json<TokenParams>,
) -> Result<Response> {
    // Validate grant type
    if params.grant_type != GrantType::AuthorizationCode.as_str() {
        return Err(Error::BadRequest("unsupported_grant_type".to_string()));
    }

    let client = stytch_client(&ctx)?;

    // Create strongly typed request for Stytch
    let token_request = StytchTokenExchangeRequest {
        grant_type: params.grant_type,
        code: params.code,
        redirect_uri: params.redirect_uri,
        client_id: params.client_id,
        client_secret: params.client_secret,
        code_verifier: params.code_verifier,
    };

    // Make token exchange request to Stytch
    let stytch_response = client.token_exchange(&token_request).await.map_err(|e| {
        tracing::error!(error = ?e, "failed to exchange token with Stytch");
        Error::InternalServerError
    })?;

    // Create response using strongly typed data
    let response = TokenResponse {
        access_token: stytch_response.access_token,
        token_type: stytch_response.token_type,
        expires_in: stytch_response.expires_in,
        scope: stytch_response.scope,
    };

    format::json(response)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("oauth")
        .add("/token", post(token_post))
}