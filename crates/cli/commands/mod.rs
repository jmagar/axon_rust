pub mod batch;
pub mod common;
pub mod crawl;
pub mod debug;
pub mod doctor;
pub mod embed;
pub mod extract;
pub mod github;
pub mod map;
pub mod probe;
pub mod reddit;
pub mod scrape;
pub mod search;
pub mod status;
pub mod youtube;

pub use crate::axon_cli::crates::vector::ops::{
    run_ask_native, run_dedupe_native, run_domains_native, run_evaluate_native, run_query_native,
    run_retrieve_native, run_sources_native, run_stats_native, run_suggest_native,
};
pub use batch::run_batch;
pub use common::start_url_from_cfg;
pub use crawl::run_crawl;
pub use debug::run_debug;
pub use doctor::run_doctor;
pub use embed::run_embed;
pub use extract::run_extract;
pub use github::run_github;
pub use map::run_map;
pub use reddit::run_reddit;
pub use scrape::run_scrape;
pub use search::run_search;
pub use status::run_status;
pub use youtube::run_youtube;
