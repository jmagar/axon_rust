use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use crate::crates::jobs::ingest::IngestSource;
use crate::crates::services::ingest as ingest_service;
use std::error::Error;

pub async fn run_reddit(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if ingest_common::maybe_handle_ingest_subcommand(cfg, "reddit").await? {
        return Ok(());
    }

    let target = cfg
        .positional
        .first()
        .cloned()
        .ok_or("reddit requires <TARGET> (subreddit name or thread URL)")?;

    let source = IngestSource::Reddit { target };

    if !cfg.wait {
        return ingest_common::enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    let IngestSource::Reddit { ref target } = source else {
        // NOTE: This branch is unreachable for current callers but guards against
        // future callers passing the wrong IngestSource variant.
        return Err(format!("reddit: expected Reddit source, got {:?}", source).into());
    };

    let result = ingest_service::ingest_reddit(cfg, target, None).await?;
    let chunks = result.payload["chunks"].as_u64().unwrap_or(0) as usize;
    ingest_common::print_ingest_sync_result(cfg, "reddit", chunks, target);
    Ok(())
}
