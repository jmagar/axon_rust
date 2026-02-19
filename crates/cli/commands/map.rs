use crate::axon_cli::crates::cli::commands::crawl::discover_sitemap_urls_with_robots;
use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{muted, primary, print_option, print_phase, Spinner};
use crate::axon_cli::crates::crawl::engine::{crawl_and_collect_map, try_auto_switch};
use std::error::Error;

pub async fn run_map(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    print_phase("◐", "Mapping", start_url);
    println!("  {}", primary("Options:"));
    print_option("maxDepth", &cfg.max_depth.to_string());
    print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
    println!();

    let initial_mode = match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    };

    let crawl_spinner = Spinner::new("mapping crawl in progress");
    let (summary, urls) = crawl_and_collect_map(cfg, start_url, initial_mode).await?;
    crawl_spinner.finish(&format!(
        "initial map crawl complete (pages={})",
        summary.pages_seen
    ));
    let (final_summary, mut final_urls) = try_auto_switch(cfg, start_url, &summary, &urls).await?;

    if cfg.discover_sitemaps {
        let sitemap_spinner = Spinner::new("discovering sitemap URLs");
        let mut sitemap = discover_sitemap_urls_with_robots(cfg, start_url)
            .await?
            .urls;
        final_urls.append(&mut sitemap);
        final_urls.sort();
        final_urls.dedup();
        sitemap_spinner.finish("sitemap/robots discovery complete");
    }

    println!("{}", primary(&format!("Map Results for {start_url}")));
    println!("{} {}", muted("Showing"), final_urls.len());
    println!();
    for url in &final_urls {
        println!("  • {url}");
    }

    log_done(&format!(
        "command=map mapped_urls={} pages_seen={} thin_pages={} elapsed_ms={}",
        final_urls.len(),
        final_summary.pages_seen,
        final_summary.thin_pages,
        final_summary.elapsed_ms
    ));

    Ok(())
}
