use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

use axum::{debug_handler, http::header::SET_COOKIE, response::AppendHeaders};
use loco_rs::{model::ModelError, prelude::*};

use crate::{
    controllers::stytch_guard::StytchAuth,
    data::{
        solid_server::{SolidServerClient, SolidServerSettings, CreateUserPodParams},
        stytch::{PasswordAuthParams, StytchClient},
    },
    models::{
        pods::{self, CreatePodParams},
        users::{self, LoginParams, RegisterParams, UpsertFromStytch},
    },
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

    // Automatically provision pod for new user
    provision_pod_for_user(&ctx, user_id, &params.email, &params.password).await;

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

/// Provisions a pod for a newly registered user
/// Does not fail registration if pod provisioning fails - logs error instead
async fn provision_pod_for_user(ctx: &AppContext, user_id: Uuid, email: &str, password: &str) {
    // Check if user already has a pod (shouldn't happen, but good to check)
    match pods::Model::user_has_pod(&ctx.db, user_id).await {
        Ok(true) => {
            tracing::info!(user_id = %user_id, "User already has a pod, skipping provisioning");
            return;
        }
        Ok(false) => {
            // Continue with provisioning
        }
        Err(err) => {
            tracing::error!(
                user_id = %user_id,
                error = %err,
                "Failed to check if user has pod, skipping pod provisioning"
            );
            return;
        }
    }

    // Generate a default pod name based on email
    let pod_name = email
        .split('@')
        .next()
        .unwrap_or("mypod")
        .to_string();

    match provision_pod_internal(ctx, user_id, email, password, &pod_name).await {
        Ok(()) => {
            tracing::info!(
                user_id = %user_id,
                email = %email,
                pod_name = %pod_name,
                "Successfully provisioned pod for new user"
            );
        }
        Err(err) => {
            tracing::error!(
                user_id = %user_id,
                email = %email,
                error = %err,
                "Failed to provision pod for new user - user registration succeeded but pod creation failed"
            );
        }
    }
}

/// Internal pod provisioning logic
async fn provision_pod_internal(
    ctx: &AppContext,
    user_id: Uuid,
    email: &str,
    password: &str,
    pod_name: &str,
) -> Result<()> {
    // Create pod on CSS with full provisioning flow
    let settings = SolidServerSettings::from_config(&ctx.config)?;
    let client = SolidServerClient::new(settings)?;
    
    let css_params = CreateUserPodParams {
        email,
        password,
        pod_name,
    };
    
    let css_result = client.create_user_and_pod(css_params).await?;
    
    // Create pod parameters for database insertion
    let create_params = CreatePodParams {
        name: pod_name.to_string(),
        email: email.to_string(),
        password: password.to_string(),
    };
    
    // Create pod in database with CSS provisioning data
    pods::Model::create_with_css_data(&ctx.db, user_id, &create_params, &css_result).await?;
    
    Ok(())
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/auth")
        .add("/register", post(register))
        .add("/login", post(login))
        .add("/current", get(current))
        .add("/logout", post(logout))
}
