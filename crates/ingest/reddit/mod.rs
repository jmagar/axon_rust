mod client;
mod comments;
mod types;

pub use client::get_access_token;
pub use types::{RedditTarget, classify_target};

use crate::crates::core::config::{Config, RedditSort};
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use reqwest::Client;
use std::error::Error;
use std::time::Duration;

use client::{build_client, fetch_reddit_json};
use comments::{collect_comments_recursive, fetch_thread_comments};
use types::{CommentWithContext, validate_subreddit};

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
                                format_comments_into(&mut content, title, &comments);
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
        let mut comments = Vec::new();
        collect_comments_recursive(
            data,
            1,
            cfg.reddit_depth,
            cfg.reddit_min_score,
            None,
            &mut comments,
        );
        format_comments_into(&mut content, title, &comments);
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

/// Append formatted comments to the content string.
fn format_comments_into(content: &mut String, title: &str, comments: &[CommentWithContext]) {
    for comment_ctx in comments {
        let mut ctx = format!("\n\n---\nPost: {title}\n\n");
        if let Some(parent) = comment_ctx.parent_text.as_deref() {
            ctx.push_str(&format!("Replying to: {parent}\n\n"));
        }
        ctx.push_str(&comment_ctx.body);
        content.push_str(&ctx);
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
    let client = build_client()?;

    match classify_target(target) {
        RedditTarget::Subreddit(name) => ingest_subreddit(cfg, &client, &token, &name).await,
        RedditTarget::Thread(url) => ingest_thread(cfg, &client, &token, &url).await,
    }
}
