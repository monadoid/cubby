use loco_rs::prelude::*;

use crate::controllers::css_files_htmx::FileInfo;

pub fn list(v: &impl ViewRenderer, files: &[FileInfo], current_path: &str) -> Result<Response> {
    format::render().view(
        v,
        "css_files/list.html",
        data!({
            "files": files,
            "current_path": current_path,
            "parent_path": get_parent_path(current_path)
        }),
    )
}

pub fn upload_form(v: &impl ViewRenderer, current_path: &str) -> Result<Response> {
    format::render().view(
        v,
        "css_files/upload_form.html",
        data!({
            "current_path": current_path
        }),
    )
}

fn get_parent_path(current_path: &str) -> Option<String> {
    if current_path.is_empty() || current_path == "/" {
        return None;
    }
    
    let trimmed = current_path.trim_end_matches('/');
    if let Some(last_slash) = trimmed.rfind('/') {
        if last_slash == 0 {
            Some(String::new())
        } else {
            Some(trimmed[..last_slash].to_string())
        }
    } else {
        Some(String::new())
    }
}