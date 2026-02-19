use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use spider::url::Url;
use std::env;
use std::path::PathBuf;
use std::process;

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
    Suggest,
    Sources,
    Domains,
    Stats,
    Status,
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
            Self::Suggest => "suggest",
            Self::Sources => "sources",
            Self::Domains => "domains",
            Self::Stats => "stats",
            Self::Status => "status",
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

#[derive(Debug, Parser)]
#[command(name = "axon", about = "Axon CLI (Rust + Spider.rs)")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,

    #[command(flatten)]
    global: GlobalArgs,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
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
    Suggest(TextArg),
    Sources,
    Domains,
    Stats,
    Status,
}

#[derive(Debug, Args)]
struct UrlArg {
    #[arg(value_name = "URL")]
    value: Option<String>,
}

#[derive(Debug, Args)]
struct TextArg {
    #[arg(value_name = "TEXT")]
    value: Vec<String>,
}

#[derive(Debug, Args)]
struct AskArgs {
    #[arg(long, action = ArgAction::SetTrue)]
    diagnostics: bool,
    #[arg(value_name = "TEXT")]
    value: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct CrawlArgs {
    #[command(subcommand)]
    job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    url: Option<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct BatchArgs {
    #[command(subcommand)]
    job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    urls: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct ExtractArgs {
    #[command(subcommand)]
    job: Option<JobSubcommand>,
    #[arg(value_name = "URL")]
    urls: Vec<String>,
}

#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
struct EmbedArgs {
    #[command(subcommand)]
    job: Option<JobSubcommand>,
    #[arg(value_name = "INPUT")]
    input: Option<String>,
}

#[derive(Debug, Subcommand)]
enum JobSubcommand {
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
struct GlobalArgs {
    #[arg(global = true, long, default_value = "https://example.com")]
    start_url: String,

    #[arg(global = true, long, default_value_t = 0)]
    max_pages: u32,

    #[arg(global = true, long, default_value_t = 5)]
    max_depth: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    include_subdomains: bool,

    #[arg(global = true, long = "exclude-path-prefix", value_delimiter = ',')]
    exclude_path_prefix: Vec<String>,

    #[arg(global = true, long, default_value = ".cache/axon-rust/output")]
    output_dir: PathBuf,

    #[arg(global = true, long)]
    output: Option<PathBuf>,

    #[arg(global = true, long, value_enum, default_value_t = RenderMode::AutoSwitch)]
    render_mode: RenderMode,

    #[arg(global = true, long, env = "AXON_CHROME_REMOTE_URL")]
    chrome_remote_url: Option<String>,

    #[arg(global = true, long, env = "AXON_CHROME_PROXY")]
    chrome_proxy: Option<String>,

    #[arg(global = true, long, env = "AXON_CHROME_USER_AGENT")]
    chrome_user_agent: Option<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    chrome_headless: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    chrome_anti_bot: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    chrome_intercept: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    chrome_stealth: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    chrome_bootstrap: bool,

    #[arg(global = true, long, default_value_t = 3000)]
    chrome_bootstrap_timeout_ms: u64,

    #[arg(global = true, long, default_value_t = 2)]
    chrome_bootstrap_retries: usize,

    #[arg(global = true, long, env = "AXON_WEBDRIVER_URL")]
    webdriver_url: Option<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    respect_robots: bool,

    #[arg(global = true, long, default_value_t = 200)]
    min_markdown_chars: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    drop_thin_markdown: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    discover_sitemaps: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    cache: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    cache_skip_browser: bool,

    #[arg(global = true, long, value_enum, default_value_t = ScrapeFormat::Markdown)]
    format: ScrapeFormat,

    #[arg(global = true, long, default_value_t = 10)]
    limit: usize,

    #[arg(global = true, long)]
    query: Option<String>,

    #[arg(global = true, long)]
    urls: Option<String>,

    #[arg(global = true, long = "url-glob", value_delimiter = ',')]
    url_glob: Vec<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    embed: bool,

    #[arg(global = true, long, env = "AXON_COLLECTION", default_value = "cortex")]
    collection: String,

    #[arg(global = true, long, default_value_t = 16)]
    batch_concurrency: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    wait: bool,

    #[arg(global = true, long, action = ArgAction::SetTrue)]
    yes: bool,

    #[arg(global = true, long, action = ArgAction::SetTrue)]
    json: bool,

    #[arg(global = true, long, value_enum, default_value_t = PerformanceProfile::HighStable)]
    performance_profile: PerformanceProfile,

    #[arg(global = true, long)]
    concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    crawl_concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    sitemap_concurrency_limit: Option<usize>,

    #[arg(global = true, long)]
    backfill_concurrency_limit: Option<usize>,

    #[arg(global = true, long, default_value_t = 512)]
    max_sitemaps: usize,

    #[arg(global = true, long, default_value_t = 0)]
    delay_ms: u64,

    #[arg(global = true, long)]
    request_timeout_ms: Option<u64>,

    #[arg(global = true, long, default_value_t = 0)]
    fetch_retries: usize,

    #[arg(global = true, long, default_value_t = 0)]
    retry_backoff_ms: u64,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    shared_queue: bool,

    #[arg(global = true, long)]
    pg_url: Option<String>,

    #[arg(global = true, long)]
    redis_url: Option<String>,

    #[arg(global = true, long)]
    amqp_url: Option<String>,

    #[arg(global = true, long)]
    crawl_queue: Option<String>,

    #[arg(global = true, long)]
    batch_queue: Option<String>,

    #[arg(global = true, long)]
    extract_queue: Option<String>,

    #[arg(global = true, long)]
    embed_queue: Option<String>,

    #[arg(global = true, long)]
    tei_url: Option<String>,

    #[arg(global = true, long)]
    qdrant_url: Option<String>,

    #[arg(global = true, long)]
    openai_base_url: Option<String>,

    #[arg(global = true, long)]
    openai_api_key: Option<String>,

    #[arg(global = true, long)]
    openai_model: Option<String>,

    #[arg(
        global = true,
        long,
        env = "AXON_JOB_STALE_TIMEOUT_SECS",
        default_value_t = 300
    )]
    watchdog_stale_timeout_secs: i64,

    #[arg(
        global = true,
        long,
        env = "AXON_JOB_STALE_CONFIRM_SECS",
        default_value_t = 60
    )]
    watchdog_confirm_secs: i64,

