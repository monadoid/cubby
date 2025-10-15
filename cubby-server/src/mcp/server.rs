use crate::server::{AppState, SearchQuery, SearchResponse};
use axum::extract::{Query, State};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use schemars::schema_for;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::error;

#[derive(Clone)]
pub struct CubbyMcpServer {
    state: Arc<AppState>,
}

impl CubbyMcpServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

impl ServerHandler for CubbyMcpServer {
    async fn initialize(
        &self,
        _params: InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, ErrorData> {
        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: None,
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "cubby-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                title: None,
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "cubby mcp server - access to screen recordings and ui automation".to_string(),
            ),
        })
    }

    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = vec![
            create_search_tool(),
            create_pixel_control_tool(),
            create_find_elements_tool(),
            create_click_element_tool(),
            create_fill_element_tool(),
            create_scroll_element_tool(),
            create_open_application_tool(),
            create_open_url_tool(),
        ];
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let arguments = params.arguments.unwrap_or_default();
        
        match params.name.as_ref() {
            "search-content" => handle_search_tool(self.state.clone(), arguments).await,
            "pixel-control" => {
                handle_pixel_control_tool(self.state.clone(), arguments).await
            }
            "find-elements" => {
                handle_find_elements_tool(self.state.clone(), arguments).await
            }
            "click-element" => {
                handle_click_element_tool(self.state.clone(), arguments).await
            }
            "fill-element" => handle_fill_element_tool(self.state.clone(), arguments).await,
            "scroll-element" => {
                handle_scroll_element_tool(self.state.clone(), arguments).await
            }
            "open-application" => {
                handle_open_application_tool(self.state.clone(), arguments).await
            }
            "open-url" => handle_open_url_tool(self.state.clone(), arguments).await,
            _ => Err(ErrorData::new(ErrorCode::METHOD_NOT_FOUND, format!("unknown tool: {}", params.name), None)),
        }
    }
}

