use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use crate::crates::jobs::ingest_jobs::IngestSource;
use std::error::Error;

pub async fn run_youtube(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if ingest_common::maybe_handle_ingest_subcommand(cfg, "youtube").await? {
        return Ok(());
    }

    let url = cfg
        .positional
        .first()
        .cloned()
        .ok_or("youtube requires <URL> (video, playlist, or channel URL)")?;

    let source = IngestSource::Youtube { target: url };

    if !cfg.wait {
        return ingest_common::enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    use crate::crates::ingest;

    let IngestSource::Youtube { ref target } = source else {
        // NOTE: This branch is unreachable for current callers but guards against
        // future callers passing the wrong IngestSource variant.
        return Err(format!("youtube: expected Youtube source, got {:?}", source).into());
    };

    let chunks = ingest::youtube::ingest_youtube(cfg, target).await?;
    ingest_common::print_ingest_sync_result(cfg, "youtube", chunks, target);
    Ok(())
}
