use loco_rs::prelude::*;

use crate::models::pods;

/// Render the pod management view showing pod details and credentials.
///
/// # Errors
///
/// When there is an issue with rendering the view.
pub fn show(v: &impl ViewRenderer, pod: &Option<pods::Model>) -> Result<Response> {
    format::render().view(v, "pod/show.html", data!({"pod": pod}))
}

/// Render the credentials view showing client credentials for the pod.
///
/// # Errors  
///
/// When there is an issue with rendering the view.
pub fn credentials(v: &impl ViewRenderer, pod: &Option<pods::Model>) -> Result<Response> {
    format::render().view(v, "pod/credentials.html", data!({"pod": pod}))
}
