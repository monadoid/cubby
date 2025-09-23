use serde::{Deserialize, Serialize};
use uuid::Uuid;

use axum::{
    extract::{Query, State},
    Form,
};
use loco_rs::{prelude::*, controller::views::engines::TeraView};

use crate::{
    controllers::{
        stytch_guard::StytchSessionAuth,
        oauth_helpers::{stytch_client, oauth_state_store},
    },
    data::stytch::{StytchAuthorizeStartRequest, StytchAuthorizeRequest, ResponseType},
    models::users,
};

#[derive(Debug, Deserialize)]
pub struct AuthorizeParams {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub nonce: Option<String>,
    pub prompt: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AuthorizeResponse {
    pub app_name: String,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub nonce: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConsentSubmitParams {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub nonce: Option<String>,
    pub approved: String, // "true" or "false"
}

async fn authorize_get(
    State(ctx): State<AppContext>,
    auth: StytchSessionAuth,
    Query(params): Query<AuthorizeParams>,
    ViewEngine(v): ViewEngine<TeraView>,
) -> Result<Response> {
    let client = stytch_client(&ctx)?;
    let state_store = oauth_state_store(&ctx)?;

    // Validate OAuth parameters
    if params.response_type != ResponseType::Code.as_str() {
        return Err(Error::BadRequest("unsupported_response_type".to_string()));
    }

    // PKCE validation - require code_challenge and code_challenge_method for security
    if params.code_challenge.is_none() || params.code_challenge_method.is_none() {
        return Err(Error::BadRequest("PKCE parameters (code_challenge and code_challenge_method) are required".to_string()));
    }
    
    // Validate PKCE method
    if let Some(ref method) = params.code_challenge_method {
        if method != "S256" {
            return Err(Error::BadRequest("only S256 code_challenge_method is supported".to_string()));
        }
    }

    // Generate state if not provided, or validate if provided
    let state = match params.state.as_ref() {
        Some(s) if s.is_empty() => {
            return Err(Error::BadRequest("state parameter cannot be empty".to_string()));
        }
        Some(s) => s.clone(),
        None => {
            // Generate a new state value
            Uuid::new_v4().to_string()
        }
    };

    // Parse scopes
    let scopes: Vec<String> = params.scope
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    // Prepare authorize start request for Stytch - match API docs exactly
    let stytch_request = StytchAuthorizeStartRequest {
        client_id: params.client_id.clone(),
        redirect_uri: params.redirect_uri.clone(),
        response_type: params.response_type.clone(),
        scopes: params.scope.split_whitespace().map(|s| s.to_string()).collect(),
        session_jwt: auth.session_jwt.clone(),
        prompt: params.prompt.clone(),
    };

    // Call Stytch authorize/start endpoint
    let authorize_response = client.authorize_start(&stytch_request).await.map_err(|e| {
        tracing::error!(error = ?e, "failed to start authorization with Stytch");
        Error::InternalServerError
    })?;

    // Store state in the state store for CSRF protection
    state_store.store_state(
        state.clone(),
        auth.user_id,
        params.client_id.clone(),
        params.redirect_uri.clone(),
        params.scope.clone(),
        params.code_challenge.clone(),
        params.code_challenge_method.clone(),
        params.nonce.clone(),
    ).await;

    let response_data = AuthorizeResponse {
        app_name: authorize_response.connected_app
            .as_ref()
            .map(|app| app.client_name.clone())
            .unwrap_or_else(|| "Unknown App".to_string()),
        scopes,
        client_id: params.client_id,
        redirect_uri: params.redirect_uri,
        state: Some(state),
        code_challenge: params.code_challenge,
        code_challenge_method: params.code_challenge_method,
        nonce: params.nonce,
    };

    // Get user from database
    let user = users::Model::find_by_id(&ctx.db, &auth.user_id.to_string()).await
        .map_err(|_| Error::Unauthorized("user not found".to_string()))?;

    format::render().view(&v, "oauth/authorize.html", data!({
        "app_name": response_data.app_name,
        "scopes": response_data.scopes,
        "client_id": response_data.client_id,
        "redirect_uri": response_data.redirect_uri,
        "state": response_data.state,
        "code_challenge": response_data.code_challenge,
        "code_challenge_method": response_data.code_challenge_method,
        "nonce": response_data.nonce,
        "user": {
            "email": user.email
        }
    }))
}

async fn consent_post(
    State(ctx): State<AppContext>,
    auth: StytchSessionAuth,
    Form(params): Form<ConsentSubmitParams>,
) -> Result<Response> {
    let client = stytch_client(&ctx)?;
    let state_store = oauth_state_store(&ctx)?;

    // Validate state parameter
    let state = params.state.as_ref()
        .ok_or_else(|| Error::BadRequest("missing state parameter".to_string()))?;
    
    // Verify and consume the state
    let _state_entry = state_store.verify_and_consume_state(
        state,
        auth.user_id,
        &params.client_id,
        &params.redirect_uri,
        &params.scope,
    ).await.ok_or_else(|| {
        tracing::warn!(state = %state, user_id = %auth.user_id, "invalid or expired OAuth state");
        Error::Unauthorized("invalid or expired state parameter".to_string())
    })?;

    // Check if user approved the authorization
    if params.approved != "true" {
        // User denied authorization - redirect with error
        use url::Url;
        
        let mut url = Url::parse(&params.redirect_uri)
            .map_err(|_| Error::BadRequest("invalid redirect_uri".to_string()))?;
        
        {
            let mut query_pairs = url.query_pairs_mut();
            query_pairs.append_pair("error", "access_denied");
            query_pairs.append_pair("error_description", "The user denied the request");
            
            if let Some(ref state) = params.state {
                query_pairs.append_pair("state", state);
            }
        }
        
        return format::redirect(url.as_str());
    }

    // Prepare authorize request for Stytch
    let stytch_request = StytchAuthorizeRequest {
        consent_granted: true, // User clicked approve
        scopes: params.scope.split_whitespace().map(|s| s.to_string()).collect(),
        client_id: params.client_id,
        redirect_uri: params.redirect_uri,
        response_type: params.response_type,
        session_jwt: Some(auth.session_jwt.clone()),
        user_id: None,
        session_token: None,
        prompt: None,
        state: params.state,
        nonce: params.nonce,
        code_challenge: params.code_challenge,
    };

    // Call Stytch authorize endpoint
    let authorize_response = client.authorize(&stytch_request).await.map_err(|e| {
        tracing::error!(error = ?e, "failed to authorize with Stytch");
        Error::InternalServerError
    })?;

    // Use the redirect_uri from the strongly typed response
    format::redirect(&authorize_response.redirect_uri)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("oauth")
        .add("/authorize", get(authorize_get))
        .add("/authorize", post(consent_post))
}