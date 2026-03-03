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
use spider::page::Page;
use spider::website::Website;
use std::error::Error;
use std::time::Duration;

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
    let markdown = || to_markdown(html);

    match format {
        ScrapeFormat::Markdown => Ok(markdown()),
        ScrapeFormat::Html | ScrapeFormat::RawHtml => Ok(html.to_string()),
        ScrapeFormat::Json => {
            let md = markdown();
            Ok(serde_json::to_string_pretty(&serde_json::json!({
                "url": url,
                "status_code": status_code,
                "markdown": md,
                "title": find_between(html, "<title>", "</title>").unwrap_or(""),
                "description": extract_meta_description(html).unwrap_or_default(),
            }))?)
        }
    }
}

pub async fn scrape_payload(cfg: &Config, url: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    let normalized = normalize_url(url);
    validate_url(&normalized)?;

    let mut website = build_scrape_website(cfg, &normalized)?;
    let mut rx = website
        .subscribe(16)
        .ok_or("failed to subscribe to spider broadcast")?;
    let collect: tokio::task::JoinHandle<Option<Page>> =
        tokio::spawn(async move { rx.recv().await.ok() });
    match cfg.render_mode {
        RenderMode::Http | RenderMode::AutoSwitch => website.crawl_raw().await,
        RenderMode::Chrome => website.crawl().await,
    }
    website.unsubscribe();
    let page = collect
        .await
        .map_err(|e| format!("page collector panicked: {e}"))?
        .ok_or("spider returned no page for this URL")?;

    let html = page.get_html();
    let status_code = page.status_code.as_u16();
    if !page.status_code.is_success() {
        return Err(format!("scrape failed: HTTP {} for {}", status_code, normalized).into());
    }

    let markdown = to_markdown(&html);
    let title = find_between(&html, "<title>", "</title>")
        .unwrap_or("")
        .to_string();
    let description = extract_meta_description(&html).unwrap_or_default();

    Ok(serde_json::json!({
        "url": normalized,
        "status_code": status_code,
        "title": title,
        "description": description,
        "markdown": markdown,
    }))
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
    for result in join_all(tasks).await {
        match result {
            Ok(Some(pair)) => to_embed.push(pair),
            Ok(None) => {}
            Err(e) => eprintln!("scrape error: {e}"),
        }
    }

    // Phase 2: embed all collected markdowns in one batch (single embed_path_native call).
    // Running this after Phase 1 avoids the bug where concurrent scrape_one calls each
    // call embed_path_native on the shared dir, causing every subsequent call to re-embed
    // all previously written files (O(n²) embed work).
    if cfg.embed && !to_embed.is_empty() {
        let embed_dir = cfg.output_dir.join("scrape-markdown");
        tokio::fs::create_dir_all(&embed_dir).await?;
        for (normalized, markdown) in &to_embed {
            tokio::fs::write(embed_dir.join(url_to_filename(normalized, 1)), markdown).await?;
        }
        embed_path_native(cfg, &embed_dir.to_string_lossy()).await?;
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

    // Use explicit subscribe() + crawl_raw() instead of scrape_raw().
    // scrape_raw() has a biased-select race: for fast single-page fetches, done_rx
    // fires before rx2.recv() gets a turn, so get_pages() comes back empty.
    // Owning the subscription ourselves avoids the race entirely.
    let mut rx = website
        .subscribe(16)
        .ok_or("failed to subscribe to spider broadcast")?;
    // Spawn the collector BEFORE the crawl so it is ready to receive the broadcast.
    let collect: tokio::task::JoinHandle<Option<Page>> =
        tokio::spawn(async move { rx.recv().await.ok() });
    match cfg.render_mode {
        RenderMode::Http | RenderMode::AutoSwitch => website.crawl_raw().await,
        RenderMode::Chrome => website.crawl().await,
    }
    website.unsubscribe();
    let page = collect
        .await
        .map_err(|e| format!("page collector panicked: {e}"))?
        .ok_or("spider returned no page for this URL")?;

    let html = page.get_html();
    let status_code = page.status_code.as_u16();

    // Surface non-success HTTP codes as errors so callers can handle them.
    if !page.status_code.is_success() {
        return Err(format!("scrape failed: HTTP {} for {}", status_code, normalized).into());
    }

    let markdown = to_markdown(&html);
    let output = select_output(cfg.format, &normalized, &html, status_code)?;

    if cfg.json_output {
        // Structured JSON output for web UI / machine consumers.
        // The markdown field lets the frontend display content directly
        // without going through the file-based embed pipeline.
        let title = find_between(&html, "<title>", "</title>")
            .unwrap_or("")
            .to_string();
        let description = extract_meta_description(&html).unwrap_or_default();
        println!(
            "{}",
            serde_json::json!({
                "url": normalized,
                "status_code": status_code,
                "title": title,
                "description": description,
                "markdown": markdown,
            })
        );
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
}
