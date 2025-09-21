use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use cubby::{app::App, models::users};
use loco_rs::{hash, testing::prelude::*};
use sea_orm::{ActiveModelTrait, ActiveValue};
use serial_test::serial;
use uuid::Uuid;

#[tokio::test]
#[serial]
async fn can_register() {
    request::<App, _, _>(|request, ctx| async move {
        let email = format!("test+{}@loco.com", Uuid::new_v4());
        let payload = serde_json::json!({
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
        assert!(
            user.verify_password("12341234"),
            "Password should be hashed correctly"
        );

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
            "email": email,
            "password": "12341234"
        });

        // First registration should succeed
        let response1 = request.post("/api/auth/register").json(&payload).await;
        assert_eq!(
            response1.status_code(),
            200,
            "First registration should succeed"
        );

        // Second registration with same email should fail
        let payload2 = serde_json::json!({
            "email": email,
            "password": "different_password"
        });

        let response2 = request.post("/api/auth/register").json(&payload2).await;
        println!(
            "Duplicate registration returned status: {}",
            response2.status_code()
        );
        assert_ne!(
            response2.status_code(),
            200,
            "Duplicate registration should fail"
        );

        // Verify only one user exists
        let saved_user = users::Model::find_by_email(&ctx.db, &email).await;
        assert!(saved_user.is_ok(), "User should exist in database");

        let user = saved_user.unwrap();
        assert_eq!(user.email, email, "Should keep the first user's data");
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
            "email": email,
            "password": password
        });

        let register_response = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;
        assert_eq!(
            register_response.status_code(),
            200,
            "Registration should succeed"
        );

        // Now try to login
        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });

        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");

        // Verify we get a token back
        let login_text = login_response.text();
        assert!(
            login_text.contains("token"),
            "Login response should contain a token"
        );
        println!("Login response: {}", login_text);
    })
    .await;
}

#[tokio::test]
#[serial]
async fn cannot_access_movies_without_auth() {
    request::<App, _, _>(|request, _ctx| async move {
        let response = request.get("/api/movies").await;
        println!(
            "Movies endpoint without auth returned status: {}",
            response.status_code()
        );
        assert_eq!(
            response.status_code(),
            401,
            "Movies endpoint should require authentication"
        );

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
            "email": email,
            "password": password
        });
        request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;

        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");
        let login_text = login_response.text();
        println!("Login response for movies test: {}", login_text);
        let login_data: serde_json::Value =
            serde_json::from_str(&login_text).expect("Login response should be valid JSON");
        let token = login_data["token"]
            .as_str()
            .expect("Login response should contain token");

        // Now try to access movies with auth header
        let response = request
            .get("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .await;

        println!(
            "Movies endpoint with auth returned status: {}",
            response.status_code()
        );
        assert_eq!(
            response.status_code(),
            200,
            "Movies endpoint should be accessible with auth"
        );

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
            "email": email,
            "password": password
        });
        request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;

        let login_payload = serde_json::json!({
            "email": email,
            "password": password
        });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200, "Login should succeed");
        let login_text = login_response.text();
        let login_data: serde_json::Value =
            serde_json::from_str(&login_text).expect("Login response should be valid JSON");
        let token = login_data["token"]
            .as_str()
            .expect("Login response should contain token");

        // Create a movie with auth
        let movie_payload = serde_json::json!({
            "title": "Test Movie"
        });
        let response = request
            .post("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .json(&movie_payload)
            .await;

        println!(
            "Create movie endpoint returned status: {}",
            response.status_code()
        );
        assert_eq!(
            response.status_code(),
            200,
            "Should be able to create movie with auth"
        );

        let movie_text = response.text();
        println!("Create movie response: {}", movie_text);
        assert!(
            movie_text.contains("Test Movie"),
            "Response should contain the created movie"
        );

        // Verify we can list our movies and see the one we created
        let list_response = request
            .get("/api/movies")
            .add_header("Authorization", format!("Bearer {}", token))
            .await;
        assert_eq!(
            list_response.status_code(),
            200,
            "Should be able to list movies with auth"
        );

        let list_text = list_response.text();
        println!("List movies response: {}", list_text);
        assert!(
            list_text.contains("Test Movie"),
            "Created movie should appear in list"
        );
    })
    .await;
}

