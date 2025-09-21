use serde::{Deserialize, Serialize};

use crate::{data::stytch::PasswordAuthResponse, models::_entities::users};

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthResponse {
    pub user_id: String,
    pub stytch_user_id: String,
    pub email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_expires_at: Option<String>,
    pub is_verified: bool,
}

impl AuthResponse {
    #[must_use]
    pub fn new(user: &users::Model, response: &PasswordAuthResponse) -> Self {
        Self {
            user_id: user.id.to_string(),
            stytch_user_id: user.auth_id.clone(),
            email: user.email.clone(),
            access_token: response.session_jwt.clone(),
            session_token: response.session_token.clone(),
            session_expires_at: response
                .session
                .as_ref()
                .and_then(|session| session.expires_at.clone()),
            is_verified: user.email_verified_at.is_some(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CurrentResponse {
    pub id: String,
    pub email: String,
    pub stytch_user_id: String,
    pub is_verified: bool,
}

impl CurrentResponse {
    #[must_use]
    pub fn new(user: &users::Model) -> Self {
        Self {
            id: user.id.to_string(),
            email: user.email.clone(),
            stytch_user_id: user.auth_id.clone(),
            is_verified: user.email_verified_at.is_some(),
        }
    }
}
