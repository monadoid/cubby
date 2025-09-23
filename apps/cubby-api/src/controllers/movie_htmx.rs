#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::debug_handler;
use loco_rs::{prelude::*, controller::views::engines::TeraView};
use sea_orm::ActiveValue::Set;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::controllers::stytch_guard::StytchSessionAuth;
use crate::models::_entities::movies::{ActiveModel, Entity, Model};
use crate::views;

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
pub async fn list(auth: StytchSessionAuth, ViewEngine(v): ViewEngine<TeraView>, State(ctx): State<AppContext>) -> Result<Response> {
    let user_id = auth.user_id;
    let movies = Entity::find()
        .filter(crate::models::_entities::movies::Column::UserId.eq(user_id))
        .all(&ctx.db)
        .await?;
    views::movie::list(&v, &movies)
}

#[debug_handler]
pub async fn show(
    auth: StytchSessionAuth,
    Path(id): Path<Uuid>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let item = load_item(&ctx, id, user_id).await?;
    views::movie::show(&v, &item)
}

#[debug_handler]
pub async fn new(
    _auth: StytchSessionAuth,
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<Response> {
    views::movie::create(&v)
}

#[debug_handler]
pub async fn create(
    auth: StytchSessionAuth,
    State(ctx): State<AppContext>,
    Form(mut params): Form<Params>,
) -> Result<Response> {
    let user_id = auth.user_id;
    params.user_id = Some(user_id);
    let mut item = ActiveModel {
        id: Set(Uuid::new_v4()),
        ..Default::default()
    };
    params.update(&mut item);
    let _item = item.insert(&ctx.db).await?;
    
    // Redirect to list after successful creation
    format::redirect("/movies")
}

#[debug_handler]
pub async fn edit(
    auth: StytchSessionAuth,
    Path(id): Path<Uuid>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let item = load_item(&ctx, id, user_id).await?;
    views::movie::edit(&v, &item)
}

#[debug_handler]
pub async fn update(
    auth: StytchSessionAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
    Form(mut params): Form<Params>,
) -> Result<Response> {
    let user_id = auth.user_id;
    params.user_id = Some(user_id);
    let item = load_item(&ctx, id, user_id).await?;
    let mut item = item.into_active_model();
    params.update(&mut item);
    let _item = item.update(&ctx.db).await?;
    
    // Redirect to list after successful update
    format::redirect("/movies")
}

#[debug_handler]
pub async fn remove(
    auth: StytchSessionAuth,
    Path(id): Path<Uuid>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    load_item(&ctx, id, user_id).await?.delete(&ctx.db).await?;
    
    // Redirect to list after successful deletion
    format::redirect("/movies")
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("movies")
        .add("/", get(list))
        .add("/new", get(new))
        .add("/", post(create))
        .add("/{id}", get(show))
        .add("/{id}/edit", get(edit))
        .add("/{id}", post(update))
        .add("/{id}/delete", post(remove))
}