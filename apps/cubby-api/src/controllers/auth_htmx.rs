#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::{debug_handler, extract::Query, http::header::SET_COOKIE, response::AppendHeaders};
use loco_rs::{controller::views::engines::TeraView, prelude::*};
use serde::{Deserialize, Serialize};

use crate::controllers::{
    oauth_helpers::{login_stash, stytch_client},
    stytch_guard::StytchSessionAuth,
};
use crate::data::stytch::PasswordAuthParams;
use crate::{models::users, views};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignUpParams {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginQuery {
    pub return_to: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginFormParams {
    pub email: String,
    pub password: String,
    pub return_to: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResumeParams {
    pub key: String,
}

#[debug_handler]
pub async fn sign_up_form(
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<Response> {
    views::auth_htmx::sign_up(&v)
}

#[debug_handler]
pub async fn dashboard(
    auth: StytchSessionAuth,
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<Response> {
    views::auth_htmx::dashboard(&v, &auth.user_id.to_string())
}

#[debug_handler]
pub async fn login_form(
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
    query: Query<LoginQuery>,
) -> Result<Response> {
    views::auth_htmx::login(&v, query.return_to.as_deref())
}

const DEFAULT_SESSION_DURATION: u32 = 60;

fn create_session_cookie(session_jwt: &str, max_age_minutes: u32) -> String {
    let max_age_seconds = max_age_minutes * 60;
    format!(
        "session_jwt={}; HttpOnly; Secure; SameSite=Strict; Max-Age={}; Path=/",
        session_jwt, max_age_seconds
    )
}

#[debug_handler]
pub async fn login_post(
    State(ctx): State<AppContext>,
    Json(params): Json<LoginFormParams>,
) -> Result<Response> {
    // First find the existing user to get their UUID
    let _user = users::Model::find_by_email(&ctx.db, &params.email)
        .await
        .map_err(|_| Error::Unauthorized("Invalid credentials".to_string()))?;

    let stytch = stytch_client(&ctx)?;

    let stytch_response = match stytch
        .authenticate_password(PasswordAuthParams {
            email: &params.email,
            password: &params.password,
            session_duration_minutes: Some(DEFAULT_SESSION_DURATION),
            trusted_metadata: None,
        })
        .await
    {
        Ok(resp) => resp,
        Err(Error::Unauthorized(_)) => {
            tracing::debug!(email = %params.email, "invalid login credentials");
            return unauthorized("Invalid credentials!");
        }
        Err(err) => return Err(err),
    };

    // Determine where to redirect after successful login
    let redirect_to = params.return_to.as_deref().unwrap_or("/dashboard");

    // Set session_jwt as httponly cookie if it exists
    if let Some(session_jwt) = &stytch_response.session_jwt {
        let cookie = create_session_cookie(session_jwt, DEFAULT_SESSION_DURATION);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            format::json(serde_json::json!({"redirect_to": redirect_to}))?,
        )
            .into_response())
    } else {
        format::json(serde_json::json!({"redirect_to": redirect_to}))
    }
}

#[debug_handler]
pub async fn resume(State(ctx): State<AppContext>, Path(key): Path<String>) -> Result<Response> {
    let stash = login_stash(&ctx)?;

    // Retrieve the OAuth parameters from the stash
    let oauth_params = match stash.retrieve_and_consume_oauth_params(&key).await {
        Some(params) => params,
        None => {
            tracing::warn!("Invalid or expired resume key: {}", key);
            return Err(Error::BadRequest("Invalid or expired session".to_string()));
        }
    };

    // Redirect to the authorize endpoint with the original OAuth parameters
    let mut redirect_url = "/oauth/authorize?".to_string();
    redirect_url.push_str(&format!(
        "client_id={}",
        urlencoding::encode(&oauth_params.client_id)
    ));
    redirect_url.push_str(&format!(
        "&redirect_uri={}",
        urlencoding::encode(&oauth_params.redirect_uri)
    ));
    redirect_url.push_str(&format!(
        "&response_type={}",
        urlencoding::encode(&oauth_params.response_type)
    ));
    redirect_url.push_str(&format!(
        "&scope={}",
        urlencoding::encode(&oauth_params.scope)
    ));
    redirect_url.push_str(&format!(
        "&code_challenge={}",
        urlencoding::encode(&oauth_params.code_challenge)
    ));
    redirect_url.push_str(&format!(
        "&code_challenge_method={}",
        urlencoding::encode(&oauth_params.code_challenge_method)
    ));

    if let Some(state) = &oauth_params.state {
        redirect_url.push_str(&format!("&state={}", urlencoding::encode(state)));
    }

    if let Some(nonce) = &oauth_params.nonce {
        redirect_url.push_str(&format!("&nonce={}", urlencoding::encode(nonce)));
    }

    if let Some(prompt) = &oauth_params.prompt {
        redirect_url.push_str(&format!("&prompt={}", urlencoding::encode(prompt)));
    }

    Ok(axum::response::Redirect::to(&redirect_url).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .add("/sign-up", get(sign_up_form))
        .add("/login", get(login_form))
        .add("/login", post(login_post))
        .add("/resume/{key}", get(resume))
        .add("/dashboard", get(dashboard))
}
