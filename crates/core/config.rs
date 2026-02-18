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
    Doctor,
    Query,
    Retrieve,
    Ask,
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
            Self::Doctor => "doctor",
            Self::Query => "query",
            Self::Retrieve => "retrieve",
            Self::Ask => "ask",
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
    pub query: Option<String>,
    pub search_limit: usize,
    pub max_pages: u32,
    pub max_depth: usize,
    pub include_subdomains: bool,
    pub exclude_path_prefix: Vec<String>,
    pub output_dir: PathBuf,
    pub output_path: Option<PathBuf>,
    pub render_mode: RenderMode,
    pub respect_robots: bool,
    pub min_markdown_chars: usize,
    pub drop_thin_markdown: bool,
    pub discover_sitemaps: bool,
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
    pub json_output: bool,
}

#[derive(Debug, Parser)]
#[command(name = "cortex", about = "Axon CLI (Rust + Spider.rs)")]
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
    Doctor,
    Query(TextArg),
    Retrieve(UrlArg),
    Ask(TextArg),
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

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = false)]
    respect_robots: bool,

    #[arg(global = true, long, default_value_t = 200)]
    min_markdown_chars: usize,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    drop_thin_markdown: bool,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    discover_sitemaps: bool,

    #[arg(global = true, long, value_enum, default_value_t = ScrapeFormat::Markdown)]
    format: ScrapeFormat,

    #[arg(global = true, long, default_value_t = 10)]
    limit: usize,

    #[arg(global = true, long)]
    query: Option<String>,

    #[arg(global = true, long)]
    urls: Option<String>,

    #[arg(global = true, long, action = ArgAction::Set, default_value_t = true)]
    embed: bool,

    #[arg(
        global = true,
        long,
        env = "AXON_COLLECTION",
        default_value = "spider_rust"
    )]
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

fn normalize_exclude_prefixes(input: Vec<String>) -> Vec<String> {
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
    out
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

fn into_config(cli: Cli) -> Config {
    let global = cli.global;

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
        CliCommand::Doctor => (CommandKind::Doctor, Vec::new()),
        CliCommand::Query(args) => (CommandKind::Query, args.value),
        CliCommand::Retrieve(args) => (
            CommandKind::Retrieve,
            args.value.into_iter().collect::<Vec<String>>(),
        ),
        CliCommand::Ask(args) => (CommandKind::Ask, args.value),
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

    let mut cfg = Config {
        command,
        start_url: global.start_url,
        positional,
        urls_csv: global.urls,
        query: global.query,
        search_limit: global.limit,
        max_pages: global.max_pages,
        max_depth: global.max_depth,
        include_subdomains: global.include_subdomains,
        exclude_path_prefix: normalize_exclude_prefixes(global.exclude_path_prefix),
        output_dir: global.output_dir,
        output_path: global.output,
        render_mode: global.render_mode,
        respect_robots: global.respect_robots,
        min_markdown_chars: global.min_markdown_chars,
        drop_thin_markdown: global.drop_thin_markdown,
        discover_sitemaps: global.discover_sitemaps,
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
        json_output: global.json,
    };

    if cfg.exclude_path_prefix.is_empty() {
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
    let colors_enabled = env::var("CORTEX_NO_COLOR").is_err();
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

    let title = bold(&colorize(primary, "CORTEX CLI"));
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
        .unwrap_or_else(|| "cortex".to_string());

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
            "{bin_name} query \"embedding pipeline\" --collection axon"
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
        dim("vector collection (default spider_rust)")
    );
    println!(
        "  {:<28} {}",
        cmd("--embed <bool>"),
        dim("run embedding where applicable")
    );
    println!(
        "  {:<28} {}",
        cmd("--max-pages <n>"),
        dim("crawl page limit (0 = uncapped)")
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
    println!("  {:<28} {}", cmd("sources"), dim("List indexed sources"));
    println!("  {:<28} {}", cmd("domains"), dim("List indexed domains"));
    println!("  {:<28} {}", cmd("stats"), dim("Show vector statistics"));
    println!();
    println!("  {}", section("Jobs & Diagnostics"));
    println!("  {:<28} {}", cmd("status"), dim("Show queued job status"));
    println!("  {:<28} {}", cmd("doctor"), dim("Run local diagnostics"));
    println!();
    println!(
        "  {}",
        dim(&format!(
            "→ Run {bin_name} <command> --help for command-specific flags"
        ))
    );
}
