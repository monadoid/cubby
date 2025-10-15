use crate::server::{AppState, SearchQuery, SearchResponse};
use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use cubby_db::ContentType;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use schemars::{schema_for, JsonSchema};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

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

// MCP-specific request structs with proper types matching Python implementation

fn default_limit() -> u32 {
    10
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Search through cubby recorded content")]
struct McpSearchRequest {
    #[schemars(description = "Search query to find in recorded content")]
    q: Option<String>,
    #[serde(default = "default_limit")]
    #[schemars(description = "Maximum number of results to return")]
    limit: u32,
    #[serde(default)]
    #[schemars(description = "Number of results to skip (for pagination)")]
    offset: u32,
    #[serde(default)]
    #[schemars(description = "Type of content to search")]
    content_type: Option<String>,
    #[serde(default)]
    #[schemars(description = "Start time in ISO format UTC")]
    start_time: Option<String>,
    #[serde(default)]
    #[schemars(description = "End time in ISO format UTC")]
    end_time: Option<String>,
    #[serde(default)]
    #[schemars(description = "Filter by application name")]
    app_name: Option<String>,
    #[serde(default)]
    #[schemars(description = "Filter by window name or title")]
    window_name: Option<String>,
    #[serde(default)]
    #[schemars(description = "Filter by frame name")]
    frame_name: Option<String>,
    #[serde(default)]
    #[schemars(description = "Include frame data in results")]
    include_frames: bool,
    #[serde(default)]
    #[schemars(description = "Minimum content length in characters")]
    min_length: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Maximum content length in characters")]
    max_length: Option<u32>,
    #[serde(default)]
    #[schemars(description = "Comma-separated list of speaker IDs to filter")]
    speaker_ids: Option<String>,
    #[serde(default)]
    #[schemars(description = "Filter by focused window")]
    focused: Option<bool>,
    #[serde(default)]
    #[schemars(description = "Filter by browser URL")]
    browser_url: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct McpPixelControlAction {
    #[schemars(description = "Type of input action to perform")]
    r#type: String,
    #[schemars(description = "Action-specific data")]
    data: Value,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Control mouse and keyboard at the pixel level")]
struct McpPixelControlRequest {
    #[schemars(description = "The action to perform")]
    action: McpPixelControlAction,
}

#[derive(Deserialize, JsonSchema)]
struct McpSelector {
    #[schemars(description = "The name of the application")]
    app_name: String,
    #[serde(default)]
    #[schemars(description = "The window name or title (optional)")]
    window_name: Option<String>,
    #[schemars(description = "The role or element locator")]
    locator: String,
    #[serde(default = "default_true")]
    #[schemars(description = "Whether to look in background apps")]
    use_background_apps: bool,
    #[serde(default = "default_true")]
    #[schemars(description = "Whether to activate the app before interaction")]
    activate_app: bool,
}

fn default_true() -> bool {
    true
}

fn default_max_results() -> u32 {
    10
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Find UI elements with a specific role in an application")]
struct McpFindElementsRequest {
    #[schemars(description = "Element selector")]
    selector: McpSelector,
    #[serde(default = "default_max_results")]
    #[schemars(description = "Maximum number of elements to return")]
    max_results: u32,
    #[serde(default)]
    #[schemars(description = "Maximum depth of element tree to search")]
    max_depth: Option<u32>,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Click an element in an application")]
struct McpClickElementRequest {
    #[schemars(description = "Element selector")]
    selector: McpSelector,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Type text into an element in an application")]
struct McpFillElementRequest {
    #[schemars(description = "Element selector")]
    selector: McpSelector,
    #[schemars(description = "The text to type into the element")]
    text: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Scroll an element in a specific direction")]
struct McpScrollElementRequest {
    #[schemars(description = "Element selector")]
    selector: McpSelector,
    #[schemars(description = "The direction to scroll")]
    direction: String,
    #[schemars(description = "The amount to scroll in pixels")]
    amount: u32,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Open an application by name")]
struct McpOpenApplicationRequest {
    #[schemars(description = "The name of the application to open")]
    app_name: String,
}

#[derive(Deserialize, JsonSchema)]
#[schemars(description = "Open a URL in a browser")]
struct McpOpenUrlRequest {
    #[schemars(description = "The URL to open")]
    url: String,
    #[serde(default)]
    #[schemars(description = "The browser to use (optional)")]
    browser: Option<String>,
}

// Tool creation functions using schemars

fn create_search_tool() -> Tool {
    let schema = schema_for!(McpSearchRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "search-content".into(),
        title: None,
        description: Some("Search through cubby recorded content (OCR text, audio transcriptions, UI elements). Use this to find specific content that has appeared on your screen or been spoken. Results include timestamps, app context, and the content itself.".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_pixel_control_tool() -> Tool {
    let schema = schema_for!(McpPixelControlRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "pixel-control".into(),
        title: None,
        description: Some("Control mouse and keyboard at the pixel level. This is a cross-platform tool that works on all operating systems. Use this to type text, press keys, move the mouse, and click buttons.".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_find_elements_tool() -> Tool {
    let schema = schema_for!(McpFindElementsRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "find-elements".into(),
        title: None,
        description: Some("Find UI elements with a specific role in an application. This tool is especially useful for identifying interactive elements.\n\nMacOS Accessibility Roles Guide:\n- Basic roles: 'button', 'textfield', 'checkbox', 'menu', 'list'\n- MacOS specific roles: 'AXButton', 'AXTextField', 'AXCheckBox', 'AXMenu', etc.\n- Text inputs can be: 'AXTextField', 'AXTextArea', 'AXComboBox', 'AXSearchField'\n- Clickable items: 'AXButton', 'AXMenuItem', 'AXMenuBarItem', 'AXImage', 'AXStaticText'\n- Web content may use: 'AXWebArea', 'AXLink', 'AXHeading', 'AXRadioButton'\n\nUse MacOS Accessibility Inspector app to identify the exact roles in your target application.".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_click_element_tool() -> Tool {
    let schema = schema_for!(McpClickElementRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "click-element".into(),
        title: None,
        description: Some("Click an element in an application using its id (MacOS only)".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_fill_element_tool() -> Tool {
    let schema = schema_for!(McpFillElementRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "fill-element".into(),
        title: None,
        description: Some("Type text into an element in an application (MacOS only)".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_scroll_element_tool() -> Tool {
    let schema = schema_for!(McpScrollElementRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "scroll-element".into(),
        title: None,
        description: Some("Scroll an element in a specific direction (MacOS only)".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_open_application_tool() -> Tool {
    let schema = schema_for!(McpOpenApplicationRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "open-application".into(),
        title: None,
        description: Some("Open an application by name".into()),
        input_schema: Arc::new(schema_obj),
        output_schema: None,
        annotations: None,
        icons: None,
    }
}

fn create_open_url_tool() -> Tool {
    let schema = schema_for!(McpOpenUrlRequest);
    let mut schema_obj = serde_json::to_value(&schema.schema)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    
    // Ensure type is set to "object" as a string (MCP requirement)
    schema_obj.insert("type".to_string(), serde_json::Value::String("object".to_string()));
    
    Tool {
        name: "open-url".into(),
        title: None,
        description: Some("Open a URL in a browser".into()),
        input_schema: Arc::new(schema_obj),
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
    // Deserialize MCP request with integer types
    let mcp_args: McpSearchRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid search params: {}", e), None)
    })?;

    // Convert to internal SearchQuery format by building a JSON object with string values
    // that will be properly deserialized by SearchQuery's deserialize_number_from_string
    let mut query_json = serde_json::json!({
        "limit": mcp_args.limit.to_string(),
        "offset": mcp_args.offset.to_string(),
        "content_type": mcp_args.content_type.unwrap_or_else(|| "all".to_string()),
        "include_frames": mcp_args.include_frames,
    });

    if let Some(q) = mcp_args.q {
        query_json["q"] = serde_json::Value::String(q);
    }
    if let Some(start_time) = mcp_args.start_time {
        query_json["start_time"] = serde_json::Value::String(start_time);
    }
    if let Some(end_time) = mcp_args.end_time {
        query_json["end_time"] = serde_json::Value::String(end_time);
    }
    if let Some(app_name) = mcp_args.app_name {
        query_json["app_name"] = serde_json::Value::String(app_name);
    }
    if let Some(window_name) = mcp_args.window_name {
        query_json["window_name"] = serde_json::Value::String(window_name);
    }
    if let Some(frame_name) = mcp_args.frame_name {
        query_json["frame_name"] = serde_json::Value::String(frame_name);
    }
    if let Some(min_length) = mcp_args.min_length {
        query_json["min_length"] = serde_json::Value::Number(min_length.into());
    }
    if let Some(max_length) = mcp_args.max_length {
        query_json["max_length"] = serde_json::Value::Number(max_length.into());
    }
    if let Some(speaker_ids) = mcp_args.speaker_ids {
        query_json["speaker_ids"] = serde_json::Value::String(speaker_ids);
    }
    if let Some(focused) = mcp_args.focused {
        query_json["focused"] = serde_json::Value::Bool(focused);
    }
    if let Some(browser_url) = mcp_args.browser_url {
        query_json["browser_url"] = serde_json::Value::String(browser_url);
    }

    let query: SearchQuery = serde_json::from_value(query_json).map_err(|e| {
        ErrorData::invalid_params(format!("failed to convert search params: {}", e), None)
    })?;

    let result = crate::server::search(Query(query), State(app_state))
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
    let mcp_args: McpPixelControlRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid pixel control params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "action": {
            "type": mcp_args.action.r#type,
            "data": mcp_args.action.data
        }
    });

    let result = reqwest::Client::new()
        .post("http://localhost:3030/experimental/operator/pixel")
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
    let mcp_args: McpFindElementsRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid find elements params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "selector": {
            "app_name": mcp_args.selector.app_name,
            "window_name": mcp_args.selector.window_name,
            "locator": mcp_args.selector.locator,
            "use_background_apps": mcp_args.selector.use_background_apps,
            "activate_app": mcp_args.selector.activate_app,
        },
        "max_results": mcp_args.max_results,
        "max_depth": mcp_args.max_depth,
    });

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
    let mcp_args: McpClickElementRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid click element params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "selector": {
            "app_name": mcp_args.selector.app_name,
            "window_name": mcp_args.selector.window_name,
            "locator": mcp_args.selector.locator,
            "use_background_apps": mcp_args.selector.use_background_apps,
            "activate_app": mcp_args.selector.activate_app,
        }
    });

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
    let mcp_args: McpFillElementRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid fill element params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "selector": {
            "app_name": mcp_args.selector.app_name,
            "window_name": mcp_args.selector.window_name,
            "locator": mcp_args.selector.locator,
            "use_background_apps": mcp_args.selector.use_background_apps,
            "activate_app": mcp_args.selector.activate_app,
        },
        "text": mcp_args.text
    });

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
    let mcp_args: McpScrollElementRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid scroll element params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "selector": {
            "app_name": mcp_args.selector.app_name,
            "window_name": mcp_args.selector.window_name,
            "locator": mcp_args.selector.locator,
            "use_background_apps": mcp_args.selector.use_background_apps,
            "activate_app": mcp_args.selector.activate_app,
        },
        "direction": mcp_args.direction,
        "amount": mcp_args.amount
    });

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
    let mcp_args: McpOpenApplicationRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid open application params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "app_name": mcp_args.app_name
    });

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
    let mcp_args: McpOpenUrlRequest = serde_json::from_value(Value::Object(arguments)).map_err(|e| {
        ErrorData::invalid_params(format!("invalid open url params: {}", e), None)
    })?;

    let payload = serde_json::json!({
        "url": mcp_args.url,
        "browser": mcp_args.browser
    });

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
