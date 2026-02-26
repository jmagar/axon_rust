use super::state::{load_target_states, upsert_target_state};
use super::{
    REFRESH_HEARTBEAT_INTERVAL_SECS, RefreshJobConfig, RefreshPageResult, RefreshRunSummary,
    RefreshTargetState, TABLE,
};
use crate::crates::core::config::Config;
use crate::crates::core::content::{to_markdown, url_to_filename};
use crate::crates::core::http::{http_client, validate_url};
use crate::crates::core::logging::log_warn;
use crate::crates::crawl::manifest::ManifestEntry;
use crate::crates::jobs::common::{mark_job_completed, mark_job_failed, touch_running_job};
use crate::crates::jobs::status::JobStatus;
use crate::crates::vector::ops::embed_text_with_metadata;
use reqwest::StatusCode;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::error::Error;
use tokio::io::AsyncWriteExt;
use tokio::time::Duration;
use uuid::Uuid;

pub(crate) async fn refresh_one_url(
    client: &reqwest::Client,
    url: &str,
    previous: Option<&RefreshTargetState>,
) -> Result<RefreshPageResult, Box<dyn Error>> {
    validate_url(url)?;
    fetch_and_process_url(client, url, previous).await
}

/// Core fetch-and-hash logic without SSRF validation (for testability).
async fn fetch_and_process_url(
    client: &reqwest::Client,
    url: &str,
    previous: Option<&RefreshTargetState>,
) -> Result<RefreshPageResult, Box<dyn Error>> {
    let mut request = client.get(url);
    if let Some(prev) = previous {
        if let Some(etag) = prev.etag.as_deref() {
            request = request.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = prev.last_modified.as_deref() {
            request = request.header(IF_MODIFIED_SINCE, last_modified);
        }
    }

    let response = request.send().await?;
    let status = response.status();
    let status_code = status.as_u16();

    if status == StatusCode::NOT_MODIFIED {
        return Ok(RefreshPageResult {
            status_code,
            etag: None,
            last_modified: None,
            content_hash: previous.and_then(|s| s.content_hash.clone()),
            markdown_chars: None,
            markdown: None,
            changed: false,
            not_modified: true,
        });
    }

    let headers = response.headers().clone();
    let etag = headers
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());
    let last_modified = headers
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string());

    if !status.is_success() {
        return Ok(RefreshPageResult {
            status_code,
            etag,
            last_modified,
            content_hash: previous.and_then(|s| s.content_hash.clone()),
            markdown_chars: None,
            markdown: None,
            changed: false,
            not_modified: false,
        });
    }

    let body = response.text().await?;
    let markdown = to_markdown(&body);
    let trimmed = markdown.trim().to_string();
    let mut hasher = Sha256::new();
    hasher.update(trimmed.as_bytes());
    let content_hash = hex::encode(hasher.finalize());
    let markdown_chars = trimmed.chars().count();
    let changed = previous
        .and_then(|s| s.content_hash.as_deref())
        .is_none_or(|hash| hash != content_hash);

    Ok(RefreshPageResult {
        status_code,
        etag,
        last_modified,
        content_hash: Some(content_hash),
        markdown_chars: Some(markdown_chars),
        markdown: Some(trimmed),
        changed,
        not_modified: false,
    })
}