#[tokio::test]
#[ignore]
async fn stytch_access_token_can_access_movies() {
    const ACCESS_TOKEN: &str = "yJhbGciOiJSUzI1NiIsImtpZCI6Imp3ay10ZXN0LTc3NzU3YWRmLTAyZWEtNDFkYS1iY2JmLTUwNzQxNmM0Zjg0YyIsInR5cCI6IkpXVCJ9.eyJhdWQiOlsicHJvamVjdC10ZXN0LTIzZmJhNGZlLWZlM2QtNGQwMy1iNDMyLWE0ZTAxODNjNTg5NiJdLCJleHAiOjE3NTg0NjkxMDcsImlhdCI6MTc1ODQ2NTUwNywiaXNzIjoic3R5dGNoLmNvbS9wcm9qZWN0LXRlc3QtMjNmYmE0ZmUtZmUzZC00ZDAzLWI0MzItYTRlMDE4M2M1ODk2IiwibmJmIjoxNzU4NDY1NTA3LCJzY29wZSI6InJlYWQ6ZXhhbXBsZSB3cml0ZTpleGFtcGxlIiwic3ViIjoibTJtLWNsaWVudC10ZXN0LWJhZGUxOGUwLTFjMmQtNDk1MC05MDU3LTJlZGQ0ZWYzM2M1OSIsInVzZXJfaWQiOiJ1c2VyLXRlc3QtOGE3NmJjNDgtNDI2My00YWZmLWIwNTAtZmUyOTllNGU4NTBlIn0.UrIla5WgS5udhdpSxj1Cjzl6O9qVDL7piHkpumoo1_nO6fVdFkrjxzBgROkopTi38UA5wGO2L0fnaN2VZl2dKRHT0njNtNBEUNOk_D7IewzFfklNQEOg_YsBJAssVLBA60LVnLTUZKllkp8-6SeuFWthghJ923YiLWBI3xubKSDULJ8W7hl9_P9uV49nVxcspRsns5x68eFMss6ifNgpBqW2IR6wce1lKaOoqtt85lYKjRx9XPKUQr_ZPgU4d1k1p0ItVKZEgkf8KrhbKfFEMXYgVMnNGZhBJNVJdtmA42gROyXxSyb-2C2LCbQ8Ai6DrCWU6Atb-qa-fqOHSboArA"; // paste a valid Stytch access token before running this test

    if ACCESS_TOKEN.is_empty() {
        println!("Skipping test because ACCESS_TOKEN is empty. Paste a Stytch access token into the constant to run this test.");
        return;
    }

    let user_id = parse_user_id_from_token(ACCESS_TOKEN)
        .expect("ACCESS_TOKEN must contain a valid custom_claims.user_id");

    request::<App, _, _>(|request, ctx| async move {
        // Ensure the user referenced by the token exists locally
        let user_id_str = &user_id;
        if users::Model::find_by_id(&ctx.db, &user_id_str)
            .await
            .is_err()
        {
            let email = format!("stytch+{}@example.com", user_id);
            let password_hash = hash::hash_password("temporary-password")
                .expect("failed to hash password for seed user");

            let now = chrono::Utc::now();
            let user = users::ActiveModel {
                created_at: ActiveValue::set(now.into()),
                updated_at: ActiveValue::set(now.into()),
                email: ActiveValue::set(email),
                password: ActiveValue::set(password_hash),
                api_key: ActiveValue::set(format!("lo-{}", Uuid::new_v4())),
                ..Default::default()
            };

            user.insert(&ctx.db)
                .await
                .expect("failed to seed user for Stytch token test");
        }

        let response = request
            .get("/api/movies")
            .add_header("Authorization", format!("Bearer {}", ACCESS_TOKEN))
            .await;

        println!(
            "Stytch token movies access returned status: {}",
            response.status_code()
        );
        assert_eq!(
            response.status_code(),
            200,
            "Movies endpoint should respond with 200 for a valid Stytch token"
        );
    })
    .await;
}

fn parse_user_id_from_token(token: &str) -> anyhow::Result<String> {
    let mut segments = token.split('.');
    let _header = segments
        .next()
        .ok_or_else(|| anyhow::anyhow!("token missing header segment"))?;
    let payload = segments
        .next()
        .ok_or_else(|| anyhow::anyhow!("token missing payload segment"))?;

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload.as_bytes()))?;
    let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)?;

    println!("{:?}", payload_json);
    let user_id = payload_json
        .get("user_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("token missing custom_claims.user_id"))?;

    if user_id.is_empty() {
        return Err(anyhow::anyhow!("user_id cannot be empty"));
    }

    Ok(user_id.to_string())
}
