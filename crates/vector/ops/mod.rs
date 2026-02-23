pub mod commands;
pub mod input;
pub mod qdrant;
pub mod ranking;
pub mod source_display;
pub mod stats;
pub mod tei;

// Re-export public API — no passthrough wrappers needed.
pub use commands::{run_ask_native, run_evaluate_native, run_query_native, run_suggest_native};
pub use input::{chunk_text, url_lookup_candidates};
pub use qdrant::{run_dedupe_native, run_domains_native, run_retrieve_native, run_sources_native};
pub use stats::run_stats_native;
pub use tei::{
    embed_path_native, embed_path_native_with_progress, embed_text_with_metadata, EmbedProgress,
    EmbedSummary,
};
