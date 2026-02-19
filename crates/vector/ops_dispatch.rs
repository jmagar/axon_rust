use crate::axon_cli::crates::core::config::Config;
use std::env;
use std::error::Error;

pub use crate::axon_cli::crates::vector::ops_legacy::{EmbedProgress, EmbedSummary};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VectorImpl {
    Legacy,
    V2,
}

pub(crate) fn selected_impl() -> VectorImpl {
    match env::var("AXON_VECTOR_IMPL") {
        Ok(value) if value.eq_ignore_ascii_case("v2") => VectorImpl::V2,
        Ok(value) if value.eq_ignore_ascii_case("legacy") => VectorImpl::Legacy,
        Ok(_) => VectorImpl::Legacy,
        Err(_) => VectorImpl::Legacy,
    }
}

pub fn chunk_text(text: &str) -> Vec<String> {
    match selected_impl() {
        VectorImpl::Legacy => crate::axon_cli::crates::vector::ops_legacy::chunk_text(text),
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::chunk_text(text),
    }
}

pub fn url_lookup_candidates(target: &str) -> Vec<String> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::url_lookup_candidates(target)
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::url_lookup_candidates(target),
    }
}

pub async fn embed_path_native(cfg: &Config, input: &str) -> Result<EmbedSummary, Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::embed_path_native(cfg, input).await
        }
        VectorImpl::V2 => {
            crate::axon_cli::crates::vector::ops_v2::embed_path_native(cfg, input).await
        }
    }
}

pub async fn embed_path_native_with_progress(
    cfg: &Config,
    input: &str,
    progress_tx: Option<tokio::sync::mpsc::UnboundedSender<EmbedProgress>>,
) -> Result<EmbedSummary, Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::embed_path_native_with_progress(
                cfg,
                input,
                progress_tx,
            )
            .await
        }
        VectorImpl::V2 => {
            crate::axon_cli::crates::vector::ops_v2::embed_path_native_with_progress(
                cfg,
                input,
                progress_tx,
            )
            .await
        }
    }
}

pub async fn run_query_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_query_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_query_native(cfg).await,
    }
}

pub async fn run_retrieve_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_retrieve_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_retrieve_native(cfg).await,
    }
}

pub async fn run_sources_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_sources_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_sources_native(cfg).await,
    }
}

pub async fn run_domains_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_domains_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_domains_native(cfg).await,
    }
}

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_stats_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_stats_native(cfg).await,
    }
}

pub async fn run_ask_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        VectorImpl::Legacy => {
            crate::axon_cli::crates::vector::ops_legacy::run_ask_native(cfg).await
        }
        VectorImpl::V2 => crate::axon_cli::crates::vector::ops_v2::run_ask_native(cfg).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{selected_impl, VectorImpl};
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn dispatch_defaults_to_legacy_when_env_is_unset() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        std::env::remove_var("AXON_VECTOR_IMPL");
        assert_eq!(selected_impl(), VectorImpl::Legacy);
    }

    #[test]
    fn dispatch_selects_v2_when_env_requests_it() {
        let _guard = env_lock().lock().expect("env lock poisoned");
        std::env::set_var("AXON_VECTOR_IMPL", "v2");
        assert_eq!(selected_impl(), VectorImpl::V2);
        std::env::remove_var("AXON_VECTOR_IMPL");
    }
}