pub fn create_mcp_service(
    app_state: Arc<AppState>,
) -> StreamableHttpService<impl ServerHandler + Clone> {
    let state_clone = app_state.clone();
    StreamableHttpService::new(
        move || Ok(CubbyMcpServer::new(state_clone.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    )
}

// Tool creation functions

fn create_search_tool() -> Tool {
    // Manually create schema since SearchQuery has types from different schemars versions
    let schema_json = json!({
        "type": "object",
        "properties": {
            "q": {
                "type": "string",
                "description": "Search query to find in recorded content"
            },
            "limit": {
                "type": "integer",
                "description": "Maximum number of results to return",
                "default": 10
            },
            "offset": {
                "type": "integer",
                "description": "Number of results to skip (for pagination)",
                "default": 0
            },
            "content_type": {
                "type": "string",
                "enum": ["ocr", "audio", "ui", "all"],
                "description": "Type of content to search",
                "default": "all"
            },
            "start_time": {
                "type": "string",
                "format": "date-time",
                "description": "Start time in ISO format UTC"
            },
            "end_time": {
                "type": "string",
                "format": "date-time",
                "description": "End time in ISO format UTC"
            },
            "app_name": {
                "type": "string",
                "description": "Filter by application name"
            },
            "window_name": {
                "type": "string",
                "description": "Filter by window name or title"
            },
            "frame_name": {
                "type": "string",
                "description": "Filter by frame name"
            },
            "include_frames": {
                "type": "boolean",
                "description": "Include frame data in results",
                "default": false
            },
            "min_length": {
                "type": "integer",
                "description": "Minimum content length in characters"
            },
            "max_length": {
                "type": "integer",
                "description": "Maximum content length in characters"
            },
            "speaker_ids": {
                "type": "string",
                "description": "Comma-separated list of speaker IDs to filter"
            },
            "focused": {
                "type": "boolean",
                "description": "Filter by focused window"
            },
            "browser_url": {
                "type": "string",
                "description": "Filter by browser URL"
            }
        }
    });

    Tool {
        name: "search-content".into(),
        title: None,
        description: Some("Search through cubby recorded content (OCR text, audio transcriptions, UI elements). Use this to find specific content that has appeared on your screen or been spoken. Results include timestamps, app context, and the content itself.".into()),
        input_schema: Arc::new(schema_json.as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_pixel_control_tool() -> Tool {
    Tool {
        name: "pixel-control".into(),
        title: None,
        description: Some("Control mouse and keyboard at the pixel level. This is a cross-platform tool that works on all operating systems. Use this to type text, press keys, move the mouse, and click buttons.".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["WriteText", "KeyPress", "MouseMove", "MouseClick"],
                            "description": "Type of input action to perform"
                        },
                        "data": {
                            "description": "Action-specific data"
                        }
                    },
                    "required": ["type", "data"]
                }
            },
            "required": ["action"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_find_elements_tool() -> Tool {
    Tool {
        name: "find-elements".into(),
        title: None,
        description: Some("Find UI elements with a specific role in an application. This tool is especially useful for identifying interactive elements.\n\nMacOS Accessibility Roles Guide:\n- Basic roles: 'button', 'textfield', 'checkbox', 'menu', 'list'\n- MacOS specific roles: 'AXButton', 'AXTextField', 'AXCheckBox', 'AXMenu', etc.\n- Text inputs can be: 'AXTextField', 'AXTextArea', 'AXComboBox', 'AXSearchField'\n- Clickable items: 'AXButton', 'AXMenuItem', 'AXMenuBarItem', 'AXImage', 'AXStaticText'\n- Web content may use: 'AXWebArea', 'AXLink', 'AXHeading', 'AXRadioButton'\n\nUse MacOS Accessibility Inspector app to identify the exact roles in your target application.".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "object",
                    "properties": {
                        "app_name": {"type": "string", "description": "The name of the application (e.g., 'Chrome', 'Finder', 'Terminal')"},
                        "window_name": {"type": "string", "description": "The window name or title (optional)"},
                        "locator": {"type": "string", "description": "The role to search for (e.g., 'button', 'textfield', 'AXButton', 'AXTextField'). For best results, use MacOS AX prefixed roles."},
                        "use_background_apps": {"type": "boolean", "description": "Whether to look in background apps", "default": true},
                        "activate_app": {"type": "boolean", "description": "Whether to activate the app before searching", "default": true}
                    },
                    "required": ["app_name", "locator"]
                },
                "max_results": {"type": "integer", "description": "Maximum number of elements to return", "default": 10},
                "max_depth": {"type": "integer", "description": "Maximum depth of element tree to search"}
            },
            "required": ["selector"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_click_element_tool() -> Tool {
    Tool {
        name: "click-element".into(),
        title: None,
        description: Some("Click an element in an application using its id (MacOS only)".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "object",
                    "properties": {
                        "app_name": {"type": "string", "description": "The name of the application"},
                        "window_name": {"type": "string", "description": "The window name (optional)"},
                        "locator": {"type": "string", "description": "The id of the element to click (e.g., '#element-id')"},
                        "use_background_apps": {"type": "boolean", "description": "Whether to look in background apps", "default": true},
                        "activate_app": {"type": "boolean", "description": "Whether to activate the app before clicking", "default": true}
                    },
                    "required": ["app_name", "locator"]
                }
            },
            "required": ["selector"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_fill_element_tool() -> Tool {
    Tool {
        name: "fill-element".into(),
        title: None,
        description: Some("Type text into an element in an application (MacOS only)".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "object",
                    "properties": {
                        "app_name": {"type": "string", "description": "The name of the application"},
                        "window_name": {"type": "string", "description": "The window name (optional)"},
                        "locator": {"type": "string", "description": "The id of the element to fill (e.g., '#element-id')"},
                        "use_background_apps": {"type": "boolean", "description": "Whether to look in background apps", "default": true},
                        "activate_app": {"type": "boolean", "description": "Whether to activate the app before typing", "default": true}
                    },
                    "required": ["app_name", "locator"]
                },
                "text": {"type": "string", "description": "The text to type into the element"}
            },
            "required": ["selector", "text"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_scroll_element_tool() -> Tool {
    Tool {
        name: "scroll-element".into(),
        title: None,
        description: Some("Scroll an element in a specific direction (MacOS only)".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "selector": {
                    "type": "object",
                    "properties": {
                        "app_name": {"type": "string", "description": "The name of the application"},
                        "window_name": {"type": "string", "description": "The window name (optional)"},
                        "locator": {"type": "string", "description": "The id of the element to scroll (e.g., '#element-id')"},
                        "use_background_apps": {"type": "boolean", "description": "Whether to look in background apps", "default": true},
                        "activate_app": {"type": "boolean", "description": "Whether to activate the app before scrolling", "default": true}
                    },
                    "required": ["app_name", "locator"]
                },
                "direction": {"type": "string", "enum": ["up", "down", "left", "right"], "description": "The direction to scroll"},
                "amount": {"type": "integer", "description": "The amount to scroll in pixels"}
            },
            "required": ["selector", "direction", "amount"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_open_application_tool() -> Tool {
    Tool {
        name: "open-application".into(),
        title: None,
        description: Some("Open an application by name".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "app_name": {"type": "string", "description": "The name of the application to open"}
            },
            "required": ["app_name"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_open_url_tool() -> Tool {
    Tool {
        name: "open-url".into(),
        title: None,
        description: Some("Open a URL in a browser".into()),
        input_schema: Arc::new(json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "The URL to open"},
                "browser": {"type": "string", "description": "The browser to use (optional)"}
            },
            "required": ["url"]
        }).as_object().unwrap().clone()),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

// Tool handler functions

async fn handle_search_tool(
    app_state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let args: SearchQuery = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid search params: {}", e), None)
    })?;

    let result = crate::server::search(Query(args), State(app_state))
        .await
        .map_err(|e| ErrorData::internal_error(format!("search failed: {:?}", e), None))?;

    let response_text = format_search_results(result.0);
    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(
        response_text,
    ), None)]))
}

fn format_search_results(response: SearchResponse) -> String {
    if response.data.is_empty() {
        return "no results found".to_string();
    }

    let mut output = format!("found {} results:\n\n", response.data.len());
    for (i, item) in response.data.iter().take(10).enumerate() {
        output.push_str(&format!("result {}:\n", i + 1));
        output.push_str(&format!("  {}\n", serde_json::to_string_pretty(item).unwrap_or_else(|_| format!("{:?}", item))));
        output.push_str("---\n");
    }
    output
}

async fn handle_pixel_control_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    // The pixel control expects an "action" object
    let action_value = arguments.get("action").ok_or_else(|| {
        ErrorData::invalid_params("missing 'action' field".to_string(), None)
    })?;

    let payload = json!({ "action": action_value });

    // Call the existing handler - note: this handler may not be public, we'll need to check
    let result = reqwest::Client::new()
        .post(format!("http://localhost:3030/experimental/operator/pixel"))
        .json(&payload)
        .send()
        .await
        .map_err(|e| ErrorData::internal_error(format!("pixel control failed: {}", e), None))?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "pixel control executed: {}",
        response_text
    )), None)]))
}

async fn handle_find_elements_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/find-elements")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            ErrorData::internal_error(format!("find elements failed: {}", e), None)
        })?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(
        response_text,
    ), None)]))
}

