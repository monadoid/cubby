use std::sync::Arc;
use loco_rs::prelude::*;

use crate::data::{
    stytch::StytchClient,
    oauth_state::OAuthStateStore,
};

pub fn stytch_client(ctx: &AppContext) -> Result<Arc<StytchClient>> {
    ctx.shared_store.get::<Arc<StytchClient>>().ok_or_else(|| {
        tracing::error!("stytch client not initialised");
        Error::InternalServerError
    })
}

pub fn oauth_state_store(ctx: &AppContext) -> Result<Arc<OAuthStateStore>> {
    ctx.shared_store.get::<Arc<OAuthStateStore>>().ok_or_else(|| {
        tracing::error!("oauth state store not initialised");
        Error::InternalServerError
    })
}