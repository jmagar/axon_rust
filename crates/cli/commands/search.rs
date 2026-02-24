use crate::crates::core::config::Config;
use crate::crates::core::http::validate_url;
use crate::crates::core::logging::{log_done, log_info, log_warn};
use crate::crates::core::ui::{muted, primary, print_phase};
use crate::crates::jobs::crawl::start_crawl_jobs_batch;
use spider::url::Url as SpiderUrl;
use spider_agent::{Agent, SearchOptions, TimeRange};
use std::collections::HashSet;
use std::error::Error;

/// Domains that have dedicated ingest handlers or are unsuitable for generic crawling.
/// Search results from these hosts are displayed but never queued as crawl jobs.
const CRAWL_SKIP_HOSTS: &[&str] = &[
    "reddit.com",
    "www.reddit.com",
    "youtube.com",
    "www.youtube.com",
    "youtu.be",
    "github.com",
    "www.github.com",
];

/// Extract the crawl seed URL from a search result URL.
///
/// By default (`from_result = false`), strips to the scheme+host+port origin so all
/// results from the same domain produce a single crawl job. When `from_result = true`,
/// returns the exact result URL so the crawl starts from that specific page.
///
/// Returns `None` if `url` cannot be parsed, has a non-http/https scheme, or belongs
/// to a domain with a dedicated ingest handler (see `CRAWL_SKIP_HOSTS`).
pub fn extract_crawl_seed(url: &str, from_result: bool) -> Option<String> {
    let parsed = SpiderUrl::parse(url).ok()?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return None,
    }
    let host = parsed.host_str()?;
    if CRAWL_SKIP_HOSTS.contains(&host) {
        return None;
    }
    if from_result {
        return Some(url.to_string());
    }
    let origin = match parsed.port() {
        Some(port) => format!("{}://{}:{}", parsed.scheme(), host, port),
        None => format!("{}://{}", parsed.scheme(), host),
    };
    Some(origin)
}

