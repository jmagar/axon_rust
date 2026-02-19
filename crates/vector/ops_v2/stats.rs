use crate::axon_cli::crates::core::config::Config;
use std::error::Error;

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_stats_native(cfg).await
}
