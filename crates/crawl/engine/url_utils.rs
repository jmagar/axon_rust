use spider::url::Url;

pub(crate) fn canonicalize_url_for_dedupe(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    parsed.set_fragment(None);

    match (parsed.scheme(), parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) => {
            let _ = parsed.set_port(None);
        }
        _ => {}
    }

    let path = parsed.path().to_string();
    if path.len() > 1 {
        let normalized_path = path.trim_end_matches('/').to_string();
        parsed.set_path(&normalized_path);
    }

    Some(parsed.to_string())
}

pub(crate) fn is_excluded_url_path(url: &str, excludes: &[String]) -> bool {
    if excludes.is_empty() {
        return false;
    }
    let path = Url::parse(url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| "/".to_string());
    excludes
        .iter()
        .any(|prefix| is_path_prefix_excluded(&path, prefix))
}

fn is_path_prefix_excluded(path: &str, prefix: &str) -> bool {
    let normalized = if prefix.starts_with('/') {
        prefix.to_owned()
    } else {
        format!("/{prefix}")
    };
    let boundary = normalized.trim_end_matches('/');
    if boundary.is_empty() {
        return false;
    }
    path == boundary
        || path
            .strip_prefix(boundary)
            .is_some_and(|rest| rest.starts_with('/') || rest.starts_with('-'))
}