pub async fn run_search(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.tavily_api_key.is_empty() {
        return Err("search requires TAVILY_API_KEY — set it in .env".into());
    }

    let query = if let Some(q) = &cfg.query {
        q.clone()
    } else if !cfg.positional.is_empty() {
        cfg.positional.join(" ")
    } else {
        return Err("search requires a query (positional or --query)".into());
    };

    print_phase("◐", "Searching", &query);

    let agent = Agent::builder()
        .with_search_tavily(&cfg.tavily_api_key)
        .build()?;

    let mut search_opts = SearchOptions::new().with_limit(cfg.search_limit);
    if let Some(ref range) = cfg.search_time_range {
        let tr = match range.as_str() {
            "day" => Some(TimeRange::Day),
            "week" => Some(TimeRange::Week),
            "month" => Some(TimeRange::Month),
            "year" => Some(TimeRange::Year),
            other => {
                log_warn(&format!("Unknown search_time_range '{other}'; ignoring"));
                None
            }
        };
        if let Some(tr) = tr {
            search_opts = search_opts.with_time_range(tr);
        }
    }

    let results = agent.search_with_options(&query, search_opts).await?;

    println!("{}", primary(&format!("Search Results for \"{}\"", query)));
    println!("{} {}", muted("Found"), results.results.len());
    println!();

    for result in &results.results {
        println!("{}. {}", result.position, primary(&result.title));
        println!("   {}", muted(&result.url));
        if let Some(ref snippet) = result.snippet {
            println!("   {snippet}");
        }
        println!();
    }

    // Deduplicate seeds — one crawl job per unique origin (or exact URL with --crawl-from-result).
    // URLs from dedicated-ingest domains (reddit, youtube, github) are skipped;
    // use `axon reddit`, `axon youtube`, or `axon github` to ingest those.
    let seeds: HashSet<String> = results
        .results
        .iter()
        .filter_map(|r| extract_crawl_seed(&r.url, cfg.crawl_from_result))
        .collect();

    if !seeds.is_empty() {
        // Validate seeds before handing to the batch path so blocked/private
        // URLs are dropped with a warning rather than surfacing a DB error.
        let valid_seeds: Vec<&str> = seeds
            .iter()
            .filter(|s| {
                if let Err(e) = validate_url(s) {
                    log_warn(&format!("Skipping blocked seed {s}: {e}"));
                    false
                } else {
                    true
                }
            })
            .map(|s| s.as_str())
            .collect();

        if !valid_seeds.is_empty() {
            // Single Postgres pool + single AMQP connection for all seeds.
            match start_crawl_jobs_batch(cfg, &valid_seeds).await {
                Ok(pairs) => {
                    let ids: Vec<String> = pairs.iter().map(|(_, id)| id.to_string()).collect();
                    log_info(&format!(
                        "Queued {} crawl job(s): {}",
                        ids.len(),
                        ids.join(", ")
                    ));
                }
                Err(e) => log_warn(&format!("Failed to batch-queue crawl jobs: {e}")),
            }
        }
    }

    log_done("command=search complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::core::config::CommandKind;
    use crate::crates::jobs::common::test_config;

    // --- extract_crawl_seed unit tests (pure logic, no I/O) ---

    #[test]
    fn test_extract_crawl_seed_strips_to_origin() {
        let seed = extract_crawl_seed(
            "https://docs.rust-lang.org/book/ch01-00-getting-started.html",
            false,
        );
        assert_eq!(seed, Some("https://docs.rust-lang.org".to_string()));
    }

    #[test]
    fn test_extract_crawl_seed_preserves_non_default_port() {
        let seed = extract_crawl_seed("https://myhost.example.com:8443/api/v1/docs", false);
        assert_eq!(seed, Some("https://myhost.example.com:8443".to_string()));
    }

    #[test]
    fn test_extract_crawl_seed_strips_deep_path() {
        let seed = extract_crawl_seed("https://crates.io/crates/tokio/0.2.22/deps", false);
        assert_eq!(seed, Some("https://crates.io".to_string()));
    }

    #[test]
    fn test_extract_crawl_seed_from_result_returns_exact_url() {
        let url = "https://blog.rust-lang.org/2024/05/02/Rust-1.78.0.html";
        let seed = extract_crawl_seed(url, true);
        assert_eq!(seed, Some(url.to_string()));
    }

    #[test]
    fn test_extract_crawl_seed_unparseable_returns_none() {
        let seed = extract_crawl_seed("not a url %%%", false);
        assert_eq!(seed, None);
    }

    #[test]
    fn test_extract_crawl_seed_rejects_non_http_scheme() {
        assert_eq!(
            extract_crawl_seed("ftp://example.com/file.tar.gz", false),
            None
        );
        assert_eq!(extract_crawl_seed("file:///etc/passwd", true), None);
    }

    #[test]
    fn test_extract_crawl_seed_deduplicates_same_domain() {
        let urls = [
            "https://docs.example.com/en/stable/guide/intro.html",
            "https://docs.example.com/en/stable/api/index.html",
            "https://docs.example.com/changelog",
        ];
        let seeds: HashSet<String> = urls
            .iter()
            .filter_map(|u| extract_crawl_seed(u, false))
            .collect();
        assert_eq!(seeds.len(), 1);
        assert!(seeds.contains("https://docs.example.com"));
    }

    #[test]
    fn test_extract_crawl_seed_private_ip_stripped_to_origin() {
        // extract_crawl_seed itself does no SSRF filtering — it only strips to origin.
        // The validate_url guard in run_search blocks the seed before enqueue.
        let seed = extract_crawl_seed("http://10.0.0.1/internal/api", false);
        assert_eq!(seed, Some("http://10.0.0.1".to_string()));
        // Confirm validate_url rejects it (documents the guard contract).
        use crate::crates::core::http::validate_url;
        assert!(
            validate_url("http://10.0.0.1").is_err(),
            "validate_url must reject RFC-1918 seeds"
        );
    }

    #[test]
    fn test_extract_crawl_seed_skips_ingest_domains() {
        // These domains have dedicated ingest handlers — crawl seeds must be suppressed.
        let blocked = [
            "https://www.reddit.com/r/rust/comments/abc123/title/",
            "https://reddit.com/r/rust/",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            "https://youtu.be/dQw4w9WgXcQ",
            "https://github.com/rust-lang/rust/issues/12345",
            "https://www.github.com/rust-lang/",
        ];
        for url in &blocked {
            assert_eq!(
                extract_crawl_seed(url, false),
                None,
                "expected None for ingest-only domain: {url}"
            );
            // Also blocked when from_result=true
            assert_eq!(
                extract_crawl_seed(url, true),
                None,
                "expected None (from_result) for ingest-only domain: {url}"
            );
        }
    }

    fn make_search_cfg(key: &str, query: &str) -> Config {
        let mut cfg = test_config("");
        cfg.command = CommandKind::Search;
        cfg.positional = vec![query.to_string()];
        cfg.tavily_api_key = key.to_string();
        cfg
    }

    #[tokio::test]
    async fn test_run_search_rejects_empty_tavily_key() {
        let cfg = make_search_cfg("", "rust async");
        let err = run_search(&cfg).await.unwrap_err();
        assert!(
            err.to_string().contains("TAVILY_API_KEY"),
            "expected TAVILY_API_KEY error, got: {err}"
        );
    }

    #[test]
    fn search_cfg_time_range_defaults_to_none() {
        let cfg = make_search_cfg("tvly-key", "rust async");
        assert!(
            cfg.search_time_range.is_none(),
            "search_time_range should default to None"
        );
    }
}
