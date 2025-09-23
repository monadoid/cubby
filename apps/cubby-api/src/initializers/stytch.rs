use std::sync::Arc;

use async_trait::async_trait;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};

use crate::data::stytch::{StytchClient, StytchSettings};

pub struct StytchInitializer;

#[async_trait]
impl Initializer for StytchInitializer {
    fn name(&self) -> String {
        "stytch-client".to_string()
    }

    async fn before_run(&self, ctx: &AppContext) -> Result<()> {
        let settings = StytchSettings::from_config(&ctx.config)?;
        let client = Arc::new(StytchClient::new(settings)?);
        ctx.shared_store.insert(client);
        Ok(())
    }
}
