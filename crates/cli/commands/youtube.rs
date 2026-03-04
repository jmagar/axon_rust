use crate::crates::cli::commands::ingest_common;
use crate::crates::core::config::Config;
use crate::crates::jobs::ingest::IngestSource;
use crate::crates::services::ingest as ingest_service;
use std::error::Error;

pub async fn run_youtube(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if ingest_common::maybe_handle_ingest_subcommand(cfg, "youtube").await? {
        return Ok(());
    }

    let url = cfg
        .positional
        .first()
        .cloned()
        .ok_or("youtube requires <URL> (video URL or bare video ID)")?;

    let source = IngestSource::Youtube { target: url };

    if !cfg.wait {
        return ingest_common::enqueue_ingest_job(cfg, source).await;
    }

    run_ingest_sync(cfg, source).await
}

async fn run_ingest_sync(cfg: &Config, source: IngestSource) -> Result<(), Box<dyn Error>> {
    let IngestSource::Youtube { ref target } = source else {
        // NOTE: This branch is unreachable for current callers but guards against
        // future callers passing the wrong IngestSource variant.
        return Err(format!("youtube: expected Youtube source, got {:?}", source).into());
    };

    let result = ingest_service::ingest_youtube(cfg, target, None).await?;
    let chunks = result.payload["chunks"].as_u64().unwrap_or(0) as usize;
    ingest_common::print_ingest_sync_result(cfg, "youtube", chunks, target);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::core::config::CommandKind;
    use crate::crates::jobs::common::test_config;

    #[tokio::test]
    async fn run_youtube_requires_video_url_or_id() {
        let mut cfg = test_config("");
        cfg.command = CommandKind::Youtube;
        cfg.positional = vec![];
        let err = run_youtube(&cfg)
            .await
            .expect_err("expected missing URL error");
        assert!(
            err.to_string()
                .contains("youtube requires <URL> (video URL or bare video ID)"),
            "unexpected error: {err}"
        );
    }
}
