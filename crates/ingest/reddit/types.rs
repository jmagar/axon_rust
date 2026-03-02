use std::error::Error;

/// Validate a subreddit name to prevent path traversal and injection attacks.
/// Reddit subreddit names are 3-21 characters, alphanumeric and underscores only.
pub(crate) fn validate_subreddit(name: &str) -> Result<(), Box<dyn Error>> {
    let len = name.len();
    let valid =
        (3..=21).contains(&len) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(format!(
            "invalid subreddit name '{name}': must be 3-21 chars, alphanumeric and underscore only"
        )
        .into());
    }
    Ok(())
}

/// Context for a single Reddit comment including optional parent text for threading.
pub(super) struct CommentWithContext {
    pub body: String,
    pub parent_text: Option<String>,
}

/// Discriminates between a subreddit name and a specific thread URL.
#[derive(Debug, PartialEq, Eq)]
pub enum RedditTarget {
    /// r/subreddit -- fetch hot posts
    Subreddit(String),
    /// Specific thread URL -- fetch that thread + comments
    Thread(String),
}

/// Classify a user-provided target string as a subreddit name or thread URL.
pub fn classify_target(target: &str) -> RedditTarget {
    if target.contains("/comments/") {
        return RedditTarget::Thread(target.to_string());
    }

    // Handle full subreddit URLs like https://www.reddit.com/r/rust/
    // Take only the first path segment after /r/ to ignore sub-paths like /hot/, /new/, etc.
    if let Some(rest) = target
        .strip_prefix("https://www.reddit.com/r/")
        .or_else(|| target.strip_prefix("http://www.reddit.com/r/"))
        .or_else(|| target.strip_prefix("https://reddit.com/r/"))
        .or_else(|| target.strip_prefix("http://reddit.com/r/"))
        .or_else(|| target.strip_prefix("https://old.reddit.com/r/"))
        .or_else(|| target.strip_prefix("http://old.reddit.com/r/"))
    {
        let name = rest.split('/').next().unwrap_or("").trim();
        if !name.is_empty() {
            return RedditTarget::Subreddit(name.to_string());
        }
    }

    let clean = target
        .strip_prefix("/r/")
        .or_else(|| target.strip_prefix("r/"))
        .unwrap_or(target)
        .trim_end_matches('/');
    RedditTarget::Subreddit(clean.to_string())
}

#[cfg(test)]
mod tests {
    use super::{RedditTarget, classify_target, validate_subreddit};

    #[test]
    fn classify_bare_subreddit_name() {
        assert_eq!(
            classify_target("rust"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn classify_subreddit_name_with_r_prefix() {
        assert_eq!(
            classify_target("r/rust"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn classify_subreddit_name_with_leading_slash() {
        assert_eq!(
            classify_target("/r/rust"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn classify_thread_url() {
        let url = "https://www.reddit.com/r/rust/comments/abc123/some_title/";
        assert_eq!(classify_target(url), RedditTarget::Thread(url.to_string()));
    }

    #[test]
    fn classify_old_reddit_thread_url() {
        let url = "https://old.reddit.com/r/rust/comments/abc123/some_title/";
        assert_eq!(classify_target(url), RedditTarget::Thread(url.to_string()));
    }

    #[test]
    fn classify_subreddit_name_with_underscores() {
        assert_eq!(
            classify_target("rust_gamedev"),
            RedditTarget::Subreddit("rust_gamedev".to_string())
        );
    }

    #[test]
    fn classify_full_subreddit_url() {
        assert_eq!(
            classify_target("https://www.reddit.com/r/rust/"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn classify_full_subreddit_url_no_trailing_slash() {
        assert_eq!(
            classify_target("https://www.reddit.com/r/rust"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn classify_full_subreddit_url_no_www() {
        assert_eq!(
            classify_target("https://reddit.com/r/programming/"),
            RedditTarget::Subreddit("programming".to_string())
        );
    }

    #[test]
    fn classify_old_reddit_subreddit_url() {
        assert_eq!(
            classify_target("https://old.reddit.com/r/rust/"),
            RedditTarget::Subreddit("rust".to_string())
        );
    }

    #[test]
    fn validate_subreddit_accepts_valid_names() {
        assert!(validate_subreddit("rust").is_ok());
        assert!(validate_subreddit("rust_gamedev").is_ok());
        assert!(validate_subreddit("AskReddit").is_ok());
        assert!(validate_subreddit("abc").is_ok());
    }

    #[test]
    fn validate_subreddit_rejects_path_traversal() {
        assert!(validate_subreddit("../../../etc/passwd").is_err());
        assert!(validate_subreddit("rust/../../admin").is_err());
    }

    #[test]
    fn validate_subreddit_rejects_too_short() {
        assert!(validate_subreddit("ab").is_err());
        assert!(validate_subreddit("a").is_err());
        assert!(validate_subreddit("").is_err());
    }

    #[test]
    fn validate_subreddit_rejects_too_long() {
        assert!(validate_subreddit("abcdefghijklmnopqrstuv").is_err());
    }

    #[test]
    fn validate_subreddit_rejects_special_chars() {
        assert!(validate_subreddit("rust-lang").is_err());
        assert!(validate_subreddit("rust.lang").is_err());
        assert!(validate_subreddit("rust lang").is_err());
    }

    #[test]
    fn min_length_boundary() {
        assert!(validate_subreddit("ab").is_err());
        assert!(validate_subreddit("abc").is_ok());
    }

    #[test]
    fn max_length_boundary() {
        assert!(validate_subreddit(&"a".repeat(21)).is_ok());
        assert!(validate_subreddit(&"a".repeat(22)).is_err());
    }

    #[test]
    fn rejects_null_byte() {
        assert!(validate_subreddit("rust\0hack").is_err());
    }

    #[test]
    fn rejects_unicode() {
        assert!(validate_subreddit("r\u{fc}st").is_err());
        assert!(validate_subreddit("caf\u{e9}").is_err());
    }
}
