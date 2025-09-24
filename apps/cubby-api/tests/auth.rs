use cubby::{
    app::App,
    models::{client_credentials, users},
};
use loco_rs::testing::prelude::*;
use loco_rs::TestServer;
use serde_json::{json, Value};
use serial_test::serial;
use stytch_mock_server::StytchMockServer;
use uuid::Uuid;

mod stytch_mock_server;

const STYTCH_USER_ID: &str = "user-test-123";
const CLIENT_ID: &str = "m2m-client-test-456";
const ROTATED_SECRET: &str = "rotated-secret";

async fn create_client_credentials(request: &TestServer, user_token: &str) -> Uuid {
    let create_payload = json!({
        "scopes": ["movies:read"],
        "description": "Primary"
    });
    let create_response = request
        .post("/api/me/clients/create")
        .add_header("Authorization", format!("Bearer {}", user_token))
        .json(&create_payload)
        .await;
    assert_eq!(create_response.status_code(), 200);

    let create_body: Value = serde_json::from_str(&create_response.text()).unwrap();
    create_body
        .get("id")
        .and_then(Value::as_str)
        .and_then(|s| Uuid::parse_str(s).ok())
        .expect("credential id uuid")
}

#[tokio::test]
#[serial]
async fn test_movies_require_authentication() {
    let mock_server = StytchMockServer::new().await;
    mock_server.setup_all_endpoints().await;

    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/movies/list").await;
        assert_eq!(response.status_code(), 401);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn test_create_user_and_credentials() {
    let mock_server = StytchMockServer::new().await;
    mock_server.setup_all_endpoints().await;

    let email = format!("test-user+{}@example.com", Uuid::new_v4());
    let password = "test-password-123";

    request::<App, _, _>(|request, ctx| async move {
        let register_payload = json!({ "email": email, "password": password });
        let register_response = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;
        assert_eq!(register_response.status_code(), 200);

        let register_body: Value = serde_json::from_str(&register_response.text()).unwrap();
        let user_access_token = register_body
            .get("access_token")
            .and_then(Value::as_str)
            .expect("access token present")
            .to_string();

        let user = users::Model::find_by_email(&ctx.db, &email)
            .await
            .expect("user created");
        assert_eq!(user.auth_id, STYTCH_USER_ID);

        let _credential_id = create_client_credentials(&request, &user_access_token).await;
        let stored_credentials = client_credentials::Model::list_for_user(&ctx.db, user.id)
            .await
            .expect("credential stored");
        assert_eq!(stored_credentials.len(), 1);
        assert_eq!(stored_credentials[0].client_id, CLIENT_ID);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn user_flow_creates_credentials_and_accesses_movies() {
    let mock_server = StytchMockServer::new().await;
    mock_server.setup_all_endpoints().await;

    let email = format!("user+{}@example.com", Uuid::new_v4());
    let password = "test-password-123";

    request::<App, _, _>(|request, ctx| async move {
        let register_payload = json!({ "email": email, "password": password });
        let register_response = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;
        assert_eq!(register_response.status_code(), 200);

        let register_body: Value = serde_json::from_str(&register_response.text()).unwrap();
        let user_access_token = register_body
            .get("access_token")
            .and_then(Value::as_str)
            .expect("access token present")
            .to_string();

        let user = users::Model::find_by_email(&ctx.db, &email)
            .await
            .expect("user created");
        assert_eq!(user.auth_id, STYTCH_USER_ID);

        let login_payload = json!({ "email": email, "password": password });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200);
        let login_body: Value = serde_json::from_str(&login_response.text()).unwrap();
        assert!(login_body.get("access_token").is_some());

        let credential_id = create_client_credentials(&request, &user_access_token).await;

        let list_response = request
            .get("/api/me/clients/list")
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(list_response.status_code(), 200);
        let list_body: Value = serde_json::from_str(&list_response.text()).unwrap();
        assert_eq!(list_body.as_array().unwrap().len(), 1);

        let rotate_response = request
            .post(&format!("/api/me/clients/{}/rotate", credential_id))
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(rotate_response.status_code(), 200);
        let rotate_body: Value = serde_json::from_str(&rotate_response.text()).unwrap();
        assert_eq!(
            rotate_body["client_secret"].as_str().unwrap(),
            ROTATED_SECRET
        );

        let delete_response = request
            .delete(&format!("/api/me/clients/{}", credential_id))
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(delete_response.status_code(), 200);
        let remaining = client_credentials::Model::list_for_user(&ctx.db, user.id)
            .await
            .expect("query to succeed");
        assert!(remaining.is_empty());

        let movies_response = request
            .get("/api/movies/list")
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(movies_response.status_code(), 200);

        let machine_token =
            mock_server.issue_token(CLIENT_ID, &user.id.to_string(), Some("movies:read"));
        let machine_response = request
            .get("/api/movies/list")
            .add_header("Authorization", format!("Bearer {}", machine_token))
            .await;
        assert_eq!(machine_response.status_code(), 200);
    })
    .await;
}
