use super::types::{PerformanceProfile, RenderMode, ScrapeFormat};
use clap::{ArgAction, Args, Parser, Subcommand};
use std::path::PathBuf;

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
    Scrape(UrlArg),
    Crawl(CrawlArgs),
    Map(UrlArg),
    Batch(BatchArgs),
    Extract(ExtractArgs),
    Search(TextArg),
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
    Reddit(RedditArgs),
    Youtube(YoutubeArgs),
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
    pub(super) url: Option<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct BatchArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    pub(super) urls: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct ExtractArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    pub(super) urls: Vec<String>,
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
#[command(args_conflicts_with_subcommands = true)]
pub(super) struct RedditArgs {
    #[command(subcommand)]
    pub(super) job: Option<JobSubcommand>,
    /// Subreddit name (e.g. "rust") or full thread URL
    #[arg(value_name = "TARGET")]
    pub(super) target: Option<String>,
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

#[derive(Debug, Subcommand)]
pub(super) enum JobSubcommand {
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
    #[command(hide = true)]
    Doctor,
}

#[derive(Debug, Args)]
pub(super) struct GlobalArgs {
    #[arg(global = true, long, default_value = "https://example.com")]
    pub(super) start_url: String,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) max_pages: u32,

    #[arg(global = true, long, default_value_t = 5)]
    pub(super) max_depth: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) include_subdomains: bool,

    #[arg(global = true, long = "exclude-path-prefix", value_delimiter = ',')]
    pub(super) exclude_path_prefix: Vec<String>,

    #[arg(global = true, long, default_value = ".cache/axon-rust/output")]
    pub(super) output_dir: PathBuf,

    #[arg(global = true, long)]
    pub(super) output: Option<PathBuf>,

    #[arg(global = true, long, value_enum, default_value_t = RenderMode::AutoSwitch)]
    pub(super) render_mode: RenderMode,

    #[arg(global = true, long, env = "AXON_CHROME_REMOTE_URL")]
    pub(super) chrome_remote_url: Option<String>,

    #[arg(global = true, long, env = "AXON_CHROME_PROXY")]
    pub(super) chrome_proxy: Option<String>,

    #[arg(global = true, long, env = "AXON_CHROME_USER_AGENT")]
    pub(super) chrome_user_agent: Option<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) chrome_headless: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) chrome_anti_bot: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) chrome_intercept: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) chrome_stealth: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) chrome_bootstrap: bool,

    #[arg(global = true, long, default_value_t = 3000)]
    pub(super) chrome_bootstrap_timeout_ms: u64,

    #[arg(global = true, long, default_value_t = 2)]
    pub(super) chrome_bootstrap_retries: usize,

    #[arg(global = true, long, env = "AXON_WEBDRIVER_URL")]
    pub(super) webdriver_url: Option<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) respect_robots: bool,

    #[arg(global = true, long, default_value_t = 200)]
    pub(super) min_markdown_chars: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) drop_thin_markdown: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) discover_sitemaps: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) cache: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) cache_skip_browser: bool,

    #[arg(global = true, long, value_enum, default_value_t = ScrapeFormat::Markdown)]
    pub(super) format: ScrapeFormat,

    #[arg(global = true, long, default_value_t = 10)]
    pub(super) limit: usize,

    #[arg(global = true, long)]
    pub(super) query: Option<String>,

    #[arg(global = true, long)]
    pub(super) urls: Option<String>,

    #[arg(global = true, long = "url-glob", value_delimiter = ',')]
    pub(super) url_glob: Vec<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) embed: bool,

    #[arg(global = true, long, env = "AXON_COLLECTION", default_value = "cortex")]
    pub(super) collection: String,

    #[arg(global = true, long, default_value_t = 16)]
    pub(super) batch_concurrency: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) wait: bool,

    #[arg(global = true, long, action = ArgAction::SetTrue)]
    pub(super) yes: bool,

    #[arg(global = true, long, action = ArgAction::SetTrue)]
    pub(super) json: bool,

    #[arg(global = true, long, value_enum, default_value_t = PerformanceProfile::HighStable)]
    pub(super) performance_profile: PerformanceProfile,

    #[arg(global = true, long)]
    pub(super) concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    pub(super) crawl_concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    pub(super) sitemap_concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    pub(super) backfill_concurrency_limit: Option<usize>,

    #[arg(global = true, long, default_value_t = 512)]
    pub(super) max_sitemaps: usize,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) delay_ms: u64,

    #[arg(global = true, long)]
    pub(super) request_timeout_ms: Option<u64>,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) fetch_retries: usize,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) retry_backoff_ms: u64,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) shared_queue: bool,

    #[arg(global = true, long)]
    pub(super) pg_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) redis_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) amqp_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) crawl_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) batch_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) extract_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) embed_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) tei_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) qdrant_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) openai_base_url: Option<String>,

    #[arg(global = true, long)]
    pub(super) openai_api_key: Option<String>,

    #[arg(global = true, long)]
    pub(super) openai_model: Option<String>,

    #[arg(
        global = true,
        long,
        env = "AXON_JOB_STALE_TIMEOUT_SECS",
        default_value_t = 300
    )]
    pub(super) watchdog_stale_timeout_secs: i64,

    #[arg(
        global = true,
        long,
        env = "AXON_JOB_STALE_CONFIRM_SECS",
        default_value_t = 60
    )]
    pub(super) watchdog_confirm_secs: i64,

    #[arg(global = true, long)]
    pub(super) cron_every_seconds: Option<u64>,

    #[arg(global = true, long)]
    pub(super) cron_max_runs: Option<usize>,
}
