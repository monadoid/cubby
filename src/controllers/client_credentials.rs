use std::sync::Arc;

use axum::{debug_handler, extract::Path};
use loco_rs::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    controllers::stytch_guard::StytchAuth,
    data::stytch::{CreateM2mClientParams, StytchClient},
    models::client_credentials::{self, CreateParams, UpdateSecretParams},
};

fn stytch_client(ctx: &AppContext) -> Result<Arc<StytchClient>> {
    ctx.shared_store.get::<Arc<StytchClient>>().ok_or_else(|| {
        tracing::error!("stytch client not initialised");
        Error::InternalServerError
    })
}


#[derive(Debug, Deserialize, Serialize)]
struct CreateClientPayload {
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub client_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Serialize)]
struct ClientCredentialResponse {
    pub id: Uuid,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_last_four: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub scopes: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ClientCredentialResponse {
    fn from_model(model: &client_credentials::Model) -> Self {
        Self {
            id: model.id,
            client_id: model.client_id.clone(),
            client_secret: None,
            client_secret_last_four: model.client_secret_last_four.clone(),
            description: model.description.clone(),
            scopes: model.scopes(),
            status: model.status.clone(),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }

    fn with_secret(mut self, secret: Option<String>) -> Self {
        self.client_secret = secret;
        self
    }
}

#[derive(Debug, Serialize)]
struct ClientCredentialListItem {
    pub id: Uuid,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_last_four: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub scopes: Vec<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&client_credentials::Model> for ClientCredentialListItem {
    fn from(model: &client_credentials::Model) -> Self {
        Self {
            id: model.id,
            client_id: model.client_id.clone(),
            client_secret_last_four: model.client_secret_last_four.clone(),
            description: model.description.clone(),
            scopes: model.scopes(),
            status: model.status.clone(),
            created_at: model.created_at.to_rfc3339(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
struct RotateSecretResponse {
    pub id: Uuid,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret_last_four: Option<String>,
    pub scopes: Vec<String>,
    pub status: String,
    pub updated_at: String,
}

impl RotateSecretResponse {
    fn from_model(model: &client_credentials::Model, secret: Option<String>) -> Self {
        Self {
            id: model.id,
            client_id: model.client_id.clone(),
            client_secret: secret,
            client_secret_last_four: model.client_secret_last_four.clone(),
            scopes: model.scopes(),
            status: model.status.clone(),
            updated_at: model.updated_at.to_rfc3339(),
        }
    }
}


#[debug_handler]
async fn create(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Json(payload): Json<CreateClientPayload>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let stytch = stytch_client(&ctx)?;

    let metadata = json!({
        "user_id": auth.auth_id,
    });

    let envelope = stytch
        .create_m2m_client(CreateM2mClientParams {
            scopes: &payload.scopes,
            trusted_metadata: metadata,
            client_name: payload.client_name.as_deref(),
            client_description: payload.description.as_deref(),
        })
        .await?;

    let client = &envelope.client;

    let record = client_credentials::create(
        &ctx.db,
        CreateParams {
            user_id: user_id,
            client_id: &client.client_id,
            client_secret_last_four: client.client_secret_last_four.as_deref(),
            description: client.client_description.as_deref(),
            scopes: &client.scopes,
            status: &client.status,
            trusted_metadata: client.trusted_metadata.as_ref(),
        },
    )
    .await?;

    let response =
        ClientCredentialResponse::from_model(&record).with_secret(client.client_secret.clone());
    format::json(response)
}

#[debug_handler]
async fn list(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    let user_id = auth.user_id;
    let records = client_credentials::Model::list_for_user(&ctx.db, user_id).await?;
    let payload: Vec<ClientCredentialListItem> =
        records.iter().map(ClientCredentialListItem::from).collect();
    format::json(payload)
}

#[debug_handler]
async fn rotate(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let record = client_credentials::Model::find_by_id_and_user(&ctx.db, id, user_id).await?;
    let stytch = stytch_client(&ctx)?;

    let envelope = stytch.rotate_m2m_client_secret(&record.client_id).await?;
    let client = envelope.client;

    let updated = client_credentials::update_secret(
        &ctx.db,
        UpdateSecretParams {
            id,
            user_id: user_id,
            client_secret_last_four: client.client_secret_last_four.as_deref(),
            status: Some(&client.status),
        },
    )
    .await?;

    let response = RotateSecretResponse::from_model(&updated, client.client_secret.clone());
    format::json(response)
}

#[debug_handler]
async fn delete_client(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let record = client_credentials::Model::find_by_id_and_user(&ctx.db, id, user_id).await?;
    let stytch = stytch_client(&ctx)?;

    match stytch.delete_m2m_client(&record.client_id).await {
        Ok(_) | Err(Error::NotFound) => {}
        Err(err) => return Err(err),
    }

    client_credentials::Model::delete_by_id_and_user(&ctx.db, id, user_id).await?;
    format::empty()
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/me/clients")
        .add("/create", post(create))
        .add("/list", get(list))
        .add("/{id}/rotate", post(rotate))
        .add("/{id}", delete(delete_client))
}
