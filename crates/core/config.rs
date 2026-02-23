mod cli;
mod help;
pub(crate) mod parse;
mod types;

pub use parse::parse_args;
pub use types::{
    CommandKind, Config, PerformanceProfile, RedditSort, RedditTime, RenderMode, ScrapeFormat,
};
