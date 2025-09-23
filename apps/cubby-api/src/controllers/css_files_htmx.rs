use axum::{
    debug_handler,
    extract::{Multipart, Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use loco_rs::prelude::*;
use reqwest_middleware::reqwest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    controllers::stytch_guard::StytchSessionAuth,
    views, 
    data::{css_auth::CssAuthService, solid_server::SolidServerSettings},
};

/// Detect MIME type from filename extension
fn detect_mime_type(filename: &str) -> &'static str {
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        // Text files
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "text/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "csv" => "text/csv",
        
        // Images
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        
        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        
        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        
        // RDF/Semantic Web
        "ttl" => "text/turtle",
        "rdf" => "application/rdf+xml",
        "jsonld" => "application/ld+json",
        "n3" => "text/n3",
        
        // Fallback
        _ => "application/octet-stream",
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileInfo {
    name: String,
    path: String,
    is_directory: bool,
    size: Option<u64>,
    content_type: Option<String>,
}

#[debug_handler]
pub async fn list(
    auth: StytchSessionAuth,
    Query(params): Query<ListQuery>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    // Get CSS base URL from config
    let settings = SolidServerSettings::from_config(&ctx.config)
        .map_err(|e| Error::string(&e.to_string()))?;
    
    // Initialize CSS auth service (same as CSS proxy does)
    let css_auth = CssAuthService::new(settings.base_url.clone());
    
    let path = params.path.unwrap_or_default();
    let list_path = if path.is_empty() { "/" } else { &path };
    
    let client = css_auth.get_pod_authenticated_client(&ctx.db, auth.user_id).await
        .map_err(|e| Error::string(&format!("Failed to get pod authenticated client: {}", e)))?;
    
    // Build path (CSS proxy expects relative paths from pod base)
    // Handle root path specially to avoid double slashes
    let path = if list_path.is_empty() || list_path == "/" {
        "/".to_string()
    } else {
        format!("/{}", list_path.trim_start_matches('/'))
    };
    
    // Get directory listing from CSS server
    let response = client
        .authenticated_request(
            "GET",
            &path,
            Some(&{
                let mut headers = reqwest::header::HeaderMap::new();
                headers.insert("Accept", "application/ld+json".parse().unwrap());
                headers
            }),
            None,
        )
        .await
        .map_err(|e| Error::string(&format!("Failed to list files: {}", e)))?;

    let mut files = Vec::new();
    
    if response.status().is_success() {
        let content = response.text().await
            .map_err(|e| Error::string(&format!("Failed to read response: {}", e)))?;
        
        // Debug: Log the raw response to see what CSS server is returning
        tracing::debug!(
            content = %content,
            path = %path,
            "Raw CSS server response for file listing"
        );
        
        // Parse the JSON-LD response to extract file information
        if let Ok(json_array) = serde_json::from_str::<Vec<Value>>(&content) {
            tracing::debug!(
                json = ?json_array,
                "Parsed JSON-LD response"
            );
            
            // Handle array format (pod-specific responses)
            for json in &json_array {
                // Try both possible keys for contains
                let contains = json.get("http://www.w3.org/ns/ldp#contains")
                    .or_else(|| json.get("ldp:contains"))
                    .and_then(|c| c.as_array());
                
                if let Some(contains) = contains {
                    for item in contains {
                        if let Some(full_url) = item.get("@id").and_then(|id| id.as_str()) {
                            tracing::debug!(
                                full_url = %full_url,
                                "Processing file URL"
                            );
                            
                            // Extract filename from full URL
                            // Handle URLs like "http://localhost:3000/pod-name//filename.txt"
                            let filename = if let Some(last_slash_pos) = full_url.rfind('/') {
                                &full_url[last_slash_pos + 1..]
                            } else {
                                full_url
                            };
                            
                            // Skip empty filenames (directories ending with /)
                            if filename.is_empty() {
                                continue;
                            }
                            
                            // URL decode the filename
                            let decoded_filename = urlencoding::decode(filename)
                                .map(|decoded| decoded.into_owned())
                                .unwrap_or_else(|_| filename.to_string());
                            
                            let is_directory = item.get("@type")
                                .and_then(|t| t.as_array())
                                .map(|types| types.iter().any(|t| t.as_str() == Some("http://www.w3.org/ns/ldp#Container") || t.as_str() == Some("ldp:Container")))
                                .unwrap_or(false);
                            
                            // Build clean file path without double slashes
                            let file_path = if path == "/" || path.is_empty() {
                                decoded_filename.clone()
                            } else {
                                format!("{}/{}", path.trim_end_matches('/'), decoded_filename)
                            };
                            
                            tracing::debug!(
                                original_url = %full_url,
                                extracted_filename = %filename,
                                decoded_filename = %decoded_filename,
                                file_path = %file_path,
                                is_directory = %is_directory,
                                "Extracted file info"
                            );
                            
                            files.push(FileInfo {
                                name: decoded_filename,
                                path: file_path,
                                is_directory,
                                size: None,
                                content_type: None,
                            });
                        }
                    }
                }
            }
        } else if let Ok(json) = serde_json::from_str::<Value>(&content) {
            // Fallback: try parsing as single object (for backward compatibility)
            tracing::debug!(
                json = %json,
                "Parsed JSON-LD response as single object"
            );
            
            let contains = json.get("http://www.w3.org/ns/ldp#contains")
                .or_else(|| json.get("ldp:contains"))
                .and_then(|c| c.as_array());
                
            if let Some(contains) = contains {
                for item in contains {
                    if let Some(full_url) = item.get("@id").and_then(|id| id.as_str()) {
                        // Same processing logic as above
                        let filename = if let Some(last_slash_pos) = full_url.rfind('/') {
                            &full_url[last_slash_pos + 1..]
                        } else {
                            full_url
                        };
                        
                        if filename.is_empty() {
                            continue;
                        }
                        
                        let decoded_filename = urlencoding::decode(filename)
                            .map(|decoded| decoded.into_owned())
                            .unwrap_or_else(|_| filename.to_string());
                        
                        let is_directory = item.get("@type")
                            .and_then(|t| t.as_array())
                            .map(|types| types.iter().any(|t| t.as_str() == Some("http://www.w3.org/ns/ldp#Container") || t.as_str() == Some("ldp:Container")))
                            .unwrap_or(false);
                        
                        let file_path = if path == "/" || path.is_empty() {
                            decoded_filename.clone()
                        } else {
                            format!("{}/{}", path.trim_end_matches('/'), decoded_filename)
                        };
                        
                        files.push(FileInfo {
                            name: decoded_filename,
                            path: file_path,
                            is_directory,
                            size: None,
                            content_type: None,
                        });
                    }
                }
            }
        }
    }

    views::css_files::list(&v, &files, &path)
}

#[debug_handler]
pub async fn upload_form(
    _auth: StytchSessionAuth,
    Query(params): Query<ListQuery>,
    ViewEngine(v): ViewEngine<TeraView>,
    State(_ctx): State<AppContext>,
) -> Result<Response> {
    let path = params.path.unwrap_or_default();
    views::css_files::upload_form(&v, &path)
}

#[debug_handler]
pub async fn upload(
    auth: StytchSessionAuth,
    Query(params): Query<ListQuery>,
    State(ctx): State<AppContext>,
    mut multipart: Multipart,
) -> Result<Response> {
    let path = params.path.unwrap_or_default();
    
    while let Some(field) = multipart.next_field().await.map_err(|e| Error::string(&format!("Multipart error: {}", e)))? {
        if field.name() == Some("file") {
            let filename = field.file_name()
                .ok_or_else(|| Error::string("No filename provided"))?
                .to_string();
            
            let data = field.bytes().await
                .map_err(|e| Error::string(&format!("Failed to read file data: {}", e)))?;
            
            let upload_path = if path.is_empty() {
                filename.clone()
            } else {
                format!("{}/{}", path.trim_end_matches('/'), filename)
            };
            
            // Detect proper MIME type from filename
            let mime_type = detect_mime_type(&filename);
            
            // Log upload details for debugging
            tracing::debug!(
                filename = %filename,
                mime_type = %mime_type,
                file_size = data.len(),
                upload_path = %upload_path,
                "Uploading file via CSS proxy"
            );
            
            // Use CSS proxy logic directly instead of HTTP roundtrip
            // This ensures we use the same authentication that works in test_css_proxy.sh
            
            // Get CSS base URL from config 
            let settings = SolidServerSettings::from_config(&ctx.config)
                .map_err(|e| Error::string(&e.to_string()))?;
            
            // Initialize CSS auth service (same as CSS proxy does)
            let css_auth_service = CssAuthService::new(settings.base_url);
            
            // Get pod-specific authenticated client for this user
            let auth_client = css_auth_service
                .get_pod_authenticated_client(&ctx.db, auth.user_id)
                .await
                .map_err(|e| Error::string(&e.to_string()))?;
            
            // Build path (CSS proxy expects relative paths)
            let path = format!("/{}", upload_path.trim_start_matches('/'));
            
            // Build headers  
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("Content-Type", mime_type.parse().unwrap());
            
            // Make authenticated request using CSS proxy's method
            let response = auth_client
                .authenticated_binary_request(
                    "PUT",
                    &path,
                    Some(&headers),
                    Some(data.to_vec()),
                )
                .await
                .map_err(|e| Error::string(&format!("Failed to upload file: {}", e)))?;
            
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                
                tracing::error!(
                    status = %status,
                    error_body = %error_text,
                    filename = %filename,
                    path = %path,
                    "File upload failed via CSS proxy logic"
                );
                
                let error_message = match status.as_u16() {
                    401 => "Authorization failed - check authentication".to_string(),
                    403 => "Access forbidden - insufficient permissions".to_string(),
                    404 => "Pod or container not found".to_string(),
                    415 => format!("Unsupported media type: {}", mime_type),
                    _ => format!("Upload failed with status {}: {}", status, error_text),
                };
                
                return Err(Error::string(&error_message));
            }
            
            tracing::info!(
                filename = %filename,
                mime_type = %mime_type,
                file_size = data.len(),
                path = %path,
                "File uploaded successfully via CSS proxy logic"
            );
            
            break;
        }
    }
    
    // Return success response that will trigger a redirect or refresh
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("HX-Trigger", "fileUploaded")
        .header("HX-Redirect", format!("/files?path={}", urlencoding::encode(&path)))
        .body("File uploaded successfully".into())?)
}

