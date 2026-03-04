mod spider_capture;
mod util;

#[cfg(test)]
mod screenshot_migration_tests;

pub(crate) use spider_capture::spider_screenshot_with_options;
pub(crate) use util::url_to_screenshot_filename;

use super::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::http::{normalize_url, validate_url};
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{primary, print_option, print_phase};
use std::error::Error;
use util::{format_screenshot_json, require_chrome};

pub async fn run_screenshot(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("screenshot requires at least one URL (positional or --urls)".into());
    }
    for (idx, url) in urls.iter().enumerate() {
        screenshot_one(cfg, url, idx + 1).await?;
    }
    Ok(())
}

async fn screenshot_one(cfg: &Config, url: &str, idx: usize) -> Result<(), Box<dyn Error>> {
    require_chrome(cfg)?;

    let normalized = normalize_url(url);
    validate_url(&normalized)?;

    print_phase("◐", "Screenshot", &normalized);
    println!("  {}", primary("Options:"));
    print_option("fullPage", &cfg.screenshot_full_page.to_string());
    print_option(
        "viewport",
        &format!("{}x{}", cfg.viewport_width, cfg.viewport_height),
    );
    print_option(
        "chromeRemoteUrl",
        cfg.chrome_remote_url.as_deref().unwrap_or("none"),
    );
    println!();

    let bytes = spider_capture::spider_screenshot(cfg, &normalized).await?;

    let path = if let Some(p) = &cfg.output_path {
        p.clone()
    } else {
        let dir = cfg.output_dir.join("screenshots");
        dir.join(url_to_screenshot_filename(&normalized, idx))
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;

    let size = bytes.len() as u64;
    if cfg.json_output {
        println!(
            "{}",
            format_screenshot_json(&normalized, &path.to_string_lossy(), size)
        );
    } else {
        log_done(&format!("saved: {} ({} bytes)", path.display(), size));
    }

    Ok(())
}
