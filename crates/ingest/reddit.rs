use crate::crates::core::config::{Config, RedditSort};
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use reqwest::Client;
use std::error::Error;
use std::time::Duration;

// Reddit requires a descriptive User-Agent for API access.
// Format: <platform>:<app id>:<version> (by /u/<username>)
const REDDIT_USER_AGENT: &str = "axon-ingest/1.0 by /u/axon_bot";

/// Validate a subreddit name to prevent path traversal and injection attacks.
/// Reddit subreddit names are 2-21 characters, alphanumeric and underscores only.
fn validate_subreddit(name: &str) -> Result<(), Box<dyn Error>> {
    let len = name.len();
    let valid =
        (2..=21).contains(&len) && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        return Err(format!(
            "invalid subreddit name '{name}': must be 2-21 chars, alphanumeric and underscore only"
        )
        .into());
    }
    Ok(())
}

struct CommentWithContext {
    data: serde_json::Value,
    parent_text: Option<String>,
}

/// Recursively traverse a Reddit comment tree up to max_depth, filtering by min_score.
async fn fetch_comments_recursive(
    client: &Client,
    data: &serde_json::Value,
    current_depth: usize,
    max_depth: usize,
    min_score: i32,
    parent_text: Option<String>,
) -> Vec<CommentWithContext> {
    if current_depth > max_depth {
        return Vec::new();
    }

    let mut comments = Vec::new();
    if let Some(children) = data["children"].as_array() {
        for child in children {
            let kind = child["kind"].as_str().unwrap_or("");
            if kind != "t1" {
                continue;
            }
            let c_data = &child["data"];
            let score = c_data["score"].as_i64().unwrap_or(0) as i32;
            if score < min_score {
                continue;
            }

            let body = c_data["body"].as_str().unwrap_or("");
            if body.is_empty() || body == "[deleted]" || body == "[removed]" {
                continue;
            }

            comments.push(CommentWithContext {
                data: child.clone(),
                parent_text: parent_text.clone(),
            });

            let replies = &c_data["replies"];
            if replies.is_object() && replies["data"].is_object() {
                let mut nested = Box::pin(fetch_comments_recursive(
                    client,
                    &replies["data"],
                    current_depth + 1,
                    max_depth,
                    min_score,
                    Some(body.to_string()),
                ))
                .await;
                comments.append(&mut nested);
            }
        }
    }
    comments
}

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

/// Obtain an OAuth2 bearer token from Reddit using client credentials.
pub async fn get_access_token(
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn Error>> {
    let client = Client::builder()
        .user_agent(REDDIT_USER_AGENT)
        .https_only(true)
        .timeout(Duration::from_secs(30))
        .build()?;

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

/// Fetch a Reddit API URL with exponential backoff retry on 429 Too Many Requests.
async fn fetch_reddit_json(
    client: &Client,
    url: &str,
    token: &str,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let mut attempt = 0usize;
    loop {
        let resp = client.get(url).bearer_auth(token).send().await?;
        if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            attempt += 1;
            if attempt > 3 {
                return Err(format!("Reddit rate limit exceeded for {url}").into());
            }
            let wait_secs = 2u64.pow(attempt as u32);
            log_warn(&format!(
                "Reddit 429 rate limit — waiting {wait_secs}s before retrying {url}"
            ));
            tokio::time::sleep(Duration::from_secs(wait_secs)).await;
            continue;
        }
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Reddit API error ({status}): {body}").into());
        }
        return Ok(resp.json().await?);
    }
}

/// Fetch comments for a thread permalink, recursively up to cfg.reddit_depth.
async fn fetch_thread_comments(
    cfg: &Config,
    client: &Client,
    token: &str,
    permalink: &str,
) -> Result<Vec<CommentWithContext>, Box<dyn Error>> {
    let clean = permalink.trim_end_matches('/');
    let json_url = format!(
        "https://oauth.reddit.com{clean}.json?limit=100&depth={}",
        cfg.reddit_depth.max(1)
    );

    let resp = fetch_reddit_json(client, &json_url, token).await?;

    let mut comments = Vec::new();
    if let Some(data) = resp[1].get("data") {
        comments = Box::pin(fetch_comments_recursive(
            client,
            data,
            1,
            cfg.reddit_depth,
            cfg.reddit_min_score,
            None,
        ))
        .await;
    }

    Ok(comments)
}

