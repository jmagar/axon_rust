use super::types::{PerformanceProfile, RedditSort, RedditTime, RenderMode, ScrapeFormat};
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
    Scrape(ScrapeArgs),
    Crawl(CrawlArgs),
    Refresh(CrawlArgs),
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

#[derive(Debug, Args)]
pub(super) struct GlobalArgs {
    #[arg(global = true, long, default_value = "")]
    pub(super) start_url: String,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) max_pages: u32,

    #[arg(global = true, long, default_value_t = 5)]
    pub(super) max_depth: usize,

    /// Include links from subdomains. Disable with `--include-subdomains false`.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) include_subdomains: bool,

    #[arg(global = true, long = "exclude-path-prefix", value_delimiter = ',')]
    pub(super) exclude_path_prefix: Vec<String>,

    #[arg(
        global = true,
        long,
        default_value = ".cache/axon-rust/output",
        env = "AXON_OUTPUT_DIR"
    )]
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

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) respect_robots: bool,

    #[arg(global = true, long, default_value_t = 200)]
    pub(super) min_markdown_chars: usize,

    /// Drop thin markdown pages. Disable with `--drop-thin-markdown false`.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) drop_thin_markdown: bool,

    /// Discover and backfill sitemap URLs. Disable with `--discover-sitemaps false`.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) discover_sitemaps: bool,

    /// Enable crawl cache reuse. Disable with `--cache false`.
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

    /// Trigger follow-up embed flows when supported. Disable with `--embed false`.
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

    /// Status mode: show only watchdog-reclaimed jobs.
    #[arg(global = true, long, action = ArgAction::SetTrue)]
    pub(super) reclaimed: bool,

    #[arg(global = true, long, value_enum, default_value_t = PerformanceProfile::HighStable)]
    pub(super) performance_profile: PerformanceProfile,

    #[arg(global = true, long)]
    pub(super) concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    pub(super) crawl_concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    pub(super) backfill_concurrency_limit: Option<usize>,

    #[arg(global = true, long, action = ArgAction::SetTrue)]
    pub(super) sitemap_only: bool,

    #[arg(global = true, long, default_value_t = 0)]
    pub(super) delay_ms: u64,

    #[arg(global = true, long)]
    pub(super) request_timeout_ms: Option<u64>,

    #[arg(global = true, long)]
    pub(super) fetch_retries: Option<usize>,

    #[arg(global = true, long)]
    pub(super) retry_backoff_ms: Option<u64>,

    /// Share one queue across supported jobs. Disable with `--shared-queue false`.
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
    pub(super) refresh_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) extract_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) embed_queue: Option<String>,

    #[arg(global = true, long)]
    pub(super) ingest_queue: Option<String>,

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

    /// Deduplicate trailing-slash URL variants during crawl. Disable with `--normalize false`.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) normalize: bool,

    /// Seconds to wait for Chrome network idle before page capture. Default: 15.
    #[arg(global = true, long, default_value_t = 15)]
    pub(super) chrome_network_idle_timeout: u64,

    /// Thin-page ratio to trigger auto-switch to Chrome (0.0–1.0). Default: 0.60.
    #[arg(global = true, long, default_value_t = 0.60)]
    pub(super) auto_switch_thin_ratio: f64,

    /// Minimum pages before auto-switch eligibility check. Default: 10.
    #[arg(global = true, long, default_value_t = 10)]
    pub(super) auto_switch_min_pages: usize,

    /// Only crawl URLs matching these regex patterns (repeatable). Default: none.
    #[arg(global = true, long)]
    pub(super) url_whitelist: Vec<String>,

    /// Block asset downloads (images/CSS/fonts) during crawl.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) block_assets: bool,

    /// Maximum response size per page in bytes (0 = unlimited). Default: 0.
    #[arg(global = true, long, default_value_t = 0)]
    pub(super) max_page_bytes: u64,

    /// Only follow same-origin redirects (strict redirect policy).
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) redirect_policy_strict: bool,

    /// CSS selector to wait for before Chrome captures the page. Default: none.
    #[arg(global = true, long)]
    pub(super) chrome_wait_for_selector: Option<String>,

    /// Capture full-page PNG screenshots during Chrome crawl.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) chrome_screenshot: bool,

    /// Research crawl depth limit for the research command. Default: none.
    #[arg(global = true, long)]
    pub(super) research_depth: Option<usize>,

    /// Time range filter for search (day|week|month|year). Default: none.
    #[arg(global = true, long)]
    pub(super) search_time_range: Option<String>,

    /// Bypass Content Security Policy in Chrome. Helps pages that block inline JS via CSP. Default: false.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) bypass_csp: bool,

    /// Accept invalid or self-signed TLS certificates. Useful for internal/staging sites. Default: false.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    pub(super) accept_invalid_certs: bool,

    /// Capture full scrollable page (true) or viewport only (false). Default: true.
    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    pub(super) screenshot_full_page: bool,

    /// Viewport dimensions as WIDTHxHEIGHT (e.g. 1920x1080). Default: 1920x1080.
    #[arg(global = true, long, default_value = "1920x1080")]
    pub(super) viewport: String,
}
