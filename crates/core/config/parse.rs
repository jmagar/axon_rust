pub mod excludes;
mod performance;

use super::cli::{Cli, CliCommand, JobSubcommand};
use super::help::maybe_print_top_level_help_and_exit;
use super::types::{CommandKind, Config, RedditSort, RedditTime};
use clap::Parser;
use spider::url::Url;
use std::env;

/// Mapping from Docker-internal service hostnames to their host-side addresses.
///
/// These names only resolve within the Docker container network.  Outside Docker
/// (i.e. when `/.dockerenv` does not exist) each entry is rewritten to the
/// corresponding `localhost:PORT` so the host CLI can reach the service.
const HOST_MAP: &[(&str, &str, u16)] = &[
    ("axon-postgres", "127.0.0.1", 53432),
    ("axon-redis", "127.0.0.1", 53379),
    ("axon-rabbitmq", "127.0.0.1", 45535),
    ("axon-qdrant", "127.0.0.1", 53333),
    ("axon-chrome", "127.0.0.1", 6000),
    ("axon-webdriver", "127.0.0.1", 4444),
];

/// Returns `true` if `host` is a known Docker-internal service hostname.
///
/// These hostnames only resolve inside the Docker container network; outside
/// Docker they must be mapped to `127.0.0.1`.  Used by CDP URL normalisation
/// to rewrite WebSocket connection URLs returned by `headless_browser`.
pub(crate) fn is_docker_service_host(host: &str) -> bool {
    HOST_MAP.iter().any(|(h, _, _)| *h == host)
}

