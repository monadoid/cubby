use anyhow::{bail, Result};
use cliclack::{input, password};

use crate::cubby_api_client::CubbyApiClient;
use crate::permission_checker::{
    check_accessibility_permission, trigger_and_check_microphone,
    trigger_and_check_screen_recording,
};
use crate::Cli;

pub struct OnboardingResult {
    pub tunnel_token: Option<String>,
    pub session_jwt: Option<String>,
}

struct AuthResult {
    tunnel_token: String,
    session_jwt: String,
}

/// Main onboarding flow - runs authentication and permission checks
pub async fn run_onboarding_flow(cli: &Cli) -> Result<OnboardingResult> {
    cliclack::intro("Welcome to cubby!")?;

    // Step 1: Always run authentication flow
    let auth_result = run_authentication_flow().await?;

    // Step 2: Permission checks (after authentication)
    run_permission_checks(cli).await?;

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

/// Run permission checks for screen recording, microphone, and accessibility
async fn run_permission_checks(cli: &Cli) -> Result<()> {
    // Check microphone (always needed unless audio disabled)
    if !cli.disable_audio {
        check_and_request_microphone().await?;
    }

    // Check screen recording (always needed unless vision disabled)
    if !cli.disable_vision {
        check_and_request_screen_recording().await?;
    }

    // Check accessibility (only if UI monitoring enabled)
    if cli.enable_ui_monitoring {
        check_and_request_accessibility().await?;
    }

    cliclack::log::success("All required permissions granted!")?;
    Ok(())
}

/// Check and request screen recording permission
async fn check_and_request_screen_recording() -> Result<()> {
    cliclack::log::step("Checking Screen Recording permission...")?;

    #[cfg(debug_assertions)]
    {
        println!("   A permission dialog will appear - please click 'Allow'");
        println!("   (Waiting up to 60 seconds for your response...)");
    }

    let has_permission = trigger_and_check_screen_recording().await?;

    if has_permission {
        cliclack::log::success("Screen Recording permission granted")?;
        return Ok(());
    }

    // Timeout or denied
    cliclack::log::error("Screen Recording permission not granted")?;

    #[cfg(debug_assertions)]
    {
        println!("To grant permission:");
        println!("   1. Open System Settings > Privacy & Security > Screen Recording");
        println!("   2. Enable your terminal");
        println!("   3. **Quit and restart your terminal**");
        println!("   4. Run setup again");
    }

    bail!("Screen Recording permission required");
}

/// Check and request microphone permission
async fn check_and_request_microphone() -> Result<()> {
    cliclack::log::step("Checking Microphone permission...")?;

    #[cfg(debug_assertions)]
    {
        println!("   A permission dialog will appear - please click 'Allow'");
        println!("   (Waiting up to 60 seconds for your response...)");
    }

    let has_permission = trigger_and_check_microphone().await?;

    if has_permission {
        cliclack::log::success("Microphone permission granted")?;
        return Ok(());
    }

    // Timeout or denied
    cliclack::log::error("Microphone permission not granted")?;

    #[cfg(debug_assertions)]
    {
        println!("To grant permission:");
        println!("   1. Open System Settings > Privacy & Security > Microphone");
        println!("   2. Enable your terminal");
        println!("   3. **Quit and restart your terminal**");
        println!("   4. Run setup again");
    }

    bail!("Microphone permission required");
}

/// Check and request accessibility permission (optional, for UI monitoring)
async fn check_and_request_accessibility() -> Result<()> {
    cliclack::log::step("Checking Accessibility permission...")?;

    if check_accessibility_permission() {
        cliclack::log::success("Accessibility permission granted (or not required)")?;
        return Ok(());
    }

    // For now, we'll skip the complex accessibility permission flow
    // since it's optional and only needed for UI monitoring
    cliclack::log::warning("Accessibility permission setup skipped (optional feature)")?;

    #[cfg(debug_assertions)]
    {
        println!("   To enable UI monitoring later, you may need to grant accessibility permissions manually.");
    }

    Ok(())
}