    #[arg(global = true, long)]
    cron_every_seconds: Option<u64>,

    #[arg(global = true, long)]
    cron_max_runs: Option<usize>,
}

fn normalize_local_service_url(url: String) -> String {
    // Keep container-internal service DNS when running inside Docker.
    if std::path::Path::new("/.dockerenv").exists() {
        return url;
    }

    // Map container hostname → (local_host, local_port).
    // Uses URL parsing so only the host component is matched, never a path or query value.
    const HOST_MAP: &[(&str, &str, u16)] = &[
        ("axon-postgres", "127.0.0.1", 53432),
        ("axon-redis", "127.0.0.1", 53379),
        ("axon-rabbitmq", "127.0.0.1", 45535),
        ("axon-qdrant", "127.0.0.1", 53333),
    ];

    let Ok(mut parsed) = Url::parse(&url) else {
        return url;
    };
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return url,
    };
    for (container_host, local_host, local_port) in HOST_MAP {
        if host == *container_host {
            let _ = parsed.set_host(Some(local_host));
            let _ = parsed.set_port(Some(*local_port));
            return parsed.to_string();
        }
    }
    url
}

fn default_exclude_prefixes() -> Vec<String> {
    vec![
        "/fr", "/de", "/es", "/ja", "/zh", "/zh-cn", "/zh-tw", "/ko", "/pt", "/pt-br", "/it",
        "/nl", "/pl", "/ru", "/tr", "/ar", "/id", "/vi", "/th", "/cs", "/da", "/fi", "/no", "/sv",
        "/he", "/uk", "/ro", "/hu", "/el",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

struct NormalizedExcludePrefixes {
    prefixes: Vec<String>,
    disable_defaults: bool,
}

fn normalize_exclude_prefixes(input: Vec<String>) -> NormalizedExcludePrefixes {
    let disable_by_empty = input.iter().any(|v| matches!(v.trim(), "" | "/"));
    let disable_by_none = input.iter().any(|v| v.trim().eq_ignore_ascii_case("none"));
    if disable_by_none {
        let ignored: Vec<&str> = input
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.eq_ignore_ascii_case("none"))
            .filter(|value| !value.is_empty() && *value != "/")
            .collect();
        if !ignored.is_empty() {
            eprintln!(
                "warning: --exclude-path-prefix 'none' disables exclusions; ignoring additional prefixes: {}",
                ignored.join(", ")
            );
        }
        return NormalizedExcludePrefixes {
            prefixes: Vec::new(),
            disable_defaults: true,
        };
    }

    let mut out = Vec::new();
    for raw in input {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "/" {
            continue;
        }
        let normalized = if trimmed.starts_with('/') {
            trimmed.to_string()
        } else {
            format!("/{trimmed}")
        };
        out.push(normalized);
    }
    out.sort();
    out.dedup();
    NormalizedExcludePrefixes {
        prefixes: out,
        disable_defaults: disable_by_empty,
    }
}

fn positional_from_job(job: JobSubcommand) -> Vec<String> {
    match job {
        JobSubcommand::Status { job_id } => vec!["status".to_string(), job_id],
        JobSubcommand::Cancel { job_id } => vec!["cancel".to_string(), job_id],
        JobSubcommand::Errors { job_id } => vec!["errors".to_string(), job_id],
        JobSubcommand::List => vec!["list".to_string()],
        JobSubcommand::Cleanup => vec!["cleanup".to_string()],
        JobSubcommand::Clear => vec!["clear".to_string()],
        JobSubcommand::Worker => vec!["worker".to_string()],
        JobSubcommand::Recover => vec!["recover".to_string()],
        JobSubcommand::Doctor => vec!["doctor".to_string()],
    }
}

fn performance_defaults(profile: PerformanceProfile) -> (usize, usize, usize, u64, usize, u64) {
    let logical_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8);

    match profile {
        PerformanceProfile::HighStable => (
            (logical_cpus.saturating_mul(8)).clamp(64, 192),
            (logical_cpus.saturating_mul(12)).clamp(64, 256),
            (logical_cpus.saturating_mul(6)).clamp(32, 128),
            20_000,
            2,
            250,
        ),
        PerformanceProfile::Extreme => (
            (logical_cpus.saturating_mul(16)).clamp(128, 384),
            (logical_cpus.saturating_mul(20)).clamp(128, 512),
            (logical_cpus.saturating_mul(10)).clamp(64, 256),
            15_000,
            1,
            100,
        ),
        PerformanceProfile::Balanced => (
            (logical_cpus.saturating_mul(4)).clamp(32, 96),
            (logical_cpus.saturating_mul(6)).clamp(32, 128),
            (logical_cpus.saturating_mul(3)).clamp(16, 64),
            30_000,
            2,
            300,
        ),
        PerformanceProfile::Max => (
            (logical_cpus.saturating_mul(24)).clamp(256, 1024),
            (logical_cpus.saturating_mul(32)).clamp(256, 1536),
            (logical_cpus.saturating_mul(20)).clamp(128, 1024),
            12_000,
            1,
            50,
        ),
    }
}

fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

fn env_f64_clamped(key: &str, default: f64, min: f64, max: f64) -> f64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
        .clamp(min, max)
}

fn into_config(cli: Cli) -> Config {
    let global = cli.global;

    let mut ask_diagnostics = false;
    let (command, positional) = match cli.command {
        CliCommand::Scrape(args) => (
            CommandKind::Scrape,
            args.value.into_iter().collect::<Vec<String>>(),
        ),
        CliCommand::Crawl(args) => (
            CommandKind::Crawl,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.url.into_iter().collect()
            },
        ),
        CliCommand::Map(args) => (
            CommandKind::Map,
            args.value.into_iter().collect::<Vec<String>>(),
        ),
        CliCommand::Batch(args) => (
            CommandKind::Batch,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.urls
            },
        ),
        CliCommand::Extract(args) => (
            CommandKind::Extract,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.urls
            },
        ),
        CliCommand::Search(args) => (CommandKind::Search, args.value),
        CliCommand::Embed(args) => (
            CommandKind::Embed,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.input.into_iter().collect()
            },
        ),
        CliCommand::Debug(args) => (CommandKind::Debug, args.value),
        CliCommand::Doctor => (CommandKind::Doctor, Vec::new()),
        CliCommand::Query(args) => (CommandKind::Query, args.value),
        CliCommand::Retrieve(args) => (
            CommandKind::Retrieve,
            args.value.into_iter().collect::<Vec<String>>(),
        ),
        CliCommand::Ask(args) => {
            ask_diagnostics = args.diagnostics;
            (CommandKind::Ask, args.value)
        }
        CliCommand::Suggest(args) => (CommandKind::Suggest, args.value),
        CliCommand::Sources => (CommandKind::Sources, Vec::new()),
        CliCommand::Domains => (CommandKind::Domains, Vec::new()),
        CliCommand::Stats => (CommandKind::Stats, Vec::new()),
        CliCommand::Status => (CommandKind::Status, Vec::new()),
    };

    let pg_url = normalize_local_service_url(
        global
            .pg_url
            .or_else(|| env::var("AXON_PG_URL").ok())
            .or_else(|| env::var("NUQ_DATABASE_URL").ok())
            .unwrap_or_else(|| {
                eprintln!("warning: AXON_PG_URL not set — using default credentials; set it in .env for production");
                "postgresql://axon:postgres@127.0.0.1:53432/axon".to_string()
            }),
    );

    let redis_url = normalize_local_service_url(
        global
            .redis_url
            .or_else(|| env::var("AXON_REDIS_URL").ok())
            .or_else(|| env::var("REDIS_URL").ok())
            .unwrap_or_else(|| {
                eprintln!("warning: AXON_REDIS_URL not set — using default; set it in .env for production");
                "redis://127.0.0.1:53379".to_string()
            }),
    );

    let amqp_url = normalize_local_service_url(
        global
            .amqp_url
            .or_else(|| env::var("AXON_AMQP_URL").ok())
            .or_else(|| env::var("NUQ_RABBITMQ_URL").ok())
            .unwrap_or_else(|| {
                eprintln!("warning: AXON_AMQP_URL not set — using default credentials; set it in .env for production");
                "amqp://axon:axonrabbit@127.0.0.1:45535/%2f".to_string()
            }),
    );

    let mut crawl_concurrency_limit = global.crawl_concurrency_limit;
    let mut sitemap_concurrency_limit = global.sitemap_concurrency_limit;
    let mut backfill_concurrency_limit = global.backfill_concurrency_limit;

    if let Some(limit) = global.concurrency_limit {
        crawl_concurrency_limit = Some(limit);
        sitemap_concurrency_limit = Some(limit);
        backfill_concurrency_limit = Some(limit);
    }

    let normalized_excludes = normalize_exclude_prefixes(global.exclude_path_prefix);

    let mut cfg = Config {
        command,
        start_url: global.start_url,
        positional,
        urls_csv: global.urls,
        url_glob: global.url_glob,
        query: global.query,
        search_limit: global.limit,
        max_pages: global.max_pages,
        max_depth: global.max_depth,
        include_subdomains: global.include_subdomains,
        exclude_path_prefix: normalized_excludes.prefixes,
        output_dir: global.output_dir,
        output_path: global.output,
        render_mode: global.render_mode,
        chrome_remote_url: global
            .chrome_remote_url
            .or_else(|| env::var("AXON_CHROME_REMOTE_URL").ok()),
        chrome_proxy: global
            .chrome_proxy
            .or_else(|| env::var("AXON_CHROME_PROXY").ok()),
        chrome_user_agent: global
            .chrome_user_agent
            .or_else(|| env::var("AXON_CHROME_USER_AGENT").ok()),
        chrome_headless: global.chrome_headless,
        chrome_anti_bot: global.chrome_anti_bot,
        chrome_intercept: global.chrome_intercept,
        chrome_stealth: global.chrome_stealth,
        chrome_bootstrap: global.chrome_bootstrap,
        chrome_bootstrap_timeout_ms: global.chrome_bootstrap_timeout_ms.max(250),
        chrome_bootstrap_retries: global.chrome_bootstrap_retries.min(10),
        webdriver_url: global
            .webdriver_url
            .or_else(|| env::var("AXON_WEBDRIVER_URL").ok())
            .or_else(|| env::var("WEBDRIVER_URL").ok()),
        respect_robots: global.respect_robots,
        min_markdown_chars: global.min_markdown_chars,
        drop_thin_markdown: global.drop_thin_markdown,
        discover_sitemaps: global.discover_sitemaps,
        cache: global.cache,
        cache_skip_browser: global.cache_skip_browser,
        format: global.format,
        collection: global.collection,
        embed: global.embed,
        batch_concurrency: global.batch_concurrency.clamp(1, 512),
        wait: global.wait,
        yes: global.yes,
        performance_profile: global.performance_profile,
        crawl_concurrency_limit,
        sitemap_concurrency_limit,
        backfill_concurrency_limit,
        max_sitemaps: global.max_sitemaps.max(1),
        delay_ms: global.delay_ms,
        request_timeout_ms: global.request_timeout_ms,
        fetch_retries: global.fetch_retries,
        retry_backoff_ms: global.retry_backoff_ms,
        shared_queue: global.shared_queue,
        pg_url,
        redis_url,
        amqp_url,
        crawl_queue: global
            .crawl_queue
            .or_else(|| env::var("AXON_CRAWL_QUEUE").ok())
            .unwrap_or_else(|| "axon.crawl.jobs".to_string()),
        batch_queue: global
            .batch_queue
            .or_else(|| env::var("AXON_BATCH_QUEUE").ok())
            .unwrap_or_else(|| "axon.batch.jobs".to_string()),
        extract_queue: global
            .extract_queue
            .or_else(|| env::var("AXON_EXTRACT_QUEUE").ok())
            .unwrap_or_else(|| "axon.extract.jobs".to_string()),
        embed_queue: global
            .embed_queue
            .or_else(|| env::var("AXON_EMBED_QUEUE").ok())
            .unwrap_or_else(|| "axon.embed.jobs".to_string()),
        tei_url: global
            .tei_url
            .or_else(|| env::var("TEI_URL").ok())
            .unwrap_or_default(),
        qdrant_url: global
            .qdrant_url
            .or_else(|| env::var("QDRANT_URL").ok())
            .map(normalize_local_service_url)
            .unwrap_or_else(|| "http://127.0.0.1:53333".to_string()),
        openai_base_url: global
            .openai_base_url
            .or_else(|| env::var("OPENAI_BASE_URL").ok())
            .unwrap_or_default(),
        openai_api_key: global
            .openai_api_key
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .unwrap_or_default(),
        openai_model: global
            .openai_model
            .or_else(|| env::var("OPENAI_MODEL").ok())
            .unwrap_or_default(),
        ask_diagnostics,
        ask_max_context_chars: env_usize_clamped(
            "AXON_ASK_MAX_CONTEXT_CHARS",
            120_000,
            20_000,
            400_000,
        ),
        ask_candidate_limit: env_usize_clamped("AXON_ASK_CANDIDATE_LIMIT", 64, 8, 200),
        ask_chunk_limit: env_usize_clamped("AXON_ASK_CHUNK_LIMIT", 10, 3, 40),
        ask_full_docs: env_usize_clamped("AXON_ASK_FULL_DOCS", 4, 1, 20),
        ask_backfill_chunks: env_usize_clamped("AXON_ASK_BACKFILL_CHUNKS", 3, 0, 20),
        ask_doc_fetch_concurrency: env_usize_clamped("AXON_ASK_DOC_FETCH_CONCURRENCY", 4, 1, 16),
        ask_doc_chunk_limit: env_usize_clamped("AXON_ASK_DOC_CHUNK_LIMIT", 192, 8, 2000),
        ask_min_relevance_score: env_f64_clamped("AXON_ASK_MIN_RELEVANCE_SCORE", 0.0, -1.0, 2.0),
        cron_every_seconds: global.cron_every_seconds.filter(|value| *value > 0),
        cron_max_runs: global.cron_max_runs.filter(|value| *value > 0),
        watchdog_stale_timeout_secs: global.watchdog_stale_timeout_secs.max(30),
        watchdog_confirm_secs: global.watchdog_confirm_secs.max(10),
        json_output: global.json,
    };

    if cfg.exclude_path_prefix.is_empty() && !normalized_excludes.disable_defaults {
        cfg.exclude_path_prefix = default_exclude_prefixes();
    }

    let (
        crawl_default,
        sitemap_default,
        backfill_default,
        timeout_default,
        retries_default,
        backoff_default,
    ) = performance_defaults(cfg.performance_profile);

    if cfg.crawl_concurrency_limit.is_none() {
        cfg.crawl_concurrency_limit = Some(crawl_default);
    }
    if cfg.sitemap_concurrency_limit.is_none() {
        cfg.sitemap_concurrency_limit = Some(sitemap_default);
    }
    if cfg.backfill_concurrency_limit.is_none() {
        cfg.backfill_concurrency_limit = Some(backfill_default);
    }
    if cfg.request_timeout_ms.is_none() {
        cfg.request_timeout_ms = Some(timeout_default);
    }
    if cfg.fetch_retries == 0 {
        cfg.fetch_retries = retries_default;
    }
    if cfg.retry_backoff_ms == 0 {
        cfg.retry_backoff_ms = backoff_default;
    }

    cfg
}

