use crate::crates::core::config::Config;
use std::error::Error;

/// Sanitize a URL into a safe filename component.
///
/// Strips the scheme, replaces non-alphanumeric chars with hyphens,
/// collapses runs of hyphens, trims edges, and truncates to 120 chars.
pub(crate) fn url_to_screenshot_filename(url: &str, idx: usize) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    let sanitized: String = stripped
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim leading/trailing hyphens.
    let mut collapsed = String::with_capacity(sanitized.len());
    let mut prev_hyphen = true; // Start true to trim leading hyphens.
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }
    let collapsed = collapsed.trim_end_matches('-');

    // Truncate to a reasonable filename length.
    let max_name = 120;
    let name = if collapsed.len() > max_name {
        &collapsed[..max_name]
    } else {
        collapsed
    };

    format!("{idx:04}-{name}.png")
}

/// Validate that Chrome is configured before attempting a screenshot.
pub(super) fn require_chrome(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.chrome_remote_url.is_none() {
        return Err(
            "screenshot requires Chrome — set AXON_CHROME_REMOTE_URL or pass --chrome-remote-url"
                .into(),
        );
    }
    Ok(())
}

/// Format screenshot result as JSON for `--json` mode.
pub(super) fn format_screenshot_json(url: &str, path: &str, size_bytes: u64) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "url": url,
        "path": path,
        "size_bytes": size_bytes,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::core::config::Config;

    // --- url_to_screenshot_filename ---

    #[test]
    fn test_url_to_screenshot_filename_basic() {
        let name = url_to_screenshot_filename("https://example.com/docs/intro", 1);
        assert_eq!(name, "0001-example-com-docs-intro.png");
    }

    #[test]
    fn test_url_to_screenshot_filename_special_chars() {
        let name = url_to_screenshot_filename("https://foo.bar/a?b=c&d=e", 3);
        assert!(name.starts_with("0003-"));
        assert!(name.ends_with(".png"));
        // Should not contain raw special chars.
        assert!(!name.contains('?'));
        assert!(!name.contains('&'));
        assert!(!name.contains('='));
    }

    #[test]
    fn test_url_to_screenshot_filename_long_url() {
        let long = format!("https://example.com/{}", "a".repeat(200));
        let name = url_to_screenshot_filename(&long, 1);
        assert!(name.ends_with(".png"));
        // The stem (before .png) should be truncated.
        assert!(name.len() < 200, "filename should be truncated: {name}");
    }

    #[test]
    fn test_url_to_screenshot_filename_no_consecutive_hyphens() {
        let name = url_to_screenshot_filename("https://example.com/a///b..c", 1);
        assert!(!name.contains("--"), "should not have consecutive hyphens");
    }

    // --- require_chrome ---

    #[test]
    fn test_require_chrome_errors_when_missing() {
        let cfg = Config {
            chrome_remote_url: None,
            ..Config::default()
        };
        let result = require_chrome(&cfg);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("requires Chrome"),
            "error should mention Chrome requirement: {msg}"
        );
    }

    #[test]
    fn test_require_chrome_ok_when_set() {
        let cfg = Config {
            chrome_remote_url: Some("ws://localhost:9222".to_string()),
            ..Config::default()
        };
        assert!(require_chrome(&cfg).is_ok());
    }

    // --- format_screenshot_json ---

    #[test]
    fn test_json_output_format() {
        let json = format_screenshot_json("https://example.com", "/tmp/out.png", 12345);
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("output should be valid JSON");
        assert_eq!(parsed["url"], "https://example.com");
        assert_eq!(parsed["path"], "/tmp/out.png");
        assert_eq!(parsed["size_bytes"], 12345);
    }
}
