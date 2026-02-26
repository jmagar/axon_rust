//! Shared job infrastructure: pool creation, AMQP channel, job lifecycle helpers.
//!
//! ## Patterns
//! - Create [`PgPool`] once per worker via [`make_pool`]; pass as `&PgPool` everywhere.
//! - All AMQP work goes through [`open_amqp_channel`] (5 s timeout).
//! - Use [`claim_next_pending`] → [`mark_job_failed`] /
//!   `mark_job_completed` — never write raw SQL job state updates.
//! - All internal channels are bounded (`channel(256)`); never use `unbounded_channel`.

mod amqp;
mod job_ops;
pub(crate) mod pool;
mod schema;
pub(crate) mod stats;
pub(crate) mod watchdog;

// Re-exported for tests (used via `use super::*` in tests.rs).
#[cfg(test)]
use anyhow::Result;
#[cfg(test)]
use chrono::{DateTime, Utc};
#[cfg(test)]
use serde_json::Value;
#[cfg(test)]
use sqlx::postgres::PgPoolOptions;

use lapin::options::QueueDeclareOptions;

// Re-export all public items so existing `use crate::crates::jobs::common::*` continues to work.
pub use amqp::{batch_enqueue_jobs, enqueue_job, open_amqp_channel};
pub(crate) use amqp::{open_amqp_connection_and_channel, purge_queue_safe};
pub use job_ops::{claim_next_pending, claim_pending_by_id, mark_job_failed, touch_running_job};
pub use pool::make_pool;
pub(crate) use schema::begin_schema_migration_tx;
pub use stats::{count_stale_and_pending_jobs, count_stale_and_pending_jobs_with_pool};
#[cfg(test)]
pub(crate) use watchdog::stale_watchdog_confirmed;
#[cfg(test)]
pub(crate) use watchdog::stale_watchdog_payload;
pub use watchdog::{WatchdogSweepStats, reclaim_stale_running_jobs};

fn durable_queue_options() -> QueueDeclareOptions {
    QueueDeclareOptions {
        durable: true,
        auto_delete: false,
        exclusive: false,
        nowait: false,
        passive: false,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum JobTable {
    Crawl,
    Extract,
    Embed,
    Ingest,
}

impl JobTable {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Crawl => "axon_crawl_jobs",
            Self::Extract => "axon_extract_jobs",
            Self::Embed => "axon_embed_jobs",
            Self::Ingest => "axon_ingest_jobs",
        }
    }
}

#[cfg(test)]
pub(crate) fn test_config(pg_url: &str) -> crate::crates::core::config::Config {
    use crate::crates::core::config::Config;
    use std::path::PathBuf;
    Config {
        pg_url: pg_url.to_string(),
        redis_url: "redis://127.0.0.1:1".to_string(),
        amqp_url: "amqp://guest:guest@127.0.0.1:1/%2f".to_string(),
        collection: "test".to_string(),
        output_dir: PathBuf::from(".cache/test-worker-jobs"),
        crawl_queue: "axon.test.crawl".to_string(),
        extract_queue: "axon.test.extract".to_string(),
        embed_queue: "axon.test.embed".to_string(),
        ingest_queue: "axon.test.ingest".to_string(),
        tei_url: "http://127.0.0.1:1".to_string(),
        qdrant_url: "http://127.0.0.1:1".to_string(),
        openai_base_url: "http://127.0.0.1:1/v1".to_string(),
        openai_api_key: "test".to_string(),
        openai_model: "test-model".to_string(),
        // Test-specific overrides from the original literal
        search_limit: 5,
        max_pages: 10,
        max_depth: 2,
        min_markdown_chars: 50,
        drop_thin_markdown: false,
        embed: false,
        batch_concurrency: 2,
        yes: true,
        crawl_concurrency_limit: Some(2),
        backfill_concurrency_limit: Some(2),
        request_timeout_ms: Some(5_000),
        fetch_retries: 0,
        retry_backoff_ms: 0,
        ask_max_context_chars: 12_000,
        ..Config::default()
    }
}

#[cfg(test)]
mod tests;
