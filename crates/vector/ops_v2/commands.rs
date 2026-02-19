use crate::axon_cli::crates::core::config::Config;
use std::error::Error;

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_query_native(cfg).await
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_ask_native(cfg).await
}
