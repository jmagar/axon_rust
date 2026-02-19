use crate::axon_cli::crates::core::config::Config;
use std::error::Error;

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_retrieve_native(cfg).await
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_sources_native(cfg).await
}

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::axon_cli::crates::vector::ops_legacy::run_domains_native(cfg).await
}
