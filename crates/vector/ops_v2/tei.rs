use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::vector::ops_legacy::{EmbedProgress, EmbedSummary};
use std::error::Error;

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::embed_path_native(cfg, input).await
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::embed_path_native_with_progress(
        cfg,
        input,
        progress_tx,
    )
    .await
}
