use crate::crates::core::config::Config;
use crate::crates::core::http::validate_url;
use crate::crates::core::logging::log_done;
use crate::crates::core::ui::{Spinner, muted, primary, print_option, print_phase};
use crate::crates::crawl::engine::map_with_sitemap;
use std::error::Error;

pub async fn map_payload(
    cfg: &Config,
    start_url: &str,
) -> Result<serde_json::Value, Box<dyn Error>> {
    validate_url(start_url)?;
    let result = map_with_sitemap(cfg, start_url).await?;
    Ok(serde_json::json!({
        "url": start_url,
        "mapped_urls": result.urls.len(),
        "sitemap_urls": result.sitemap_urls,
        "pages_seen": result.summary.pages_seen,
        "thin_pages": result.summary.thin_pages,
        "elapsed_ms": result.summary.elapsed_ms,
        "urls": result.urls,
    }))
}

pub async fn run_map(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    if !cfg.json_output {
        print_phase("◐", "Mapping", start_url);
        println!("  {}", primary("Options:"));
        print_option("maxDepth", &cfg.max_depth.to_string());
        print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
        println!();
    }

    let map_spinner = if cfg.json_output {
        None
    } else {
        Some(Spinner::new("mapping in progress"))
    };

    let result = map_with_sitemap(cfg, start_url).await?;

    if let Some(s) = map_spinner {
        s.finish(&format!(
            "map complete (pages={} sitemap_urls={})",
            result.summary.pages_seen, result.sitemap_urls
        ));
    }

    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({
                "url": start_url,
                "mapped_urls": result.urls.len(),
                "sitemap_urls": result.sitemap_urls,
                "pages_seen": result.summary.pages_seen,
                "thin_pages": result.summary.thin_pages,
                "elapsed_ms": result.summary.elapsed_ms,
                "urls": result.urls,
            })
        );
    } else {
        println!("{}", primary(&format!("Map Results for {start_url}")));
        println!("{} {}", muted("Showing"), result.urls.len());
        println!();
        for url in &result.urls {
            println!("  • {url}");
        }
    }

    log_done(&format!(
        "command=map mapped_urls={} sitemap_urls={} pages_seen={} thin_pages={} elapsed_ms={}",
        result.urls.len(),
        result.sitemap_urls,
        result.summary.pages_seen,
        result.summary.thin_pages,
        result.summary.elapsed_ms
    ));

    Ok(())
}

#[cfg(test)]
mod map_migration_tests;
