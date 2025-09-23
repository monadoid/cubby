use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::data::oauth_state::OAuthStateStore;

pub struct OAuthStateInitializer;

#[async_trait]
impl Initializer for OAuthStateInitializer {
    fn name(&self) -> String {
        "oauth-state".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        // Initialize OAuth state store with 10 minute TTL
        let state_store = OAuthStateStore::new(600); // 10 minutes
        
        ctx.shared_store
            .insert(Arc::new(state_store));

        tracing::info!("OAuth state store initialized");
        Ok(())
    }
}