#[debug_handler]
pub async fn view(
    auth: StytchSessionAuth,
    Path(encoded_path): Path<String>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    // Get CSS base URL from config
    let settings = SolidServerSettings::from_config(&ctx.config)
        .map_err(|e| Error::string(&e.to_string()))?;
    
    // Initialize CSS auth service
    let css_auth = CssAuthService::new(settings.base_url.clone());
    let client = css_auth.get_pod_authenticated_client(&ctx.db, auth.user_id).await
        .map_err(|e| Error::string(&format!("Failed to get pod authenticated client: {}", e)))?;
    
    let file_path = urlencoding::decode(&encoded_path)
        .map_err(|e| Error::string(&format!("Invalid path encoding: {}", e)))?;
    
    // Build path (CSS proxy expects relative paths from pod base)
    let path = format!("/{}", file_path.trim_start_matches('/'));
    
    let response = client
        .authenticated_request(
            "GET",
            &path,
            None,
            None,
        )
        .await
        .map_err(|e| Error::string(&format!("Failed to fetch file: {}", e)))?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        
        let error_message = match status.as_u16() {
            404 => "File not found".to_string(),
            403 => "Access denied".to_string(),
            _ => format!("Failed to fetch file: {}", error_text),
        };
        
        return Err(Error::string(&error_message));
    }
    
    // Get content type from response headers, with fallback to filename detection
    let content_type = response.headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Extract filename from path and detect MIME type
            let filename = file_path.split('/').last().unwrap_or("");
            detect_mime_type(filename).to_string()
        });
    
    // Get file content
    let file_content = response.bytes().await
        .map_err(|e| Error::string(&format!("Failed to read file content: {}", e)))?;
    
    // Extract filename for Content-Disposition header
    let filename = file_path.split('/').last().unwrap_or("file");
    
    tracing::debug!(
        filename = %filename,
        content_type = %content_type,
        file_size = file_content.len(),
        path = %path,
        "Serving file content"
    );
    
    // Build response with proper headers
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Disposition", format!("inline; filename=\"{}\"", filename))
        .header("Content-Length", file_content.len().to_string())
        .body(file_content.into())?)
}

