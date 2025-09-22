use wiremock::{Mock, MockServer, ResponseTemplate, Request};
use serde_json::{json, Value};
use wiremock::matchers::{method, path, path_regex, header};
use uuid::Uuid;

const CSS_ACCOUNT_TOKEN: &str = "css-account-test-token-123";
const TEST_USER_EMAIL: &str = "test@example.com";
const TEST_USER_PASSWORD: &str = "test-password";
const TEST_POD_NAME: &str = "test-pod";
const TEST_BASE_URL: &str = "http://localhost:3000/";

pub struct CssMockServer {
    server: MockServer,
}

impl CssMockServer {
    pub async fn new() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    pub fn base_url(&self) -> String {
        format!("{}/.account/", self.server.uri())
    }

    pub fn setup_env(&self) {
        let server_base = self.server.uri();
        std::env::set_var("CSS_TEST_BASE", &server_base);
        std::env::set_var("CSS_TEST_ACCOUNT_INDEX", &self.base_url());
        std::env::set_var("CSS_TEST_AUTH_TOKEN", CSS_ACCOUNT_TOKEN);
    }

    pub fn auth_token(&self) -> &str {
        CSS_ACCOUNT_TOKEN
    }

    /// Sets up the CSS account controls index endpoint
    async fn setup_controls_endpoint(&self) {
        let controls_response = json!({
            "main": {
                "index": format!("{}/.account/", self.server.uri()),
                "logins": format!("{}/.account/login/", self.server.uri())
            },
            "account": {
                "create": format!("{}/.account/account/", self.server.uri()),
                "logout": format!("{}/.account/account/test-account-id/logout/", self.server.uri()),
                "webId": format!("{}/.account/account/test-account-id/webid/", self.server.uri()),
                "pod": format!("{}/.account/account/test-account-id/pod/", self.server.uri()),
                "clientCredentials": format!("{}/.account/account/test-account-id/client-credentials/", self.server.uri())
            },
            "password": {
                "create": format!("{}/.account/account/test-account-id/login/password/", self.server.uri()),
                "login": format!("{}/.account/login/password/", self.server.uri()),
                "forgot": format!("{}/.account/login/password/forgot/", self.server.uri()),
                "reset": format!("{}/.account/login/password/reset/", self.server.uri())
            }
        });

        Mock::given(method("GET"))
            .and(path("/.account/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(controls_response))
            .mount(&self.server)
            .await;
    }

    /// Sets up password authentication endpoints
    async fn setup_auth_endpoints(&self) {
        // Login endpoint
        Mock::given(method("POST"))
            .and(path("/.account/login/password/"))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
                
                if body.get("email").and_then(|e| e.as_str()) == Some(TEST_USER_EMAIL)
                    && body.get("password").and_then(|p| p.as_str()) == Some(TEST_USER_PASSWORD)
                {
                    ResponseTemplate::new(200)
                        .insert_header("set-cookie", format!("css-account={}", CSS_ACCOUNT_TOKEN))
                        .set_body_json(json!({
                            "authorization": CSS_ACCOUNT_TOKEN,
                            "location": format!("{}/.account/account/test-account-id/", TEST_BASE_URL)
                        }))
                } else {
                    ResponseTemplate::new(401)
                        .set_body_json(json!({
                            "error": "Invalid credentials"
                        }))
                }
            })
            .mount(&self.server)
            .await;

        // Logout endpoint
        Mock::given(method("POST"))
            .and(path("/.account/account/test-account-id/logout/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&self.server)
            .await;
    }

    /// Sets up pod management endpoints
    async fn setup_pod_endpoints(&self) {
        // Get all pods for account
        Mock::given(method("GET"))
            .and(path("/.account/account/test-account-id/pod/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "pods": {}
            })))
            .mount(&self.server)
            .await;

        // Create pod endpoint
        Mock::given(method("POST"))
            .and(path("/.account/account/test-account-id/pod/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
                let pod_name = body.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or(TEST_POD_NAME);
                
                let pod_base_url = format!("http://localhost:3000/{}/", pod_name);
                let default_web_id = format!("{}profile/card#me", pod_base_url);
                let web_id = body.get("settings")
                    .and_then(|s| s.get("webId"))
                    .and_then(|w| w.as_str())
                    .unwrap_or(&default_web_id);

                let pod_resource_url = format!(
                    "{}/.account/account/test-account-id/pod/{}/", 
                    TEST_BASE_URL,
                    Uuid::new_v4()
                );

                ResponseTemplate::new(201)
                    .set_body_json(json!({
                        "baseUrl": pod_base_url,
                        "webId": web_id,
                        "podResourceUrl": pod_resource_url
                    }))
            })
            .mount(&self.server)
            .await;

        // Update pod owners endpoint (generic pod resource)
        Mock::given(method("POST"))
            .and(path_regex(r"/.account/account/test-account-id/pod/[^/]+/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
                
                if body.get("remove").and_then(|r| r.as_bool()) == Some(true) {
                    // Remove owner/delete pod
                    ResponseTemplate::new(200).set_body_json(json!({}))
                } else {
                    // Add/update owner
                    ResponseTemplate::new(200).set_body_json(json!({
                        "baseUrl": format!("http://localhost:3000/{}/", TEST_POD_NAME),
                        "owners": [
                            {
                                "webId": body.get("webId").unwrap_or(&json!("http://localhost:3000/test-pod/profile/card#me")),
                                "visible": body.get("visible").unwrap_or(&json!(false))
                            }
                        ]
                    }))
                }
            })
            .mount(&self.server)
            .await;

        // Get specific pod info endpoint
        Mock::given(method("GET"))
            .and(path_regex(r"/.account/account/test-account-id/pod/[^/]+/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "baseUrl": format!("http://localhost:3000/{}/", TEST_POD_NAME),
                "owners": [
                    {
                        "webId": format!("http://localhost:3000/{}/profile/card#me", TEST_POD_NAME),
                        "visible": false
                    }
                ]
            })))
            .mount(&self.server)
            .await;
    }

    /// Sets up WebID management endpoints
    async fn setup_webid_endpoints(&self) {
        // Get WebIDs
        Mock::given(method("GET"))
            .and(path("/.account/account/test-account-id/webid/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "webIdLinks": {}
            })))
            .mount(&self.server)
            .await;

        // Link WebID
        Mock::given(method("POST"))
            .and(path("/.account/account/test-account-id/webid/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
                let web_id = body.get("webId")
                    .and_then(|w| w.as_str())
                    .unwrap_or("http://localhost:3000/test-pod/profile/card#me");

                let webid_resource_url = format!(
                    "{}/.account/account/test-account-id/webid/{}/", 
                    TEST_BASE_URL,
                    Uuid::new_v4()
                );

                ResponseTemplate::new(200)
                    .set_body_json(json!({
                        "webId": web_id,
                        "resourceUrl": webid_resource_url
                    }))
            })
            .mount(&self.server)
            .await;
    }

    /// Sets up client credentials endpoints
    async fn setup_client_credentials_endpoints(&self) {
        // Create client credentials endpoint
        Mock::given(method("POST"))
            .and(path("/.account/account/test-account-id/client-credentials/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(|req: &Request| {
                let body: Value = serde_json::from_slice(&req.body).unwrap_or_default();
                let name = body.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("cubby-test");
                let web_id = body.get("webId")
                    .and_then(|w| w.as_str())
                    .unwrap_or("http://localhost:3000/test-pod/profile/card#me");

                let client_id = format!("client-{}", Uuid::new_v4());
                let client_secret = format!("secret-{}", Uuid::new_v4());
                let resource_url = format!(
                    "{}/.account/account/test-account-id/client-credentials/{}/", 
                    TEST_BASE_URL,
                    Uuid::new_v4()
                );

                ResponseTemplate::new(201)
                    .set_body_json(json!({
                        "id": client_id,
                        "secret": client_secret,
                        "resource": resource_url,
                        "name": name,
                        "webId": web_id
                    }))
            })
            .mount(&self.server)
            .await;

        // Get client credentials endpoint
        Mock::given(method("GET"))
            .and(path("/.account/account/test-account-id/client-credentials/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "clientCredentials": {}
            })))
            .mount(&self.server)
            .await;

        // Delete client credentials endpoint (for cleanup)
        Mock::given(method("DELETE"))
            .and(path_regex(r"/.account/account/test-account-id/client-credentials/[^/]+/"))
            .and(header("authorization", format!("CSS-Account-Token {}", CSS_ACCOUNT_TOKEN)))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
            .mount(&self.server)
            .await;
    }

    /// Sets up account creation endpoint (for completeness)
    async fn setup_account_endpoints(&self) {
        Mock::given(method("POST"))
            .and(path("/.account/account/"))
            .respond_with(ResponseTemplate::new(201)
                .insert_header("set-cookie", format!("css-account={}", CSS_ACCOUNT_TOKEN))
                .set_body_json(json!({
                    "authorization": CSS_ACCOUNT_TOKEN,
                    "accountId": "test-account-id"
                })))
            .mount(&self.server)
            .await;
    }

    pub async fn setup_all_endpoints(&self) {
        self.setup_env();
        self.setup_controls_endpoint().await;
        self.setup_auth_endpoints().await;
        self.setup_pod_endpoints().await;
        self.setup_webid_endpoints().await;
        self.setup_client_credentials_endpoints().await;
        self.setup_account_endpoints().await;
    }

    // Helper methods for tests
    pub fn test_user_email(&self) -> &str {
        TEST_USER_EMAIL
    }

    pub fn test_user_password(&self) -> &str {
        TEST_USER_PASSWORD
    }

    pub fn test_pod_name(&self) -> &str {
        TEST_POD_NAME
    }
}