/// Embed posts from a subreddit concurrently, including recursive comments per post.
async fn ingest_subreddit(
    cfg: &Config,
    client: &Client,
    token: &str,
    name: &str,
) -> Result<usize, Box<dyn Error>> {
    validate_subreddit(name)?;

    use futures_util::StreamExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let mut after = String::new();
    let total_count = AtomicUsize::new(0);
    let mut fetched_posts = 0usize;
    let max_posts = cfg.reddit_max_posts;

    loop {
        let limit = if max_posts > 0 {
            (max_posts - fetched_posts).min(100)
        } else {
            100
        };
        let mut url = format!(
            "https://oauth.reddit.com/r/{name}/{}?limit={limit}&raw_json=1",
            cfg.reddit_sort,
        );
        if cfg.reddit_sort == RedditSort::Top {
            url.push_str(&format!("&t={}", cfg.reddit_time));
        }
        if !after.is_empty() {
            url.push_str(&format!("&after={after}"));
        }

        let resp = fetch_reddit_json(client, &url, token).await?;
        if let Some(msg) = resp["message"].as_str() {
            return Err(format!("Reddit API error for r/{name}: {msg}").into());
        }

        let data = &resp["data"];
        let posts = data["children"].as_array().cloned().unwrap_or_default();
        if posts.is_empty() {
            break;
        }
        let posts_on_page = posts.len();
        let concurrency = cfg.batch_concurrency.clamp(1, 10);

        futures_util::stream::iter(posts)
            .for_each_concurrent(concurrency, |post| {
                let count_ref = &total_count;
                async move {
                    let data = &post["data"];
                    let score = data["score"].as_i64().unwrap_or(0) as i32;
                    if score < cfg.reddit_min_score {
                        return;
                    }

                    let title = data["title"].as_str().unwrap_or("Untitled");
                    let selftext = data["selftext"].as_str().unwrap_or("");
                    let permalink = data["permalink"].as_str().unwrap_or("");
                    let post_url = format!("https://www.reddit.com{permalink}");

                    let mut content = format!("# {title}");
                    if !selftext.is_empty() {
                        content.push_str(&format!("\n\n{selftext}"));
                    }

                    if !permalink.is_empty() {
                        if cfg.delay_ms > 0 {
                            tokio::time::sleep(Duration::from_millis(cfg.delay_ms)).await;
                        }
                        match fetch_thread_comments(cfg, client, token, permalink).await {
                            Ok(comments) => {
                                for comment_ctx in &comments {
                                    let c_data = &comment_ctx.data["data"];
                                    let body = c_data["body"].as_str().unwrap_or("");
                                    let mut ctx = format!("\n\n---\nPost: {title}\n\n");
                                    if let Some(parent) = comment_ctx.parent_text.as_deref() {
                                        ctx.push_str(&format!("Replying to: {parent}\n\n"));
                                    }
                                    ctx.push_str(body);
                                    content.push_str(&ctx);
                                }
                            }
                            Err(e) => log_warn(&format!(
                                "command=ingest_reddit fetch_comments_failed permalink={permalink} err={e}"
                            )),
                        }
                    }

                    if content.trim().is_empty() {
                        return;
                    }

                    match embed_text_with_metadata(cfg, &content, &post_url, "reddit", Some(title))
                        .await
                    {
                        Ok(n) => {
                            count_ref.fetch_add(n, Ordering::SeqCst);
                        }
                        Err(e) => log_warn(&format!(
                            "command=ingest_reddit embed_failed url={post_url} err={e}"
                        )),
                    }
                }
            })
            .await;

        fetched_posts += posts_on_page;
        if max_posts > 0 && fetched_posts >= max_posts {
            break;
        }
        after = data["after"].as_str().unwrap_or("").to_string();
        if after.is_empty() {
            break;
        }
    }

    Ok(total_count.into_inner())
}

/// Embed a single Reddit thread (post + full recursive comment tree) by its URL.
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
        .or_else(|| url.strip_prefix("http://old.reddit.com"))
        .or_else(|| url.strip_prefix("http://reddit.com"))
        .unwrap_or(url);

    let json_url = format!(
        "https://oauth.reddit.com{}.json?limit=100&depth={}&raw_json=1",
        permalink.trim_end_matches('/'),
        cfg.reddit_depth
    );
    let resp = fetch_reddit_json(client, &json_url, token).await?;

    let post_data = &resp[0]["data"]["children"][0]["data"];
    let title = post_data["title"].as_str().unwrap_or("Reddit Thread");
    let selftext = post_data["selftext"].as_str().unwrap_or("");
    let permalink_field = post_data["permalink"].as_str().unwrap_or(permalink);
    let canonical_url = format!("https://www.reddit.com{permalink_field}");

    let mut content = format!("# {title}");
    if !selftext.is_empty() {
        content.push_str(&format!("\n\n{selftext}"));
    }

    if let Some(data) = resp[1].get("data") {
        let comments = fetch_comments_recursive(
            client,
            data,
            1,
            cfg.reddit_depth,
            cfg.reddit_min_score,
            None,
        )
        .await;

        for comment_ctx in &comments {
            let c_data = &comment_ctx.data["data"];
            let body = c_data["body"].as_str().unwrap_or("");
            let mut ctx = format!("\n\n---\nPost: {title}\n\n");
            if let Some(parent) = comment_ctx.parent_text.as_deref() {
                ctx.push_str(&format!("Replying to: {parent}\n\n"));
            }
            ctx.push_str(body);
            content.push_str(&ctx);
        }
    }

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
/// - For a subreddit: fetches posts (configurable sort/limit/score/depth) + recursive comments
/// - For a thread URL: fetches that thread + full recursive comment tree
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
    let client = Client::builder()
        .user_agent(REDDIT_USER_AGENT)
        .https_only(true)
        .timeout(Duration::from_secs(30))
        .build()?;

    match classify_target(target) {
        RedditTarget::Subreddit(name) => ingest_subreddit(cfg, &client, &token, &name).await,
        RedditTarget::Thread(url) => ingest_thread(cfg, &client, &token, &url).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_target, validate_subreddit, RedditTarget};

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
        assert!(validate_subreddit("ab").is_ok());
    }

    #[test]
    fn validate_subreddit_rejects_path_traversal() {
        assert!(validate_subreddit("../../../etc/passwd").is_err());
        assert!(validate_subreddit("rust/../../admin").is_err());
    }

    #[test]
    fn validate_subreddit_rejects_too_short() {
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
    fn test_min_length_boundary() {
        assert!(validate_subreddit("a").is_err());
        assert!(validate_subreddit("ab").is_ok());
    }

    #[test]
    fn test_max_length_boundary() {
        assert!(validate_subreddit(&"a".repeat(21)).is_ok());
        assert!(validate_subreddit(&"a".repeat(22)).is_err());
    }

    #[test]
    fn test_rejects_null_byte() {
        assert!(validate_subreddit("rust\0hack").is_err());
    }

    #[test]
    fn test_rejects_unicode() {
        assert!(validate_subreddit("r\u{fc}st").is_err());
        assert!(validate_subreddit("caf\u{e9}").is_err());
    }
}