#[debug_handler]
pub async fn delete(
    auth: StytchSessionAuth,
    Path(encoded_path): Path<String>,
    State(ctx): State<AppContext>,
) -> Result<Response> {
    // Get CSS base URL from config
    let settings = SolidServerSettings::from_config(&ctx.config)
        .map_err(|e| Error::string(&e.to_string()))?;
    
    // Initialize CSS auth service (same as CSS proxy does)
    let css_auth = CssAuthService::new(settings.base_url.clone());
    let client = css_auth.get_pod_authenticated_client(&ctx.db, auth.user_id).await
        .map_err(|e| Error::string(&format!("Failed to get pod authenticated client: {}", e)))?;
    
    let file_path = urlencoding::decode(&encoded_path)
        .map_err(|e| Error::string(&format!("Invalid path encoding: {}", e)))?;
    
    // Build path (CSS proxy expects relative paths from pod base)
    let path = format!("/{}", file_path.trim_start_matches('/'));
    
    let response = client
        .authenticated_request(
            "DELETE",
            &path,
            None,
            None,
        )
        .await
        .map_err(|e| Error::string(&format!("Failed to delete file: {}", e)))?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(Error::string(&format!("Delete failed: {}", error_text)));
    }
    
    // Return success response that will trigger a refresh
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("HX-Trigger", "fileDeleted")
        .body("File deleted successfully".into())?)
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("files")
        .add("/", get(list))
        .add("/upload", get(upload_form))
        .add("/upload", post(upload))
        .add("/view/{path}", get(view))
        .add("/delete/{path}", post(delete))
}