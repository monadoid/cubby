use chrono::{Duration, Utc};
use cubby::{
    app::App,
    models::{client_credentials, users},
};
use jsonwebtoken::{EncodingKey, Header};
use loco_rs::testing::prelude::*;
use serde::Serialize;
use serde_json::{json, Value};
use serial_test::serial;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PROJECT_ID: &str = "project-test";
const PRIVATE_KEY_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----\nMIIEowIBAAKCAQEAxve31HutbK7EGGVALDYqlQBVO1BqT4NecEbFAkwD8MkWxpfN\ngcpWHggqqCSopX/4CrP6uAPih6+NUGoMG1Q4LHqF9EdEiD/RVnQ4477Cx/Tirq/C\n4x9V3xc2Q2vSrD81EjInoFCvAebiTF+nrXffxS2bAacYydER2/FiYEp9QVyt6KWM\n3BbFIy3a5ENRNmb4EEbtOS10iWi1+QSvhKBkXDGaKO4NNH+YKOT0LtWxYQ8xcM8N\ncMehfUnqNiOOZwjBVVRRA/ZWGbClT8iEwjCOQ/giuRaGbwVEODOMmBlmSRlmRCA+\npkvtYW+FNxwkwgsWYLh21X1Fp54gyZxeS0EwUQIDAQABAoIBAQC2eSkWrfZ23jDp\nTrJz2Zlj3VJNe4qEMa7CuaSkXqKeiU1iBIZsqewgzsyQOE6SubH53uXpLWbrmYy4\nWwmPZsW9xQBk75difWS3LQ+fjquERoo+OuM4+NwAJYGWg6RKZA2ACo9c76IQ1HZm\nJLPa2z1V0GWANob9T8hZNh9KwAXvkjnS6utteaG/XRWmk2tylAiHB4GSkWJ85tPd\npL5IBe9uc9dL/XauL4hPggCZHEUPtQ5NE9pU8FPR2qTeFTeF1ySASw1nAByv9pqr\nZWZNwYOBoADOZpCbvNCkQOgk2jdIuXNH0BLvP5BNcQZFRWmTbO5TsERNhRvxIVG6\nN6iU4YChAoGBAOMb+gPStS1C7sWMDG06212+T7221dfT7al4endPmQ6ixWXo4Xgs\n0h5r+yWMB57pRsCVrF4XsJv4JxkkUY3CsTrwXWgLBojIJT4wSztR/7QnqPNyQeo3\nQ3Bzo/pqqSDfIAwN57e1HyJotI6m+nceMkaR3Q+L6fQ7BFPXswPqbtbbAoGBAOBH\nSH0KczaFgvjXyK+NAraWujCoqccaxyIngFJA78q3d2yTX+Hi0R+mT+1kSsH3sFMi\nV283g/QqvlZMAqL+Ktf4eVJ1JeVxWxu+AKuGTklq+MvJXykSLycknA66fvk+1Tdz\nXq24tAkaDJ+UIvpfKNhBfSdxD8BGe7whnA+IIm9DAoGAOkJ3BHwNFitRbUPb/DlZ\nBNdJRXWdrdwj35GUeP7mWKbQ1K/FBzsYO82fg6ZEXjOhfs3mhcy19YzXGtACS8di\nB6iZjZMmffg59ZYV9oW2ftSdtrXcyuSaXEKOEjNCZ7hVVEJM6wd/kSgjCWU0Y1JP\nu2K3vsE5pvlsxsVSmvKMtHkCgYBafShIGxFFLDUdxaJZAiHYHZRd0Y8+oBU8OIfT\nqBOXzNJIYmXLM8KAbI/PDioDfLYNtMtmOhXpS92j3+ModDhBDyWUGWQC4OuLk+ud\nSQEJKjnbrxHP9mBEAMdeQey9D5bjWo8WtHEfQv1Y3WIHdqF6L2IZqcpbH5UI4N6g\nfaK3FQKBgHAFENzWTqAKiLwplFmvSxMRI1QnexG9mOLe6/Ahv+HwV5/t7aIFHmsC\n6POy+zbTnPfXSaqckjcEsQcYJ54e1JwUR8H1q6F+9jC0m4fjqk/8tqDB5PWjkXdD\nENfV5XMDDs7jG2Zy5nuRanX3hf+hjv4Flsf4h0P5BGBThfai0UHd\n-----END RSA PRIVATE KEY-----\n";
const JWK_N: &str = "xve31HutbK7EGGVALDYqlQBVO1BqT4NecEbFAkwD8MkWxpfNgcpWHggqqCSopX_4CrP6uAPih6-NUGoMG1Q4LHqF9EdEiD_RVnQ4477Cx_Tirq_C4x9V3xc2Q2vSrD81EjInoFCvAebiTF-nrXffxS2bAacYydER2_FiYEp9QVyt6KWM3BbFIy3a5ENRNmb4EEbtOS10iWi1-QSvhKBkXDGaKO4NNH-YKOT0LtWxYQ8xcM8NcMehfUnqNiOOZwjBVVRRA_ZWGbClT8iEwjCOQ_giuRaGbwVEODOMmBlmSRlmRCA-pkvtYW-FNxwkwgsWYLh21X1Fp54gyZxeS0EwUQ";
const JWKS_PATH: &str = "sessions/jwks/project-test";
const STYTCH_USER_ID: &str = "user-test-123";
const CLIENT_ID: &str = "m2m-client-test-456";
const CLIENT_SECRET: &str = "secret-test-credential";
const ROTATED_SECRET: &str = "rotated-secret";
const SESSION_ID: &str = "session-test-789";

