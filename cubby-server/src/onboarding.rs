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
    cliclack::intro("welcome to cubby!")?;

    // Step 1: Always run authentication flow
    let auth_result = run_authentication_flow().await?;

    // Step 2: Create M2M client for API/MCP access
    if let Err(e) = create_api_credentials(&auth_result.session_jwt).await {
        cliclack::log::warning(format!("failed to create api credentials: {}", e))?;
        cliclack::log::info("you can generate credentials later at https://cubby.sh/dashboard")?;
    }

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
    cliclack::log::success("account created!")?;

    cliclack::log::step("enrolling device...")?;
    let enroll_response = client.enroll_device(&signup_response.session_jwt).await?;

    cliclack::log::success("device enrolled successfully!")?;
    cliclack::log::info(format!("hostname: {}", enroll_response.hostname))?;

    Ok(AuthResult {
        tunnel_token: enroll_response.tunnel_token,
        session_jwt: signup_response.session_jwt,
    })
}

/// Create M2M client credentials and print access token for API/MCP usage
async fn create_api_credentials(session_jwt: &str) -> Result<()> {
    cliclack::log::step("generating api credentials...")?;
    
    let client = CubbyApiClient::new();
    
    // Create M2M client
    let m2m_response = client.create_m2m_client(session_jwt).await?;
    
    // Exchange for access token
    let token_response = client
        .exchange_for_token(&m2m_response.client_id, &m2m_response.client_secret)
        .await?;
    
    cliclack::log::success("api credentials generated!")?;
    cliclack::outro_note(
        "api access token",
        format!(
            "save these credentials:\n\n\
            client_id: {}\n\
            client_secret: {}\n\n\
            access_token (expires in {} minutes):\n{}\n\n\
            use this token for:\n\
            - cursor mcp: add to ~/.cursor/mcp.json\n\
            - rest api: authorization: bearer <token>\n\n\
            regenerate at: https://cubby.sh/dashboard",
            m2m_response.client_id,
            m2m_response.client_secret,
            token_response.expires_in / 60,
            token_response.access_token,
        ),
    )?;
    
    Ok(())
}