pub(crate) fn normalize_local_service_url(url: String) -> String {
    if std::path::Path::new("/.dockerenv").exists() {
        return url;
    }

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

fn into_config(cli: Cli) -> Result<Config, String> {
    let global = cli.global;
    let fetch_retries_was_set = global.fetch_retries.is_some();
    let retry_backoff_was_set = global.retry_backoff_ms.is_some();

    let mut ask_diagnostics = false;
    let mut github_include_source = false;
    let mut reddit_sort = RedditSort::Hot;
    let mut reddit_time = RedditTime::Day;
    let mut reddit_max_posts = 25usize;
    let mut reddit_min_score = 0i32;
    let mut reddit_depth = 2usize;
    let mut reddit_scrape_links = false;
    let mut sessions_claude = false;
    let mut sessions_codex = false;
    let mut sessions_gemini = false;
    let mut sessions_project = None;
    let (command, positional) = match cli.command {
        CliCommand::Scrape(args) => (CommandKind::Scrape, args.positional_urls),
        CliCommand::Crawl(args) => (
            CommandKind::Crawl,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.positional_urls
            },
        ),
        CliCommand::Map(args) => (
            CommandKind::Map,
            args.value.into_iter().collect::<Vec<String>>(),
        ),
        CliCommand::Extract(args) => (
            CommandKind::Extract,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                args.positional_urls
            },
        ),
        CliCommand::Search(args) => (CommandKind::Search, args.value),
        CliCommand::Research(args) => (CommandKind::Research, args.value),
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
        CliCommand::Ingest(args) => (
            CommandKind::Ingest,
            if let Some(job) = args.job {
                positional_from_job(job)
            } else {
                Vec::new()
            },
        ),
        CliCommand::Reddit(args) => {
            reddit_sort = args.sort;
            reddit_time = args.time;
            reddit_max_posts = args.max_posts;
            reddit_min_score = args.min_score;
            reddit_depth = args.depth;
            reddit_scrape_links = args.scrape_links;
            (
                CommandKind::Reddit,
                if let Some(job) = args.job {
                    positional_from_job(job)
                } else {
                    args.target.into_iter().collect()
                },
            )
        }
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
            .ok_or_else(|| {
                "AXON_PG_URL environment variable is required (or pass --pg-url). Copy .env.example to .env and fill in credentials.".to_string()
            })?,
    );

    let redis_url = normalize_local_service_url(
        global
            .redis_url
            .or_else(|| env::var("AXON_REDIS_URL").ok())
            .ok_or_else(|| {
                "AXON_REDIS_URL environment variable is required (or pass --redis-url). Copy .env.example to .env and fill in credentials.".to_string()
            })?,
    );

    let amqp_url = normalize_local_service_url(
        global
            .amqp_url
            .or_else(|| env::var("AXON_AMQP_URL").ok())
            .ok_or_else(|| {
                "AXON_AMQP_URL environment variable is required (or pass --amqp-url). Copy .env.example to .env and fill in credentials.".to_string()
            })?,
    );

    let mut crawl_concurrency_limit = global.crawl_concurrency_limit;
    let mut backfill_concurrency_limit = global.backfill_concurrency_limit;

    if let Some(limit) = global.concurrency_limit {
        crawl_concurrency_limit = Some(limit);
        backfill_concurrency_limit = Some(limit);
    }

    let normalized_excludes = excludes::normalize_exclude_prefixes(global.exclude_path_prefix);

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
            .or_else(|| env::var("AXON_CHROME_REMOTE_URL").ok())
            .map(normalize_local_service_url),
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
        backfill_concurrency_limit,
        sitemap_only: global.sitemap_only,
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
        reddit_sort,
        reddit_time,
        reddit_max_posts,
        reddit_min_score,
        reddit_depth,
        reddit_scrape_links,
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
        tavily_api_key: env::var("TAVILY_API_KEY").ok().unwrap_or_default(),
        ask_diagnostics,
        ask_max_context_chars: performance::env_usize_clamped(
            "AXON_ASK_MAX_CONTEXT_CHARS",
            120_000,
            20_000,
            400_000,
        ),
        ask_candidate_limit: performance::env_usize_clamped("AXON_ASK_CANDIDATE_LIMIT", 64, 8, 200),
        ask_chunk_limit: performance::env_usize_clamped("AXON_ASK_CHUNK_LIMIT", 10, 3, 40),
        ask_full_docs: performance::env_usize_clamped("AXON_ASK_FULL_DOCS", 4, 1, 20),
        ask_backfill_chunks: performance::env_usize_clamped("AXON_ASK_BACKFILL_CHUNKS", 3, 0, 20),
        ask_doc_fetch_concurrency: performance::env_usize_clamped(
            "AXON_ASK_DOC_FETCH_CONCURRENCY",
            4,
            1,
            16,
        ),
        ask_doc_chunk_limit: performance::env_usize_clamped(
            "AXON_ASK_DOC_CHUNK_LIMIT",
            192,
            8,
            2000,
        ),
        ask_min_relevance_score: performance::env_f64_clamped(
            "AXON_ASK_MIN_RELEVANCE_SCORE",
            0.45,
            -1.0,
            2.0,
        ),
        cron_every_seconds: global.cron_every_seconds.filter(|value| *value > 0),
        cron_max_runs: global.cron_max_runs.filter(|value| *value > 0),
        watchdog_stale_timeout_secs: global.watchdog_stale_timeout_secs.max(30),
        watchdog_confirm_secs: global.watchdog_confirm_secs.max(10),
        json_output: global.json,
        crawl_from_result: global.crawl_from_result,
        normalize: global.normalize,
        chrome_network_idle_timeout_secs: global.chrome_network_idle_timeout,
        auto_switch_thin_ratio: global.auto_switch_thin_ratio,
        auto_switch_min_pages: global.auto_switch_min_pages,
        crawl_broadcast_buffer_min: 4096, // placeholder — overwritten below from profile
        crawl_broadcast_buffer_max: 16_384, // placeholder — overwritten below from profile
        url_whitelist: global.url_whitelist,
        block_assets: global.block_assets,
        max_page_bytes: if global.max_page_bytes == 0 {
            None
        } else {
            Some(global.max_page_bytes)
        },
        redirect_policy_strict: global.redirect_policy_strict,
        chrome_wait_for_selector: global.chrome_wait_for_selector,
        chrome_screenshot: global.chrome_screenshot,
        research_depth: global.research_depth,
        search_time_range: global.search_time_range,
        bypass_csp: global.bypass_csp,
        accept_invalid_certs: global.accept_invalid_certs,
    };

    if cfg.exclude_path_prefix.is_empty() && !normalized_excludes.disable_defaults {
        cfg.exclude_path_prefix = excludes::default_exclude_prefixes();
    }

    let ps = performance::profile_settings(cfg.performance_profile);

    if cfg.crawl_concurrency_limit.is_none() {
        cfg.crawl_concurrency_limit = Some(ps.crawl_concurrency);
    }
    if cfg.backfill_concurrency_limit.is_none() {
        cfg.backfill_concurrency_limit = Some(ps.backfill_concurrency);
    }
    if cfg.request_timeout_ms.is_none() {
        cfg.request_timeout_ms = Some(ps.request_timeout_ms);
    }
    if !fetch_retries_was_set {
        cfg.fetch_retries = ps.fetch_retries;
    }
    if !retry_backoff_was_set {
        cfg.retry_backoff_ms = ps.retry_backoff_ms;
    }
    cfg.crawl_broadcast_buffer_min = ps.broadcast_buffer_min;
    cfg.crawl_broadcast_buffer_max = ps.broadcast_buffer_max;

    Ok(cfg)
}

