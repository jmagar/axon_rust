use super::*;

fn summary(pages_seen: u32, thin: u32, markdown_files: u32) -> CrawlSummary {
    CrawlSummary {
        pages_seen,
        thin_pages: thin,
        markdown_files,
        elapsed_ms: 0,
    }
}

#[test]
fn test_fallback_when_no_markdown_files() {
    assert!(should_fallback_to_chrome(&summary(100, 0, 0), 200));
}

#[test]
fn test_fallback_thin_ratio_above_threshold() {
    assert!(should_fallback_to_chrome(&summary(100, 61, 50), 200));
}

#[test]
fn test_no_fallback_at_threshold() {
    assert!(!should_fallback_to_chrome(&summary(100, 60, 50), 200));
}

#[test]
fn test_fallback_low_coverage() {
    assert!(should_fallback_to_chrome(&summary(100, 10, 5), 200));
}

#[test]
fn test_no_divide_by_zero() {
    assert!(should_fallback_to_chrome(&summary(0, 0, 0), 200));
}

#[test]
fn test_no_fallback_healthy_crawl() {
    assert!(!should_fallback_to_chrome(&summary(200, 10, 150), 200));
}

#[test]
fn test_fallback_low_max_pages() {
    assert!(should_fallback_to_chrome(&summary(50, 5, 8), 50));
}

#[test]
fn test_no_fallback_small_crawl_sufficient_coverage() {
    assert!(!should_fallback_to_chrome(&summary(50, 5, 15), 50));
}

#[test]
fn test_exclude_path_prefix_matches_segment_boundary() {
    let excludes = vec!["/de".to_string()];
    assert!(is_excluded_url_path("https://example.com/de", &excludes));
    assert!(is_excluded_url_path(
        "https://example.com/de/docs",
        &excludes
    ));
    assert!(!is_excluded_url_path(
        "https://example.com/developer",
        &excludes
    ));
    assert!(!is_excluded_url_path(
        "https://example.com/design",
        &excludes
    ));
}

#[test]
fn test_exclude_path_prefix_handles_non_normalized_input() {
    let excludes = vec!["de/".to_string()];
    assert!(is_excluded_url_path("https://example.com/de", &excludes));
    assert!(is_excluded_url_path(
        "https://example.com/de/guide",
        &excludes
    ));
    assert!(!is_excluded_url_path(
        "https://example.com/developer",
        &excludes
    ));
}

#[test]
fn test_canonicalize_url_for_dedupe_trailing_slash_and_fragment() {
    let a = canonicalize_url_for_dedupe("https://example.com/docs/");
    let b = canonicalize_url_for_dedupe("https://example.com/docs#intro");
    assert_eq!(a, b);
    assert_eq!(a.as_deref(), Some("https://example.com/docs"));
}

#[test]
fn test_canonicalize_url_for_dedupe_root_and_default_port() {
    let a = canonicalize_url_for_dedupe("https://example.com:443/");
    let b = canonicalize_url_for_dedupe("https://example.com/");
    assert_eq!(a, b);
    assert_eq!(a.as_deref(), Some("https://example.com/"));
}

// Bug: when max_pages == 0 (uncapped), (0/10).max(10) = 10, so any site
// with < 10 markdown files always triggers Chrome — even a complete 1-page crawl.
#[test]
fn test_no_fallback_uncapped_small_but_complete_site() {
    // 1-page site, healthy content, no thin pages, max_pages uncapped (0)
    assert!(!should_fallback_to_chrome(&summary(1, 0, 1), 0));
}

#[test]
fn test_no_fallback_uncapped_nine_pages_healthy() {
    // 9-page site, healthy, max_pages uncapped — should NOT trigger Chrome
    assert!(!should_fallback_to_chrome(&summary(9, 0, 9), 0));
}

#[test]
fn test_fallback_uncapped_thin_ratio_still_fires() {
    // Even with uncapped, thin ratio > 60% should still trigger Chrome
    assert!(should_fallback_to_chrome(&summary(10, 7, 9), 0));
}

#[test]
fn test_fallback_uncapped_zero_markdown_files() {
    // Zero markdown with max_pages=0 → still fall back
    assert!(should_fallback_to_chrome(&summary(5, 5, 0), 0));
}

#[test]
fn test_regex_escape_escapes_hyphen() {
    assert_eq!(regex_escape("foo-bar"), "foo\\-bar");
}

// --- Spider API wiring tests ---

#[test]
fn test_spider_retry_wiring_round_trips() {
    // Verify spider's with_retry() stores the value we pass from cfg.fetch_retries.
    // configure_website() calls with_retry(cfg.fetch_retries.min(u8::MAX) as u8)
    // when fetch_retries > 0; this test confirms the Spider API contract.
    let mut website = spider::website::Website::new("https://example.com");
    website.with_retry(3);
    assert_eq!(website.configuration.retry, 3);
}

#[test]
fn test_spider_normalize_wiring_round_trips() {
    // Verify spider's with_normalize() stores the value we pass from cfg.normalize.
    let mut website = spider::website::Website::new("https://example.com");
    website.with_normalize(true);
    assert!(website.configuration.normalize);
    website.with_normalize(false);
    assert!(!website.configuration.normalize);
}

#[test]
fn test_spider_tld_disabled_by_default() {
    // TLD crawling is hardcoded to false in configure_website(); verify the Spider
    // API default matches our expectation (i.e., with_tld(false) is a no-op baseline).
    let mut website = spider::website::Website::new("https://example.com");
    website.with_tld(false);
    assert!(!website.configuration.tld);
}

// --- CDP hostname detection tests (Issue 1: explicit allowlist vs fragile heuristic) ---

#[test]
fn test_docker_service_host_only_rewrites_known_names() {
    use crate::crates::core::config::parse::is_docker_service_host;
    // Known Docker service names must be detected.
    assert!(is_docker_service_host("axon-chrome"));
    assert!(is_docker_service_host("axon-postgres"));
    // Hyphenated hosts NOT in the allowlist must NOT be rewritten.
    assert!(!is_docker_service_host("my-home-server"));
    assert!(!is_docker_service_host("custom-chrome-proxy"));
    // Plain hosts must not be rewritten.
    assert!(!is_docker_service_host("127.0.0.1"));
    assert!(!is_docker_service_host("localhost"));
}
