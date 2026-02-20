use clap::ValueEnum;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum CommandKind {
    Scrape,
    Crawl,
    Map,
    Batch,
    Extract,
    Search,
    Embed,
    Debug,
    Doctor,
    Query,
    Retrieve,
    Ask,
    Evaluate,
    Suggest,
    Sources,
    Domains,
    Stats,
    Status,
    Dedupe,
    Github,
    Reddit,
    Youtube,
}

impl CommandKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scrape => "scrape",
            Self::Crawl => "crawl",
            Self::Map => "map",
            Self::Batch => "batch",
            Self::Extract => "extract",
            Self::Search => "search",
            Self::Embed => "embed",
            Self::Debug => "debug",
            Self::Doctor => "doctor",
            Self::Query => "query",
            Self::Retrieve => "retrieve",
            Self::Ask => "ask",
            Self::Evaluate => "evaluate",
            Self::Suggest => "suggest",
            Self::Sources => "sources",
            Self::Domains => "domains",
            Self::Stats => "stats",
            Self::Status => "status",
            Self::Dedupe => "dedupe",
            Self::Github => "github",
            Self::Reddit => "reddit",
            Self::Youtube => "youtube",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RenderMode {
    Http,
    Chrome,
    #[value(name = "auto-switch")]
    AutoSwitch,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScrapeFormat {
    Markdown,
    Html,
    #[value(name = "rawHtml")]
    RawHtml,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PerformanceProfile {
    #[value(name = "high-stable")]
    HighStable,
    Extreme,
    Balanced,
    Max,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub command: CommandKind,
    pub start_url: String,
    pub positional: Vec<String>,
    pub urls_csv: Option<String>,
    pub url_glob: Vec<String>,
    pub query: Option<String>,
    pub search_limit: usize,
    pub max_pages: u32,
    pub max_depth: usize,
    pub include_subdomains: bool,
    pub exclude_path_prefix: Vec<String>,
    pub output_dir: PathBuf,
    pub output_path: Option<PathBuf>,
    pub render_mode: RenderMode,
    pub chrome_remote_url: Option<String>,
    pub chrome_proxy: Option<String>,
    pub chrome_user_agent: Option<String>,
    pub chrome_headless: bool,
    pub chrome_anti_bot: bool,
    pub chrome_intercept: bool,
    pub chrome_stealth: bool,
    pub chrome_bootstrap: bool,
    pub chrome_bootstrap_timeout_ms: u64,
    pub chrome_bootstrap_retries: usize,
    pub webdriver_url: Option<String>,
    pub respect_robots: bool,
    pub min_markdown_chars: usize,
    pub drop_thin_markdown: bool,
    pub discover_sitemaps: bool,
    pub cache: bool,
    pub cache_skip_browser: bool,
    pub format: ScrapeFormat,
    pub collection: String,
    pub embed: bool,
    pub batch_concurrency: usize,
    pub wait: bool,
    pub yes: bool,
    pub performance_profile: PerformanceProfile,
    pub crawl_concurrency_limit: Option<usize>,
    pub sitemap_concurrency_limit: Option<usize>,
    pub backfill_concurrency_limit: Option<usize>,
    pub max_sitemaps: usize,
    pub delay_ms: u64,
    pub request_timeout_ms: Option<u64>,
    pub fetch_retries: usize,
    pub retry_backoff_ms: u64,
    pub shared_queue: bool,
    pub pg_url: String,
    pub redis_url: String,
    pub amqp_url: String,
    pub crawl_queue: String,
    pub batch_queue: String,
    pub extract_queue: String,
    pub embed_queue: String,
    pub ingest_queue: String,
    pub github_token: Option<String>,
    pub github_include_source: bool,
    pub reddit_client_id: Option<String>,
    pub reddit_client_secret: Option<String>,
    pub tei_url: String,
    pub qdrant_url: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub ask_diagnostics: bool,
    pub ask_max_context_chars: usize,
    pub ask_candidate_limit: usize,
    pub ask_chunk_limit: usize,
    pub ask_full_docs: usize,
    pub ask_backfill_chunks: usize,
    pub ask_doc_fetch_concurrency: usize,
    pub ask_doc_chunk_limit: usize,
    pub ask_min_relevance_score: f64,
    pub cron_every_seconds: Option<u64>,
    pub cron_max_runs: Option<usize>,
    pub watchdog_stale_timeout_secs: i64,
    pub watchdog_confirm_secs: i64,
    pub json_output: bool,
}
