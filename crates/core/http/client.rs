//! HTTP client construction and shared singleton.

use std::sync::LazyLock;
use std::time::Duration;

use super::error::HttpError;
use super::normalize::normalize_url;
use super::ssrf::validate_url;

pub(crate) static HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> =
    LazyLock::new(|| build_client(30).map_err(|e| e.to_string()));

pub fn http_client() -> anyhow::Result<&'static reqwest::Client> {
    HTTP_CLIENT
        .as_ref()
        .map_err(|err| anyhow::Error::msg(format!("failed to initialize HTTP client: {err}")))
}

pub fn build_client(timeout_secs: u64) -> Result<reqwest::Client, HttpError> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
}

pub async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, anyhow::Error> {
    let normalized = normalize_url(url);
    validate_url(&normalized)?;
    let body = client
        .get(&normalized)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(body)
}
