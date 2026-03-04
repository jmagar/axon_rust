mod cli;
mod help;
pub(crate) mod parse;
mod types;

pub use parse::parse_args;
pub use types::{
    CommandKind, Config, EvaluateResponsesMode, PerformanceProfile, RedditSort, RedditTime,
    RenderMode, ScrapeFormat,
};
