use wiremock::{Mock, MockServer, ResponseTemplate};
use jsonwebtoken::{EncodingKey, Header};
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use serde::Serialize;

const PROJECT_ID: &str = "project-test";
const PRIVATE_KEY_PEM: &str = "-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEAxve31HutbK7EGGVALDYqlQBVO1BqT4NecEbFAkwD8MkWxpfN
gcpWHggqqCSopX/4CrP6uAPih6+NUGoMG1Q4LHqF9EdEiD/RVnQ4477Cx/Tirq/C
4x9V3xc2Q2vSrD81EjInoFCvAebiTF+nrXffxS2bAacYydER2/FiYEp9QVyt6KWM
3BbFIy3a5ENRNmb4EEbtOS10iWi1+QSvhKBkXDGaKO4NNH+YKOT0LtWxYQ8xcM8N
cMehfUnqNiOOZwjBVVRRA/ZWGbClT8iEwjCOQ/giuRaGbwVEODOMmBlmSRlmRCA+
pkvtYW+FNxwkwgsWYLh21X1Fp54gyZxeS0EwUQIDAQABAoIBAQC2eSkWrfZ23jDp
TrJz2Zlj3VJNe4qEMa7CuaSkXqKeiU1iBIZsqewgzsyQOE6SubH53uXpLWbrmYy4
WwmPZsW9xQBk75difWS3LQ+fjquERoo+OuM4+NwAJYGWg6RKZA2ACo9c76IQ1HZm
JLPa2z1V0GWANob9T8hZNh9KwAXvkjnS6utteaG/XRWmk2tylAiHB4GSkWJ85tPd
pL5IBe9uc9dL/XauL4hPggCZHEUPtQ5NE9pU8FPR2qTeFTeF1ySASw1nAByv9pqr
ZWZNwYOBoADOZpCbvNCkQOgk2jdIuXNH0BLvP5BNcQZFRWmTbO5TsERNhRvxIVG6
N6iU4YChAoGBAOMb+gPStS1C7sWMDG06212+T7221dfT7al4endPmQ6ixWXo4Xgs
0h5r+yWMB57pRsCVrF4XsJv4JxkkUY3CsTrwXWgLBojIJT4wSztR/7QnqPNyQeo3
Q3Bzo/pqqSDfIAwN57e1HyJotI6m+nceMkaR3Q+L6fQ7BFPXswPqbtbbAoGBAOBH
SH0KczaFgvjXyK+NAraWujCoqccaxyIngFJA78q3d2yTX+Hi0R+mT+1kSsH3sFMi
V283g/QqvlZMAqL+Ktf4eVJ1JeVxWxu+AKuGTklq+MvJXykSLycknA66fvk+1Tdz
Xq24tAkaDJ+UIvpfKNhBfSdxD8BGe7whnA+IIm9DAoGAOkJ3BHwNFitRbUPb/DlZ
BNdJRXWdrdwj35GUeP7mWKbQ1K/FBzsYO82fg6ZEXjOhfs3mhcy19YzXGtACS8di
B6iZjZMmffg59ZYV9oW2ftSdtrXcyuSaXEKOEjNCZ7hVVEJM6wd/kSgjCWU0Y1JP
u2K3vsE5pvlsxsVSmvKMtHkCgYBafShIGxFFLDUdxaJZAiHYHZRd0Y8+oBU8OIfT
qBOXzNJIYmXLM8KAbI/PDioDfLYNtMtmOhXpS92j3+ModDhBDyWUGWQC4OuLk+ud
SQEJKjnbrxHP9mBEAMdeQey9D5bjWo8WtHEfQv1Y3WIHdqF6L2IZqcpbH5UI4N6g
faK3FQKBgHAFENzWTqAKiLwplFmvSxMRI1QnexG9mOLe6/Ahv+HwV5/t7aIFHmsC
6POy+zbTnPfXSaqckjcEsQcYJ54e1JwUR8H1q6F+9jC0m4fjqk/8tqDB5PWjkXdD
ENfV5XMDDs7jG2Zy5nuRanX3hf+hjv4Flsf4h0P5BGBThfai0UHd
-----END RSA PRIVATE KEY-----
";
const JWK_N: &str = "xve31HutbK7EGGVALDYqlQBVO1BqT4NecEbFAkwD8MkWxpfNgcpWHggqqCSopX_4CrP6uAPih6-NUGoMG1Q4LHqF9EdEiD_RVnQ4477Cx_Tirq_C4x9V3xc2Q2vSrD81EjInoFCvAebiTF-nrXffxS2bAacYydER2_FiYEp9QVyt6KWM3BbFIy3a5ENRNmb4EEbtOS10iWi1-QSvhKBkXDGaKO4NNH-YKOT0LtWxYQ8xcM8NcMehfUnqNiOOZwjBVVRRA_ZWGbClT8iEwjCOQ_giuRaGbwVEODOMmBlmSRlmRCA-pkvtYW-FNxwkwgsWYLh21X1Fp54gyZxeS0EwUQ";
const JWKS_PATH: &str = "sessions/jwks/project-test";
const STYTCH_USER_ID: &str = "user-test-123";
const CLIENT_ID: &str = "m2m-client-test-456";
const CLIENT_SECRET: &str = "secret-test-credential";
const ROTATED_SECRET: &str = "rotated-secret";
const SESSION_ID: &str = "session-test-789";

