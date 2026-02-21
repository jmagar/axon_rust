use crate::crates::core::config::Config;
use std::error::Error;

pub mod commands;
pub mod input;
pub mod qdrant;
pub mod ranking;
pub mod stats;
pub mod tei;

pub use tei::{embed_text_with_metadata, EmbedProgress, EmbedSummary};

pub fn chunk_text(text: &str) -> Vec<String> {
    input::chunk_text(text)
}

pub fn url_lookup_candidates(target: &str) -> Vec<String> {
    input::url_lookup_candidates(target)
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    tei::embed_path_native(cfg, input).await
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    tei::embed_path_native_with_progress(cfg, input, progress_tx).await
}

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    commands::run_query_native(cfg).await
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    qdrant::run_retrieve_native(cfg).await
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    qdrant::run_sources_native(cfg).await
}

pub async fn run_dedupe_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    qdrant::run_dedupe_native(cfg).await
}

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    qdrant::run_domains_native(cfg).await
}

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    stats::run_stats_native(cfg).await
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    commands::run_ask_native(cfg).await
}

pub async fn run_evaluate_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    commands::run_evaluate_native(cfg).await
}

pub async fn run_suggest_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    commands::run_suggest_native(cfg).await
}
