use super::cli::{Cli, CliCommand, JobSubcommand};
use super::help::maybe_print_top_level_help_and_exit;
use super::types::{CommandKind, Config, PerformanceProfile};
use clap::Parser;
use spider::url::Url;
use std::env;

pub(crate) fn normalize_local_service_url(url: String) -> String {
    if std::path::Path::new("/.dockerenv").exists() {
        return url;
    }

    const HOST_MAP: &[(&str, &str, u16)] = &[
        ("axon-postgres", "127.0.0.1", 53432),
        ("axon-redis", "127.0.0.1", 53379),
        ("axon-rabbitmq", "127.0.0.1", 45535),
        ("axon-qdrant", "127.0.0.1", 53333),
        ("axon-webdriver", "127.0.0.1", 4444),
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
    let fetch_retries_was_set = global.fetch_retries.is_some();
    let retry_backoff_was_set = global.retry_backoff_ms.is_some();

    let mut ask_diagnostics = false;
    let mut github_include_source = false;
    let mut sessions_claude = false;
    let mut sessions_codex = false;
    let mut sessions_gemini = false;
    let mut sessions_project = None;
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
        CliCommand::Evaluate(args) => {
            ask_diagnostics = args.diagnostics;
            (CommandKind::Evaluate, args.value)
        }
        CliCommand::Suggest(args) => (CommandKind::Suggest, args.value),
        CliCommand::Sources => (CommandKind::Sources, Vec::new()),
        CliCommand::Domains => (CommandKind::Domains, Vec::new()),
        CliCommand::Stats => (CommandKind::Stats, Vec::new()),
        CliCommand::Status => (CommandKind::Status, Vec::new()),
        CliCommand::Dedupe => (CommandKind::Dedupe, Vec::new()),
        CliCommand::Github(args) => {
            github_include_source = args.include_source;
            (
                CommandKind::Github,
                if let Some(job) = args.job {
                    positional_from_job(job)
                } else {
                    args.repo.into_iter().collect()
                },
            )
        }
        CliCommand::Reddit(args) => (
            CommandKind::Reddit,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.target.into_iter().collect()
            },
        ),
        CliCommand::Youtube(args) => (
            CommandKind::Youtube,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.url.into_iter().collect()
            },
        ),
        CliCommand::Sessions(args) => {
            sessions_claude = args.claude;
            sessions_codex = args.codex;
            sessions_gemini = args.gemini;
            sessions_project = args.project;
            (
                CommandKind::Sessions,
                if let Some(job) = args.job {
                    positional_from_job(job)
                } else {
                    Vec::new()
                },
            )
        }
    };

    let pg_url = normalize_local_service_url(
        global
            .pg_url
            .or_else(|| env::var("AXON_PG_URL").ok())
            .unwrap_or_else(|| {
                eprintln!("warning: AXON_PG_URL not set — using default credentials; set it in .env for production");
                "postgresql://axon:postgres@127.0.0.1:53432/axon".to_string()
            }),
    );

    let redis_url = normalize_local_service_url(
        global
            .redis_url
            .or_else(|| env::var("AXON_REDIS_URL").ok())
            .unwrap_or_else(|| {
                eprintln!("warning: AXON_REDIS_URL not set — using default; set it in .env for production");
                "redis://127.0.0.1:53379".to_string()
            }),
    );

    let amqp_url = normalize_local_service_url(
        global
            .amqp_url
            .or_else(|| env::var("AXON_AMQP_URL").ok())
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
            .map(normalize_local_service_url),
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
        fetch_retries: global.fetch_retries.unwrap_or(0),
        retry_backoff_ms: global.retry_backoff_ms.unwrap_or(0),
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
        ingest_queue: global
            .ingest_queue
            .or_else(|| env::var("AXON_INGEST_QUEUE").ok())
            .unwrap_or_else(|| "axon.ingest.jobs".to_string()),
        sessions_claude,
        sessions_codex,
        sessions_gemini,
        sessions_project,
        github_token: env::var("GITHUB_TOKEN").ok(),
        github_include_source,
        reddit_client_id: env::var("REDDIT_CLIENT_ID").ok(),
        reddit_client_secret: env::var("REDDIT_CLIENT_SECRET").ok(),
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
        ask_min_relevance_score: env_f64_clamped("AXON_ASK_MIN_RELEVANCE_SCORE", 0.45, -1.0, 2.0),
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
    if !fetch_retries_was_set {
        cfg.fetch_retries = retries_default;
    }
    if !retry_backoff_was_set {
        cfg.retry_backoff_ms = backoff_default;
    }

    cfg
}

pub fn parse_args() -> Config {
    maybe_print_top_level_help_and_exit();
    let cli = Cli::parse();
    into_config(cli)
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
