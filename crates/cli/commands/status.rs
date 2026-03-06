mod metrics;
mod presentation;

use crate::crates::core::config::Config;
use crate::crates::jobs::crawl::{CrawlJob, list_jobs};
use crate::crates::jobs::embed::{EmbedJob, list_embed_jobs};
use crate::crates::jobs::extract::{ExtractJob, list_extract_jobs};
use crate::crates::jobs::ingest::{IngestJob, list_ingest_jobs};
use crate::crates::jobs::refresh::{RefreshJob, list_refresh_jobs};
use std::error::Error;

const WATCHDOG_RECLAIM_PREFIX: &str = "watchdog reclaimed stale running ";

pub async fn run_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.json_output {
        // JSON path: route through the service layer for a stable payload shape.
        let result = crate::crates::services::system::full_status(cfg).await?;
        println!("{}", serde_json::to_string_pretty(&result.payload)?);
    } else {
        // Human path: use the detailed per-job renderer for rich terminal output.
        run_status_impl(cfg).await?;
    }
    Ok(())
}

pub async fn status_snapshot(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    Ok(status_payload(
        &jobs.crawl,
        &jobs.extract,
        &jobs.embed,
        &jobs.ingest,
        &jobs.refresh,
    ))
}

pub async fn status_text(cfg: &Config) -> Result<String, Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    let mut lines = Vec::new();
    lines.push("Axon Status".to_string());
    lines.push(format!("crawl jobs:   {}", jobs.crawl.len()));
    lines.push(format!("extract jobs: {}", jobs.extract.len()));
    lines.push(format!("embed jobs:   {}", jobs.embed.len()));
    lines.push(format!("ingest jobs:  {}", jobs.ingest.len()));
    lines.push(format!("refresh jobs: {}", jobs.refresh.len()));
    Ok(lines.join("\n"))
}

pub(crate) async fn status_full(
    cfg: &Config,
) -> Result<(serde_json::Value, String), Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    let json = status_payload(
        &jobs.crawl,
        &jobs.extract,
        &jobs.embed,
        &jobs.ingest,
        &jobs.refresh,
    );
    let text = [
        "Axon Status".to_string(),
        format!("crawl jobs:   {}", jobs.crawl.len()),
        format!("extract jobs: {}", jobs.extract.len()),
        format!("embed jobs:   {}", jobs.embed.len()),
        format!("ingest jobs:  {}", jobs.ingest.len()),
        format!("refresh jobs: {}", jobs.refresh.len()),
    ]
    .join("\n");
    Ok((json, text))
}

struct StatusJobs {
    crawl: Vec<CrawlJob>,
    extract: Vec<ExtractJob>,
    embed: Vec<EmbedJob>,
    ingest: Vec<IngestJob>,
    refresh: Vec<RefreshJob>,
}

async fn run_status_impl(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    presentation::emit_status_human(
        &jobs.crawl,
        &jobs.extract,
        &jobs.embed,
        &jobs.ingest,
        &jobs.refresh,
    );
    Ok(())
}

