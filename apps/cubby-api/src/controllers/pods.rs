#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::debug_handler;
use loco_rs::prelude::*;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::controllers::stytch_guard::StytchAuth;
use crate::data::solid_server::{CreateUserPodParams, SolidServerClient, SolidServerSettings};
use crate::models::pods::{ActiveModel, CreatePodParams, Model};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateParams {
    pub name: Option<String>,
    #[serde(skip_deserializing)]
    pub user_id: Option<Uuid>,
}

impl UpdateParams {
    fn update(&self, item: &mut ActiveModel) {
        item.name = Set(self.name.clone());
        if let Some(user_id) = self.user_id {
            item.user_id = Set(Some(user_id));
        }
    }
}

async fn load_item(ctx: &AppContext, id: Uuid, user_id: Uuid) -> Result<Model> {
    let id_str = id.to_string();
    Model::find_by_id_and_user(&ctx.db, &id_str, user_id)
        .await
        .map_err(|_| Error::NotFound)
}

#[debug_handler]
pub async fn list(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    let user_id = auth.user_id;

    // Get user's pod (at most one)
    let pod = Model::find_by_user(&ctx.db, user_id)
        .await
        .map_err(|_| Error::InternalServerError)?;

    // Return as array for consistent API
    let pods = pod.map(|p| vec![p]).unwrap_or_default();
    format::json(pods)
}

#[debug_handler]
pub async fn add(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Json(params): Json<CreatePodParams>,
) -> Result<Response> {
    let user_id = auth.user_id;

    // Check if user already has a pod
    if Model::user_has_pod(&ctx.db, user_id)
        .await
        .map_err(|_| Error::InternalServerError)?
    {
        return Err(Error::BadRequest("User already has a pod".to_string()));
    }

    // Create pod on CSS with full provisioning flow
    let settings = SolidServerSettings::from_config(&ctx.config)?;
    let client = SolidServerClient::new(settings)?;

    let css_params = CreateUserPodParams {
        email: &params.email,
        password: &params.password,
        pod_name: &params.name,
    };

    let css_result = client.create_user_and_pod(css_params).await?;

    // Generate DPoP keypair
    let dpop_keypair =
        crate::data::dpop::generate_dpop_keypair().map_err(|e| Error::string(&e.to_string()))?;

    // Create pod in database with CSS provisioning data and DPoP keys
    let item = Model::create_with_css_data(
        &ctx.db,
        user_id,
        &params,
        &css_result,
        &dpop_keypair.private_jwk,
        &dpop_keypair.public_jwk_thumbprint,
    )
    .await?;

    format::json(item)
}

#[debug_handler]
pub async fn update(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
    Json(params): Json<UpdateParams>,
) -> Result<Response> {
    let user_id = auth.user_id;

    let item = load_item(&ctx, id, user_id).await?;
    let mut item = item.into_active_model();
    let mut update_params = params;
    update_params.user_id = Some(user_id);
    update_params.update(&mut item);
    let item = item.update(&ctx.db).await?;

    format::json(item)
}

#[debug_handler]
pub async fn remove(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let pod = load_item(&ctx, id, user_id).await?;

    // Delete pod from CSS server if CSS data exists
    if let (Some(ref account_token), Some(ref client_resource_url)) =
        (&pod.css_account_token, &pod.css_client_resource_url)
    {
        let settings = SolidServerSettings::from_config(&ctx.config)?;
        let client = SolidServerClient::new(settings)?;
        client
            .delete_user_pod(account_token, client_resource_url)
            .await?;
    }

    // Delete from database
    pod.delete(&ctx.db).await?;
    format::empty()
}

#[debug_handler]
pub async fn get_one(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    format::json(load_item(&ctx, id, user_id).await?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/pods")
        .add("/list", get(list))
        .add("/create", post(add))
        .add("/{id}", get(get_one))
        .add("/{id}", delete(remove))
        .add("/{id}", put(update))
        .add("/{id}", patch(update))
}
