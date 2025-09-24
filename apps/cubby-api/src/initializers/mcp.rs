use std::sync::Arc;

use async_trait::async_trait;
use axum::{routing::get, Json, Router as AxumRouter};
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};

use crate::mcp::counter::Counter;

#[derive(Default)]
pub struct McpInitializer;

#[async_trait]
impl Initializer for McpInitializer {
    fn name(&self) -> String {
        "mcp-server".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        let service = StreamableHttpService::new(
            || Ok(Counter::new()),
            Arc::new(LocalSessionManager::default()),
            Default::default(),
        );

        let router = router
            .route("/mcp-status", get(health_check))
            .nest_service("/mcp", service);

        Ok(router)
    }
}

#[derive(serde::Serialize)]
struct HealthResponse<'a> {
    status: &'a str,
}

async fn health_check() -> Json<HealthResponse<'static>> {
    Json(HealthResponse { status: "ok" })
}
