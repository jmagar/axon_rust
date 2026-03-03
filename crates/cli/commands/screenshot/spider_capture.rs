use crate::crates::core::config::Config;
use crate::crates::core::http::cdp_discovery_url;
use crate::crates::crawl::engine::resolve_cdp_ws_url;
use spider::configuration::Viewport;
use spider::features::chrome_common::{ScreenShotConfig, ScreenshotParams};
use spider::website::Website;
use std::error::Error;

/// Capture a screenshot using Spider's Chrome screenshot support, reading
/// viewport and full_page settings from the Config.
pub(super) async fn spider_screenshot(cfg: &Config, url: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    spider_screenshot_with_options(
        cfg,
        url,
        cfg.viewport_width,
        cfg.viewport_height,
        cfg.screenshot_full_page,
    )
    .await
}

/// Capture a screenshot using Spider's built-in Chrome screenshot support
/// with explicit viewport and full_page parameters.
///
/// Used by both the CLI handler (via `spider_screenshot`) and the MCP
/// handler (which may override viewport/full_page from request params).
pub(crate) async fn spider_screenshot_with_options(
    cfg: &Config,
    url: &str,
    width: u32,
    height: u32,
    full_page: bool,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let remote_url = cfg
        .chrome_remote_url
        .as_deref()
        .ok_or("screenshot requires Chrome — set AXON_CHROME_REMOTE_URL")?;

    // Resolve the Chrome connection URL using the same logic as the crawl
    // engine: try the CDP WS discovery first, fall back to the discovery
    // URL or the raw remote URL.
    let chrome_url = match resolve_cdp_ws_url(remote_url).await {
        Some(ws_url) => ws_url,
        None => cdp_discovery_url(remote_url).unwrap_or_else(|| remote_url.to_string()),
    };

    let params = ScreenshotParams {
        full_page: Some(full_page),
        ..Default::default()
    };

    let screenshot_config = ScreenShotConfig::new(
        params, true,  // bytes — return PNG bytes on page.screenshot_bytes
        false, // save — we handle file writing ourselves
        None,  // output_dir — not needed since save=false
    );

    let mut website = Website::new(url);
    website.with_chrome_connection(Some(chrome_url));
    website.with_screenshot(Some(screenshot_config));
    website.with_viewport(Some(Viewport::new(width, height)));

    // Single page only — no crawling beyond the target URL.
    website.with_limit(1);
    website.with_depth(0);
    website.with_subdomains(false);

    // Wait for network idle so JS-rendered pages finish loading before capture.
    website.with_wait_for_idle_network0(Some(spider::configuration::WaitForIdleNetwork::new(
        Some(std::time::Duration::from_secs(
            cfg.chrome_network_idle_timeout_secs,
        )),
    )));

    // Dismiss browser dialogs that would otherwise block capture indefinitely.
    website.with_dismiss_dialogs(true);

    // Build the website config (required after Chrome settings).
    let mut website = website
        .build()
        .map_err(|_| "failed to build Spider website config for screenshot")?;

    website.crawl().await;

    let pages = website
        .get_pages()
        .ok_or("no pages returned from screenshot crawl")?;

    let page = pages
        .first()
        .ok_or("screenshot crawl returned zero pages — Chrome may not be reachable")?;

    page.screenshot_bytes
        .clone()
        .ok_or_else(|| "screenshot bytes not captured — Chrome may not be reachable".into())
}
