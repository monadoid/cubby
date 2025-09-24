#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::debug_handler;
use loco_rs::prelude::*;
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::controllers::stytch_guard::StytchAuth;
use crate::models::_entities::movies::{ActiveModel, Entity, Model};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Params {
    pub title: Option<String>,
    #[serde(skip_deserializing)]
    pub user_id: Option<Uuid>,
}

impl Params {
    fn update(&self, item: &mut ActiveModel) {
        item.title = Set(self.title.clone());
        if let Some(user_id) = self.user_id {
            item.user_id = Set(user_id);
        }
    }
}

async fn load_item(ctx: &AppContext, id: Uuid, user_id: Uuid) -> Result<Model> {
    let item = Entity::find_by_id(id)
        .filter(crate::models::_entities::movies::Column::UserId.eq(user_id))
        .one(&ctx.db)
        .await?;
    item.ok_or_else(|| Error::NotFound)
}

#[debug_handler]
pub async fn list(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    let user_id = auth.user_id;
    let movies = Entity::find()
        .filter(crate::models::_entities::movies::Column::UserId.eq(user_id))
        .all(&ctx.db)
        .await?;
    format::json(movies)
}

#[debug_handler]
pub async fn add(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Json(mut params): Json<Params>,
) -> Result<Response> {
    let user_id = auth.user_id;
    params.user_id = Some(user_id);
    let mut item = ActiveModel {
        id: Set(Uuid::new_v4()),
        ..Default::default()
    };
    params.update(&mut item);
    let item = item.insert(&ctx.db).await?;
    format::json(item)
}

#[debug_handler]
pub async fn update(
    auth: StytchAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
    Json(mut params): Json<Params>,
) -> Result<Response> {
    let user_id = auth.user_id;
    params.user_id = Some(user_id);
    let item = load_item(&ctx, id, user_id).await?;
    let mut item = item.into_active_model();
    params.update(&mut item);
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
    load_item(&ctx, id, user_id).await?.delete(&ctx.db).await?;
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
        .prefix("api/movies")
        .add("/list", get(list))
        .add("/create", post(add))
        .add("/{id}", get(get_one))
        .add("/{id}", delete(remove))
        .add("/{id}", put(update))
        .add("/{id}", patch(update))
}
