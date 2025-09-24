#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::debug_handler;
use loco_rs::{controller::views::engines::TeraView, prelude::*};
use uuid::Uuid;

use crate::controllers::stytch_guard::StytchSessionAuth;
use crate::models::pods::Model;
use crate::views;

async fn load_item(ctx: &AppContext, user_id: Uuid) -> Result<Option<Model>> {
    Model::find_by_user(&ctx.db, user_id)
        .await
        .map_err(|_| Error::InternalServerError)
}

#[debug_handler]
pub async fn show(
    auth: StytchSessionAuth,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let pod = load_item(&ctx, user_id).await?;
    views::pod::show(&v, &pod)
}

#[debug_handler]
pub async fn credentials(
    auth: StytchSessionAuth,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    let user_id = auth.user_id;
    let pod = load_item(&ctx, user_id).await?;
    views::pod::credentials(&v, &pod)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("pod")
        .add("/", get(show))
        .add("/credentials", get(credentials))
}
