use once_cell::sync::Lazy;

/// Simple matcher that marks windows as private/sensitive based on app and title substrings.
struct WindowRule {
    app_substrings: &'static [&'static str],
    title_substrings: &'static [&'static str],
}

static PRIVATE_WINDOW_RULES: Lazy<Vec<WindowRule>> = Lazy::new(|| {
    vec![
        WindowRule {
            app_substrings: &["firefox"],
            title_substrings: &["private browsing", "private window"],
        },
        WindowRule {
            app_substrings: &["chrome", "chromium"],
            title_substrings: &["incognito"],
        },
        WindowRule {
            app_substrings: &["safari"],
            title_substrings: &["private browsing"],
        },
        WindowRule {
            app_substrings: &["edge"],
            title_substrings: &["inprivate"],
        },
        WindowRule {
            app_substrings: &["brave"],
            title_substrings: &["private window"],
        },
        WindowRule {
            app_substrings: &["opera"],
            title_substrings: &["private", "incognito"],
        },
        WindowRule {
            app_substrings: &["arc", "vivaldi"],
            title_substrings: &["private"],
        },
    ]
});

/// Returns `true` when the `(app_name, window_name)` pair is known to represent a private browsing
/// or otherwise sensitive window that should be skipped entirely.
pub fn is_private_window(app_name: &str, window_name: &str) -> bool {
    let app_lower = app_name.to_lowercase();
    let title_lower = window_name.to_lowercase();

    PRIVATE_WINDOW_RULES.iter().any(|rule| {
        let app_matches = rule
            .app_substrings
            .iter()
            .any(|needle| app_lower.contains(needle));
        if !app_matches {
            return false;
        }

        rule.title_substrings
            .iter()
            .any(|needle| title_lower.contains(needle))
    })
}

#[cfg(test)]
mod tests {
    use super::is_private_window;

    #[test]
    fn detects_firefox_private_window() {
        assert!(is_private_window(
            "Firefox",
            "Example Page — Private Browsing"
        ));
    }

    #[test]
    fn detects_chrome_incognito() {
        assert!(is_private_window("Google Chrome", "New Tab — Incognito"));
    }

    #[test]
    fn ignores_regular_window() {
        assert!(!is_private_window("Firefox", "Mozilla Firefox"));
        assert!(!is_private_window("Google Chrome", "Example Page"));
    }
}