pub fn parse_args() -> Config {
    maybe_print_top_level_help_and_exit();
    let cli = Cli::parse();
    into_config(cli)
}

fn maybe_print_top_level_help_and_exit() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 && matches!(args[1].as_str(), "-h" | "--help" | "help") {
        print_top_level_help();
        process::exit(0);
    }
}

fn print_top_level_help() {
    let colors_enabled = env::var("AXON_NO_COLOR").is_err();
    let colorize = |code: &str, text: &str| {
        if colors_enabled {
            format!("{code}{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    let bold = |text: &str| {
        if colors_enabled {
            format!("\x1b[1m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    };
    let dim = |text: &str| colorize("\x1b[2m", text);

    // Match Axon's theme.ts palette.
    let primary = "\x1b[38;2;244;143;177m"; // #F48FB1
    let accent = "\x1b[38;2;144;202;249m"; // #90CAF9

    let title = bold(&colorize(primary, "AXON CLI"));
    let divider = colorize(primary, "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    let section = |name: &str| bold(&colorize(primary, name));
    let cmd = |name: &str| colorize(accent, name);
    let bin_name = env::args()
        .next()
        .and_then(|p| {
            std::path::Path::new(&p)
                .file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_string())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "axon".to_string());

    println!("  {title}");
    println!("  {divider}");
    println!(
        "  Version {}  |  {}",
        env!("CARGO_PKG_VERSION"),
        dim("Spider-powered web and local RAG CLI")
    );
    println!();
    println!("  {}", section("Usage"));
    println!("  {}", cmd(&format!("[{bin_name} [options] [command]]")));
    println!();
    println!("  {}", section("Quick Start"));
    println!(
        "  {}",
        dim(&format!(
            "{bin_name} scrape https://example.com --wait true --embed false"
        ))
    );
    println!(
        "  {}",
        dim(&format!(
            "{bin_name} crawl https://docs.rs/spider --wait false"
        ))
    );
    println!(
        "  {}",
        dim(&format!(
            "{bin_name} query \"embedding pipeline\" --collection cortex"
        ))
    );
    println!();
    println!("  {}", section("Global Options"));
    println!("  {:<28} {}", cmd("-h, --help"), dim("display help"));
    println!(
        "  {:<28} {}",
        cmd("--wait <bool>"),
        dim("run synchronously (default false)")
    );
    println!(
        "  {:<28} {}",
        cmd("--collection <name>"),
        dim("vector collection (default cortex)")
    );
    println!(
        "  {:<28} {}",
        cmd("--embed <bool>"),
        dim("run embedding where applicable")
    );
    println!(
        "  {:<28} {}",
        cmd("--cache <bool>"),
        dim("reuse prior crawl artifacts when possible")
    );
    println!(
        "  {:<28} {}",
        cmd("--cache-skip-browser <bool>"),
        dim("force HTTP crawl path when cache flow is enabled")
    );
    println!(
        "  {:<28} {}",
        cmd("--max-pages <n>"),
        dim("crawl page limit (0 = uncapped)")
    );
    println!(
        "  {:<28} {}",
        cmd("--url-glob <pattern[,..]>"),
        dim("expand URL seeds via brace globs (e.g. {1..10}, {a,b})")
    );
    println!(
        "  {:<28} {}",
        cmd("--cron-every-seconds <n>"),
        dim("repeat command every n seconds")
    );
    println!(
        "  {:<28} {}",
        cmd("--cron-max-runs <n>"),
        dim("stop cron loop after n runs")
    );
    println!("  {:<28} {}", cmd("--max-depth <n>"), dim("crawl depth"));
    println!(
        "  {:<28} {}",
        cmd("--output-dir <dir>"),
        dim("output directory")
    );
    println!();
    println!("  {}", section("Core Web Operations"));
    println!("  {:<28} {}", cmd("scrape [url]"), dim("Scrape a URL"));
    println!("  {:<28} {}", cmd("crawl [url]"), dim("Crawl a website"));
    println!(
        "  {:<28} {}",
        cmd("map [url]"),
        dim("Map URLs on a website")
    );
    println!(
        "  {:<28} {}",
        cmd("search <query>"),
        dim("Search web results")
    );
    println!(
        "  {:<28} {}",
        cmd("extract [urls...]"),
        dim("Extract structured data")
    );
    println!(
        "  {:<28} {}",
        cmd("batch [urls...]"),
        dim("Batch scrape multiple URLs")
    );
    println!();
    println!("  {}", section("Vector Search"));
    println!(
        "  {:<28} {}",
        cmd("embed [input]"),
        dim("Embed content into Qdrant")
    );
    println!(
        "  {:<28} {}",
        cmd("query <query>"),
        dim("Semantic vector search")
    );
    println!(
        "  {:<28} {}",
        cmd("retrieve <url-or-path>"),
        dim("Retrieve stored document")
    );
    println!(
        "  {:<28} {}",
        cmd("ask <query>"),
        dim("Ask over embedded documents")
    );
    println!(
        "  {:<28} {}",
        cmd("suggest [focus]"),
        dim("Suggest new docs URLs to crawl")
    );
    println!("  {:<28} {}", cmd("sources"), dim("List indexed sources"));
    println!("  {:<28} {}", cmd("domains"), dim("List indexed domains"));
    println!("  {:<28} {}", cmd("stats"), dim("Show vector statistics"));
    println!();
    println!("  {}", section("Jobs & Diagnostics"));
    println!("  {:<28} {}", cmd("status"), dim("Show queued job status"));
    println!(
        "  {:<28} {}",
        cmd("debug [context]"),
        dim("LLM-assisted stack troubleshooting")
    );
    println!("  {:<28} {}", cmd("doctor"), dim("Run local diagnostics"));
    println!();
    println!(
        "  {}",
        dim(&format!(
            "→ Run {bin_name} <command> --help for command-specific flags"
        ))
    );
}

#[cfg(test)]
mod tests {
    use super::normalize_exclude_prefixes;

    #[test]
    fn normalize_exclude_prefixes_none_disables_defaults() {
        let normalized = normalize_exclude_prefixes(vec!["none".to_string()]);
        assert!(normalized.disable_defaults);
        assert!(normalized.prefixes.is_empty());
    }

    #[test]
    fn normalize_exclude_prefixes_none_with_values_still_disables() {
        let normalized = normalize_exclude_prefixes(vec!["none".to_string(), "/fr".to_string()]);
        assert!(normalized.disable_defaults);
        assert!(normalized.prefixes.is_empty());
    }
}