async fn handle_click_element_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/click")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ErrorData::internal_error(format!("click element failed: {}", e), None))?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "clicked element: {}",
        response_text
    )), None)]))
}

async fn handle_fill_element_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/type")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ErrorData::internal_error(format!("fill element failed: {}", e), None))?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "filled element: {}",
        response_text
    )), None)]))
}

async fn handle_scroll_element_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/scroll")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ErrorData::internal_error(format!("scroll element failed: {}", e), None))?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "scrolled element: {}",
        response_text
    )), None)]))
}

async fn handle_open_application_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/open-application")
        .json(&payload)
        .send()
        .await
        .map_err(|e| {
            ErrorData::internal_error(format!("open application failed: {}", e), None)
        })?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "opened application: {}",
        response_text
    )), None)]))
}

async fn handle_open_url_tool(
    _state: Arc<AppState>,
    arguments: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let payload = Value::Object(arguments);

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/open-url")
        .json(&payload)
        .send()
        .await
        .map_err(|e| ErrorData::internal_error(format!("open url failed: {}", e), None))?;

    let response_text = result
        .text()
        .await
        .map_err(|e| ErrorData::internal_error(format!("failed to read response: {}", e), None))?;

    Ok(CallToolResult::success(vec![Annotated::new(RawContent::text(format!(
        "opened url: {}",
        response_text
    )), None)]))
}