/// Set up context for a refresh job: load config, create directories, start heartbeat.
/// Returns `None` if setup failed (job already marked failed).
async fn setup_refresh_job_context(
    pool: &PgPool,
    id: Uuid,
) -> Option<(
    RefreshJobConfig,
    std::path::PathBuf,
    tokio::io::BufWriter<tokio::fs::File>,
    tokio::sync::watch::Sender<bool>,
    tokio::task::JoinHandle<()>,
)> {
    let cfg_row = sqlx::query_scalar::<_, serde_json::Value>(
        "SELECT config_json FROM axon_refresh_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await;

    let job_cfg: RefreshJobConfig = match cfg_row {
        Ok(Some(v)) => match serde_json::from_value(v) {
            Ok(c) => c,
            Err(e) => {
                let _ =
                    mark_job_failed(pool, TABLE, id, &format!("invalid config_json: {e}")).await;
                return None;
            }
        },
        Ok(None) => {
            let _ = mark_job_failed(pool, TABLE, id, "job not found in DB").await;
            return None;
        }
        Err(e) => {
            let _ = mark_job_failed(pool, TABLE, id, &format!("DB read error: {e}")).await;
            return None;
        }
    };

    let run_dir = std::path::PathBuf::from(&job_cfg.output_dir)
        .join("refresh")
        .join(id.to_string());
    let markdown_dir = run_dir.join("markdown");

    if let Err(err) = tokio::fs::create_dir_all(&markdown_dir).await {
        let _ = mark_job_failed(
            pool,
            TABLE,
            id,
            &format!("create refresh output dir failed: {err}"),
        )
        .await;
        return None;
    }

    let manifest_path = run_dir.join("manifest.jsonl");
    let manifest_file = match tokio::fs::File::create(&manifest_path).await {
        Ok(file) => file,
        Err(err) => {
            let _ = mark_job_failed(
                pool,
                TABLE,
                id,
                &format!("create refresh manifest failed: {err}"),
            )
            .await;
            return None;
        }
    };
    let manifest = tokio::io::BufWriter::new(manifest_file);

    let (heartbeat_stop_tx, mut heartbeat_stop_rx) = tokio::sync::watch::channel(false);
    let heartbeat_pool = pool.clone();
    let heartbeat_task = tokio::spawn(async move {
        let mut ticker =
            tokio::time::interval(Duration::from_secs(REFRESH_HEARTBEAT_INTERVAL_SECS));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let _ = touch_running_job(&heartbeat_pool, TABLE, id).await;
                }
                changed = heartbeat_stop_rx.changed() => {
                    if changed.is_err() || *heartbeat_stop_rx.borrow() {
                        break;
                    }
                }
            }
        }
    });

    Some((
        job_cfg,
        run_dir,
        manifest,
        heartbeat_stop_tx,
        heartbeat_task,
    ))
}

/// Process a single URL within a refresh job: fetch, hash-compare, write markdown, embed.
#[allow(clippy::too_many_arguments)]
async fn process_single_refresh_url(
    cfg: &Config,
    pool: &PgPool,
    client: &reqwest::Client,
    url: &str,
    previous: Option<&RefreshTargetState>,
    markdown_dir: &std::path::Path,
    manifest: &mut tokio::io::BufWriter<tokio::fs::File>,
    embed: bool,
    id: Uuid,
    summary: &mut RefreshRunSummary,
    changed_idx: &mut u32,
) {
    match refresh_one_url(client, url, previous).await {
        Ok(result) => {
            if result.status_code >= 400 {
                summary.failed += 1;
                let error_text = format!("HTTP {}", result.status_code);
                let _ = upsert_target_state(pool, url, &result, Some(&error_text)).await;
                return;
            }

            if result.not_modified {
                summary.not_modified += 1;
                summary.unchanged += 1;
                let _ = upsert_target_state(pool, url, &result, None).await;
                return;
            }

            if result.changed {
                *changed_idx += 1;
                summary.changed += 1;

                if let Some(markdown) = result.markdown.as_deref() {
                    let filename = url_to_filename(url, *changed_idx);
                    let file_path = markdown_dir.join(&filename);
                    if let Err(err) = tokio::fs::write(&file_path, markdown.as_bytes()).await {
                        summary.failed += 1;
                        let _ = upsert_target_state(
                            pool,
                            url,
                            &result,
                            Some(&format!("write markdown failed: {err}")),
                        )
                        .await;
                        return;
                    }

                    let entry = ManifestEntry {
                        url: url.to_string(),
                        relative_path: format!("markdown/{filename}"),
                        markdown_chars: result.markdown_chars.unwrap_or(0),
                        content_hash: result.content_hash.clone(),
                        changed: true,
                    };
                    if let Ok(mut line) = serde_json::to_string(&entry) {
                        line.push('\n');
                        let _ = manifest.write_all(line.as_bytes()).await;
                    }

                    if embed {
                        match embed_text_with_metadata(cfg, markdown, url, "refresh", None).await {
                            Ok(chunks) => {
                                summary.embedded_chunks += chunks;
                            }
                            Err(err) => {
                                log_warn(&format!(
                                    "refresh embed failed for url={} job_id={}: {}",
                                    url, id, err
                                ));
                            }
                        }
                    }
                }
            } else {
                summary.unchanged += 1;
            }

            let _ = upsert_target_state(pool, url, &result, None).await;
        }
        Err(err) => {
            summary.failed += 1;
            let fallback = RefreshPageResult {
                status_code: 0,
                etag: previous.and_then(|s| s.etag.clone()),
                last_modified: previous.and_then(|s| s.last_modified.clone()),
                content_hash: previous.and_then(|s| s.content_hash.clone()),
                markdown_chars: None,
                markdown: None,
                changed: false,
                not_modified: false,
            };
            let _ = upsert_target_state(pool, url, &fallback, Some(&err.to_string())).await;
        }
    }
}

