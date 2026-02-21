use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use reqwest::Client;
use std::error::Error;

// Reddit requires a descriptive User-Agent for API access.
// Format: <platform>:<app id>:<version> (by /u/<username>)
const REDDIT_USER_AGENT: &str = "axon-ingest/1.0 by /u/axon_bot";

/// Discriminates between a subreddit name and a specific thread URL.
#[derive(Debug, PartialEq, Eq)]
pub enum RedditTarget {
    /// r/subreddit — fetch hot posts
    Subreddit(String),
    /// Specific thread URL — fetch that thread + comments
    Thread(String),
}

/// Classify a user-provided target string as a subreddit name or thread URL.
pub fn classify_target(target: &str) -> RedditTarget {
    if target.contains("/comments/") {
        return RedditTarget::Thread(target.to_string());
    }

    // Handle full subreddit URLs like https://www.reddit.com/r/rust/
    if let Some(rest) = target
        .strip_prefix("https://www.reddit.com/r/")
        .or_else(|| target.strip_prefix("http://www.reddit.com/r/"))
        .or_else(|| target.strip_prefix("https://reddit.com/r/"))
        .or_else(|| target.strip_prefix("http://reddit.com/r/"))
        .or_else(|| target.strip_prefix("https://old.reddit.com/r/"))
        .or_else(|| target.strip_prefix("http://old.reddit.com/r/"))
    {
        let name = rest.trim_end_matches('/');
        if !name.is_empty() {
            return RedditTarget::Subreddit(name.to_string());
        }
    }

    let clean = target
        .strip_prefix("/r/")
        .or_else(|| target.strip_prefix("r/"))
        .unwrap_or(target);
    RedditTarget::Subreddit(clean.to_string())
}

