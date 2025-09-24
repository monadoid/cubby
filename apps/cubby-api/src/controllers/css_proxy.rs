use axum::{
    body::Body,
    debug_handler,
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Response,
};
use loco_rs::{app::AppContext, prelude::*};
use std::collections::HashMap;

use crate::{
    controllers::stytch_guard::StytchAuth,
    data::{css_auth::CssAuthService, solid_server::SolidServerSettings},
};

/// Proxy GET requests to CSS server
#[debug_handler]
pub async fn get_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    proxy_request(auth, ctx, "GET", resource_path, params, headers, None).await
}

/// Proxy PUT requests to CSS server (create/update resources)
#[debug_handler]
pub async fn put_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response> {
    proxy_request(auth, ctx, "PUT", resource_path, params, headers, Some(body)).await
}

/// Proxy POST requests to CSS server (create resources)
#[debug_handler]
pub async fn post_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response> {
    proxy_request(
        auth,
        ctx,
        "POST",
        resource_path,
        params,
        headers,
        Some(body),
    )
    .await
}

/// Proxy DELETE requests to CSS server
#[debug_handler]
pub async fn delete_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    proxy_request(auth, ctx, "DELETE", resource_path, params, headers, None).await
}

/// Proxy PATCH requests to CSS server (modify resources)
#[debug_handler]
pub async fn patch_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: String,
) -> Result<Response> {
    proxy_request(
        auth,
        ctx,
        "PATCH",
        resource_path,
        params,
        headers,
        Some(body),
    )
    .await
}

/// Proxy HEAD requests to CSS server
#[debug_handler]
pub async fn head_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    proxy_request(auth, ctx, "HEAD", resource_path, params, headers, None).await
}

/// Proxy OPTIONS requests to CSS server
#[debug_handler]
pub async fn options_resource(
    auth: StytchAuth,
    State(ctx): State<AppContext>,
    Path(resource_path): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Response> {
    proxy_request(auth, ctx, "OPTIONS", resource_path, params, headers, None).await
}

/// Core proxy logic - handles authentication and forwarding
async fn proxy_request(
    auth: StytchAuth,
    ctx: AppContext,
    method: &str,
    resource_path: String,
    query_params: HashMap<String, String>,
    mut headers: HeaderMap,
    body: Option<String>,
) -> Result<Response> {
    let user_id = auth.user_id;

    // Get CSS base URL from config
    let settings =
        SolidServerSettings::from_config(&ctx.config).map_err(|e| Error::string(&e.to_string()))?;

    // Initialize CSS auth service
    let css_auth_service = CssAuthService::new(settings.base_url);

    // Get authenticated client for this user
    let auth_client = css_auth_service
        .get_authenticated_client(&ctx.db, user_id)
        .await
        .map_err(|e| Error::string(&e.to_string()))?;

    // Build the full path with query parameters
    let mut path = format!("/{}", resource_path.trim_start_matches('/'));
    if !query_params.is_empty() {
        let query_string = query_params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        path.push_str(&format!("?{}", query_string));
    }

    // Remove headers that shouldn't be forwarded or might conflict
    headers.remove("host");
    headers.remove("authorization");
    headers.remove("dpop");
    headers.remove("content-length");

    // Make authenticated request to CSS server
    let css_response = auth_client
        .authenticated_request(method, &path, Some(&headers), body)
        .await
        .map_err(|e| Error::string(&e.to_string()))?;

    // Convert CSS response to Axum response
    convert_css_response_to_axum(css_response).await
}

/// Convert reqwest::Response to axum::Response
async fn convert_css_response_to_axum(
    css_response: reqwest_middleware::reqwest::Response,
) -> Result<Response> {
    let status = css_response.status();
    let headers = css_response.headers().clone();
    let body_bytes = css_response
        .bytes()
        .await
        .map_err(|e| Error::string(&e.to_string()))?;

    let mut response_builder = Response::builder().status(status);

    // Copy headers from CSS response
    for (key, value) in headers.iter() {
        // Skip headers that Axum handles automatically or might cause issues
        if !matches!(
            key.as_str().to_lowercase().as_str(),
            "content-length" | "transfer-encoding" | "connection"
        ) {
            response_builder = response_builder.header(key, value);
        }
    }

    let response = response_builder
        .body(Body::from(body_bytes))
        .map_err(|e| Error::string(&e.to_string()))?;

    Ok(response)
}

/// Get user's pod information
#[debug_handler]
pub async fn get_pod_info(auth: StytchAuth, State(ctx): State<AppContext>) -> Result<Response> {
    let user_id = auth.user_id;

    // Get user's pod
    let pod = crate::models::pods::Model::find_by_user(&ctx.db, user_id)
        .await?
        .ok_or_else(|| Error::string("User has no pod"))?;

    let pod_info = serde_json::json!({
        "webid": pod.webid,
        "pod_url": pod.link,
        "css_email": pod.css_email,
        "created_at": pod.created_at,
    });

    format::json(pod_info)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("/api/css-proxy")
        .add("/pod-info", get(get_pod_info))
        // Catch-all routes for different HTTP methods
        .add("/{*path}", get(get_resource))
        .add("/{*path}", put(put_resource))
        .add("/{*path}", post(post_resource))
        .add("/{*path}", delete(delete_resource))
        .add("/{*path}", patch(patch_resource))
    // Note: Axum doesn't support HEAD and OPTIONS in the same way
    // We'll need to handle these specially in the application setup
}

/// Routes specifically for HEAD and OPTIONS methods
pub fn additional_routes() -> Routes {
    Routes::new()
        .prefix("/api/css-proxy")
        .add("/{*path}", head(head_resource))
        .add("/{*path}", options(options_resource))
}