/// Finalize a refresh job: stop heartbeat, write final result to DB.
#[allow(clippy::too_many_arguments)]
async fn finalize_refresh_job(
    pool: &PgPool,
    id: Uuid,
    summary: &RefreshRunSummary,
    run_dir: &std::path::Path,
    manifest_path: &std::path::Path,
    total: usize,
    heartbeat_stop_tx: tokio::sync::watch::Sender<bool>,
    heartbeat_task: tokio::task::JoinHandle<()>,
) {
    let _ = heartbeat_stop_tx.send(true);
    if let Err(err) = heartbeat_task.await {
        log_warn(&format!(
            "command=refresh_worker heartbeat_task_panicked job_id={id} err={err:?}"
        ));
    }

    let final_result = serde_json::json!({
        "phase": "completed",
        "checked": summary.checked,
        "changed": summary.changed,
        "unchanged": summary.unchanged,
        "not_modified": summary.not_modified,
        "failed": summary.failed,
        "embedded_chunks": summary.embedded_chunks,
        "total": total,
        "output_dir": run_dir.to_string_lossy(),
        "manifest_path": manifest_path.to_string_lossy(),
    });

    match mark_job_completed(pool, TABLE, id, Some(&final_result)).await {
        Ok(false) => {
            log_warn(&format!(
                "command=refresh_worker completion_update_skipped job_id={id} reason=not_running_state"
            ));
        }
        Ok(true) => {}
        Err(err) => {
            let _ =
                mark_job_failed(pool, TABLE, id, &format!("mark completed failed: {err}")).await;
        }
    }
}

