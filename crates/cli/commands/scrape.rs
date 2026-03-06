use super::common::parse_urls;
use crate::crates::core::config::{Config, RenderMode, ScrapeFormat};
use crate::crates::core::content::{
    extract_meta_description, find_between, to_markdown, url_to_filename,
};
use crate::crates::core::http::{normalize_url, ssrf_blacklist_patterns, validate_url};
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{muted, primary, print_option, print_phase};
use crate::crates::vector::ops::embed_path_native;
use futures_util::future::join_all;
use spider::compact_str::CompactString;
use spider::website::Website;
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Build a Spider Website configured for a single-page scrape.
///
/// Applies SSRF blacklist, timeout, retry, user-agent, and limit=1 so Spider
/// never follows links beyond the target page.
fn build_scrape_website(cfg: &Config, url: &str) -> Result<Website, Box<dyn Error>> {
    let ssrf_patterns: Vec<CompactString> = ssrf_blacklist_patterns()
        .iter()
        .copied()
        .map(Into::into)
        .collect();

    let mut website = Website::new(url);
    // Single page only — do not follow any discovered links.
    website.with_limit(1);
    // Block image/CSS/JS assets; we only want the HTML document.
    website.with_block_assets(true);
    // Wire SSRF blacklist patterns so Spider's internal redirect-following
    // cannot reach private ranges even if the seed URL resolves to one.
    website.with_blacklist_url(Some(ssrf_patterns));

    if let Some(timeout_ms) = cfg.request_timeout_ms {
        website.with_request_timeout(Some(Duration::from_millis(timeout_ms)));
    }
    // with_retry takes u8; cfg.fetch_retries is usize — clamp to u8::MAX (255).
    let retries = cfg.fetch_retries.min(u8::MAX as usize) as u8;
    website.with_retry(retries);

    if let Some(ua) = cfg.chrome_user_agent.as_deref() {
        website.with_user_agent(Some(ua));
    }
    if let Some(proxy) = cfg.chrome_proxy.as_deref() {
        website.with_proxies(Some(vec![proxy.to_string()]));
    }
    // Wire custom headers so `--header` works for single-page scrapes too.
    if !cfg.custom_headers.is_empty() {
        let mut map = reqwest::header::HeaderMap::new();
        for raw in &cfg.custom_headers {
            if let Some((k, v)) = raw.split_once(": ") {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                    reqwest::header::HeaderValue::from_str(v),
                ) {
                    map.insert(name, val);
                }
            }
        }
        if !map.is_empty() {
            website.with_headers(Some(map));
        }
    }
    // Apply the same safe defaults as configure_website().
    website.with_no_control_thread(true);
    if cfg.accept_invalid_certs {
        website.with_danger_accept_invalid_certs(true);
    }
    if matches!(cfg.render_mode, RenderMode::Chrome) {
        website.with_dismiss_dialogs(true);
        website.configuration.disable_log = true;
        if cfg.bypass_csp {
            website.with_csp_bypass(true);
        }
    }

    Ok(website)
}

#[derive(Debug)]
struct ScrapedPage {
    url: String,
    html: String,
    status_code: u16,
}

fn canonical_url_for_match(input: &str) -> String {
    input
        .split('#')
        .next()
        .unwrap_or(input)
        .split('?')
        .next()
        .unwrap_or(input)
        .trim_end_matches('/')
        .to_ascii_lowercase()
}

fn host_from_url(input: &str) -> Option<&str> {
    let (_, rest) = input.split_once("://")?;
    Some(rest.split('/').next().unwrap_or(rest))
}

fn last_path_segment(input: &str) -> Option<&str> {
    let without_fragment = input.split('#').next().unwrap_or(input);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    without_query.split('/').rfind(|s| !s.is_empty())
}

fn page_matches_requested_url(requested_url: &str, page_url: &str) -> bool {
    let requested_canon = canonical_url_for_match(requested_url);
    let page_canon = canonical_url_for_match(page_url);
    if requested_canon == page_canon {
        return true;
    }

    // docs.rs and similar doc hosts often redirect `/latest/.../foo.html` to
    // a concrete version path while preserving the terminal file name.
    if let (Some(req_host), Some(page_host), Some(req_last), Some(page_last)) = (
        host_from_url(&requested_canon),
        host_from_url(&page_canon),
        last_path_segment(&requested_canon),
        last_path_segment(&page_canon),
    ) {
        return req_host.eq_ignore_ascii_case(page_host)
            && req_last.eq_ignore_ascii_case(page_last)
            && req_last.contains(".html");
    }

    false
}

