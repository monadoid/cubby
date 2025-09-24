use loco_rs::prelude::*;

/// Render a sign-up form.
///
/// # Errors
///
/// When there is an issue with rendering the view.
pub fn sign_up(v: &impl ViewRenderer) -> Result<Response> {
    format::render().view(v, "auth/sign_up.html", data!({}))
}

/// Render a login form.
///
/// # Errors
///
/// When there is an issue with rendering the view.
pub fn login(v: &impl ViewRenderer, return_to: Option<&str>) -> Result<Response> {
    let data = match return_to {
        Some(url) => data!({"return_to": url}),
        None => data!({}),
    };
    format::render().view(v, "auth/login.html", data)
}

/// Render a dashboard view.
///
/// # Errors
///
/// When there is an issue with rendering the view.
pub fn dashboard(v: &impl ViewRenderer, user_id: &str) -> Result<Response> {
    format::render().view(v, "auth/dashboard.html", data!({"user_id": user_id}))
}
