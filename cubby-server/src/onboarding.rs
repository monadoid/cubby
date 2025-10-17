use anyhow::Result;
use cliclack::{input, password};

use crate::cubby_api_client::CubbyApiClient;
use crate::Cli;

pub struct OnboardingResult {
    pub tunnel_token: Option<String>,
    pub session_jwt: Option<String>,
}

struct AuthResult {
    tunnel_token: String,
    session_jwt: String,
}

/// Main onboarding flow - runs authentication and device enrollment
pub async fn run_onboarding_flow(_cli: &Cli) -> Result<OnboardingResult> {
    cliclack::intro("Welcome to cubby!")?;

    // Step 1: Always run authentication flow
    let auth_result = run_authentication_flow().await?;

    Ok(OnboardingResult {
        tunnel_token: Some(auth_result.tunnel_token),
        session_jwt: Some(auth_result.session_jwt),
    })
}

/// Run authentication flow (email/password, sign up, device enrollment)
async fn run_authentication_flow() -> Result<AuthResult> {
    let email: String = input("What's your email?")
        .validate(|value: &String| -> std::result::Result<(), &'static str> {
            if value.trim().is_empty() {
                Err("Email is required")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let pw: String = password("Choose a password (8 characters minimum):")
        .mask('▪')
        .validate(|value: &String| -> std::result::Result<(), &'static str> {
            if value.trim().is_empty() {
                Err("Password is required")
            } else {
                Ok(())
            }
        })
        .interact()?;

    let client = CubbyApiClient::new();

    let signup_response = client.sign_up(email, pw).await?;
    cliclack::log::success("Account created!")?;

    cliclack::log::step("Enrolling device...")?;
    let enroll_response = client.enroll_device(&signup_response.session_jwt).await?;

    cliclack::log::success("Device enrolled successfully!")?;
    cliclack::log::info(format!("Hostname: {}", enroll_response.hostname))?;

    Ok(AuthResult {
        tunnel_token: enroll_response.tunnel_token,
        session_jwt: signup_response.session_jwt,
    })
}