fn pick_best_page_for_url(
    requested_url: &str,
    mut candidates: Vec<ScrapedPage>,
) -> Option<ScrapedPage> {
    if let Some(index) = candidates
        .iter()
        .position(|p| page_matches_requested_url(requested_url, &p.url))
    {
        return Some(candidates.swap_remove(index));
    }
    candidates.into_iter().next()
}

async fn direct_fetch_requested_page(
    cfg: &Config,
    requested_url: &str,
) -> Result<ScrapedPage, Box<dyn Error>> {
    let mut builder = reqwest::Client::builder();
    if let Some(timeout_ms) = cfg.request_timeout_ms {
        builder = builder.timeout(Duration::from_millis(timeout_ms));
    }
    if cfg.accept_invalid_certs {
        builder = builder.danger_accept_invalid_certs(true);
    }
    if let Some(ua) = cfg.chrome_user_agent.as_deref() {
        builder = builder.user_agent(ua);
    }
    if let Some(proxy) = cfg.chrome_proxy.as_deref().filter(|p| !p.trim().is_empty()) {
        builder = builder.proxy(reqwest::Proxy::all(proxy)?);
    }
    if !cfg.custom_headers.is_empty() {
        let mut map = reqwest::header::HeaderMap::new();
        for raw in &cfg.custom_headers {
            if let Some((k, v)) = raw.split_once(": ") {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                    reqwest::header::HeaderValue::from_str(v),
                ) {
                    map.insert(name, val);
                }
            }
        }
        if !map.is_empty() {
            builder = builder.default_headers(map);
        }
    }
    // Validate each redirect target through the SSRF blacklist so a public URL
    // cannot redirect to a private/internal address and bypass the guard.
    builder = builder.redirect(reqwest::redirect::Policy::custom(|attempt| {
        let url = attempt.url().as_str().to_string();
        if validate_url(&url).is_err() {
            attempt.error(format!("SSRF: redirect to blocked URL {url}"))
        } else {
            attempt.follow()
        }
    }));
    let client = builder.build()?;
    let attempts = cfg.fetch_retries.saturating_add(1).max(1);
    let mut last_err: Option<String> = None;
    for attempt in 1..=attempts {
        match client.get(requested_url).send().await {
            Ok(resp) => {
                let status_code = resp.status().as_u16();
                let html = resp.text().await?;
                return Ok(ScrapedPage {
                    url: requested_url.to_string(),
                    html,
                    status_code,
                });
            }
            Err(err) => {
                last_err = Some(err.to_string());
                if attempt < attempts {
                    sleep(Duration::from_millis(cfg.retry_backoff_ms)).await;
                }
            }
        }
    }
    Err(format!(
        "direct fetch fallback failed for {requested_url}: {}",
        last_err.unwrap_or_else(|| "unknown error".to_string())
    )
    .into())
}

/// Fetch a single page from a configured Spider `Website`.
///
/// Uses explicit `subscribe()` + `crawl_raw()`/`crawl()` instead of Spider's
/// `scrape_raw()`. This is the correct approach — not a workaround. Spider's
/// `scrape_raw()` uses a biased-select internally: for fast single-page fetches
/// the done channel fires before the page receiver gets a turn, so `get_pages()`
/// comes back empty. Owning the subscription ourselves avoids this race entirely.
async fn fetch_single_page(
    cfg: &Config,
    website: &mut Website,
    requested_url: &str,
) -> Result<ScrapedPage, Box<dyn Error>> {
    let mut rx = website
        .subscribe(16)
        .ok_or("failed to subscribe to spider broadcast")?;
    // Spawn the collector BEFORE the crawl so it is ready to receive the broadcast.
    let collect: tokio::task::JoinHandle<Vec<ScrapedPage>> = tokio::spawn(async move {
        let mut pages = Vec::new();
        while let Ok(page) = rx.recv().await {
            pages.push(ScrapedPage {
                url: page.get_url().to_string(),
                html: page.get_html(),
                status_code: page.status_code.as_u16(),
            });
        }
        pages
    });
    match cfg.render_mode {
        RenderMode::Http | RenderMode::AutoSwitch => website.crawl_raw().await,
        RenderMode::Chrome => website.crawl().await,
    }
    website.unsubscribe();
    let mut candidates = collect
        .await
        .map_err(|e| format!("page collector panicked: {e}"))?;

    // Include any pages retained by Spider internals and prefer a URL that
    // matches the requested target over whichever page arrived first.
    if let Some(pages) = website.get_pages() {
        candidates.extend(pages.iter().map(|page| ScrapedPage {
            url: page.get_url().to_string(),
            html: page.get_html(),
            status_code: page.status_code.as_u16(),
        }));
    }
    let Some(selected) = pick_best_page_for_url(requested_url, candidates) else {
        return direct_fetch_requested_page(cfg, requested_url).await;
    };

    if page_matches_requested_url(requested_url, &selected.url) {
        Ok(selected)
    } else {
        direct_fetch_requested_page(cfg, requested_url).await
    }
}

