use clap::ValueEnum;
use std::fmt;
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
    Sessions,
    Research,
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
            Self::Sessions => "sessions",
            Self::Research => "research",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_kind_research_as_str() {
        assert_eq!(CommandKind::Research.as_str(), "research");
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
    pub backfill_concurrency_limit: Option<usize>,
    pub sitemap_only: bool,
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
    pub sessions_claude: bool,
    pub sessions_codex: bool,
    pub sessions_gemini: bool,
    pub sessions_project: Option<String>,
    pub github_token: Option<String>,
    pub github_include_source: bool,
    pub reddit_client_id: Option<String>,
    pub reddit_client_secret: Option<String>,
    pub tei_url: String,
    pub qdrant_url: String,
    pub openai_base_url: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub tavily_api_key: String,
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
    pub crawl_from_result: bool,
    /// Deduplicate trailing-slash URL variants (e.g. /about and /about/ treated as one).
    /// Spider: with_normalize(bool). Default false.
    pub normalize: bool,
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
            .field("batch_queue", &self.batch_queue)
            .field("extract_queue", &self.extract_queue)
            .field("embed_queue", &self.embed_queue)
            .field("ingest_queue", &self.ingest_queue)
            .field("github_token", &"[REDACTED]")
            .field("github_include_source", &self.github_include_source)
            .field("reddit_client_id", &"[REDACTED]")
            .field("reddit_client_secret", &"[REDACTED]")
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
            .finish()
    }
}
