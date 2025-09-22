#![allow(clippy::missing_errors_doc)]
#![allow(clippy::unnecessary_struct_initialization)]
#![allow(clippy::unused_async)]
use axum::debug_handler;
use loco_rs::{prelude::*, controller::views::engines::TeraView};
use serde::{Deserialize, Serialize};

use crate::controllers::stytch_guard::StytchSessionAuth;
use crate::views;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignUpParams {
    pub email: String,
    pub password: String,
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

pub fn routes() -> Routes {
    Routes::new()
        .add("/sign-up", get(sign_up_form))
        .add("/dashboard", get(dashboard))
}