#[derive(Serialize)]
struct JwtClaims {
    iss: String,
    aud: Vec<String>,
    sub: String,
    user_id: String,
    exp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
}

fn issue_token(sub: &str, user_id: &str, scope: Option<&str>) -> String {
    let header = Header {
        kid: Some("test-key".to_string()),
        ..Header::new(jsonwebtoken::Algorithm::RS256)
    };
    let exp = (Utc::now() + Duration::minutes(60)).timestamp();
    let claims = JwtClaims {
        iss: format!("stytch.com/{}", PROJECT_ID),
        aud: vec![PROJECT_ID.to_string()],
        sub: sub.to_string(),
        user_id: user_id.to_string(),
        exp,
        scope: scope.map(|s| s.to_string()),
    };

    jsonwebtoken::encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(PRIVATE_KEY_PEM.as_bytes()).expect("valid key"),
    )
    .expect("token to be encoded")
}

fn jwks_body() -> Value {
    json!({
        "keys": [
            {
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": "test-key",
                "n": JWK_N,
                "e": "AQAB"
            }
        ]
    })
}

fn setup_stytch_env(base_url: &str) {
    std::env::set_var("STYTCH_TEST_PROJECT_ID", PROJECT_ID);
    std::env::set_var("STYTCH_TEST_SECRET", "test-secret");
    std::env::set_var("STYTCH_TEST_BASE_URL", base_url);
}

