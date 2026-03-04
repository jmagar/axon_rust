use crate::crates::cli::commands::scrape::scrape_payload;
use crate::crates::core::config::Config;
use crate::crates::services::types::ScrapeResult;
use std::error::Error;

/// Map a raw JSON payload into a [`ScrapeResult`].
///
/// This is a pure function — no network required. Tests call it with JSON literals.
pub fn map_scrape_payload(payload: serde_json::Value) -> Result<ScrapeResult, Box<dyn Error>> {
    Ok(ScrapeResult { payload })
}

/// Scrape a single URL and return a typed [`ScrapeResult`].
///
/// Delegates to [`scrape_payload`] from the CLI commands layer; wraps the raw
/// JSON value into the typed service result.
pub async fn scrape(cfg: &Config, url: &str) -> Result<ScrapeResult, Box<dyn Error>> {
    let payload = scrape_payload(cfg, url).await?;
    map_scrape_payload(payload)
}