pub(crate) fn regex_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 8);
    for ch in value.chars() {
        match ch {
            '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
            | '-' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

pub(super) fn build_exclude_blacklist_patterns(
    start_url: &str,
    excludes: &[String],
) -> Vec<String> {
    let host_pattern = Url::parse(start_url)
        .ok()
        .and_then(|u| u.host_str().map(regex_escape))
        .unwrap_or_else(|| "[^/]+".to_string());

    excludes
        .iter()
        .map(|prefix| {
            let normalized = if prefix.starts_with('/') {
                prefix.clone()
            } else {
                format!("/{prefix}")
            };
            format!(
                "^https?://{}{}(?:/|-|$|\\?|#)",
                host_pattern,
                regex_escape(&normalized)
            )
        })
        .collect()
}

/// Returns `true` if the URL is garbage extracted from minified JS/CSS bundles
/// rather than a real hyperlink.
///
/// Spider's link extractor pulls anything that resembles a relative path from
/// page content — including `<script>` tags and inline JS. This produces URLs
/// like `https://example.com/belonging%20toclaimed%20that%3Cmeta%20name=` or
/// `https://example.com/$%7BshareBaseUrl%7D/s/$%7BshareId%7D`.
///
/// Heuristics (applied to the URL path, not query string):
/// - URL length > 2048 (standard browser limit)
/// - Encoded HTML tags: `%3C` (`<`) or `%3E` (`>`)
/// - Template literals: `%7B` (`{`) or `%7D` (`}`)
/// - 3+ encoded spaces (`%20`) — prose, not a URL
/// - JS concatenation artifacts: `'%20` or `%20'`
pub(crate) fn is_junk_discovered_url(url: &str) -> bool {
    if url.len() > 2048 {
        return true;
    }

    let path = url_path_portion(url);

    // Encoded HTML tags: < or > never appear in real URL paths.
    if path.contains("%3C") || path.contains("%3c") || path.contains("%3E") || path.contains("%3e")
    {
        return true;
    }

    // Template literal variables: { or } from JS `${variable}` expressions.
    if path.contains("%7B") || path.contains("%7b") || path.contains("%7D") || path.contains("%7d")
    {
        return true;
    }

    // 3+ encoded spaces in path = extracted prose, not a URL.
    // Real URLs use hyphens/underscores for word separation.
    if path.matches("%20").count() >= 3 {
        return true;
    }

    // JS string concatenation: `' + var + '` shows up as `'%20+%20var%20+%20'`.
    if path.contains("'%20") || path.contains("%20'") {
        return true;
    }

    false
}

/// Extract the path portion of a URL (between host and query/fragment).
/// For relative URLs (no scheme), treats the whole string up to `?` or `#` as path.
fn url_path_portion(url: &str) -> &str {
    let after_host = match url.find("://") {
        Some(i) => {
            let rest = &url[i + 3..];
            let path_start = rest.find('/').unwrap_or(rest.len());
            &rest[path_start..]
        }
        None => url,
    };
    let end = after_host
        .find('?')
        .or_else(|| after_host.find('#'))
        .unwrap_or(after_host.len());
    &after_host[..end]
}

#[cfg(test)]
#[path = "url_utils_proptest.rs"]
mod url_utils_proptest;

#[cfg(test)]
mod tests {
    use super::*;

    fn excludes(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // 1. Empty excludes list → empty result.
    #[test]
    fn build_exclude_blacklist_patterns_returns_empty_for_no_excludes() {
        let patterns = build_exclude_blacklist_patterns("https://example.com", &[]);
        assert!(patterns.is_empty());
    }

    // 2. Pattern starts with `^https?://` and contains the escaped host.
    #[test]
    fn build_exclude_blacklist_patterns_generates_anchored_host_scoped_regex() {
        let patterns = build_exclude_blacklist_patterns("https://example.com", &excludes(&["/fr"]));
        assert_eq!(patterns.len(), 1);
        assert!(
            patterns[0].starts_with("^https?://"),
            "pattern should start with ^https?://, got: {}",
            patterns[0]
        );
        assert!(
            patterns[0].contains("example"),
            "pattern should contain host, got: {}",
            patterns[0]
        );
    }

    // 3. Dots in hostname are escaped to `\.`.
    #[test]
    fn build_exclude_blacklist_patterns_escapes_dots_in_hostname() {
        let patterns = build_exclude_blacklist_patterns("https://example.com", &excludes(&["/fr"]));
        assert_eq!(patterns.len(), 1);
        assert!(
            patterns[0].contains("example\\.com"),
            "dots in hostname should be escaped, got: {}",
            patterns[0]
        );
    }

    // 4. Prefix without leading slash and with leading slash produce the same pattern.
    #[test]
    fn build_exclude_blacklist_patterns_normalizes_prefix_without_leading_slash() {
        let with_slash =
            build_exclude_blacklist_patterns("https://example.com", &excludes(&["/fr"]));
        let without_slash =
            build_exclude_blacklist_patterns("https://example.com", &excludes(&["fr"]));
        assert_eq!(
            with_slash, without_slash,
            "prefix 'fr' and '/fr' should yield identical patterns"
        );
    }

    // 5. Three excludes → three patterns (one per exclude).
    #[test]
    fn build_exclude_blacklist_patterns_multiple_excludes_produces_one_pattern_each() {
        let patterns = build_exclude_blacklist_patterns(
            "https://example.com",
            &excludes(&["/fr", "/de", "/ja"]),
        );
        assert_eq!(patterns.len(), 3);
    }

    // 6. Unparseable URL falls back to `[^/]+` as the host wildcard.
    #[test]
    fn build_exclude_blacklist_patterns_invalid_start_url_uses_wildcard_host() {
        let patterns = build_exclude_blacklist_patterns("not-a-valid-url", &excludes(&["/fr"]));
        assert_eq!(patterns.len(), 1);
        assert!(
            patterns[0].contains("[^/]+"),
            "invalid URL should fall back to [^/]+ host pattern, got: {}",
            patterns[0]
        );
    }

    // 7. Pattern ends with the boundary alternation group.
    //    The format! uses `\\?` which produces `\?` in the output — a regex-escaped
    //    literal question mark matching the start of a query string.
    #[test]
    fn build_exclude_blacklist_patterns_pattern_ends_with_boundary_alternation() {
        let patterns = build_exclude_blacklist_patterns("https://example.com", &excludes(&["/fr"]));
        assert_eq!(patterns.len(), 1);
        assert!(
            patterns[0].ends_with("(?:/|-|$|\\?|#)"),
            "pattern should end with boundary alternation group, got: {}",
            patterns[0]
        );
    }
}