#[tokio::test]
#[serial]
async fn user_flow_creates_credentials_and_accesses_movies() {
    let mock_server = MockServer::start().await;
    let base_url = format!("{}/", mock_server.uri());
    setup_stytch_env(&base_url);

    // JWKS fetch (multiple possible paths to handle different request patterns)
    Mock::given(method("GET"))
        .and(path(format!("/{JWKS_PATH}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(jwks_body()))
        .mount(&mock_server)
        .await;
        
    Mock::given(method("GET"))
        .and(path(JWKS_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(jwks_body()))
        .mount(&mock_server)
        .await;

    // Register mock with dynamic JWT response  
    Mock::given(method("POST"))
        .and(path("/passwords"))
        .respond_with(move |req: &wiremock::Request| {
            // Extract the trusted_metadata.stytch_user_id from the request body
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let db_user_id = body["trusted_metadata"]["stytch_user_id"]
                .as_str()
                .unwrap_or(STYTCH_USER_ID);
            
            // Create JWT with the correct database user_id
            let session_jwt = issue_token(SESSION_ID, db_user_id, None);
            
            let response = json!({
                "user_id": STYTCH_USER_ID,
                "session_jwt": session_jwt,
                "session_token": "test-session-token",
                "session": {
                    "expires_at": (Utc::now() + Duration::minutes(60)).to_rfc3339()
                }
            });
            
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(response.to_string())
        })
        .mount(&mock_server)
        .await;

    // Login mock with dynamic JWT response
    Mock::given(method("POST"))
        .and(path("/passwords/authenticate"))
        .respond_with(move |req: &wiremock::Request| {
            // Extract the trusted_metadata.stytch_user_id from the request body
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let db_user_id = body["trusted_metadata"]["stytch_user_id"]
                .as_str()
                .unwrap_or(STYTCH_USER_ID);
            
            // Create JWT with the correct database user_id
            let login_jwt = issue_token("login-session", db_user_id, None);
            
            let response = json!({
                "user_id": STYTCH_USER_ID,
                "session_jwt": login_jwt,
                "session_token": "login-session-token",
                "session": {
                    "expires_at": (Utc::now() + Duration::minutes(60)).to_rfc3339()
                }
            });
            
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/json")
                .set_body_string(response.to_string())
        })
        .mount(&mock_server)
        .await;

    let trusted_metadata = json!({
        "user_id": "placeholder", // overwritten by server data
        "stytch_user_id": STYTCH_USER_ID,
    });

    // Create client credential mock
    Mock::given(method("POST"))
        .and(path("/m2m/clients"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "m2m_client": {
                "client_id": CLIENT_ID,
                "client_secret": CLIENT_SECRET,
                "client_secret_last_four": &CLIENT_SECRET[CLIENT_SECRET.len()-4..],
                "status": "active",
                "scopes": ["movies:read"],
                "trusted_metadata": trusted_metadata,
                "client_name": Value::Null,
                "client_description": "Primary"
            },
            "status_code": 200
        })))
        .mount(&mock_server)
        .await;

    // Rotate mock
    Mock::given(method("POST"))
        .and(path(format!("/m2m/clients/{CLIENT_ID}/secret/rotate")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "m2m_client": {
                "client_id": CLIENT_ID,
                "client_secret": ROTATED_SECRET,
                "client_secret_last_four": &ROTATED_SECRET[ROTATED_SECRET.len()-4..],
                "status": "active",
                "scopes": ["movies:read"],
                "trusted_metadata": trusted_metadata,
                "client_name": Value::Null,
                "client_description": "Primary"
            },
            "status_code": 200
        })))
        .mount(&mock_server)
        .await;

    // Delete mock
    Mock::given(method("DELETE"))
        .and(path(format!("/m2m/clients/{CLIENT_ID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status_code": 200
        })))
        .mount(&mock_server)
        .await;

    // Unique email per test run
    let email = format!("user+{}@example.com", Uuid::new_v4());
    let password = "test-password-123";

    request::<App, _, _>(|request, ctx| async move {
        let register_payload = json!({ "email": email, "password": password });
        let register_response = request
            .post("/api/auth/register")
            .json(&register_payload)
            .await;
        if register_response.status_code() != 200 {
            println!("Registration failed with status: {}", register_response.status_code());
            println!("Response body: {}", register_response.text());
        }
        assert_eq!(register_response.status_code(), 200);
        let register_response_text = register_response.text();
        let register_body: Value =
            serde_json::from_str(&register_response_text).expect("register response to be json");
        let user_access_token = register_body
            .get("access_token")
            .and_then(Value::as_str)
            .expect("access token present")
            .to_string();

        let user = users::Model::find_by_email(&ctx.db, &email)
            .await
            .expect("user created");
        assert_eq!(user.auth_id, STYTCH_USER_ID);

        // Login succeeds via Stytch
        let login_payload = json!({ "email": email, "password": password });
        let login_response = request.post("/api/auth/login").json(&login_payload).await;
        assert_eq!(login_response.status_code(), 200);
        let login_body: Value =
            serde_json::from_str(&login_response.text()).expect("login body json");
        assert!(login_body.get("access_token").is_some());

        
        // Create client credentials
        let create_payload = json!({
            "scopes": ["movies:read"],
            "description": "Primary"
        });
        let create_response = request
            .post("/api/me/clients/create")
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .json(&create_payload)
            .await;
        assert_eq!(create_response.status_code(), 200);
        let response_text = create_response.text();
        let create_body: Value =
            serde_json::from_str(&response_text).expect("create response json");
        let credential_id = create_body
            .get("id")
            .and_then(Value::as_str)
            .and_then(|s| Uuid::parse_str(s).ok())
            .expect("credential id uuid");
        assert_eq!(
            create_body
                .get("client_secret")
                .and_then(Value::as_str)
                .unwrap(),
            CLIENT_SECRET
        );

        let stored_credentials = client_credentials::Model::list_for_user(&ctx.db, user.id)
            .await
            .expect("credential stored");
        assert_eq!(stored_credentials.len(), 1);
        assert_eq!(stored_credentials[0].client_id, CLIENT_ID);

        // List endpoint
        let list_response = request
            .get("/api/me/clients/list")
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(list_response.status_code(), 200);
        let list_body: Value = serde_json::from_str(&list_response.text()).unwrap();
        assert_eq!(list_body.as_array().unwrap().len(), 1);

        // Rotate secret
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

        // Delete credential
        let delete_response = request
            .delete(&format!("/api/me/clients/{}", credential_id))
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(delete_response.status_code(), 200);
        let remaining = client_credentials::Model::list_for_user(&ctx.db, user.id)
            .await
            .expect("query to succeed");
        assert!(remaining.is_empty());

        // Access movies with user token
        let movies_response = request
            .get("/api/movies/list")
            .add_header("Authorization", format!("Bearer {}", user_access_token))
            .await;
        assert_eq!(movies_response.status_code(), 200);

        // Access movies with machine token  
        let machine_token = issue_token(CLIENT_ID, &user.id.to_string(), Some("movies:read"));
        let machine_response = request
            .get("/api/movies/list")
            .add_header("Authorization", format!("Bearer {}", machine_token))
            .await;
        assert_eq!(machine_response.status_code(), 200);
    })
    .await;
}
