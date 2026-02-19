use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::content::{to_markdown, url_to_filename};
use crate::axon_cli::crates::core::health::redis_healthy;
use crate::axon_cli::crates::core::http::{build_client, fetch_html};
use crate::axon_cli::crates::core::logging::{log_done, log_info, log_warn};
use crate::axon_cli::crates::jobs::common::{
    claim_next_pending, claim_pending_by_id, enqueue_job, make_pool, mark_job_failed,
    open_amqp_channel, reclaim_stale_running_jobs, JobTable,
};
use crate::axon_cli::crates::jobs::extract_jobs::start_extract_job;
use crate::axon_cli::crates::vector::ops::embed_path_native;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use lapin::options::{BasicAckOptions, BasicConsumeOptions};
use lapin::types::FieldTable;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use spider::tokio;
use sqlx::{FromRow, PgPool};
use std::error::Error;
use std::fmt::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use uuid::Uuid;

const TABLE: JobTable = JobTable::Batch;
const STALE_RUNNING_TIMEOUT_SECS: i64 = 300;
const STALE_CONFIRMATION_SECS: i64 = 60;
const STALE_SWEEP_INTERVAL_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchJobConfig {
    embed: bool,
    collection: String,
    output_dir: String,
    extraction_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionCandidate {
    pub url: String,
    pub markdown_chars: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionRule {
    pub name: String,
    pub min_markdown_chars: usize,
    pub min_quality_score: f64,
    pub max_urls: usize,
    pub url_contains_any: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionDecision {
    pub url: String,
    pub markdown_chars: usize,
    pub quality_score: f64,
    pub selected: bool,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSelectionStats {
    pub name: String,
    pub selected: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionObservability {
    pub input_tokens_estimated: u64,
    pub output_tokens_estimated: u64,
    pub total_tokens_estimated: u64,
    pub estimated_cost_usd: f64,
    pub avg_quality_score: f64,
    pub quality_band: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInjectionEvaluation {
    pub phase: String,
    pub total_candidates: usize,
    pub selected_candidates: usize,
    pub rules: Vec<QueueInjectionRule>,
    pub selected_urls: Vec<String>,
    pub selected_by_rule: Vec<RuleSelectionStats>,
    pub decisions: Vec<QueueInjectionDecision>,
    pub observability: ExtractionObservability,
}

fn default_queue_injection_rules() -> Vec<QueueInjectionRule> {
    vec![
        QueueInjectionRule {
            name: "docs-first".to_string(),
            min_markdown_chars: 800,
            min_quality_score: 0.55,
            max_urls: 12,
            url_contains_any: vec![
                "docs".to_string(),
                "api".to_string(),
                "reference".to_string(),
                "guide".to_string(),
            ],
        },
        QueueInjectionRule {
            name: "tutorial-longform".to_string(),
            min_markdown_chars: 1600,
            min_quality_score: 0.60,
            max_urls: 8,
            url_contains_any: vec![
                "tutorial".to_string(),
                "blog".to_string(),
                "article".to_string(),
                "learn".to_string(),
            ],
        },
        QueueInjectionRule {
            name: "high-signal-catchall".to_string(),
            min_markdown_chars: 2200,
            min_quality_score: 0.72,
            max_urls: 4,
            url_contains_any: vec![],
        },
    ]
}

fn load_queue_injection_rules() -> Vec<QueueInjectionRule> {
    let maybe_rules = std::env::var("AXON_QUEUE_INJECTION_RULES_JSON")
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<QueueInjectionRule>>(&raw).ok())
        .filter(|rules| !rules.is_empty());
    maybe_rules.unwrap_or_else(default_queue_injection_rules)
}

fn estimate_quality_score(url: &str, markdown_chars: usize) -> f64 {
    let normalized_url = url.to_ascii_lowercase();
    let density_score = (markdown_chars as f64 / 3500.0).clamp(0.0, 1.0);
    let signal_bonus = if normalized_url.contains("docs")
        || normalized_url.contains("api")
        || normalized_url.contains("reference")
        || normalized_url.contains("guide")
        || normalized_url.contains("tutorial")
    {
        0.20
    } else {
        0.05
    };
    let depth_bonus = (normalized_url.matches('/').count() as f64 / 12.0).clamp(0.0, 0.10);
    (0.70 * density_score + signal_bonus + depth_bonus).clamp(0.0, 1.0)
}

fn quality_band(score: f64) -> String {
    if score >= 0.80 {
        "high".to_string()
    } else if score >= 0.55 {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn estimate_input_tokens(markdown_chars: usize) -> u64 {
    ((markdown_chars as f64 / 4.0).ceil() as u64).max(64)
}

pub fn evaluate_queue_injection(
    candidates: &[InjectionCandidate],
    phase: &str,
) -> QueueInjectionEvaluation {
    let rules = load_queue_injection_rules();
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| b.markdown_chars.cmp(&a.markdown_chars));

    let mut selected_urls = Vec::new();
    let mut decisions = Vec::new();
    let mut selected_by_rule: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    for candidate in sorted {
        let quality = estimate_quality_score(&candidate.url, candidate.markdown_chars);
        let candidate_url = candidate.url.to_ascii_lowercase();
        let mut matched_rule = None;

        for rule in &rules {
            if candidate.markdown_chars < rule.min_markdown_chars {
                continue;
            }
            if quality < rule.min_quality_score {
                continue;
            }

            let keyword_match = rule.url_contains_any.is_empty()
                || rule
                    .url_contains_any
                    .iter()
                    .any(|k| candidate_url.contains(&k.to_ascii_lowercase()));
            if !keyword_match {
                continue;
            }

            let already_selected = selected_by_rule.get(&rule.name).copied().unwrap_or(0);
            if already_selected >= rule.max_urls {
                continue;
            }

            *selected_by_rule.entry(rule.name.clone()).or_insert(0) += 1;
            selected_urls.push(candidate.url.clone());
            matched_rule = Some(rule.name.clone());
            break;
        }

        decisions.push(QueueInjectionDecision {
            url: candidate.url,
            markdown_chars: candidate.markdown_chars,
            quality_score: quality,
            selected: matched_rule.is_some(),
            matched_rule,
        });
    }

    let input_tokens_estimated: u64 = decisions
        .iter()
        .filter(|d| d.selected)
        .map(|d| estimate_input_tokens(d.markdown_chars))
        .sum();
    let output_tokens_estimated = (selected_urls.len() as u64).saturating_mul(220);
    let total_tokens_estimated = input_tokens_estimated.saturating_add(output_tokens_estimated);

    let price_per_1k_tokens = std::env::var("AXON_EXTRACT_EST_COST_PER_1K_TOKENS")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .filter(|v| *v > 0.0)
        .unwrap_or(0.0025);
    let estimated_cost_usd =
        ((total_tokens_estimated as f64 / 1000.0) * price_per_1k_tokens * 100_000.0).round()
            / 100_000.0;

    let selected_quality_total: f64 = decisions
        .iter()
        .filter(|d| d.selected)
        .map(|d| d.quality_score)
        .sum();
    let avg_quality_score = if selected_urls.is_empty() {
        0.0
    } else {
        selected_quality_total / selected_urls.len() as f64
    };

    let selected_by_rule = selected_by_rule
        .into_iter()
        .map(|(name, selected)| RuleSelectionStats { name, selected })
        .collect();

    QueueInjectionEvaluation {
        phase: phase.to_string(),
        total_candidates: decisions.len(),
        selected_candidates: selected_urls.len(),
        rules,
        selected_urls,
        selected_by_rule,
        decisions,
        observability: ExtractionObservability {
            input_tokens_estimated,
            output_tokens_estimated,
            total_tokens_estimated,
            estimated_cost_usd,
            avg_quality_score,
            quality_band: quality_band(avg_quality_score),
        },
    }
}

pub async fn apply_queue_injection(
    cfg: &Config,
    candidates: &[InjectionCandidate],
    extraction_prompt: Option<&str>,
    phase: &str,
    enqueue_enabled: bool,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let evaluation = evaluate_queue_injection(candidates, phase);
    let mut payload = serde_json::to_value(&evaluation)?;
    let selected_urls = evaluation.selected_urls.clone();
    let prompt = extraction_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);

    let mut queue_status = String::new();
    let mut extract_job_id: Option<Uuid> = None;

    if selected_urls.is_empty() {
        queue_status.push_str("skipped_no_candidates");
    } else if prompt.is_none() {
        queue_status.push_str("skipped_missing_prompt");
    } else if !enqueue_enabled {
        queue_status.push_str("deferred");
    } else {
        let job_id = start_extract_job(cfg, &selected_urls, prompt).await?;
        write!(&mut queue_status, "enqueued").ok();
        extract_job_id = Some(job_id);
        log_info(&format!(
            "command=queue_injection phase={} selected={} extract_job_id={}",
            phase,
            selected_urls.len(),
            job_id
        ));
    }

    if let Some(object) = payload.as_object_mut() {
        object.insert("queue_status".to_string(), serde_json::json!(queue_status));
        object.insert(
            "enqueue_enabled".to_string(),
            serde_json::json!(enqueue_enabled),
        );
        object.insert(
            "extract_job_id".to_string(),
            serde_json::json!(extract_job_id),
        );
    }

    Ok(payload)
}

#[derive(Debug, FromRow, Serialize)]
pub struct BatchJob {
    pub id: Uuid,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error_text: Option<String>,
    pub urls_json: serde_json::Value,
    pub result_json: Option<serde_json::Value>,
}

async fn ensure_schema(pool: &PgPool) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS axon_batch_jobs (
            id UUID PRIMARY KEY,
            status TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            started_at TIMESTAMPTZ,
            finished_at TIMESTAMPTZ,
            error_text TEXT,
            urls_json JSONB NOT NULL,
            result_json JSONB,
            config_json JSONB NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn start_batch_job(cfg: &Config, urls: &[String]) -> Result<Uuid, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    let id = Uuid::new_v4();
    let cfg_json = serde_json::to_value(BatchJobConfig {
        embed: cfg.embed,
        collection: cfg.collection.clone(),
        output_dir: cfg.output_dir.to_string_lossy().to_string(),
        extraction_prompt: cfg.query.clone(),
    })?;

    sqlx::query(
        r#"INSERT INTO axon_batch_jobs (id, status, urls_json, config_json) VALUES ($1, 'pending', $2, $3)"#,
    )
    .bind(id)
    .bind(serde_json::to_value(urls)?)
    .bind(cfg_json)
    .execute(&pool)
    .await?;

    if let Err(err) = enqueue_job(cfg, &cfg.batch_queue, id).await {
        log_warn(&format!(
            "batch enqueue failed for {id}; polling fallback will pick up: {err}"
        ));
    }

    Ok(id)
}

pub async fn get_batch_job(cfg: &Config, id: Uuid) -> Result<Option<BatchJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, BatchJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_batch_jobs WHERE id=$1"#,
    )
    .bind(id)
    .fetch_optional(&pool)
    .await?)
}

pub async fn list_batch_jobs(cfg: &Config, limit: i64) -> Result<Vec<BatchJob>, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(sqlx::query_as::<_, BatchJob>(
        r#"SELECT id,status,created_at,updated_at,started_at,finished_at,error_text,urls_json,result_json FROM axon_batch_jobs ORDER BY created_at DESC LIMIT $1"#,
    )
    .bind(limit)
    .fetch_all(&pool)
    .await?)
}

pub async fn cancel_batch_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("UPDATE axon_batch_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ('pending','running')")
        .bind(id)
        .execute(&pool)
        .await?
        .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:batch:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 86400).await?;
    Ok(rows > 0)
}