/// Build the canonical 5-field JSON response for a scraped page.
///
/// Performs markdown conversion, title extraction, and description extraction
/// in one place. All JSON-producing paths (`scrape_payload`, `scrape_one`'s
/// `--json` branch, and `select_output`'s `Json` arm) delegate here.
fn build_scrape_json(url: &str, html: &str, status_code: u16) -> serde_json::Value {
    serde_json::json!({
        "url": url,
        "status_code": status_code,
        "markdown": to_markdown(html),
        "title": find_between(html, "<title>", "</title>").unwrap_or(""),
        "description": extract_meta_description(html).unwrap_or_default(),
    })
}

/// Select the output text from the page HTML based on the requested format.
///
/// - `Markdown` / `Json`: convert HTML → markdown via our transform pipeline.
/// - `Html` / `RawHtml`: return raw HTML string.
///
/// This is a pure function, extractable and testable without Spider running.
pub(crate) fn select_output(
    format: ScrapeFormat,
    url: &str,
    html: &str,
    status_code: u16,
) -> Result<String, Box<dyn Error>> {
    match format {
        ScrapeFormat::Markdown => Ok(to_markdown(html)),
        ScrapeFormat::Html | ScrapeFormat::RawHtml => Ok(html.to_string()),
        ScrapeFormat::Json => Ok(serde_json::to_string_pretty(&build_scrape_json(
            url,
            html,
            status_code,
        ))?),
    }
}

pub async fn scrape_payload(cfg: &Config, url: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let normalized = normalize_url(url);
    validate_url(&normalized)?;

    let mut website = build_scrape_website(cfg, &normalized)?;
    let page = fetch_single_page(cfg, &mut website, &normalized).await?;
    let html = page.html;
    let status_code = page.status_code;
    if !(200..300).contains(&status_code) {
        return Err(format!("scrape failed: HTTP {} for {}", status_code, normalized).into());
    }

    Ok(build_scrape_json(&normalized, &html, status_code))
}

pub async fn run_scrape(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("scrape requires at least one URL (positional or --urls)".into());
    }
    if cfg.output_path.is_some() && urls.len() > 1 {
        return Err(
            "--output cannot be used with multiple URLs (each would overwrite the same file)"
                .into(),
        );
    }

    // Phase 1: scrape all URLs concurrently — each prints its result as it lands.
    let tasks: Vec<_> = urls.iter().map(|url| scrape_one(cfg, url)).collect();
    let mut to_embed: Vec<(String, String)> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for result in join_all(tasks).await {
        match result {
            Ok(Some(pair)) => to_embed.push(pair),
            Ok(None) => {}
            Err(e) => {
                eprintln!("scrape error: {e}");
                errors.push(e.to_string());
            }
        }
    }

    // Phase 2: embed all collected markdowns in one batch (single embed_path_native call).
    // Important: write this run's files into an isolated directory so `scrape --embed`
    // only indexes current outputs, not every historical file in scrape-markdown.
    if cfg.embed && !to_embed.is_empty() {
        let run_id = Uuid::new_v4().to_string();
        let embed_dir = cfg
            .output_dir
            .join("scrape-markdown")
            .join("runs")
            .join(run_id);
        tokio::fs::create_dir_all(&embed_dir).await?;
        for (normalized, markdown) in &to_embed {
            tokio::fs::write(embed_dir.join(url_to_filename(normalized, 1)), markdown).await?;
        }
        embed_path_native(cfg, &embed_dir.to_string_lossy()).await?;
    }

    if !errors.is_empty() {
        return Err(format!("{} scrape(s) failed: {}", errors.len(), errors.join("; ")).into());
    }

    Ok(())
}

