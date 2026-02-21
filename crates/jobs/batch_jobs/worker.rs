use super::*;
use crate::crates::jobs::worker_lane::{run_job_worker, ProcessFn, WorkerConfig};
use futures_util::stream::{self, StreamExt};
use std::path::Path;

const BATCH_FETCH_CONCURRENCY: usize = 16;

async fn load_batch_job_inputs(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(Vec<String>, BatchJobConfig)>, Box<dyn Error>> {
    let row = sqlx::query_as::<_, (serde_json::Value, serde_json::Value)>(
        "SELECT urls_json, config_json FROM axon_batch_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((urls_json, cfg_json)) = row else {
        return Ok(None);
    };
    let job_cfg: BatchJobConfig = serde_json::from_value(cfg_json)?;
    let urls: Vec<String> = serde_json::from_value(urls_json)?;
    Ok(Some((urls, job_cfg)))
}

async fn mark_batch_canceled(
    cfg: &Config,
    pool: &PgPool,
    id: Uuid,
) -> Result<bool, Box<dyn Error>> {
    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:batch:cancel:{id}");
    let cancel_before: Option<String> = redis_conn
        .get(&cancel_key)
        .await
        .map_err(|e| format!("failed to check batch cancellation key {cancel_key}: {e}"))?;
    if cancel_before.is_none() {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE axon_batch_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(true)
}

async fn fetch_batch_results(
    urls: &[String],
    out_dir: &Path,
) -> Result<(Vec<serde_json::Value>, Vec<InjectionCandidate>), Box<dyn Error>> {
    let client = build_client(20)?;
    let mut pending = stream::iter(urls.iter().enumerate().map(|(idx, url)| {
        let client = client.clone();
        let out_dir = out_dir.to_path_buf();
        let url = url.clone();
        async move {
            let html = fetch_html(&client, &url).await;
            match html {
                Ok(v) => {
                    let md = to_markdown(&v);
                    let file = out_dir.join(url_to_filename(&url, idx as u32 + 1));
                    match tokio::fs::write(&file, &md).await {
                        Ok(()) => {
                            let markdown_chars = md.chars().count();
                            (
                                idx,
                                serde_json::json!({
                                    "url": url,
                                    "file_path": file.to_string_lossy(),
                                    "markdown_chars": markdown_chars
                                }),
                                Some(InjectionCandidate {
                                    url,
                                    markdown_chars,
                                }),
                            )
                        }
                        Err(err) => (
                            idx,
                            serde_json::json!({"url": url, "error": err.to_string()}),
                            None,
                        ),
                    }
                }
                Err(err) => (
                    idx,
                    serde_json::json!({"url": url, "error": err.to_string()}),
                    None,
                ),
            }
        }
    }))
    .buffer_unordered(BATCH_FETCH_CONCURRENCY);

    let mut ordered = Vec::with_capacity(urls.len());
    while let Some(row) = pending.next().await {
        ordered.push(row);
    }
    ordered.sort_by_key(|(idx, _, _)| *idx);

    let mut results = Vec::with_capacity(ordered.len());
    let mut candidates = Vec::new();
    for (_idx, result, candidate) in ordered {
        results.push(result);
        if let Some(candidate) = candidate {
            candidates.push(candidate);
        }
    }
    Ok((results, candidates))
}

async fn maybe_embed_batch_output(
    cfg: &Config,
    job_cfg: &BatchJobConfig,
    out_dir: &Path,
    id: Uuid,
) {
    if !job_cfg.embed {
        return;
    }
    let mut embed_cfg = cfg.clone();
    embed_cfg.collection = job_cfg.collection.clone();
    if let Err(e) = embed_path_native(&embed_cfg, &out_dir.to_string_lossy()).await {
        log_warn(&format!("batch job {id}: embed failed (non-fatal): {e:#}"));
    }
}

async fn process_batch_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let Some((urls, job_cfg)) = load_batch_job_inputs(pool, id).await? else {
        return Ok(());
    };
    if mark_batch_canceled(cfg, pool, id).await? {
        return Ok(());
    }

    let out_dir = PathBuf::from(job_cfg.output_dir.clone())
        .join("batch-jobs")
        .join(id.to_string());
    if out_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&out_dir).await;
    }
    tokio::fs::create_dir_all(&out_dir).await?;

    let (results, candidates) = fetch_batch_results(&urls, &out_dir).await?;
    let queue_injection = apply_queue_injection(
        cfg,
        &candidates,
        job_cfg.extraction_prompt.as_deref(),
        "batch-post-fetch",
        true,
    )
    .await?;
    maybe_embed_batch_output(cfg, &job_cfg, &out_dir, id).await;

    sqlx::query(
        "UPDATE axon_batch_jobs SET status='completed',updated_at=NOW(),finished_at=NOW(),result_json=$2,error_text=NULL WHERE id=$1 AND status='running'",
    )
    .bind(id)
    .bind(serde_json::json!({
        "results": results,
        "queue_injection": queue_injection,
        "extraction_observability": queue_injection["observability"].clone(),
    }))
    .execute(pool)
    .await?;

    log_done(&format!("worker completed batch job {id}"));
    Ok(())
}

async fn process_claimed_batch_job(cfg: Config, pool: PgPool, id: Uuid) {
    let fail_msg = match process_batch_job(&cfg, &pool, id).await {
        Ok(()) => None,
        Err(err) => Some(err.to_string()),
    };
    if let Some(error_text) = fail_msg {
        mark_job_failed(&pool, TABLE, id, &error_text).await;
        log_warn(&format!("worker failed batch job {id}: {error_text}"));
    }
}

pub async fn run_batch_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let wc = WorkerConfig {
        table: TABLE,
        queue_name: cfg.batch_queue.clone(),
        job_kind: "batch",
        consumer_tag_prefix: "axon-rust-batch-worker",
        lane_count: WORKER_CONCURRENCY,
    };

    let process_fn: ProcessFn =
        std::sync::Arc::new(|cfg, pool, id| Box::pin(process_claimed_batch_job(cfg, pool, id)));

    run_job_worker(cfg, pool, &wc, process_fn).await
}