async fn load_status_jobs(cfg: &Config) -> Result<StatusJobs, Box<dyn Error>> {
    let (crawl_raw, extract_raw, embed_raw, ingest_raw, refresh_raw) = tokio::join!(
        async {
            list_jobs(cfg, 20, 0)
                .await
                .map_err(|e| format!("crawl status lookup failed: {e}"))
        },
        async {
            list_extract_jobs(cfg, 20, 0)
                .await
                .map_err(|e| format!("extract status lookup failed: {e}"))
        },
        async {
            list_embed_jobs(cfg, 20, 0)
                .await
                .map_err(|e| format!("embed status lookup failed: {e}"))
        },
        async {
            list_ingest_jobs(cfg, 20, 0)
                .await
                .map_err(|e| format!("ingest status lookup failed: {e}"))
        },
        async {
            list_refresh_jobs(cfg, 20, 0)
                .await
                .map_err(|e| format!("refresh status lookup failed: {e}"))
        }
    );
    let crawl: Vec<_> = crawl_raw?
        .into_iter()
        .filter(|job| {
            include_status_job(
                &job.status,
                job.error_text.as_deref(),
                cfg.reclaimed_status_only,
            )
        })
        .collect();
    let extract: Vec<_> = extract_raw?
        .into_iter()
        .filter(|job| {
            include_status_job(
                &job.status,
                job.error_text.as_deref(),
                cfg.reclaimed_status_only,
            )
        })
        .collect();
    let embed: Vec<_> = embed_raw?
        .into_iter()
        .filter(|job| {
            include_status_job(
                &job.status,
                job.error_text.as_deref(),
                cfg.reclaimed_status_only,
            )
        })
        .collect();
    let ingest: Vec<_> = ingest_raw?
        .into_iter()
        .filter(|job| {
            include_status_job(
                &job.status,
                job.error_text.as_deref(),
                cfg.reclaimed_status_only,
            )
        })
        .collect();
    let refresh: Vec<_> = refresh_raw?
        .into_iter()
        .filter(|job| {
            include_status_job(
                &job.status,
                job.error_text.as_deref(),
                cfg.reclaimed_status_only,
            )
        })
        .collect();
    Ok(StatusJobs {
        crawl,
        extract,
        embed,
        ingest,
        refresh,
    })
}

fn include_status_job(status: &str, error_text: Option<&str>, reclaimed_only: bool) -> bool {
    let reclaimed = is_watchdog_reclaimed_failure(status, error_text);
    if reclaimed_only {
        reclaimed
    } else {
        !reclaimed
    }
}

fn is_watchdog_reclaimed_failure(status: &str, error_text: Option<&str>) -> bool {
    if status != "failed" {
        return false;
    }
    error_text
        .map(str::trim_start)
        .is_some_and(|text| text.starts_with(WATCHDOG_RECLAIM_PREFIX))
}

fn status_payload(
    crawl_jobs: &[CrawlJob],
    extract_jobs: &[ExtractJob],
    embed_jobs: &[EmbedJob],
    ingest_jobs: &[IngestJob],
    refresh_jobs: &[RefreshJob],
) -> serde_json::Value {
    presentation::status_payload(
        crawl_jobs,
        extract_jobs,
        embed_jobs,
        ingest_jobs,
        refresh_jobs,
    )
}

#[cfg(test)]
mod tests {
    use super::{include_status_job, is_watchdog_reclaimed_failure, status_payload};

    #[test]
    fn watchdog_reclaim_detection_matches_prefix_on_failed_jobs() {
        assert!(is_watchdog_reclaimed_failure(
            "failed",
            Some("watchdog reclaimed stale running ingest job (idle=360s marker=amqp)")
        ));
        assert!(!is_watchdog_reclaimed_failure(
            "error",
            Some("watchdog reclaimed stale running crawl job (idle=361s marker=polling)")
        ));
        assert!(!is_watchdog_reclaimed_failure(
            "completed",
            Some("watchdog reclaimed stale running ingest job (idle=360s marker=amqp)")
        ));
        assert!(!is_watchdog_reclaimed_failure(
            "failed",
            Some("network timeout")
        ));
    }

    #[test]
    fn status_filter_hides_reclaimed_by_default_and_shows_in_reclaimed_mode() {
        let reclaimed_err =
            Some("watchdog reclaimed stale running extract job (idle=360s marker=amqp)");
        assert!(!include_status_job("failed", reclaimed_err, false));
        assert!(include_status_job("failed", reclaimed_err, true));
        assert!(include_status_job("completed", None, false));
        assert!(!include_status_job("completed", None, true));
    }

    #[test]
    fn status_snapshot_includes_refresh_jobs_key() {
        let payload = status_payload(&[], &[], &[], &[], &[]);
        assert!(payload.get("local_refresh_jobs").is_some());
    }
}