pub(crate) async fn process_refresh_job(cfg: Config, pool: PgPool, id: Uuid) {
    let Some((job_cfg, run_dir, mut manifest, heartbeat_stop_tx, heartbeat_task)) =
        setup_refresh_job_context(&pool, id).await
    else {
        return;
    };

    let mut summary = RefreshRunSummary::default();
    let client = match http_client() {
        Ok(c) => c,
        Err(err) => {
            let _ = heartbeat_stop_tx.send(true);
            let _ = heartbeat_task.await;
            let _ =
                mark_job_failed(&pool, TABLE, id, &format!("http client unavailable: {err}")).await;
            return;
        }
    };

    let states = match load_target_states(&pool, &job_cfg.urls).await {
        Ok(s) => s,
        Err(err) => {
            let _ = heartbeat_stop_tx.send(true);
            let _ = heartbeat_task.await;
            let _ = mark_job_failed(
                &pool,
                TABLE,
                id,
                &format!("load refresh target state failed: {err}"),
            )
            .await;
            return;
        }
    };

    let markdown_dir = run_dir.join("markdown");
    let mut changed_idx: u32 = 0;

    for url in &job_cfg.urls {
        summary.checked += 1;
        let previous = states.get(url);

        process_single_refresh_url(
            &cfg,
            &pool,
            client,
            url,
            previous,
            &markdown_dir,
            &mut manifest,
            job_cfg.embed,
            id,
            &mut summary,
            &mut changed_idx,
        )
        .await;

        let _ = sqlx::query(&format!(
            "UPDATE axon_refresh_jobs SET updated_at=NOW(), result_json=$2 WHERE id=$1 AND status='{running}'",
            running = JobStatus::Running.as_str(),
        ))
        .bind(id)
        .bind(serde_json::json!({
            "phase": "refreshing",
            "checked": summary.checked,
            "changed": summary.changed,
            "unchanged": summary.unchanged,
            "not_modified": summary.not_modified,
            "failed": summary.failed,
            "embedded_chunks": summary.embedded_chunks,
            "total": job_cfg.urls.len(),
            "output_dir": run_dir.to_string_lossy(),
        }))
        .execute(&pool)
        .await;
    }

    let _ = manifest.flush().await;

    let manifest_path = run_dir.join("manifest.jsonl");
    finalize_refresh_job(
        &pool,
        id,
        &summary,
        &run_dir,
        &manifest_path,
        job_cfg.urls.len(),
        heartbeat_stop_tx,
        heartbeat_task,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    #[tokio::test]
    async fn refresh_url_304_not_modified() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/page");
            then.status(304);
        });
        let url = format!("{}/page", server.base_url());
        let prev = RefreshTargetState {
            etag: Some("\"abc\"".into()),
            last_modified: None,
            content_hash: Some("oldhash".into()),
        };
        let client = reqwest::Client::new();
        let result = fetch_and_process_url(&client, &url, Some(&prev))
            .await
            .unwrap();
        assert!(result.not_modified);
        assert!(!result.changed);
        assert_eq!(result.status_code, 304);
        assert_eq!(result.content_hash.as_deref(), Some("oldhash"));
    }

    #[tokio::test]
    async fn refresh_url_200_matching_hash() {
        let body = "<html><body><p>Hello World</p></body></html>";
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/page");
            then.status(200)
                .header("content-type", "text/html")
                .body(body);
        });
        let url = format!("{}/page", server.base_url());

        // First fetch to get the hash
        let client = reqwest::Client::new();
        let first = fetch_and_process_url(&client, &url, None).await.unwrap();
        assert!(first.changed); // first time = changed
        let hash = first.content_hash.clone().unwrap();

        // Second fetch with matching hash
        let prev = RefreshTargetState {
            etag: None,
            last_modified: None,
            content_hash: Some(hash),
        };
        let result = fetch_and_process_url(&client, &url, Some(&prev))
            .await
            .unwrap();
        assert!(!result.changed);
        assert!(!result.not_modified);
        assert_eq!(result.status_code, 200);
    }

    #[tokio::test]
    async fn refresh_url_200_new_content() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/page");
            then.status(200)
                .header("content-type", "text/html")
                .header("etag", "\"new-etag\"")
                .body("<html><body><p>New content here</p></body></html>");
        });
        let url = format!("{}/page", server.base_url());
        let prev = RefreshTargetState {
            etag: Some("\"old-etag\"".into()),
            last_modified: None,
            content_hash: Some("stale-hash-that-wont-match".into()),
        };
        let client = reqwest::Client::new();
        let result = fetch_and_process_url(&client, &url, Some(&prev))
            .await
            .unwrap();
        assert!(result.changed);
        assert!(!result.not_modified);
        assert_eq!(result.status_code, 200);
        assert!(result.markdown.is_some());
        assert_eq!(result.etag.as_deref(), Some("\"new-etag\""));
    }

    #[tokio::test]
    async fn refresh_url_404_not_changed() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/gone");
            then.status(404);
        });
        let url = format!("{}/gone", server.base_url());
        let client = reqwest::Client::new();
        let result = fetch_and_process_url(&client, &url, None).await.unwrap();
        assert!(!result.changed);
        assert!(!result.not_modified);
        assert_eq!(result.status_code, 404);
        assert!(result.markdown.is_none());
    }

    #[tokio::test]
    async fn refresh_url_first_time_fetch() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/new");
            then.status(200)
                .header("content-type", "text/html")
                .header("last-modified", "Wed, 25 Feb 2026 00:00:00 GMT")
                .body("<html><body><p>Brand new page</p></body></html>");
        });
        let url = format!("{}/new", server.base_url());
        let client = reqwest::Client::new();
        let result = fetch_and_process_url(&client, &url, None).await.unwrap();
        assert!(result.changed);
        assert!(!result.not_modified);
        assert_eq!(result.status_code, 200);
        assert!(result.content_hash.is_some());
        assert!(result.markdown.is_some());
        assert_eq!(
            result.last_modified.as_deref(),
            Some("Wed, 25 Feb 2026 00:00:00 GMT")
        );
    }
}
