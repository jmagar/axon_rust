use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use std::error::Error;

pub async fn run_sessions(cfg: &Config) -> Result<(), Box<dyn Error>> {
    // Note: We don't support async/job mode for sessions yet because it's local ingestion
    // and usually fast enough to run synchronously.

    let total_chunks = crate::crates::ingest::sessions::ingest_sessions(cfg).await?;

    ingest_common::print_ingest_sync_result(cfg, "sessions", total_chunks, "local history paths");
    Ok(())
}