#[derive(Serialize)]
pub struct JwtClaims {
    pub iss: String,
    pub aud: Vec<String>,
    pub sub: String,
    pub user_id: String,
    pub exp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

pub struct StytchMockServer {
    server: MockServer,
}

impl StytchMockServer {
    pub(crate) async fn new() -> Self {
        let server = MockServer::start().await;
        Self { server }
    }

    fn base_url(&self) -> String {
        format!("{}/", self.server.uri())
    }

    fn setup_env(&self) {
        std::env::set_var("STYTCH_TEST_PROJECT_ID", PROJECT_ID);
        std::env::set_var("STYTCH_TEST_SECRET", "test-secret");
        std::env::set_var("STYTCH_TEST_BASE_URL", &self.base_url());
    }

    pub(crate) fn issue_token(&self, sub: &str, user_id: &str, scope: Option<&str>) -> String {
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

    fn jwks_body(&self) -> Value {
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

    async fn setup_jwks_endpoint(&self) {
        let jwks_body = self.jwks_body();
        Mock::given(method("GET"))
            .and(path(format!("/{JWKS_PATH}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(jwks_body.clone()))
            .mount(&self.server)
            .await;
        
        Mock::given(method("GET"))
            .and(path(JWKS_PATH))
            .respond_with(ResponseTemplate::new(200).set_body_json(jwks_body))
            .mount(&self.server)
            .await;
    }

    async fn setup_auth_endpoints(&self) {
        Mock::given(method("POST"))
            .and(path("/passwords"))
            .respond_with(|req: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                let db_user_id = body["trusted_metadata"]["user_id"]
                    .as_str()
                    .unwrap_or(STYTCH_USER_ID);
                
                let header = Header {
                    kid: Some("test-key".to_string()),
                    ..Header::new(jsonwebtoken::Algorithm::RS256)
                };
                let exp = (Utc::now() + Duration::minutes(60)).timestamp();
                let claims = JwtClaims {
                    iss: format!("stytch.com/{}", PROJECT_ID),
                    aud: vec![PROJECT_ID.to_string()],
                    sub: SESSION_ID.to_string(),
                    user_id: db_user_id.to_string(),
                    exp,
                    scope: None,
                };
                let session_jwt = jsonwebtoken::encode(
                    &header,
                    &claims,
                    &EncodingKey::from_rsa_pem(PRIVATE_KEY_PEM.as_bytes()).expect("valid key"),
                )
                .expect("token to be encoded");
                
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
            .mount(&self.server)
            .await;

        Mock::given(method("POST"))
            .and(path("/passwords/authenticate"))
            .respond_with(|req: &wiremock::Request| {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                let db_user_id = body["trusted_metadata"]["user_id"]
                    .as_str()
                    .unwrap_or(STYTCH_USER_ID);
                
                let header = Header {
                    kid: Some("test-key".to_string()),
                    ..Header::new(jsonwebtoken::Algorithm::RS256)
                };
                let exp = (Utc::now() + Duration::minutes(60)).timestamp();
                let claims = JwtClaims {
                    iss: format!("stytch.com/{}", PROJECT_ID),
                    aud: vec![PROJECT_ID.to_string()],
                    sub: "login-session".to_string(),
                    user_id: db_user_id.to_string(),
                    exp,
                    scope: None,
                };
                let login_jwt = jsonwebtoken::encode(
                    &header,
                    &claims,
                    &EncodingKey::from_rsa_pem(PRIVATE_KEY_PEM.as_bytes()).expect("valid key"),
                )
                .expect("token to be encoded");
                
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
            .mount(&self.server)
            .await;
    }

    async fn setup_client_credential_endpoints(&self) {
        let trusted_metadata = json!({
            "user_id": STYTCH_USER_ID,
        });

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
            .mount(&self.server)
            .await;

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
            .mount(&self.server)
            .await;

        Mock::given(method("DELETE"))
            .and(path(format!("/m2m/clients/{CLIENT_ID}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status_code": 200
            })))
            .mount(&self.server)
            .await;
    }

    pub(crate) async fn setup_all_endpoints(&self) {
        self.setup_env();
        self.setup_jwks_endpoint().await;
        self.setup_auth_endpoints().await;
        self.setup_client_credential_endpoints().await;
    }
}