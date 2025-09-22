use cubby::app::App;
use loco_rs::testing::prelude::*;
use serial_test::serial;
use serde_json::{json, Value};
use uuid::Uuid;

// Import our mock servers
mod stytch_mock_server;
mod css_mock_server;

use stytch_mock_server::StytchMockServer;
use css_mock_server::CssMockServer;

#[tokio::test]
#[serial]
async fn can_list_empty_pods_for_authenticated_user() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create a test user
        let user = prepare_data::user(&ctx.db, "test@example.com").await;
        
        // Get JWT token for authentication
        let user_id = user.id.to_string();
        let token = stytch_mock.issue_token("session-test", &user_id, Some("pods:read"));
        
        // Test listing pods (should be empty initially)
        let res = request
            .get("/api/pods/list")
            .add_header("authorization", format!("Bearer {}", token))
            .await;
        
        assert_eq!(res.status_code(), 200);
        
        let body: Value = serde_json::from_str(&res.text()).unwrap();
        assert!(body.as_array().unwrap().is_empty(), "Pod list should be empty initially");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_create_pod_for_authenticated_user() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create a test user
        let user = prepare_data::user(&ctx.db, "test@example.com").await;
        
        // Get JWT token for authentication
        let user_id = user.id.to_string();
        let token = stytch_mock.issue_token("session-test", &user_id, Some("pods:write"));
        
        // Create a pod
        let pod_data = json!({
            "name": "my-awesome-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", token))
            .json(&pod_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        
        let body: Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["name"], "my-awesome-pod");
        assert!(body["link"].as_str().unwrap().contains("my-awesome-pod"));
        assert_eq!(body["user_id"], user.id.to_string());
        
        // Check CSS provisioning fields are present
        assert!(body["css_account_token"].is_string());
        assert!(body["css_client_id"].is_string());
        assert!(body["css_client_secret"].is_string());
        assert!(body["css_client_resource_url"].is_string());
        assert!(body["webid"].is_string());
        assert_eq!(body["css_email"], css_mock.test_user_email());
        
        // Verify pod exists in database
        let created_id = body["id"].as_str().unwrap();
        let res = request
            .get(&format!("/api/pods/{}", created_id))
            .add_header("authorization", format!("Bearer {}", token))
            .await;
        
        assert_eq!(res.status_code(), 200);
        let retrieved: Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(retrieved["id"], created_id);
        assert_eq!(retrieved["name"], "my-awesome-pod");
        
        // Verify CSS provisioning fields are persisted
        assert!(retrieved["css_account_token"].is_string());
        assert!(retrieved["css_client_id"].is_string());
        assert!(retrieved["css_client_secret"].is_string());
        assert!(retrieved["css_client_resource_url"].is_string());
        assert!(retrieved["webid"].is_string());
        assert_eq!(retrieved["css_email"], css_mock.test_user_email());
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_create_multiple_pods_per_user() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create a test user
        let user = prepare_data::user(&ctx.db, "test@example.com").await;
        
        // Get JWT token for authentication
        let user_id = user.id.to_string();
        let token = stytch_mock.issue_token("session-test", &user_id, Some("pods:write"));
        
        // Create first pod
        let pod_data = json!({
            "name": "first-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", token))
            .json(&pod_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        
        // Try to create second pod (should fail)
        let second_pod_data = json!({
            "name": "second-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", token))
            .json(&second_pod_data)
            .await;
        
        assert_eq!(res.status_code(), 400);
        let error_body: Value = serde_json::from_str(&res.text()).unwrap();
        assert!(error_body["error"].as_str().unwrap().contains("already has a pod"));
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_update_pod_for_authenticated_user() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create a test user
        let user = prepare_data::user(&ctx.db, "test@example.com").await;
        
        // Get JWT token for authentication
        let user_id = user.id.to_string();
        let token = stytch_mock.issue_token("session-test", &user_id, Some("pods:write"));
        
        // Create a pod
        let pod_data = json!({
            "name": "original-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", token))
            .json(&pod_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        let created: Value = serde_json::from_str(&res.text()).unwrap();
        let pod_id = created["id"].as_str().unwrap();
        
        // Update the pod
        let update_data = json!({
            "name": "updated-pod"
        });
        
        let res = request
            .put(&format!("/api/pods/{}", pod_id))
            .add_header("authorization", format!("Bearer {}", token))
            .json(&update_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        
        let body: Value = serde_json::from_str(&res.text()).unwrap();
        assert_eq!(body["name"], "updated-pod");
        assert_eq!(body["id"], pod_id);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_delete_pod_for_authenticated_user() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create a test user
        let user = prepare_data::user(&ctx.db, "test@example.com").await;
        
        // Get JWT token for authentication
        let user_id = user.id.to_string();
        let token = stytch_mock.issue_token("session-test", &user_id, Some("pods:write"));
        
        // Create a pod
        let pod_data = json!({
            "name": "temporary-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", token))
            .json(&pod_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        let created: Value = serde_json::from_str(&res.text()).unwrap();
        let pod_id = created["id"].as_str().unwrap();
        
        // Delete the pod
        let res = request
            .delete(&format!("/api/pods/{}", pod_id))
            .add_header("authorization", format!("Bearer {}", token))
            .await;
        
        assert_eq!(res.status_code(), 200);
        
        // Verify pod is deleted
        let res = request
            .get(&format!("/api/pods/{}", pod_id))
            .add_header("authorization", format!("Bearer {}", token))
            .await;
        
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_access_other_users_pods() {
    // Setup mock servers
    let stytch_mock = StytchMockServer::new().await;
    let css_mock = CssMockServer::new().await;
    
    stytch_mock.setup_all_endpoints().await;
    css_mock.setup_all_endpoints().await;

    request::<App, _, _>(|request, ctx| async move {
        // Create two test users
        let user1 = prepare_data::user(&ctx.db, "user1@example.com").await;
        let user2 = prepare_data::user(&ctx.db, "user2@example.com").await;
        
        // Get JWT tokens for both users
        let user1_token = stytch_mock.issue_token("session-test", &user1.id.to_string(), Some("pods:write"));
        let user2_token = stytch_mock.issue_token("session-test", &user2.id.to_string(), Some("pods:write"));
        
        // User 1 creates a pod
        let pod_data = json!({
            "name": "user1-pod",
            "email": css_mock.test_user_email(),
            "password": css_mock.test_user_password()
        });
        
        let res = request
            .post("/api/pods/create")
            .add_header("authorization", format!("Bearer {}", user1_token))
            .json(&pod_data)
            .await;
        
        assert_eq!(res.status_code(), 200);
        let created: Value = serde_json::from_str(&res.text()).unwrap();
        let pod_id = created["id"].as_str().unwrap();
        
        // User 2 tries to access User 1's pod (should fail)
        let res = request
            .get(&format!("/api/pods/{}", pod_id))
            .add_header("authorization", format!("Bearer {}", user2_token))
            .await;
        
        assert_eq!(res.status_code(), 404);
        
        // User 2 tries to delete User 1's pod (should fail)
        let res = request
            .delete(&format!("/api/pods/{}", pod_id))
            .add_header("authorization", format!("Bearer {}", user2_token))
            .await;
        
        assert_eq!(res.status_code(), 404);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn requires_authentication_for_pod_operations() {
    request::<App, _, _>(|request, _ctx| async move {
        // Try to list pods without authentication
        let res = request.get("/api/pods/list").await;
        assert_eq!(res.status_code(), 401);
        
        // Try to create pod without authentication
        let pod_data = json!({
            "name": "unauthorized-pod",
            "email": "test@example.com",
            "password": "test-password"
        });
        
        let res = request
            .post("/api/pods/create")
            .json(&pod_data)
            .await;
        assert_eq!(res.status_code(), 401);
        
        // Try to get specific pod without authentication
        let fake_id = Uuid::new_v4();
        let res = request.get(&format!("/api/pods/{}", fake_id)).await;
        assert_eq!(res.status_code(), 401);
    })
    .await;
}
