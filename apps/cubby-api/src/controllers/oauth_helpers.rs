use loco_rs::prelude::*;
use std::sync::Arc;

use crate::data::{login_stash::LoginStash, oauth_state::OAuthStateStore, stytch::StytchClient};

pub fn stytch_client(ctx: &AppContext) -> Result<Arc<StytchClient>> {
    ctx.shared_store.get::<Arc<StytchClient>>().ok_or_else(|| {
        tracing::error!("stytch client not initialised");
        Error::InternalServerError
    })
}

pub fn oauth_state_store(ctx: &AppContext) -> Result<Arc<OAuthStateStore>> {
    ctx.shared_store
        .get::<Arc<OAuthStateStore>>()
        .ok_or_else(|| {
            tracing::error!("oauth state store not initialised");
            Error::InternalServerError
        })
}

pub fn login_stash(ctx: &AppContext) -> Result<Arc<LoginStash>> {
    ctx.shared_store.get::<Arc<LoginStash>>().ok_or_else(|| {
        tracing::error!("login stash not initialised");
        Error::InternalServerError
    })
}
