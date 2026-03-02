//! URL normalization utilities.

/// Normalize a URL by prepending `https://` when the scheme is absent and the
/// input looks like a hostname.
pub fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        return trimmed.to_string();
    }

    let looks_like_host = trimmed.contains('.')
        || trimmed.starts_with("localhost")
        || trimmed.starts_with("127.0.0.1")
        || trimmed.starts_with("[::1]");
    let has_no_spaces = !trimmed.chars().any(char::is_whitespace);

    if looks_like_host && has_no_spaces {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    }
}