pub async fn cleanup_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    Ok(
        sqlx::query("DELETE FROM axon_batch_jobs WHERE status IN ('failed','canceled')")
            .execute(&pool)
            .await?
            .rows_affected(),
    )
}

pub async fn clear_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;
    let rows = sqlx::query("DELETE FROM axon_batch_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    if let Ok(ch) = open_amqp_channel(cfg, &cfg.batch_queue).await {
        let _ = ch
            .queue_purge(
                &cfg.batch_queue,
                lapin::options::QueuePurgeOptions::default(),
            )
            .await;
    }
    Ok(rows)
}

async fn process_batch_job(cfg: &Config, pool: &PgPool, id: Uuid) -> Result<(), Box<dyn Error>> {
    let row = sqlx::query_as::<_, (serde_json::Value, serde_json::Value)>(
        "SELECT urls_json, config_json FROM axon_batch_jobs WHERE id=$1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    let Some((urls_json, cfg_json)) = row else {
        return Ok(());
    };

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut redis_conn = redis_client.get_multiplexed_async_connection().await?;
    let cancel_key = format!("axon:batch:cancel:{id}");
    let cancel_before: Option<String> = redis_conn.get(&cancel_key).await.ok();
    if cancel_before.is_some() {
        sqlx::query("UPDATE axon_batch_jobs SET status='canceled',updated_at=NOW(),finished_at=NOW() WHERE id=$1")
            .bind(id)
            .execute(pool)
            .await?;
        return Ok(());
    }

    let job_cfg: BatchJobConfig = serde_json::from_value(cfg_json)?;
    let urls: Vec<String> = serde_json::from_value(urls_json)?;
    let out_dir = PathBuf::from(job_cfg.output_dir)
        .join("batch-jobs")
        .join(id.to_string());
    if out_dir.exists() {
        let _ = tokio::fs::remove_dir_all(&out_dir).await;
    }
    tokio::fs::create_dir_all(&out_dir).await?;

    let client = build_client(20)?;
    let mut results = Vec::new();
    let mut candidates = Vec::new();
    for (idx, url) in urls.iter().enumerate() {
        let html = match fetch_html(&client, url).await {
            Ok(v) => v,
            Err(err) => {
                results.push(serde_json::json!({"url": url, "error": err.to_string()}));
                continue;
            }
        };
        let md = to_markdown(&html);
        let file = out_dir.join(url_to_filename(url, idx as u32 + 1));
        tokio::fs::write(&file, &md).await?;
        let markdown_chars = md.chars().count();
        candidates.push(InjectionCandidate {
            url: url.to_string(),
            markdown_chars,
        });
        results.push(serde_json::json!({
            "url": url,
            "file_path": file.to_string_lossy(),
            "markdown_chars": markdown_chars
        }));
    }

    let queue_injection = apply_queue_injection(
        cfg,
        &candidates,
        job_cfg.extraction_prompt.as_deref(),
        "batch-post-fetch",
        true,
    )
    .await?;

    if job_cfg.embed {
        let mut embed_cfg = cfg.clone();
        embed_cfg.collection = job_cfg.collection;
        embed_path_native(&embed_cfg, &out_dir.to_string_lossy()).await?;
    }

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

pub async fn run_batch_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    ensure_schema(&pool).await?;

    if let Ok(ch) = open_amqp_channel(cfg, &cfg.batch_queue).await {
        let mut consumer = ch
            .basic_consume(
                &cfg.batch_queue,
                "axon-rust-batch-worker",
                BasicConsumeOptions::default(),
                FieldTable::default(),
            )
            .await?;
        log_info(&format!(
            "batch worker listening on queue={}",
            cfg.batch_queue
        ));
        loop {
            let msg = match tokio::time::timeout(
                Duration::from_secs(STALE_SWEEP_INTERVAL_SECS),
                consumer.next(),
            )
            .await
            {
                Ok(Some(msg)) => msg,
                Ok(None) => break,
                Err(_) => {
                    if let Ok(reclaimed) = reclaim_stale_running_jobs(
                        &pool,
                        TABLE,
                        "batch",
                        STALE_RUNNING_TIMEOUT_SECS,
                        STALE_CONFIRMATION_SECS,
                        "amqp",
                    )
                    .await
                    {
                        if reclaimed > 0 {
                            log_warn(&format!("watchdog reclaimed {} stale batch jobs", reclaimed));
                        }
                    }
                    continue;
                }
            };
            let delivery = match msg {
                Ok(d) => d,
                Err(_) => continue,
            };
            let parsed = std::str::from_utf8(&delivery.data)
                .ok()
                .and_then(|s| Uuid::parse_str(s.trim()).ok());
            if let Some(job_id) = parsed {
                if claim_pending_by_id(&pool, TABLE, job_id)
                    .await
                    .unwrap_or(false)
                {
                    if let Err(err) = process_batch_job(cfg, &pool, job_id).await {
                        let error_text = err.to_string();
                        mark_job_failed(&pool, TABLE, job_id, &error_text).await;
                        log_warn(&format!("worker failed batch job {job_id}: {error_text}"));
                    }
                }
            }
            delivery.ack(BasicAckOptions::default()).await?;
        }
        return Ok(());
    }

    log_warn("amqp unavailable; running batch worker in postgres polling mode");
    let mut last_sweep = Instant::now();
    loop {
        if last_sweep.elapsed() >= Duration::from_secs(STALE_SWEEP_INTERVAL_SECS) {
            if let Ok(reclaimed) = reclaim_stale_running_jobs(
                &pool,
                TABLE,
                "batch",
                STALE_RUNNING_TIMEOUT_SECS,
                STALE_CONFIRMATION_SECS,
                "polling",
            )
            .await
            {
                if reclaimed > 0 {
                    log_warn(&format!("watchdog reclaimed {} stale batch jobs", reclaimed));
                }
            }
            last_sweep = Instant::now();
        }
        if let Some(id) = claim_next_pending(&pool, TABLE).await? {
            if let Err(err) = process_batch_job(cfg, &pool, id).await {
                let error_text = err.to_string();
                mark_job_failed(&pool, TABLE, id, &error_text).await;
                log_warn(&format!("worker failed batch job {id}: {error_text}"));
            }
        } else {
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    }
}

pub async fn batch_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = make_pool(cfg).await.is_ok();
    let amqp_ok = open_amqp_channel(cfg, &cfg.batch_queue).await.is_ok();
    let redis_ok = redis_healthy(&cfg.redis_url).await;
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}
