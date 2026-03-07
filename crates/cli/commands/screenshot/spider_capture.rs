use crate::crates::core::config::Config;
use crate::crates::crawl::screenshot::spider_screenshot_with_options as crawl_screenshot;
use std::error::Error;

// Re-export for MCP handler compatibility.
pub(crate) use crate::crates::crawl::screenshot::spider_screenshot_with_options;

/// Capture a screenshot using Spider's Chrome screenshot support, reading
/// viewport and full_page settings from the Config.
pub(super) async fn spider_screenshot(cfg: &Config, url: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    crawl_screenshot(
        cfg,
        url,
        cfg.viewport_width,
        cfg.viewport_height,
        cfg.screenshot_full_page,
    )
    .await
}
