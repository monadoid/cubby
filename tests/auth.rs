use cubby::{app::App, models::users};
use loco_rs::testing::prelude::*;
use serial_test::serial;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn can_register() {
    request::<App, _, _>(|request, ctx| async move {
        let email = format!("test+{}@loco.com", Uuid::new_v4());
        let payload = serde_json::json!({
            "name": "loco",
            "email": email,
            "password": "12341234"
        });

        let response = request.post("/api/auth/register").json(&payload).await;
        assert_eq!(
            response.status_code(),
            200,
            "Register request should succeed"
        );
        
        let saved_user = users::Model::find_by_email(&ctx.db, &email).await;
        assert!(saved_user.is_ok(), "User should be saved to database");
        
        let user = saved_user.unwrap();
        assert_eq!(user.email, email);
        assert_eq!(user.name, "loco");
        assert!(user.verify_password("12341234"), "Password should be hashed correctly");

        let deliveries = ctx.mailer.unwrap().deliveries();
        assert_eq!(deliveries.count, 1, "Exactly one email should be sent");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_register_duplicate_email() {
    request::<App, _, _>(|request, ctx| async move {
        let email = format!("duplicate+{}@loco.com", Uuid::new_v4());
        let payload = serde_json::json!({
            "name": "first_user",
            "email": email,
            "password": "12341234"
        });

        // First registration should succeed
        let response1 = request.post("/api/auth/register").json(&payload).await;
        assert_eq!(response1.status_code(), 200, "First registration should succeed");

        // Second registration with same email should fail
        let payload2 = serde_json::json!({
            "name": "second_user",
            "email": email,
            "password": "different_password"
        });
        
        let response2 = request.post("/api/auth/register").json(&payload2).await;
        println!("Duplicate registration returned status: {}", response2.status_code());
        assert_ne!(response2.status_code(), 200, "Duplicate registration should fail");
        
        // Verify only one user exists
        let saved_user = users::Model::find_by_email(&ctx.db, &email).await;
        assert!(saved_user.is_ok(), "User should exist in database");
        
        let user = saved_user.unwrap();
        assert_eq!(user.name, "first_user", "Should keep the first user's data");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_login_after_register() {
    request::<App, _, _>(|request, _ctx| async move {
        let email = format!("login+{}@loco.com", Uuid::new_v4());
        let password = "test_password123";
        
        // First register a user
        let register_payload = serde_json::json!({
            "name": "login_user",
            "email": email,
            "password": password
        });

        let register_response = request.post("/api/auth/register").json(&register_payload).await;
        assert_eq!(register_response.status_code(), 200, "Registration should succeed");

        // Now try to login
        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });

        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");
        
        // Verify we get a token back
        let login_text = login_response.text();
        assert!(login_text.contains("token"), "Login response should contain a token");
        println!("Login response: {}", login_text);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_access_movies_without_auth() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/movies").await;
        println!("Movies endpoint without auth returned status: {}", response.status_code());
        assert_eq!(response.status_code(), 401, "Movies endpoint should require authentication");
        
        let movies_text = response.text();
        println!("Movies response: {}", movies_text);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_access_movies_with_auth() {
    request::<App, _, _>(|request, _ctx| async move {
        let email = format!("movies+{}@loco.com", Uuid::new_v4());
        let password = "test_password123";
        
        // Register and login to get a token
        let register_payload = serde_json::json!({
            "name": "movies_user",
            "email": email,
            "password": password
        });
        request.post("/api/auth/register").json(&register_payload).await;

        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");
        let login_text = login_response.text();
        println!("Login response for movies test: {}", login_text);
        let login_data: serde_json::Value = serde_json::from_str(&login_text)
            .expect("Login response should be valid JSON");
        let token = login_data["token"].as_str().expect("Login response should contain token");

        // Now try to access movies with auth header
        let response = request
            .get("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .await;
            
        println!("Movies endpoint with auth returned status: {}", response.status_code());
        assert_eq!(response.status_code(), 200, "Movies endpoint should be accessible with auth");
        
        let movies_text = response.text();
        println!("Movies response with auth: {}", movies_text);
        assert!(movies_text.contains("["), "Response should be a JSON array");
    })
    .await;
}

#[tokio::test]
#[serial]
async fn can_create_movie_with_auth() {
    request::<App, _, _>(|request, _ctx| async move {
        let email = format!("movie_create+{}@loco.com", Uuid::new_v4());
        let password = "test_password123";
        
        // Register and login to get a token
        let register_payload = serde_json::json!({
            "name": "movie_creator",
            "email": email,
            "password": password
        });
        request.post("/api/auth/register").json(&register_payload).await;

        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");
        let login_text = login_response.text();
        let login_data: serde_json::Value = serde_json::from_str(&login_text)
            .expect("Login response should be valid JSON");
        let token = login_data["token"].as_str().expect("Login response should contain token");

        // Create a movie with auth
        let movie_payload = serde_json::json!({
            "title": "Test Movie"
        });
        let response = request
            .post("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .json(&movie_payload)
            .await;
            
        println!("Create movie endpoint returned status: {}", response.status_code());
        assert_eq!(response.status_code(), 200, "Should be able to create movie with auth");
        
        let movie_text = response.text();
        println!("Create movie response: {}", movie_text);
        assert!(movie_text.contains("Test Movie"), "Response should contain the created movie");
        
        // Verify we can list our movies and see the one we created
        let list_response = request
            .get("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .await;
        assert_eq!(list_response.status_code(), 200, "Should be able to list movies with auth");
        
        let list_text = list_response.text();
        println!("List movies response: {}", list_text);
        assert!(list_text.contains("Test Movie"), "Created movie should appear in list");
    })
    .await;
}