use clap::ValueEnum;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum CommandKind {
    Scrape,
    Crawl,
    Map,
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
    Ingest,
    Reddit,
    Youtube,
    Sessions,
    Research,
}

impl CommandKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scrape => "scrape",
            Self::Crawl => "crawl",
            Self::Map => "map",
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
            Self::Ingest => "ingest",
            Self::Reddit => "reddit",
            Self::Youtube => "youtube",
            Self::Sessions => "sessions",
            Self::Research => "research",
        }
    }
}

impl fmt::Display for CommandKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
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

impl fmt::Display for RenderMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Http => "http",
            Self::Chrome => "chrome",
            Self::AutoSwitch => "auto-switch",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScrapeFormat {
    Markdown,
    Html,
    #[value(name = "rawHtml")]
    #[serde(rename = "rawHtml")]
    RawHtml,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RedditSort {
    Hot,
    Top,
    New,
    Rising,
}

impl fmt::Display for RedditSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Hot => "hot",
            Self::Top => "top",
            Self::New => "new",
            Self::Rising => "rising",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RedditTime {
    Hour,
    Day,
    Week,
    Month,
    Year,
    All,
}

impl fmt::Display for RedditTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Hour => "hour",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::All => "all",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PerformanceProfile {
    #[value(name = "high-stable")]
    HighStable,
    Extreme,
    Balanced,
    Max,
}

#[derive(Clone)]
pub struct Config {
    /// The subcommand being executed (scrape, crawl, ask, etc.).
    pub command: CommandKind,

    /// Primary URL argument; used by scrape, crawl, map, and similar single-URL commands.
    pub start_url: String,

    /// Positional arguments after the subcommand (URLs, query text, job sub-subcommand tokens).
    pub positional: Vec<String>,

    /// Comma-separated URL list provided via `--urls` (alternative to positional arguments).
    pub urls_csv: Option<String>,

    /// Glob patterns to expand into seed URLs (e.g. `https://docs.rs/foo/**`).
    pub url_glob: Vec<String>,

    /// Query text for `query`, `ask`, and `evaluate` commands; also settable via `--query`.
    pub query: Option<String>,

    /// Maximum number of results returned by `query`/`search` commands. Flag: `--limit`.
    pub search_limit: usize,

    /// Maximum pages to crawl (0 = uncapped). Flag: `--max-pages`.
    pub max_pages: u32,

    /// Maximum crawl depth from the start URL. Flag: `--max-depth`.
    pub max_depth: usize,

    /// Whether to follow links from subdomains of the start URL. Flag: `--include-subdomains`.
    pub include_subdomains: bool,

    /// URL path prefixes to skip during crawl (e.g. `/blog/`, `/legacy/`). Flag: `--exclude-path-prefix`.
    pub exclude_path_prefix: Vec<String>,

    /// Directory for saved markdown/HTML output files. Flag: `--output-dir`.
    pub output_dir: PathBuf,

    /// Explicit single-file output path (overrides `output_dir` for single-URL commands). Flag: `--output`.
    pub output_path: Option<PathBuf>,

    /// Browser rendering strategy: `http`, `chrome`, or `auto-switch`. Flag: `--render-mode`.
    pub render_mode: RenderMode,

    /// URL of the Chrome DevTools Protocol (CDP) management endpoint. Env: `AXON_CHROME_REMOTE_URL`.
    pub chrome_remote_url: Option<String>,

    /// HTTP proxy URL for Chrome requests. Env: `AXON_CHROME_PROXY`.
    pub chrome_proxy: Option<String>,

    /// Custom `User-Agent` header sent by Chrome. Env: `AXON_CHROME_USER_AGENT`.
    pub chrome_user_agent: Option<String>,

    /// Run Chrome in headless mode (no visible window). Flag: `--chrome-headless`.
    pub chrome_headless: bool,

    /// Enable Chrome's anti-bot evasion mode. Flag: `--chrome-anti-bot`.
    pub chrome_anti_bot: bool,

    /// Enable Chrome network interception (for blocking ads/trackers). Flag: `--chrome-intercept`.
    pub chrome_intercept: bool,

    /// Enable Chrome stealth mode (patches `navigator.webdriver`). Flag: `--chrome-stealth`.
    pub chrome_stealth: bool,

    /// Bootstrap Chrome connection before starting the crawl. Flag: `--chrome-bootstrap`.
    pub chrome_bootstrap: bool,

    /// Timeout in milliseconds to wait for Chrome bootstrap. Flag: `--chrome-bootstrap-timeout-ms`.
    pub chrome_bootstrap_timeout_ms: u64,

    /// Number of retries for Chrome bootstrap failures. Flag: `--chrome-bootstrap-retries`.
    pub chrome_bootstrap_retries: usize,

    /// URL of the WebDriver endpoint (alternative to direct CDP). Env: `AXON_WEBDRIVER_URL`.
    pub webdriver_url: Option<String>,

    /// Whether to honour `robots.txt` directives. Defaults `false`. Flag: `--respect-robots`.
    pub respect_robots: bool,

    /// Pages with fewer than this many markdown characters are treated as "thin". Flag: `--min-markdown-chars`.
    pub min_markdown_chars: usize,

    /// Drop thin pages — do not save or embed them. Flag: `--drop-thin-markdown`.
    pub drop_thin_markdown: bool,

    /// Discover and backfill URLs from `sitemap.xml` after the main crawl. Flag: `--discover-sitemaps`.
    pub discover_sitemaps: bool,

    /// Enable Spider's built-in crawl-result caching. Flag: `--cache`.
    pub cache: bool,

    /// Skip the cache for browser (Chrome) fetches only. Flag: `--cache-skip-browser`.
    pub cache_skip_browser: bool,

    /// Output format for scraped pages: `markdown`, `html`, `rawHtml`, or `json`. Flag: `--format`.
    pub format: ScrapeFormat,

    /// Qdrant collection name to read from and write to. Env: `AXON_COLLECTION`. Flag: `--collection`.
    pub collection: String,

    /// Automatically embed scraped content into Qdrant after fetching. Flag: `--embed`.
    pub embed: bool,

    /// Number of concurrent connections for batch operations (clamped 1–512). Flag: `--batch-concurrency`.
    pub batch_concurrency: usize,

    /// Block until async jobs complete instead of fire-and-forgetting. Flag: `--wait`.
    pub wait: bool,

    /// Skip confirmation prompts (non-interactive mode). Flag: `--yes`.
    pub yes: bool,

    /// Concurrency/timeout preset. Profiles scale linearly with CPU count. Flag: `--performance-profile`.
    pub performance_profile: PerformanceProfile,

    /// Override concurrency limit for the primary crawl spider. Flag: `--crawl-concurrency-limit`.
    pub crawl_concurrency_limit: Option<usize>,

    /// Override concurrency limit for sitemap backfill fetches. Flag: `--backfill-concurrency-limit`.
    pub backfill_concurrency_limit: Option<usize>,

    /// Only run sitemap discovery, not a full crawl. Flag: `--sitemap-only`.
    pub sitemap_only: bool,

    /// Millisecond delay between spider requests (polite crawling). Flag: `--delay-ms`.
    pub delay_ms: u64,

    /// Per-request timeout in milliseconds; `None` uses the profile default. Flag: `--request-timeout-ms`.
    pub request_timeout_ms: Option<u64>,

    /// Number of retries on transient fetch failures. Flag: `--fetch-retries`.
    pub fetch_retries: usize,

    /// Backoff in milliseconds between retries. Flag: `--retry-backoff-ms`.
    pub retry_backoff_ms: u64,

    /// Route all job types through a single shared AMQP queue. Flag: `--shared-queue`.
    pub shared_queue: bool,

    /// PostgreSQL connection URL. Env: `AXON_PG_URL`. Flag: `--pg-url`. **Secret.**
    pub pg_url: String,

    /// Redis connection URL. Env: `AXON_REDIS_URL`. Flag: `--redis-url`. **Secret.**
    pub redis_url: String,

    /// RabbitMQ AMQP connection URL. Env: `AXON_AMQP_URL`. Flag: `--amqp-url`. **Secret.**
    pub amqp_url: String,

    /// AMQP queue name for crawl jobs. Env: `AXON_CRAWL_QUEUE`. Flag: `--crawl-queue`.
    pub crawl_queue: String,

    /// AMQP queue name for extract jobs. Env: `AXON_EXTRACT_QUEUE`. Flag: `--extract-queue`.
    pub extract_queue: String,

    /// AMQP queue name for embed jobs. Env: `AXON_EMBED_QUEUE`. Flag: `--embed-queue`.
    pub embed_queue: String,

    /// AMQP queue name for ingest jobs. Env: `AXON_INGEST_QUEUE`. Flag: `--ingest-queue`.
    pub ingest_queue: String,

    /// Index Claude Code session files when running the `sessions` command. Flag: `--claude`.
    pub sessions_claude: bool,

    /// Index Codex session files when running the `sessions` command. Flag: `--codex`.
    pub sessions_codex: bool,

    /// Index Gemini session files when running the `sessions` command. Flag: `--gemini`.
    pub sessions_gemini: bool,

    /// Filter sessions by project name (substring match). Flag: `--project`.
    pub sessions_project: Option<String>,

    /// GitHub personal access token for authenticated API requests. Env: `GITHUB_TOKEN`. **Secret.**
    pub github_token: Option<String>,

    /// Also index source code files when ingesting a GitHub repository. Flag: `--include-source`.
    pub github_include_source: bool,

    /// Reddit OAuth2 client ID. Env: `REDDIT_CLIENT_ID`. **Secret.**
    pub reddit_client_id: Option<String>,

    /// Reddit OAuth2 client secret. Env: `REDDIT_CLIENT_SECRET`. **Secret.**
    pub reddit_client_secret: Option<String>,

    /// Sort order for subreddit posts. Flag: `--reddit-sort`.
    pub reddit_sort: RedditSort,

    /// Time range for top posts. Flag: `--reddit-time`.
    pub reddit_time: RedditTime,

    /// Max posts to fetch per subreddit (0 = unlimited). Flag: `--reddit-max-posts`.
    pub reddit_max_posts: usize,

    /// Minimum score for posts/comments to be indexed. Flag: `--reddit-min-score`.
    pub reddit_min_score: i32,

    /// Max comment tree depth to traverse. Flag: `--reddit-depth`.
    pub reddit_depth: usize,

    /// Scrape external links in posts and include their content. Flag: `--reddit-scrape-links`.
    pub reddit_scrape_links: bool,

    /// Base URL of the TEI (Text Embeddings Inference) service. Env: `TEI_URL`. Flag: `--tei-url`.
    pub tei_url: String,

    /// Base URL of the Qdrant vector store. Env: `QDRANT_URL`. Flag: `--qdrant-url`.
    pub qdrant_url: String,

    /// OpenAI-compatible API base URL (e.g. `http://ollama:11434/v1`). Env: `OPENAI_BASE_URL`.
    pub openai_base_url: String,

    /// API key for the OpenAI-compatible LLM endpoint. Env: `OPENAI_API_KEY`. **Secret.**
    pub openai_api_key: String,

    /// Model name to use for LLM completions (e.g. `llama3`). Env: `OPENAI_MODEL`.
    pub openai_model: String,

    /// Tavily search API key. Env: `TAVILY_API_KEY`. **Secret.**
    pub tavily_api_key: String,

    /// Print verbose RAG diagnostics (retrieved chunks, scores) during `ask`/`evaluate`. Flag: `--diagnostics`.
    pub ask_diagnostics: bool,

    /// Maximum total characters of context passed to the LLM in a single `ask` request.
    /// Env: `AXON_ASK_MAX_CONTEXT_CHARS` (clamped 20_000–400_000). Default: 120_000.
    pub ask_max_context_chars: usize,

    /// Number of candidate chunks retrieved from Qdrant before reranking.
    /// Env: `AXON_ASK_CANDIDATE_LIMIT` (clamped 8–200). Default: 64.
    pub ask_candidate_limit: usize,

    /// Maximum chunks included in the LLM context after reranking.
    /// Env: `AXON_ASK_CHUNK_LIMIT` (clamped 3–40). Default: 10.
    pub ask_chunk_limit: usize,

    /// Number of top-scoring documents for which full-doc backfill is attempted.
    /// Env: `AXON_ASK_FULL_DOCS` (clamped 1–20). Default: 4.
    pub ask_full_docs: usize,

    /// Extra chunks added from each full-doc backfill pass.
    /// Env: `AXON_ASK_BACKFILL_CHUNKS` (clamped 0–20). Default: 3.
    pub ask_backfill_chunks: usize,

    /// Maximum concurrent Qdrant fetches during full-doc backfill.
    /// Env: `AXON_ASK_DOC_FETCH_CONCURRENCY` (clamped 1–16). Default: 4.
    pub ask_doc_fetch_concurrency: usize,

    /// Maximum chunks fetched per document during backfill.
    /// Env: `AXON_ASK_DOC_CHUNK_LIMIT` (clamped 8–2000). Default: 192.
    pub ask_doc_chunk_limit: usize,

    /// Minimum Qdrant similarity score for a chunk to be included in RAG context.
    /// Env: `AXON_ASK_MIN_RELEVANCE_SCORE` (clamped -1.0–2.0). Default: 0.45.
    pub ask_min_relevance_score: f64,

    /// Run the command on a recurring schedule every N seconds (`None` = one-shot). Flag: `--cron-every-seconds`.
    pub cron_every_seconds: Option<u64>,

    /// Stop cron after this many runs (`None` = run forever). Flag: `--cron-max-runs`.
    pub cron_max_runs: Option<usize>,

    /// Seconds a running job may remain idle before the watchdog marks it stale.
    /// Env: `AXON_JOB_STALE_TIMEOUT_SECS`. Flag: `--watchdog-stale-timeout-secs`.
    pub watchdog_stale_timeout_secs: i64,

    /// Seconds a stale-marked job must remain unchanged before the watchdog reclaims it.
    /// Env: `AXON_JOB_STALE_CONFIRM_SECS`. Flag: `--watchdog-confirm-secs`.
    pub watchdog_confirm_secs: i64,

    /// Emit machine-readable JSON output on stdout instead of human-readable text. Flag: `--json`.
    pub json_output: bool,

    /// Re-crawl URLs found in a previous crawl result rather than starting fresh. Flag: `--crawl-from-result`.
    pub crawl_from_result: bool,

    /// Deduplicate trailing-slash URL variants (e.g. `/about` and `/about/` treated as one).
    /// Spider: `with_normalize(bool)`. Default false. Flag: `--normalize`.
    pub normalize: bool,

    // P2 — engine tuning (previously hardcoded in engine.rs)
    /// Seconds to wait for Chrome network idle before capturing the page.
    /// Used by `WaitForIdleNetwork`. Default: 15. Flag: `--chrome-network-idle-timeout`.
    pub chrome_network_idle_timeout_secs: u64,

    /// Thin-page ratio threshold for auto-switch from HTTP to Chrome mode (0.0–1.0).
    /// If more than this fraction of crawled pages are thin, retry with Chrome.
    /// Default: 0.60. Flag: `--auto-switch-thin-ratio`.
    pub auto_switch_thin_ratio: f64,

    /// Minimum pages crawled before auto-switch eligibility is evaluated.
    /// Prevents triggering Chrome on tiny crawls. Default: 10. Flag: `--auto-switch-min-pages`.
    pub auto_switch_min_pages: usize,

    /// Minimum broadcast channel buffer for crawl page receiver (entries, not bytes).
    /// Set by performance profile. Default (high-stable): 4096.
    pub crawl_broadcast_buffer_min: usize,

    /// Maximum broadcast channel buffer for crawl page receiver (entries, not bytes).
    /// Set by performance profile. Default (high-stable): 16_384.
    pub crawl_broadcast_buffer_max: usize,

    // P3 — missing spider builder methods
    /// URL allow-list: only crawl URLs matching at least one of these regex patterns.
    /// Complement to the URL blacklist. Default: [] (no restriction). Flag: `--url-whitelist` (repeatable).
    pub url_whitelist: Vec<String>,

    /// Block asset downloads (images, CSS, fonts, JS) during crawl to reduce bandwidth.
    /// Spider: `with_block_assets(true)`. Default: false. Flag: `--block-assets`.
    pub block_assets: bool,

    /// Maximum response size per page in bytes; pages exceeding this are skipped.
    /// Spider: `with_max_page_bytes(u64)`. Default: None (unlimited). Flag: `--max-page-bytes`.
    pub max_page_bytes: Option<u64>,

    /// Use strict redirect policy — only follow same-origin redirects.
    /// Spider: `with_redirect_policy(RedirectPolicy::Strict)`. Default: false. Flag: `--redirect-policy-strict`.
    pub redirect_policy_strict: bool,

    /// CSS selector to wait for before capturing a Chrome page.
    /// Spider: `with_wait_for_selector`. Default: None. Flag: `--chrome-wait-for-selector`.
    pub chrome_wait_for_selector: Option<String>,

    /// Capture full-page PNG screenshots during Chrome crawl.
    /// Spider: `with_screenshot`. Saved to `output_dir`. Default: false. Flag: `--chrome-screenshot`.
    pub chrome_screenshot: bool,

    // P4 — spider_agent improvements
    /// Research crawl depth limit for the `research` command.
    /// Passed to `ResearchOptions::with_depth` if available. Default: None. Flag: `--research-depth`.
    pub research_depth: Option<usize>,

    /// Time range filter for the `search` command (values: day, week, month, year).
    /// Passed to `SearchOptions::with_time_range`. Default: None. Flag: `--search-time-range`.
    pub search_time_range: Option<String>,

    // P5 — opt-in crawl safety/compat flags
    /// Bypass Content Security Policy in Chrome — helps on pages that block inline JS via CSP.
    /// Spider: `with_csp_bypass(true)`. Chrome only. Default: false. Flag: `--bypass-csp`.
    pub bypass_csp: bool,

    /// Accept invalid/self-signed TLS certificates. Useful for internal or staging sites.
    /// Spider: `with_danger_accept_invalid_certs(true)`. Default: false. Flag: `--accept-invalid-certs`.
    pub accept_invalid_certs: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            command: CommandKind::Status,
            start_url: String::new(),
            positional: Vec::new(),
            urls_csv: None,
            url_glob: Vec::new(),
            query: None,
            search_limit: 10,
            max_pages: 0,
            max_depth: 5,
            include_subdomains: true,
            exclude_path_prefix: Vec::new(),
            output_dir: PathBuf::from(".cache/axon-rust/output"),
            output_path: None,
            render_mode: RenderMode::AutoSwitch,
            chrome_remote_url: None,
            chrome_proxy: None,
            chrome_user_agent: None,
            chrome_headless: true,
            chrome_anti_bot: true,
            chrome_intercept: true,
            chrome_stealth: true,
            chrome_bootstrap: true,
            chrome_bootstrap_timeout_ms: 3_000,
            chrome_bootstrap_retries: 2,
            webdriver_url: None,
            respect_robots: false,
            min_markdown_chars: 200,
            drop_thin_markdown: true,
            discover_sitemaps: true,
            cache: true,
            cache_skip_browser: false,
            format: ScrapeFormat::Markdown,
            collection: "cortex".to_string(),
            embed: true,
            batch_concurrency: 16,
            wait: false,
            yes: false,
            performance_profile: PerformanceProfile::HighStable,
            crawl_concurrency_limit: None,
            backfill_concurrency_limit: None,
            sitemap_only: false,
            delay_ms: 0,
            request_timeout_ms: None,
            fetch_retries: 2,
            retry_backoff_ms: 250,
            shared_queue: true,
            pg_url: String::new(),
            redis_url: String::new(),
            amqp_url: String::new(),
            crawl_queue: "axon.crawl.jobs".to_string(),
            extract_queue: "axon.extract.jobs".to_string(),
            embed_queue: "axon.embed.jobs".to_string(),
            ingest_queue: "axon.ingest.jobs".to_string(),
            sessions_claude: false,
            sessions_codex: false,
            sessions_gemini: false,
            sessions_project: None,
            github_token: None,
            github_include_source: false,
            reddit_client_id: None,
            reddit_client_secret: None,
            reddit_sort: RedditSort::Hot,
            reddit_time: RedditTime::Day,
            reddit_max_posts: 25,
            reddit_min_score: 0,
            reddit_depth: 2,
            reddit_scrape_links: false,
            tei_url: String::new(),
            qdrant_url: "http://127.0.0.1:53333".to_string(),
            openai_base_url: String::new(),
            openai_api_key: String::new(),
            openai_model: String::new(),
            tavily_api_key: String::new(),
            ask_diagnostics: false,
            ask_max_context_chars: 120_000,
            ask_candidate_limit: 64,
            ask_chunk_limit: 10,
            ask_full_docs: 4,
            ask_backfill_chunks: 3,
            ask_doc_fetch_concurrency: 4,
            ask_doc_chunk_limit: 192,
            ask_min_relevance_score: 0.45,
            cron_every_seconds: None,
            cron_max_runs: None,
            watchdog_stale_timeout_secs: 300,
            watchdog_confirm_secs: 60,
            json_output: false,
            crawl_from_result: false,
            normalize: false,
            chrome_network_idle_timeout_secs: 15,
            auto_switch_thin_ratio: 0.60,
            auto_switch_min_pages: 10,
            crawl_broadcast_buffer_min: 4096,
            crawl_broadcast_buffer_max: 16_384,
            url_whitelist: vec![],
            block_assets: false,
            max_page_bytes: None,
            redirect_policy_strict: false,
            chrome_wait_for_selector: None,
            chrome_screenshot: false,
            research_depth: None,
            search_time_range: None,
            bypass_csp: false,
            accept_invalid_certs: false,
        }
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("command", &self.command)
            .field("start_url", &self.start_url)
            .field("positional", &self.positional)
            .field("urls_csv", &self.urls_csv)
            .field("url_glob", &self.url_glob)
            .field("query", &self.query)
            .field("search_limit", &self.search_limit)
            .field("max_pages", &self.max_pages)
            .field("max_depth", &self.max_depth)
            .field("include_subdomains", &self.include_subdomains)
            .field("exclude_path_prefix", &self.exclude_path_prefix)
            .field("output_dir", &self.output_dir)
            .field("output_path", &self.output_path)
            .field("render_mode", &self.render_mode)
            .field("chrome_remote_url", &self.chrome_remote_url)
            .field("chrome_proxy", &self.chrome_proxy)
            .field("chrome_user_agent", &self.chrome_user_agent)
            .field("chrome_headless", &self.chrome_headless)
            .field("chrome_anti_bot", &self.chrome_anti_bot)
            .field("chrome_intercept", &self.chrome_intercept)
            .field("chrome_stealth", &self.chrome_stealth)
            .field("chrome_bootstrap", &self.chrome_bootstrap)
            .field(
                "chrome_bootstrap_timeout_ms",
                &self.chrome_bootstrap_timeout_ms,
            )
            .field("chrome_bootstrap_retries", &self.chrome_bootstrap_retries)
            .field("webdriver_url", &self.webdriver_url)
            .field("respect_robots", &self.respect_robots)
            .field("min_markdown_chars", &self.min_markdown_chars)
            .field("drop_thin_markdown", &self.drop_thin_markdown)
            .field("discover_sitemaps", &self.discover_sitemaps)
            .field("cache", &self.cache)
            .field("cache_skip_browser", &self.cache_skip_browser)
            .field("format", &self.format)
            .field("collection", &self.collection)
            .field("embed", &self.embed)
            .field("batch_concurrency", &self.batch_concurrency)
            .field("wait", &self.wait)
            .field("yes", &self.yes)
            .field("performance_profile", &self.performance_profile)
            .field("crawl_concurrency_limit", &self.crawl_concurrency_limit)
            .field(
                "backfill_concurrency_limit",
                &self.backfill_concurrency_limit,
            )
            .field("sitemap_only", &self.sitemap_only)
            .field("delay_ms", &self.delay_ms)
            .field("request_timeout_ms", &self.request_timeout_ms)
            .field("fetch_retries", &self.fetch_retries)
            .field("retry_backoff_ms", &self.retry_backoff_ms)
            .field("shared_queue", &self.shared_queue)
            .field("pg_url", &"[REDACTED]")
            .field("redis_url", &"[REDACTED]")
            .field("amqp_url", &"[REDACTED]")
            .field("crawl_queue", &self.crawl_queue)
            .field("extract_queue", &self.extract_queue)
            .field("embed_queue", &self.embed_queue)
            .field("ingest_queue", &self.ingest_queue)
            .field("sessions_claude", &self.sessions_claude)
            .field("sessions_codex", &self.sessions_codex)
            .field("sessions_gemini", &self.sessions_gemini)
            .field("sessions_project", &self.sessions_project)
            .field("github_token", &"[REDACTED]")
            .field("github_include_source", &self.github_include_source)
            .field("reddit_client_id", &"[REDACTED]")
            .field("reddit_client_secret", &"[REDACTED]")
            .field("reddit_sort", &self.reddit_sort)
            .field("reddit_time", &self.reddit_time)
            .field("reddit_max_posts", &self.reddit_max_posts)
            .field("reddit_min_score", &self.reddit_min_score)
            .field("reddit_depth", &self.reddit_depth)
            .field("reddit_scrape_links", &self.reddit_scrape_links)
            .field("tei_url", &self.tei_url)
            .field("qdrant_url", &self.qdrant_url)
            .field("openai_base_url", &self.openai_base_url)
            .field("openai_api_key", &"[REDACTED]")
            .field("openai_model", &self.openai_model)
            .field("tavily_api_key", &"[REDACTED]")
            .field("ask_diagnostics", &self.ask_diagnostics)
            .field("ask_max_context_chars", &self.ask_max_context_chars)
            .field("ask_candidate_limit", &self.ask_candidate_limit)
            .field("ask_chunk_limit", &self.ask_chunk_limit)
            .field("ask_full_docs", &self.ask_full_docs)
            .field("ask_backfill_chunks", &self.ask_backfill_chunks)
            .field("ask_doc_fetch_concurrency", &self.ask_doc_fetch_concurrency)
            .field("ask_doc_chunk_limit", &self.ask_doc_chunk_limit)
            .field("ask_min_relevance_score", &self.ask_min_relevance_score)
            .field("cron_every_seconds", &self.cron_every_seconds)
            .field("cron_max_runs", &self.cron_max_runs)
            .field(
                "watchdog_stale_timeout_secs",
                &self.watchdog_stale_timeout_secs,
            )
            .field("watchdog_confirm_secs", &self.watchdog_confirm_secs)
            .field("json_output", &self.json_output)
            .field("crawl_from_result", &self.crawl_from_result)
            .field("normalize", &self.normalize)
            .field(
                "chrome_network_idle_timeout_secs",
                &self.chrome_network_idle_timeout_secs,
            )
            .field("auto_switch_thin_ratio", &self.auto_switch_thin_ratio)
            .field("auto_switch_min_pages", &self.auto_switch_min_pages)
            .field(
                "crawl_broadcast_buffer_min",
                &self.crawl_broadcast_buffer_min,
            )
            .field(
                "crawl_broadcast_buffer_max",
                &self.crawl_broadcast_buffer_max,
            )
            .field("url_whitelist", &self.url_whitelist)
            .field("block_assets", &self.block_assets)
            .field("max_page_bytes", &self.max_page_bytes)
            .field("redirect_policy_strict", &self.redirect_policy_strict)
            .field("chrome_wait_for_selector", &self.chrome_wait_for_selector)
            .field("chrome_screenshot", &self.chrome_screenshot)
            .field("research_depth", &self.research_depth)
            .field("search_time_range", &self.search_time_range)
            .field("bypass_csp", &self.bypass_csp)
            .field("accept_invalid_certs", &self.accept_invalid_certs)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_kind_research_as_str() {
        assert_eq!(CommandKind::Research.as_str(), "research");
    }

    #[test]
    fn test_config_default_sensible_values() {
        let cfg = Config::default();
        assert_eq!(cfg.search_limit, 10);
        assert_eq!(cfg.max_depth, 5);
        assert_eq!(cfg.min_markdown_chars, 200);
        assert_eq!(cfg.batch_concurrency, 16);
        assert_eq!(cfg.collection, "cortex");
        assert!(cfg.embed);
        assert!(!cfg.wait);
        assert!(cfg.discover_sitemaps);
        assert!(cfg.drop_thin_markdown);
        assert!(!cfg.respect_robots);
        assert!(cfg.shared_queue);
        assert!(!cfg.json_output);
        assert_eq!(cfg.qdrant_url, "http://127.0.0.1:53333");
        assert_eq!(cfg.crawl_queue, "axon.crawl.jobs");
        assert_eq!(cfg.embed_queue, "axon.embed.jobs");
        assert_eq!(cfg.ask_max_context_chars, 120_000);
        assert_eq!(cfg.ask_candidate_limit, 64);
        assert!((cfg.ask_min_relevance_score - 0.45).abs() < f64::EPSILON);
        assert_eq!(cfg.watchdog_stale_timeout_secs, 300);
        assert_eq!(cfg.watchdog_confirm_secs, 60);
    }

    #[test]
    fn test_config_default_secrets_are_empty() {
        let cfg = Config::default();
        assert!(cfg.pg_url.is_empty());
        assert!(cfg.redis_url.is_empty());
        assert!(cfg.amqp_url.is_empty());
        assert!(cfg.openai_api_key.is_empty());
        assert!(cfg.tavily_api_key.is_empty());
        assert!(cfg.github_token.is_none());
        assert!(cfg.reddit_client_id.is_none());
        assert!(cfg.reddit_client_secret.is_none());
    }

    #[test]
    fn test_config_default_sessions_flags_off() {
        let cfg = Config::default();
        assert!(!cfg.sessions_claude);
        assert!(!cfg.sessions_codex);
        assert!(!cfg.sessions_gemini);
        assert!(cfg.sessions_project.is_none());
    }

    #[test]
    fn test_config_debug_redacts_secrets() {
        let cfg = Config {
            pg_url: "postgresql://user:password@host/db".to_string(),
            redis_url: "redis://:secret@host:6379".to_string(),
            amqp_url: "amqp://user:password@host/%2f".to_string(),
            openai_api_key: "sk-supersecret".to_string(),
            tavily_api_key: "tvly-supersecret".to_string(),
            github_token: Some("ghp_supersecret".to_string()),
            reddit_client_id: Some("my-reddit-id".to_string()),
            reddit_client_secret: Some("my-reddit-secret".to_string()),
            ..Config::default()
        };

        let debug_output = format!("{cfg:?}");

        // Secrets must NOT appear in Debug output.
        assert!(!debug_output.contains("password"), "pg_url password leaked");
        assert!(!debug_output.contains("secret@"), "redis_url secret leaked");
        assert!(
            !debug_output.contains("sk-supersecret"),
            "openai_api_key leaked"
        );
        assert!(
            !debug_output.contains("tvly-supersecret"),
            "tavily_api_key leaked"
        );
        assert!(
            !debug_output.contains("ghp_supersecret"),
            "github_token leaked"
        );
        assert!(
            !debug_output.contains("my-reddit-id"),
            "reddit_client_id leaked"
        );
        assert!(
            !debug_output.contains("my-reddit-secret"),
            "reddit_client_secret leaked"
        );

        // Redaction markers must be present.
        assert!(
            debug_output.contains("[REDACTED]"),
            "no [REDACTED] marker found"
        );
    }

    #[test]
    fn test_config_debug_includes_sessions_fields() {
        let cfg = Config {
            sessions_claude: true,
            sessions_codex: false,
            sessions_gemini: true,
            ..Config::default()
        };

        let debug_output = format!("{cfg:?}");
        assert!(debug_output.contains("sessions_claude: true"));
        assert!(debug_output.contains("sessions_codex: false"));
        assert!(debug_output.contains("sessions_gemini: true"));
    }

    // --- Performance profile range tests ---

    /// Replicates the computation from `crates/core/config/parse/performance.rs`
    /// so we can test all four profiles without depending on the private module.
    /// Returns (crawl_concurrency, backfill_concurrency, timeout_ms, retries, backoff_ms).
    fn profile_defaults(profile: PerformanceProfile) -> (usize, usize, u64, usize, u64) {
        let logical_cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(8);
        match profile {
            PerformanceProfile::HighStable => (
                (logical_cpus.saturating_mul(8)).clamp(64, 192),
                (logical_cpus.saturating_mul(6)).clamp(32, 128),
                20_000,
                2,
                250,
            ),
            PerformanceProfile::Extreme => (
                (logical_cpus.saturating_mul(16)).clamp(128, 384),
                (logical_cpus.saturating_mul(10)).clamp(64, 256),
                15_000,
                1,
                100,
            ),
            PerformanceProfile::Balanced => (
                (logical_cpus.saturating_mul(4)).clamp(32, 96),
                (logical_cpus.saturating_mul(3)).clamp(16, 64),
                30_000,
                2,
                300,
            ),
            PerformanceProfile::Max => (
                (logical_cpus.saturating_mul(24)).clamp(256, 1024),
                (logical_cpus.saturating_mul(20)).clamp(128, 1024),
                12_000,
                1,
                50,
            ),
        }
    }

    #[test]
    fn test_high_stable_profile_within_bounds() {
        let (crawl, backfill, timeout, retries, backoff) =
            profile_defaults(PerformanceProfile::HighStable);
        assert!((64..=192).contains(&crawl), "crawl={crawl} out of [64,192]");
        assert!(
            (32..=128).contains(&backfill),
            "backfill={backfill} out of [32,128]"
        );
        assert_eq!(timeout, 20_000, "timeout should be 20s");
        assert_eq!(retries, 2);
        assert_eq!(backoff, 250);
    }

    #[test]
    fn test_extreme_profile_within_bounds() {
        let (crawl, backfill, timeout, retries, backoff) =
            profile_defaults(PerformanceProfile::Extreme);
        assert!(
            (128..=384).contains(&crawl),
            "crawl={crawl} out of [128,384]"
        );
        assert!(
            (64..=256).contains(&backfill),
            "backfill={backfill} out of [64,256]"
        );
        assert_eq!(timeout, 15_000, "timeout should be 15s");
        assert_eq!(retries, 1);
        assert_eq!(backoff, 100);
    }

    #[test]
    fn test_balanced_profile_within_bounds() {
        let (crawl, backfill, timeout, retries, backoff) =
            profile_defaults(PerformanceProfile::Balanced);
        assert!((32..=96).contains(&crawl), "crawl={crawl} out of [32,96]");
        assert!(
            (16..=64).contains(&backfill),
            "backfill={backfill} out of [16,64]"
        );
        assert_eq!(timeout, 30_000, "timeout should be 30s");
        assert_eq!(retries, 2);
        assert_eq!(backoff, 300);
    }

    #[test]
    fn test_max_profile_within_bounds() {
        let (crawl, backfill, timeout, retries, backoff) =
            profile_defaults(PerformanceProfile::Max);
        assert!(
            (256..=1024).contains(&crawl),
            "crawl={crawl} out of [256,1024]"
        );
        assert!(
            (128..=1024).contains(&backfill),
            "backfill={backfill} out of [128,1024]"
        );
        assert_eq!(timeout, 12_000, "timeout should be 12s");
        assert_eq!(retries, 1);
        assert_eq!(backoff, 50);
    }

    #[test]
    fn test_extreme_crawl_concurrency_exceeds_balanced() {
        let (extreme_crawl, ..) = profile_defaults(PerformanceProfile::Extreme);
        let (balanced_crawl, ..) = profile_defaults(PerformanceProfile::Balanced);
        assert!(
            extreme_crawl > balanced_crawl,
            "extreme crawl concurrency ({extreme_crawl}) should exceed balanced ({balanced_crawl})"
        );
    }

    #[test]
    fn test_max_crawl_concurrency_exceeds_extreme() {
        let (max_crawl, ..) = profile_defaults(PerformanceProfile::Max);
        let (extreme_crawl, ..) = profile_defaults(PerformanceProfile::Extreme);
        assert!(
            max_crawl > extreme_crawl,
            "max crawl concurrency ({max_crawl}) should exceed extreme ({extreme_crawl})"
        );
    }

    #[test]
    fn new_engine_tuning_defaults() {
        let cfg = Config::default();
        assert_eq!(cfg.chrome_network_idle_timeout_secs, 15);
        assert!((cfg.auto_switch_thin_ratio - 0.60).abs() < f64::EPSILON);
        assert_eq!(cfg.auto_switch_min_pages, 10);
        assert_eq!(cfg.crawl_broadcast_buffer_min, 4096);
        assert_eq!(cfg.crawl_broadcast_buffer_max, 16_384);
    }

    #[test]
    fn new_spider_builder_defaults() {
        let cfg = Config::default();
        assert!(cfg.url_whitelist.is_empty());
        assert!(!cfg.block_assets);
        assert!(cfg.max_page_bytes.is_none());
        assert!(!cfg.redirect_policy_strict);
        assert!(cfg.chrome_wait_for_selector.is_none());
        assert!(!cfg.chrome_screenshot);
    }

    #[test]
    fn new_spider_agent_defaults() {
        let cfg = Config::default();
        assert!(cfg.research_depth.is_none());
        assert!(cfg.search_time_range.is_none());
    }
}
