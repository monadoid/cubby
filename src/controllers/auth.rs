use std::sync::Arc;
use uuid::Uuid;
use serde_json::json;

use axum::debug_handler;
use loco_rs::{model::ModelError, prelude::*};

use crate::{
    controllers::stytch_guard::StytchAuth,
    data::stytch::{PasswordAuthParams, StytchClient},
    models::users::{self, LoginParams, RegisterParams, UpsertFromStytch},
    views::auth::{AuthResponse, CurrentResponse},
};

const DEFAULT_SESSION_DURATION: u32 = 60;

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
        "stytch_user_id": user_id.to_string()
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

    format::json(AuthResponse::new(&user, &stytch_response))
}

#[debug_handler]
async fn login(State(ctx): State<AppContext>, Json(params): Json<LoginParams>) -> Result<Response> {
    // First find the existing user to get their UUID
    let user = users::Model::find_by_email(&ctx.db, &params.email).await
        .map_err(|_| Error::Unauthorized("Invalid credentials".to_string()))?;

    let stytch = stytch_client(&ctx)?;

    let trusted_metadata = json!({
        "stytch_user_id": user.id.to_string()
    });

    let stytch_response = match stytch
        .authenticate_password(PasswordAuthParams {
            email: &params.email,
            password: &params.password,
            session_duration_minutes: Some(DEFAULT_SESSION_DURATION),
            trusted_metadata: Some(trusted_metadata),
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

    format::json(AuthResponse::new(&user, &stytch_response))
}

#[debug_handler]
async fn current(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    // auth.auth_id now contains our database UUID
    let user = users::Model::find_by_id(&ctx.db, &auth.auth_id).await?;
    format::json(CurrentResponse::new(&user))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/auth")
        .add("/register", post(register))
        .add("/login", post(login))
        .add("/current", get(current))
}