pub fn parse_args() -> Config {
    maybe_print_top_level_help_and_exit();
    let cli = Cli::parse();
    match into_config(cli) {
        Ok(cfg) => cfg,
        Err(msg) => {
            eprintln!("error: {msg}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::is_docker_service_host;
    use std::env;
    use std::sync::Mutex;

    /// Serializes tests that mutate process-wide environment variables.
    /// Prevents parallel test data races on `std::env::set_var` / `remove_var`.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // --- is_docker_service_host tests ---

    #[test]
    fn test_is_docker_service_host_recognizes_all_known_services() {
        assert!(is_docker_service_host("axon-postgres"));
        assert!(is_docker_service_host("axon-redis"));
        assert!(is_docker_service_host("axon-rabbitmq"));
        assert!(is_docker_service_host("axon-qdrant"));
        assert!(is_docker_service_host("axon-chrome"));
        assert!(is_docker_service_host("axon-webdriver"));
    }

    #[test]
    fn test_is_docker_service_host_rejects_unknown_hyphenated_hosts() {
        // These look like Docker-style names but are NOT in HOST_MAP.
        assert!(!is_docker_service_host("my-home-server"));
        assert!(!is_docker_service_host("custom-chrome-host"));
        assert!(!is_docker_service_host("prod-infra"));
        assert!(!is_docker_service_host("axon-unknown"));
    }

    #[test]
    fn test_is_docker_service_host_rejects_plain_hosts() {
        assert!(!is_docker_service_host("localhost"));
        assert!(!is_docker_service_host("127.0.0.1"));
        assert!(!is_docker_service_host("example.com"));
        assert!(!is_docker_service_host(""));
    }

    #[allow(unsafe_code)]
    #[test]
    fn test_tavily_api_key_read_from_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        const VAR: &str = "AXON_TEST_TAVILY_KEY_PRESENT";
        // SAFETY: guarded by ENV_LOCK; no other test mutates this var concurrently.
        unsafe { env::set_var(VAR, "test-key-123") };
        let key = env::var(VAR).ok().unwrap_or_default();
        assert_eq!(key, "test-key-123");
        unsafe { env::remove_var(VAR) };
    }

    #[test]
    fn test_tavily_api_key_defaults_to_empty_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        const VAR: &str = "AXON_TEST_TAVILY_KEY_ABSENT";
        // This var is never set anywhere, so it should always be absent.
        let key = env::var(VAR).ok().unwrap_or_default();
        assert_eq!(key, "");
    }

    // --- exclude prefix disable-by-empty tests ---

    #[test]
    fn test_empty_string_disables_default_exclude_prefixes() {
        // Passing "" should set `disable_defaults = true`, suppressing the
        // built-in locale-prefix exclusions without adding any custom prefixes.
        let normalized = super::excludes::normalize_exclude_prefixes(vec!["".to_string()]);
        assert!(
            normalized.disable_defaults,
            "empty string should set disable_defaults = true"
        );
        assert!(
            normalized.prefixes.is_empty(),
            "empty string should not produce any prefix entries"
        );
    }

    #[test]
    fn test_slash_disables_default_exclude_prefixes() {
        // "/" is treated identically to "" — it disables default exclusions.
        let normalized = super::excludes::normalize_exclude_prefixes(vec!["/".to_string()]);
        assert!(
            normalized.disable_defaults,
            "bare slash should set disable_defaults = true"
        );
        assert!(
            normalized.prefixes.is_empty(),
            "bare slash should not produce any prefix entries"
        );
    }
}
