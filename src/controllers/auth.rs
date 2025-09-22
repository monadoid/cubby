use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

use axum::{debug_handler, http::header::SET_COOKIE, response::AppendHeaders};
use loco_rs::{model::ModelError, prelude::*};

use crate::{
    controllers::stytch_guard::StytchAuth,
    data::stytch::{PasswordAuthParams, StytchClient},
    models::users::{self, LoginParams, RegisterParams, UpsertFromStytch},
    views::auth::{AuthResponse, CurrentResponse},
};

const DEFAULT_SESSION_DURATION: u32 = 60;

fn create_session_cookie(session_jwt: &str, max_age_minutes: u32) -> String {
    let max_age_seconds = max_age_minutes * 60;
    format!(
        "session_jwt={}; HttpOnly; Secure; SameSite=Strict; Max-Age={}; Path=/",
        session_jwt, max_age_seconds
    )
}

fn create_logout_cookie() -> String {
    "session_jwt=; HttpOnly; Secure; SameSite=Strict; Max-Age=0; Path=/".to_string()
}

fn stytch_client(ctx: &AppContext) -> Result<Arc<StytchClient>> {
    ctx.shared_store.get::<Arc<StytchClient>>().ok_or_else(|| {
        tracing::error!("stytch client not initialised");
        Error::InternalServerError
    })
}

#[debug_handler]
async fn register(
    State(ctx): State<AppContext>,
    Json(params): Json<RegisterParams>,
) -> Result<Response> {
    match users::Model::find_by_email(&ctx.db, &params.email).await {
        Ok(_) => {
            tracing::info!(email = %params.email, "register attempt for existing email");
            return Err(Error::BadRequest("User already exists".to_string()));
        }
        Err(ModelError::EntityNotFound) => {}
        Err(err) => return Err(err.into()),
    }

    // Generate our own UUID for the user
    let user_id = Uuid::new_v4();
    
    let stytch = stytch_client(&ctx)?;

    let trusted_metadata = json!({
        "user_id": user_id.to_string()
    });

    let stytch_response = stytch
        .login_or_create_password(PasswordAuthParams {
            email: &params.email,
            password: &params.password,
            session_duration_minutes: Some(DEFAULT_SESSION_DURATION),
            trusted_metadata: Some(trusted_metadata),
        })
        .await?;

    let user = users::Model::upsert_from_stytch(
        &ctx.db,
        UpsertFromStytch {
            id: user_id,
            auth_id: &stytch_response.user_id,
            email: &params.email,
        },
    )
    .await?;

    let response = AuthResponse::new(&user, &stytch_response);
    
    // Set session_jwt as httponly cookie if it exists
    if let Some(session_jwt) = &stytch_response.session_jwt {
        let cookie = create_session_cookie(session_jwt, DEFAULT_SESSION_DURATION);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            format::json(response)?,
        ).into_response())
    } else {
        format::json(response)
    }
}

#[debug_handler]
async fn login(State(ctx): State<AppContext>, Json(params): Json<LoginParams>) -> Result<Response> {
    // First find the existing user to get their UUID
    let user = users::Model::find_by_email(&ctx.db, &params.email).await
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

    let response = AuthResponse::new(&user, &stytch_response);
    
    // Set session_jwt as httponly cookie if it exists
    if let Some(session_jwt) = &stytch_response.session_jwt {
        let cookie = create_session_cookie(session_jwt, DEFAULT_SESSION_DURATION);
        Ok((
            AppendHeaders([(SET_COOKIE, cookie)]),
            format::json(response)?,
        ).into_response())
    } else {
        format::json(response)
    }
}

#[debug_handler]
async fn current(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    // auth.auth_id now contains our database UUID
    let user = users::Model::find_by_id(&ctx.db, &auth.auth_id).await?;
    format::json(CurrentResponse::new(&user))
}

#[debug_handler]
async fn logout() -> Result<Response> {
    let cookie = create_logout_cookie();
    Ok((
        AppendHeaders([(SET_COOKIE, cookie)]),
        format::json(serde_json::json!({"message": "Logged out successfully"}))?,
    ).into_response())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/auth")
        .add("/register", post(register))
        .add("/login", post(login))
        .add("/current", get(current))
        .add("/logout", post(logout))
}
