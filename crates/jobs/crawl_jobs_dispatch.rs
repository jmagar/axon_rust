use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::jobs::{crawl_jobs_legacy, crawl_jobs_v2};
use std::error::Error;
use uuid::Uuid;

pub use crawl_jobs_legacy::CrawlJob;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrawlJobsImpl {
    Legacy,
    V2,
}

fn selected_impl_from_env(raw: Option<&str>) -> CrawlJobsImpl {
    match raw.map(str::trim).unwrap_or("legacy") {
        "v2" => CrawlJobsImpl::V2,
        "legacy" => CrawlJobsImpl::Legacy,
        _ => CrawlJobsImpl::Legacy,
    }
}

fn selected_impl() -> CrawlJobsImpl {
    selected_impl_from_env(std::env::var("AXON_CRAWL_JOBS_IMPL").ok().as_deref())
}

pub async fn doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::doctor(cfg).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::doctor(cfg).await,
    }
}

pub async fn start_crawl_job(cfg: &Config, start_url: &str) -> Result<Uuid, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::start_crawl_job(cfg, start_url).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::start_crawl_job(cfg, start_url).await,
    }
}

pub async fn get_job(cfg: &Config, id: Uuid) -> Result<Option<CrawlJob>, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::get_job(cfg, id).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::get_job(cfg, id).await,
    }
}

pub async fn list_jobs(cfg: &Config, limit: i64) -> Result<Vec<CrawlJob>, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::list_jobs(cfg, limit).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::list_jobs(cfg, limit).await,
    }
}

pub async fn cancel_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::cancel_job(cfg, id).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::cancel_job(cfg, id).await,
    }
}

pub async fn cleanup_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::cleanup_jobs(cfg).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::cleanup_jobs(cfg).await,
    }
}

pub async fn clear_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::clear_jobs(cfg).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::clear_jobs(cfg).await,
    }
}

pub async fn recover_stale_crawl_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::recover_stale_crawl_jobs(cfg).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::recover_stale_crawl_jobs(cfg).await,
    }
}

pub async fn run_worker(cfg: &Config) -> Result<(), Box<dyn Error>> {
    match selected_impl() {
        CrawlJobsImpl::Legacy => crawl_jobs_legacy::run_worker(cfg).await,
        CrawlJobsImpl::V2 => crawl_jobs_v2::run_worker(cfg).await,
    }
}

#[cfg(test)]
mod tests {
    use super::{selected_impl_from_env, CrawlJobsImpl};

    #[test]
    fn dispatch_defaults_to_legacy() {
        assert_eq!(selected_impl_from_env(None), CrawlJobsImpl::Legacy);
        assert_eq!(selected_impl_from_env(Some("")), CrawlJobsImpl::Legacy);
        assert_eq!(
            selected_impl_from_env(Some("unknown")),
            CrawlJobsImpl::Legacy
        );
    }

    #[test]
    fn dispatch_selects_v2_with_env_value() {
        assert_eq!(selected_impl_from_env(Some("v2")), CrawlJobsImpl::V2);
    }
}
