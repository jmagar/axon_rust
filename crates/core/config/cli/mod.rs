mod global_args;

use super::types::{RedditSort, RedditTime};
use clap::{ArgAction, Args, Parser, Subcommand};

pub(super) use global_args::GlobalArgs;

#[derive(Debug, Parser)]
#[command(name = "axon", about = "Axon CLI (Rust + Spider.rs)")]
pub(super) struct Cli {
    #[command(subcommand)]
    pub(super) command: CliCommand,

    #[command(flatten)]
    pub(super) global: GlobalArgs,
}

#[derive(Debug, Subcommand)]
pub(super) enum CliCommand {
    Scrape(ScrapeArgs),
    Crawl(CrawlArgs),
    Refresh(RefreshArgs),
    Map(UrlArg),
    Extract(ExtractArgs),
    Search(TextArg),
    Research(TextArg),
    Embed(EmbedArgs),
    Debug(TextArg),
    Doctor,
    Query(TextArg),
    Retrieve(UrlArg),
    Ask(AskArgs),
    Evaluate(EvaluateArgs),
    Suggest(TextArg),
    Sources,
    Domains,
    Stats,
    Status,
    Dedupe,
    Github(GithubArgs),
    Ingest(IngestArgs),
    Reddit(RedditArgs),
    Youtube(YoutubeArgs),
    Sessions(SessionsArgs),
    Screenshot(ScrapeArgs),
    Mcp,
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub(super) struct ServeArgs {
    /// Port to bind the web UI server on
    #[arg(long, default_value_t = 3939)]
    pub(super) port: u16,
}

#[derive(Debug, Args)]
pub(super) struct ScrapeArgs {
    #[arg(value_name = "URL")]
    pub(super) positional_urls: Vec<String>,
}

#[derive(Debug, Args)]
pub(super) struct UrlArg {
    #[arg(value_name = "URL")]
    pub(super) value: Option<String>,
}

#[derive(Debug, Args)]
pub(super) struct TextArg {
    #[arg(value_name = "TEXT")]
    pub(super) value: Vec<String>,
}

#[derive(Debug, Args)]
pub(super) struct AskArgs {
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) diagnostics: bool,
    #[arg(value_name = "TEXT")]
    pub(super) value: Vec<String>,
}

#[derive(Debug, Args)]
pub(super) struct EvaluateArgs {
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) diagnostics: bool,
    #[arg(value_name = "TEXT")]
    pub(super) value: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct CrawlArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    pub(super) positional_urls: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct RefreshArgs {
    #[command(subcommand)]
    pub(super) action: Option<RefreshSubcommand>,
    #[arg(value_name = "URL")]
    pub(super) positional_urls: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub(super) enum RefreshSubcommand {
    Status {
        job_id: String,
    },
    Cancel {
        job_id: String,
    },
    Errors {
        job_id: String,
    },
    List,
    Cleanup,
    Clear,
    Worker,
    Recover,
    Schedule {
        #[command(subcommand)]
        action: RefreshScheduleSubcommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum RefreshScheduleSubcommand {
    Add {
        name: String,
        seed_url: Option<String>,
        #[arg(long = "every-seconds")]
        every_seconds: Option<i64>,
        #[arg(long, value_parser = ["high", "medium", "low"])]
        tier: Option<String>,
        #[arg(long)]
        urls: Option<String>,
    },
    List,
    Enable {
        name: String,
    },
    Disable {
        name: String,
    },
    Delete {
        name: String,
    },
    #[command(name = "run-due")]
    RunDue {
        #[arg(long, default_value_t = 25)]
        batch: usize,
    },
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct ExtractArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    pub(super) positional_urls: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct EmbedArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    #[arg(value_name = "INPUT")]
    pub(super) input: Option<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct GithubArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    /// GitHub repository in "owner/repo" format
    #[arg(value_name = "REPO")]
    pub(super) repo: Option<String>,
    /// Also index source code files (in addition to markdown, issues, and PRs)
    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub(super) include_source: bool,
}

#[derive(Debug, Args)]
pub(super) struct IngestArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct RedditArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    /// Subreddit name (e.g. "rust") or full thread URL
    #[arg(value_name = "TARGET")]
    pub(super) target: Option<String>,
    /// Subreddit sorting (hot, top, new, rising)
    #[arg(long, value_enum, default_value_t = RedditSort::Hot)]
    pub(super) sort: RedditSort,
    /// Time range for top sort (hour, day, week, month, year, all)
    #[arg(long, value_enum, default_value_t = RedditTime::Day)]
    pub(super) time: RedditTime,
    /// Maximum posts to fetch (0 for unlimited)
    #[arg(long, default_value_t = 25)]
    pub(super) max_posts: usize,
    /// Minimum score threshold for posts and comments
    #[arg(long, default_value_t = 0)]
    pub(super) min_score: i32,
    /// Comment traversal depth
    #[arg(long, default_value_t = 2)]
    pub(super) depth: usize,
    /// Scrape content of linked URLs in link posts
    #[arg(long, action = ArgAction::Set, default_value_t = false)]
    pub(super) scrape_links: bool,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct YoutubeArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    /// Video URL, playlist URL, or channel URL
    #[arg(value_name = "URL")]
    pub(super) url: Option<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct SessionsArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    /// Index Claude Code sessions
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) claude: bool,
    /// Index Codex sessions
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) codex: bool,
    /// Index Gemini sessions
    #[arg(long, action = ArgAction::SetTrue)]
    pub(super) gemini: bool,
    /// Filter sessions by project name (substring match)
    #[arg(long, value_name = "NAME")]
    pub(super) project: Option<String>,
}

#[derive(Debug, Subcommand)]
pub(super) enum JobSubcommand {
    Status { job_id: String },
    Cancel { job_id: String },
    Errors { job_id: String },
    List,
    Cleanup,
    Clear,
    Worker,
    Recover,
}
