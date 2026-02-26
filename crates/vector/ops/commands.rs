pub(crate) mod ask;
mod evaluate;
mod query;
pub(crate) mod streaming;
mod suggest;

pub use ask::run_ask_native;
pub use evaluate::run_evaluate_native;
pub use query::query_results;
pub use query::run_query_native;
pub use suggest::run_suggest_native;

use crate::crates::core::config::Config;

/// Resolve query text from `--query` flag or positional args, trimming whitespace.
/// Returns `None` if both are empty/whitespace-only.
fn resolve_query_text(cfg: &Config) -> Option<String> {
    cfg.query
        .clone()
        .filter(|q| !q.trim().is_empty())
        .or_else(|| {
            if cfg.positional.is_empty() {
                None
            } else {
                Some(cfg.positional.join(" "))
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}
