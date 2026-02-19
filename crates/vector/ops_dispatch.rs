use crate::axon_cli::crates::core::config::Config;
use std::error::Error;

pub use crate::axon_cli::crates::vector::ops_v2::{EmbedProgress, EmbedSummary};

pub fn chunk_text(text: &str) -> Vec<String> {
    crate::axon_cli::crates::vector::ops_v2::chunk_text(text)
}

pub fn url_lookup_candidates(target: &str) -> Vec<String> {
    crate::axon_cli::crates::vector::ops_v2::url_lookup_candidates(target)
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::embed_path_native(cfg, input).await
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::embed_path_native_with_progress(
        cfg,
        input,
        progress_tx,
    )
    .await
}

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_query_native(cfg).await
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_retrieve_native(cfg).await
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_sources_native(cfg).await
}

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_domains_native(cfg).await
}

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_stats_native(cfg).await
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_v2::run_ask_native(cfg).await
}