/// Obtain an OAuth2 bearer token from Reddit using client credentials.
pub async fn get_access_token(
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn Error>> {
    let client = Client::builder().user_agent(REDDIT_USER_AGENT).build()?;

    let resp: serde_json::Value = client
        .post("https://www.reddit.com/api/v1/access_token")
        .basic_auth(client_id, Some(client_secret))
        .form(&[("grant_type", "client_credentials")])
        .send()
        .await?
        .json()
        .await?;

    let token = resp["access_token"]
        .as_str()
        .ok_or_else(|| {
            let err = resp["error"].as_str().unwrap_or("unknown");
            format!("Reddit OAuth2 failed: {err}")
        })?
        .to_string();

    Ok(token)
}

/// Fetch top-level comments for a thread permalink (e.g. `/r/rust/comments/abc123/title/`).
/// Returns comment body strings, filtering deleted/removed content.
async fn fetch_thread_comments(
    client: &Client,
    token: &str,
    permalink: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let clean = permalink.trim_end_matches('/');
    let json_url = format!("https://oauth.reddit.com{clean}.json?limit=50&depth=2");

    let resp: serde_json::Value = client
        .get(&json_url)
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    let mut comments = Vec::new();
    // resp[1]["data"]["children"] is the comment listing array
    if let Some(arr) = resp[1]["data"]["children"].as_array() {
        for comment in arr {
            if let Some(body) = comment["data"]["body"].as_str() {
                if !body.is_empty() && body != "[deleted]" && body != "[removed]" {
                    comments.push(body.to_string());
                }
            }
        }
    }

    Ok(comments)
}

/// Embed hot posts from a subreddit, including each post's top-level comments.
async fn ingest_subreddit(
    cfg: &Config,
    client: &Client,
    token: &str,
    name: &str,
) -> Result<usize, Box<dyn Error>> {
    let resp: serde_json::Value = client
        .get(format!(
            "https://oauth.reddit.com/r/{name}/hot?limit=25&raw_json=1"
        ))
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    if let Some(msg) = resp["message"].as_str() {
        return Err(format!("Reddit API error for r/{name}: {msg}").into());
    }

    let posts = resp["data"]["children"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let mut count = 0usize;
    for post in &posts {
        let data = &post["data"];
        let title = data["title"].as_str().unwrap_or("Untitled");
        let selftext = data["selftext"].as_str().unwrap_or("");
        let permalink = data["permalink"].as_str().unwrap_or("");
        let post_url = format!("https://www.reddit.com{permalink}");

        let mut content = if selftext.is_empty() {
            format!("# {title}")
        } else {
            format!("# {title}\n\n{selftext}")
        };

        if !permalink.is_empty() {
            match fetch_thread_comments(client, token, permalink).await {
                Ok(comments) => {
                    for c in &comments {
                        content.push_str(&format!("\n\n---\n{c}"));
                    }
                }
                Err(e) => log_warn(&format!(
                    "command=ingest_reddit fetch_comments_failed permalink={permalink} err={e}"
                )),
            }
        }

        if content.trim().is_empty() {
            continue;
        }

        match embed_text_with_metadata(cfg, &content, &post_url, "reddit", Some(title)).await {
            Ok(n) => count += n,
            Err(e) => log_warn(&format!(
                "command=ingest_reddit embed_failed url={post_url} err={e}"
            )),
        }
    }
    Ok(count)
}

/// Embed a single Reddit thread (post + all top-level comments) by its URL.
async fn ingest_thread(
    cfg: &Config,
    client: &Client,
    token: &str,
    url: &str,
) -> Result<usize, Box<dyn Error>> {
    let permalink = url
        .strip_prefix("https://www.reddit.com")
        .or_else(|| url.strip_prefix("https://old.reddit.com"))
        .or_else(|| url.strip_prefix("https://reddit.com"))
        .or_else(|| url.strip_prefix("http://www.reddit.com"))
        .unwrap_or(url);

    let json_url = format!(
        "https://oauth.reddit.com{}.json?limit=50&depth=3&raw_json=1",
        permalink.trim_end_matches('/')
    );
    let resp: serde_json::Value = client
        .get(&json_url)
        .bearer_auth(token)
        .send()
        .await?
        .json()
        .await?;

    let post_data = &resp[0]["data"]["children"][0]["data"];
    let title = post_data["title"].as_str().unwrap_or("Reddit Thread");
    let selftext = post_data["selftext"].as_str().unwrap_or("");

    let mut content = if selftext.is_empty() {
        format!("# {title}")
    } else {
        format!("# {title}\n\n{selftext}")
    };

    if let Some(arr) = resp[1]["data"]["children"].as_array() {
        for comment in arr {
            if let Some(body) = comment["data"]["body"].as_str() {
                if !body.is_empty() && body != "[deleted]" && body != "[removed]" {
                    content.push_str(&format!("\n\n---\n{body}"));
                }
            }
        }
    }

    let canonical_url = format!("https://www.reddit.com{permalink}");
    match embed_text_with_metadata(cfg, &content, &canonical_url, "reddit", Some(title)).await {
        Ok(n) => Ok(n),
        Err(e) => {
            log_warn(&format!(
                "command=ingest_reddit embed_failed url={canonical_url} err={e}"
            ));
            Ok(0)
        }
    }
}

/// Ingest Reddit content:
/// - For a subreddit: fetches 25 hot posts + their top-level comments
/// - For a thread URL: fetches that thread + full comment tree
/// - Embeds all content into Qdrant via embed_text_with_metadata
pub async fn ingest_reddit(cfg: &Config, target: &str) -> Result<usize, Box<dyn Error>> {
    let client_id = cfg
        .reddit_client_id
        .as_deref()
        .ok_or("REDDIT_CLIENT_ID not configured (--reddit-client-id or env var)")?;
    let client_secret = cfg
        .reddit_client_secret
        .as_deref()
        .ok_or("REDDIT_CLIENT_SECRET not configured (--reddit-client-secret or env var)")?;

    let token = get_access_token(client_id, client_secret).await?;
    let client = Client::builder().user_agent(REDDIT_USER_AGENT).build()?;

    match classify_target(target) {
        RedditTarget::Subreddit(name) => ingest_subreddit(cfg, &client, &token, &name).await,
        RedditTarget::Thread(url) => ingest_thread(cfg, &client, &token, &url).await,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- classify_target ---

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
    fn classify_subreddit_name_with_numbers() {
        assert_eq!(
            classify_target("web_dev"),
            RedditTarget::Subreddit("web_dev".to_string())
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
}