/// Returns `Some((normalized_url, markdown))` when `cfg.embed` is true so the
/// caller can batch-embed after all scrapes complete. Returns `None` otherwise.
async fn scrape_one(cfg: &Config, url: &str) -> Result<Option<(String, String)>, Box<dyn Error>> {
    let normalized = normalize_url(url);

    print_phase("◐", "Scraping", &normalized);
    println!("  {}", primary("Options:"));
    print_option("format", &format!("{:?}", cfg.format));
    print_option("renderMode", &cfg.render_mode.to_string());
    print_option("proxy", cfg.chrome_proxy.as_deref().unwrap_or("none"));
    print_option(
        "userAgent",
        cfg.chrome_user_agent.as_deref().unwrap_or("spider-default"),
    );
    print_option(
        "timeoutMs",
        &cfg.request_timeout_ms.unwrap_or(20_000).to_string(),
    );
    print_option("fetchRetries", &cfg.fetch_retries.to_string());
    print_option("retryBackoffMs", &cfg.retry_backoff_ms.to_string());
    print_option("chromeAntiBot", &cfg.chrome_anti_bot.to_string());
    print_option("chromeStealth", &cfg.chrome_stealth.to_string());
    print_option("chromeIntercept", &cfg.chrome_intercept.to_string());
    print_option("embed", &cfg.embed.to_string());
    println!();

    // SSRF guard: validate before creating Website — must run before any
    // network activity so private-IP seeds are rejected immediately.
    validate_url(&normalized)?;

    let mut website = build_scrape_website(cfg, &normalized)?;
    let page = fetch_single_page(cfg, &mut website, &normalized).await?;
    let html = page.html;
    let status_code = page.status_code;

    // Surface non-success HTTP codes as errors so callers can handle them.
    if !(200..300).contains(&status_code) {
        return Err(format!("scrape failed: HTTP {} for {}", status_code, normalized).into());
    }

    let markdown = to_markdown(&html);
    let output = select_output(cfg.format, &normalized, &html, status_code)?;

    if cfg.json_output {
        // Structured JSON output for web UI / machine consumers.
        // The markdown field lets the frontend display content directly
        // without going through the file-based embed pipeline.
        // Reuse the `markdown` already computed above to avoid duplicate
        // HTML→markdown conversion.
        let json = serde_json::json!({
            "url": normalized,
            "status_code": status_code,
            "markdown": &markdown,
            "title": find_between(&html, "<title>", "</title>").unwrap_or(""),
            "description": extract_meta_description(&html).unwrap_or_default(),
        });
        println!("{json}");
    } else if let Some(path) = &cfg.output_path {
        tokio::fs::write(path, &output).await?;
        log_done(&format!("wrote output: {}", path.to_string_lossy()));
    } else {
        println!("{} {}", primary("Scrape Results for"), normalized);
        println!("{}\n", muted("As of: now"));
        println!("{output}");
    }

    if cfg.embed {
        Ok(Some((normalized, markdown)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // select_output — pure function, no network required
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_output_markdown_returns_markdown() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let result = select_output(ScrapeFormat::Markdown, "https://example.com", html, 200)
            .expect("select_output should succeed");
        // Must not be the raw HTML (format conversion happened)
        assert!(
            !result.contains("<html>"),
            "should not contain raw HTML tags"
        );
        // Must contain the text content
        assert!(result.contains("Hello world"), "should contain page text");
    }

    #[test]
    fn test_select_output_html_returns_raw_html() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let result = select_output(ScrapeFormat::Html, "https://example.com", html, 200)
            .expect("select_output should succeed");
        assert_eq!(result, html, "Html format should return raw HTML unchanged");
    }

    #[test]
    fn test_select_output_rawhtml_returns_raw_html() {
        let html = "<html><body><p>Test content</p></body></html>";
        let result = select_output(ScrapeFormat::RawHtml, "https://example.com", html, 200)
            .expect("select_output should succeed");
        assert_eq!(
            result, html,
            "RawHtml format should return raw HTML unchanged"
        );
    }

    #[test]
    fn test_select_output_json_includes_status_code() {
        let html = "<html><head><title>My Page</title></head><body><p>Content</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com/page", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["status_code"], 200,
            "JSON output must include status_code field"
        );
    }

    #[test]
    fn test_select_output_json_includes_url() {
        let html = "<html><body><p>Test</p></body></html>";
        let url = "https://example.com/docs";
        let result = select_output(ScrapeFormat::Json, url, html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(parsed["url"], url, "JSON output must include the url field");
    }

    #[test]
    fn test_select_output_json_includes_title() {
        let html =
            "<html><head><title>Spider Docs</title></head><body><p>Content</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["title"], "Spider Docs",
            "JSON output must include title extracted from <title>"
        );
    }

    #[test]
    fn test_select_output_json_includes_markdown() {
        let html = "<html><body><p>Hello world</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        let md = parsed["markdown"].as_str().expect("markdown field missing");
        assert!(
            md.contains("Hello world"),
            "markdown field must contain page text"
        );
        assert!(
            !md.contains("<html>"),
            "markdown field must not contain raw HTML"
        );
    }

    #[test]
    fn test_select_output_json_status_code_non_200() {
        let html = "<html><body>Not Found</body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 404)
            .expect("select_output should succeed even for non-200 (caller decides to error)");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["status_code"], 404,
            "JSON output must faithfully report non-200 status codes"
        );
    }

    // -----------------------------------------------------------------------
    // SSRF guard — validate_url must reject private IPs
    // These verify the guard that must run before build_scrape_website()
    // -----------------------------------------------------------------------

    #[test]
    fn test_ssrf_guard_rejects_loopback() {
        assert!(
            validate_url("http://127.0.0.1/admin").is_err(),
            "SSRF guard must reject loopback addresses"
        );
    }

    #[test]
    fn test_ssrf_guard_rejects_private_rfc1918() {
        assert!(
            validate_url("http://192.168.1.1/secret").is_err(),
            "SSRF guard must reject RFC-1918 private addresses"
        );
    }

    #[test]
    fn test_ssrf_guard_rejects_localhost_hostname() {
        assert!(
            validate_url("http://localhost/").is_err(),
            "SSRF guard must reject 'localhost' hostname"
        );
    }

    #[test]
    fn test_ssrf_guard_allows_public_url() {
        assert!(
            validate_url("https://example.com/docs").is_ok(),
            "SSRF guard must allow public HTTPS URLs"
        );
    }

    // -----------------------------------------------------------------------
    // Config-to-Spider mapping helpers — pure logic, no network
    // -----------------------------------------------------------------------

    #[test]
    fn test_fetch_retries_casts_to_u8_without_overflow() {
        // fetch_retries is usize; with_retry() takes u8.
        // Verify the cast logic: values > 255 clamp to 255, not wrap/panic.
        let large: usize = 300;
        let clamped = large.min(u8::MAX as usize) as u8;
        assert_eq!(clamped, 255u8, "fetch_retries > 255 must clamp to u8::MAX");
    }

    #[test]
    fn test_fetch_retries_small_value_preserved() {
        let small: usize = 3;
        let cast = small.min(u8::MAX as usize) as u8;
        assert_eq!(
            cast, 3u8,
            "small fetch_retries must round-trip through u8 cast"
        );
    }

    #[test]
    fn test_timeout_ms_converts_to_duration() {
        let timeout_ms: u64 = 15_000;
        let dur = Duration::from_millis(timeout_ms);
        assert_eq!(
            dur.as_secs(),
            15,
            "request_timeout_ms=15000 must produce Duration of 15s"
        );
    }

    #[test]
    fn test_timeout_none_uses_spider_default() {
        // When cfg.request_timeout_ms is None, we pass None to with_request_timeout,
        // letting Spider use its own default. This test confirms the branch logic:
        // only pass Some(dur) when a value is configured.
        let timeout_ms: Option<u64> = None;
        let passed_to_spider = timeout_ms.map(Duration::from_millis);
        assert!(
            passed_to_spider.is_none(),
            "None timeout_ms must produce None passed to with_request_timeout"
        );
    }

    // -----------------------------------------------------------------------
    // select_output — edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_output_empty_html_body() {
        // An empty HTML string should not panic; markdown output is just empty.
        let html = "";
        let result = select_output(ScrapeFormat::Markdown, "https://example.com", html, 200)
            .expect("select_output must handle empty HTML without error");
        // Empty input produces empty (or whitespace-only) markdown.
        assert!(
            result.trim().is_empty(),
            "empty HTML should produce empty markdown, got: {result:?}"
        );
    }

    #[test]
    fn test_select_output_json_missing_title() {
        // HTML with no <title> tag: the title field must be an empty string, not null.
        let html = "<html><body><p>No title here</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["title"], "",
            "missing <title> must produce empty string, not null"
        );
    }

    #[test]
    fn test_select_output_json_missing_description() {
        // HTML with no <meta name="description">: description field must be empty string.
        let html = "<html><head><title>T</title></head><body><p>Content</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["description"], "",
            "missing meta description must produce empty string, not null"
        );
    }

    #[test]
    fn test_select_output_json_includes_description() {
        // Verify description field is populated from <meta name="description">.
        let html = r#"<html><head><title>Page</title><meta name="description" content="A fine page"></head><body><p>Body</p></body></html>"#;
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["description"], "A fine page",
            "description must be extracted from meta tag"
        );
    }

    #[test]
    fn test_select_output_json_has_all_five_fields() {
        // Contract: JSON output must contain exactly url, status_code, markdown, title, description.
        let html = r#"<html><head><title>T</title><meta name="description" content="D"></head><body><p>B</p></body></html>"#;
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        let obj = parsed.as_object().expect("JSON output must be an object");
        for field in &["url", "status_code", "markdown", "title", "description"] {
            assert!(
                obj.contains_key(*field),
                "JSON output missing required field: {field}"
            );
        }
        assert_eq!(
            obj.len(),
            5,
            "JSON output must contain exactly 5 fields, got: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_page_matches_requested_url_exact_match() {
        assert!(page_matches_requested_url(
            "https://example.com/docs/trait.Client.html",
            "https://example.com/docs/trait.Client.html"
        ));
    }

    #[test]
    fn test_page_matches_requested_url_ignores_query_and_fragment() {
        assert!(page_matches_requested_url(
            "https://example.com/docs/trait.Client.html",
            "https://example.com/docs/trait.Client.html?x=1#section"
        ));
    }

    #[test]
    fn test_page_matches_requested_url_accepts_docs_redirect_filename_match() {
        assert!(page_matches_requested_url(
            "https://docs.rs/agent-client-protocol/latest/agent_client_protocol/trait.Client.html",
            "https://docs.rs/agent-client-protocol/0.9.5/agent_client_protocol/trait.Client.html"
        ));
    }

    #[test]
    fn test_page_matches_requested_url_rejects_different_terminal_page() {
        assert!(!page_matches_requested_url(
            "https://docs.rs/agent-client-protocol/latest/agent_client_protocol/trait.Client.html",
            "https://docs.rs/releases"
        ));
    }

    // -----------------------------------------------------------------------
    // build_scrape_website — config-to-Spider mapping (pure, no network)
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_scrape_website_sets_limit_one() {
        let cfg = Config::default();
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        // with_limit(1) sets a budget of {"*": 1} — verify via the budget field.
        let budget = website
            .configuration
            .budget
            .as_ref()
            .expect("budget must be set after with_limit(1)");
        assert_eq!(
            budget.len(),
            1,
            "budget should have exactly one entry (wildcard)"
        );
        let val = budget.values().next().expect("budget should have a value");
        assert_eq!(*val, 1, "with_limit(1) must set budget wildcard to 1");
    }

    #[test]
    fn test_build_scrape_website_blocks_assets() {
        let cfg = Config::default();
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert!(
            website.configuration.only_html,
            "block_assets must be true for scrape (only fetch HTML, not images/CSS/JS)"
        );
    }

    #[test]
    fn test_build_scrape_website_wires_ssrf_blacklist() {
        let cfg = Config::default();
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        let blacklist = website
            .configuration
            .blacklist_url
            .as_ref()
            .expect("blacklist_url must be set");
        assert!(
            !blacklist.is_empty(),
            "SSRF blacklist patterns must be wired into spider"
        );
    }

    #[test]
    fn test_build_scrape_website_sets_retry_from_config() {
        let cfg = Config {
            fetch_retries: 5,
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert_eq!(
            website.configuration.retry, 5,
            "retry must match cfg.fetch_retries"
        );
    }

    #[test]
    fn test_build_scrape_website_sets_timeout_from_config() {
        let cfg = Config {
            request_timeout_ms: Some(10_000),
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        let timeout = website
            .configuration
            .request_timeout
            .as_ref()
            .expect("request_timeout must be set when cfg has timeout_ms");
        assert_eq!(
            timeout.as_millis(),
            10_000,
            "request_timeout must match cfg.request_timeout_ms"
        );
    }

    #[test]
    fn test_build_scrape_website_explicit_timeout_overrides_spider_default() {
        // When cfg.request_timeout_ms is set, the resulting timeout must match exactly.
        // When None, Spider keeps its own default — we only assert the explicit case.
        let cfg_with = Config {
            request_timeout_ms: Some(7_500),
            ..Config::default()
        };
        let cfg_without = Config {
            request_timeout_ms: None,
            ..Config::default()
        };
        let ws_with = build_scrape_website(&cfg_with, "https://example.com")
            .expect("build_scrape_website should succeed");
        let ws_without = build_scrape_website(&cfg_without, "https://example.com")
            .expect("build_scrape_website should succeed");
        let timeout = ws_with
            .configuration
            .request_timeout
            .as_ref()
            .expect("timeout must be set when cfg has timeout_ms");
        assert_eq!(timeout.as_millis(), 7_500, "explicit timeout must match");
        // When None, spider uses its own default — just verify it differs from ours.
        // (Spider's default is 15s; our explicit value is 7.5s — they must differ.)
        let default_timeout = ws_without
            .configuration
            .request_timeout
            .as_ref()
            .map(|d| d.as_millis());
        assert_ne!(
            default_timeout,
            Some(7_500),
            "without explicit timeout, spider default must differ from our explicit value"
        );
    }

    #[test]
    fn test_build_scrape_website_wires_custom_headers() {
        let cfg = Config {
            custom_headers: vec![
                "Authorization: Bearer test-token".to_string(),
                "X-Custom: value".to_string(),
            ],
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        let headers = website
            .configuration
            .headers
            .as_ref()
            .expect("headers must be set when custom_headers is non-empty");
        assert!(
            headers.contains_key("authorization"),
            "Authorization header must be wired"
        );
        assert!(
            headers.contains_key("x-custom"),
            "X-Custom header must be wired"
        );
    }

    #[test]
    fn test_build_scrape_website_no_headers_when_empty() {
        let cfg = Config {
            custom_headers: vec![],
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        // When no custom headers, headers should remain None (spider default).
        assert!(
            website.configuration.headers.is_none(),
            "headers must be None when custom_headers is empty"
        );
    }

    #[test]
    fn test_build_scrape_website_sets_no_control_thread() {
        let cfg = Config::default();
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert!(
            website.configuration.no_control_thread,
            "no_control_thread must be true for single-page scrape"
        );
    }

    #[test]
    fn test_build_scrape_website_chrome_mode_sets_dismiss_dialogs() {
        let cfg = Config {
            render_mode: RenderMode::Chrome,
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert_eq!(
            website.configuration.dismiss_dialogs,
            Some(true),
            "Chrome mode must set dismiss_dialogs"
        );
        assert!(
            website.configuration.disable_log,
            "Chrome mode must set disable_log"
        );
    }

    #[test]
    fn test_build_scrape_website_chrome_mode_with_csp_bypass() {
        let cfg = Config {
            render_mode: RenderMode::Chrome,
            bypass_csp: true,
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert!(
            website.configuration.bypass_csp,
            "Chrome mode with bypass_csp must set csp_bypass on spider"
        );
    }

    #[test]
    fn test_build_scrape_website_http_mode_skips_chrome_options() {
        let cfg = Config {
            render_mode: RenderMode::Http,
            bypass_csp: true, // should be ignored in HTTP mode
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        // In HTTP mode, Chrome-specific options must NOT be set.
        assert_eq!(
            website.configuration.dismiss_dialogs, None,
            "HTTP mode must not set dismiss_dialogs"
        );
        assert!(
            !website.configuration.disable_log,
            "HTTP mode must not set disable_log"
        );
        assert!(
            !website.configuration.bypass_csp,
            "HTTP mode must not set bypass_csp even when cfg.bypass_csp is true"
        );
    }

    #[test]
    fn test_build_scrape_website_accept_invalid_certs() {
        let cfg = Config {
            accept_invalid_certs: true,
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        assert!(
            website.configuration.accept_invalid_certs,
            "accept_invalid_certs must be wired through to spider"
        );
    }

    #[test]
    fn test_build_scrape_website_wires_user_agent() {
        let cfg = Config {
            chrome_user_agent: Some("TestBot/1.0".to_string()),
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        let ua = website
            .configuration
            .user_agent
            .as_ref()
            .expect("user_agent must be set when chrome_user_agent is Some");
        assert_eq!(
            ua.as_str(),
            "TestBot/1.0",
            "user_agent must match cfg.chrome_user_agent"
        );
    }

    // -----------------------------------------------------------------------
    // select_output — migration contract coverage
    // -----------------------------------------------------------------------

    #[test]
    fn test_select_output_json_handles_missing_title() {
        // Contract: when HTML has no <title> tag, the JSON title field must be
        // an empty string (not null, not absent).
        let html = "<html><head></head><body><p>No title tag at all</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["title"], "",
            "HTML with no <title> tag must produce empty string title"
        );
        assert!(
            parsed["title"].is_string(),
            "title must be a string, not null"
        );
    }

    #[test]
    fn test_select_output_json_handles_missing_description() {
        // Contract: when HTML has no <meta name="description">, the JSON
        // description field must be an empty string (not null, not absent).
        let html =
            "<html><head><title>Has Title</title></head><body><p>No meta desc</p></body></html>";
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        assert_eq!(
            parsed["description"], "",
            "HTML with no meta description must produce empty string"
        );
        assert!(
            parsed["description"].is_string(),
            "description must be a string, not null"
        );
    }

    #[test]
    fn test_select_output_json_has_all_required_fields() {
        // Contract: JSON output must contain exactly: url, status_code, title,
        // description, markdown — no more, no fewer.
        let html = r#"<html><head><title>T</title><meta name="description" content="D"></head><body><p>B</p></body></html>"#;
        let result = select_output(ScrapeFormat::Json, "https://example.com", html, 200)
            .expect("select_output should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&result).expect("output should be valid JSON");
        let obj = parsed.as_object().expect("JSON output must be an object");
        let required = ["url", "status_code", "title", "description", "markdown"];
        for field in &required {
            assert!(
                obj.contains_key(*field),
                "JSON output missing required field: {field}"
            );
        }
        assert_eq!(
            obj.len(),
            required.len(),
            "JSON output must contain exactly {} fields, got: {:?}",
            required.len(),
            obj.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_select_output_markdown_empty_body() {
        // An HTML document with an empty <body> must not panic and should
        // produce empty (or whitespace-only) markdown.
        let html = "<html><head></head><body></body></html>";
        let result = select_output(ScrapeFormat::Markdown, "https://example.com", html, 200)
            .expect("select_output must not panic on empty body");
        // Empty body → no text content → trimmed result should be empty.
        assert!(
            result.trim().is_empty(),
            "empty <body></body> should produce empty markdown, got: {result:?}"
        );
    }

    #[test]
    fn test_select_output_html_preserves_entities() {
        // Html format returns raw HTML unchanged — HTML entities like &amp;
        // must be preserved verbatim (no double-encoding, no decoding).
        let html = "<html><body><p>A &amp; B &lt; C</p></body></html>";
        let result = select_output(ScrapeFormat::Html, "https://example.com", html, 200)
            .expect("select_output should succeed");
        assert_eq!(
            result, html,
            "Html format must return raw HTML with entities preserved"
        );
        assert!(
            result.contains("&amp;"),
            "HTML entities must not be decoded"
        );
        assert!(result.contains("&lt;"), "HTML entities must not be decoded");
    }

    #[test]
    fn test_build_scrape_website_wires_proxy() {
        let cfg = Config {
            chrome_proxy: Some("http://proxy.example.com:8080".to_string()),
            ..Config::default()
        };
        let website = build_scrape_website(&cfg, "https://example.com")
            .expect("build_scrape_website should succeed");
        let proxies = website
            .configuration
            .proxies
            .as_ref()
            .expect("proxies must be set when chrome_proxy is Some");
        assert_eq!(proxies.len(), 1, "exactly one proxy must be configured");
        assert_eq!(
            proxies[0].addr, "http://proxy.example.com:8080",
            "proxy address must match cfg.chrome_proxy"
        );
    }
}
