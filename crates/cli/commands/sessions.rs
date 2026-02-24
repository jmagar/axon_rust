use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use crate::crates::jobs::ingest::IngestSource;
use std::error::Error;

pub async fn run_sessions(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if ingest_common::maybe_handle_ingest_subcommand(cfg, "sessions").await? {
        return Ok(());
    }

    let source = IngestSource::Sessions {
        sessions_claude: cfg.sessions_claude,
        sessions_codex: cfg.sessions_codex,
        sessions_gemini: cfg.sessions_gemini,
        sessions_project: cfg.sessions_project.clone(),
    };

    if !cfg.wait {
        return ingest_common::enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    use crate::crates::ingest;

    let IngestSource::Sessions { .. } = source else {
        // NOTE: This branch is unreachable for current callers but guards against
        // future callers passing the wrong IngestSource variant.
        return Err(format!("sessions: expected Sessions source, got {:?}", source).into());
    };

    let chunks = ingest::sessions::ingest_sessions(cfg).await?;
    ingest_common::print_ingest_sync_result(cfg, "sessions", chunks, "local history paths");
    Ok(())
}
