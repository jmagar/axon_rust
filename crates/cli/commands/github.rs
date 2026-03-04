use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use crate::crates::jobs::ingest::IngestSource;
use crate::crates::services::ingest as ingest_service;
use std::error::Error;

pub async fn run_github(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if ingest_common::maybe_handle_ingest_subcommand(cfg, "github").await? {
        return Ok(());
    }

    let repo = cfg
        .positional
        .first()
        .cloned()
        .ok_or("github requires <REPO> (e.g. rust-lang/rust)")?;

    let source = IngestSource::Github {
        repo,
        include_source: cfg.github_include_source,
    };

    if !cfg.wait {
        return ingest_common::enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    let IngestSource::Github {
        ref repo,
        include_source: _,
    } = source
    else {
        // NOTE: This branch is unreachable for current callers but guards against
        // future callers passing the wrong IngestSource variant.
        return Err(format!("github: expected Github source, got {:?}", source).into());
    };

    // Split "owner/repo" slug into owner and repo parts for the service layer.
    // The service recombines them as "{owner}/{repo}" internally.
    let (owner, repo_name) = repo
        .split_once('/')
        .ok_or_else(|| format!("github: repo must be in 'owner/repo' format, got '{repo}'"))?;

    let result = ingest_service::ingest_github(cfg, owner, repo_name, None).await?;
    let chunks = result.payload["chunks"].as_u64().unwrap_or(0) as usize;
    ingest_common::print_ingest_sync_result(cfg, "github", chunks, repo);
    Ok(())
}
