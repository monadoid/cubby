use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::data::login_stash::LoginStash;

pub struct LoginStashInitializer;

#[async_trait]
impl Initializer for LoginStashInitializer {
    fn name(&self) -> String {
        "login-stash".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // Initialize login stash with 10 minute TTL
        let login_stash = LoginStash::new(600); // 10 minutes

        ctx.shared_store.insert(Arc::new(login_stash));

        tracing::info!("Login stash initialized");
        Ok(())
    }
}
