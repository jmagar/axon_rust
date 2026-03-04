use crate::crates::core::logging::log_warn;
use reqwest::Client;
use std::error::Error;
use std::sync::LazyLock;
use std::time::Duration;

/// Reddit requires a descriptive User-Agent for API access.
/// Format: <platform>:<app id>:<version> (by /u/<username>)
pub(super) const REDDIT_USER_AGENT: &str = "axon-ingest/1.0 by /u/axon_bot";

/// Shared HTTP client for all Reddit API requests.
///
/// A single connection pool is reused across `get_access_token` and every
/// subsequent API call — avoids spawning a new pool per ingest invocation.
static REDDIT_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(REDDIT_USER_AGENT)
        .https_only(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to build Reddit HTTP client")
});

/// Obtain an OAuth2 bearer token from Reddit using client credentials.
pub async fn get_access_token(
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn Error>> {
    let resp: serde_json::Value = REDDIT_CLIENT
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
pub(super) async fn fetch_reddit_json(
    url: &str,
    token: &str,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let mut attempt = 0usize;
    loop {
        let resp = REDDIT_CLIENT.get(url).bearer_auth(token).send().await?;
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
