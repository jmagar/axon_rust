mod audit_diff;
mod backfill;
mod manifest_audit;
mod sitemap;
#[cfg(test)]
mod sitemap_migration_tests;

pub(super) use backfill::append_robots_backfill;
use manifest_audit::CrawlAuditSnapshot;
pub(crate) use sitemap::discover_sitemap_urls_with_robots;

use crate::crates::core::config::Config;
use crate::crates::core::http::validate_url;
use crate::crates::core::ui::{muted, primary};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct CrawlAuditSnapshotDiff {
    generated_at_epoch_ms: u128,
    previous_report: String,
    current_report: String,
    discovered_added: usize,
    discovered_removed: usize,
    manifest_added: usize,
    manifest_removed: usize,
    manifest_changed: usize,
}

pub(super) fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub(super) async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    if validate_url(url).is_err() {
        return None;
    }
    for attempt in 0..=retries {
        let response = client.get(url).send().await;
        if let Ok(resp) = response {
            if resp.status().is_success() {
                if let Ok(text) = resp.text().await {
                    return Some(text);
                }
            } else if resp.status().is_client_error()
                && resp.status() != reqwest::StatusCode::TOO_MANY_REQUESTS
            {
                return None;
            }
        }
        if attempt < retries {
            let delay = backoff_ms.saturating_mul((attempt + 1) as u64);
            if delay > 0 {
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }
    }
    None
}

pub(super) async fn run_crawl_audit(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    let (path, snapshot) = manifest_audit::persist_audit_snapshot(cfg, start_url).await?;
    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "audit_report_path": path.to_string_lossy(),
                "snapshot": snapshot,
            }))?
        );
    } else {
        println!("{}", primary("Crawl Audit"));
        println!("  {} {}", muted("Report:"), path.to_string_lossy());
        println!(
            "  {} {}",
            muted("Discovered URLs:"),
            snapshot.discovered_url_count
        );
        println!(
            "  {} {}",
            muted("Manifest entries:"),
            snapshot.manifest_entry_count
        );
    }
    Ok(())
}

pub(super) async fn run_crawl_audit_diff(cfg: &Config) -> Result<(), Box<dyn Error>> {
    audit_diff::run_crawl_audit_diff(cfg).await
}
