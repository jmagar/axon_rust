use crate::axon_cli::crates::cli::commands::crawl::discover_sitemap_urls_with_robots;
use crate::axon_cli::crates::core::config::{Config, RenderMode};
use crate::axon_cli::crates::core::http::validate_url;
use crate::axon_cli::crates::core::logging::log_done;
use crate::axon_cli::crates::core::ui::{muted, primary, print_option, print_phase, Spinner};
use crate::axon_cli::crates::crawl::engine::crawl_and_collect_map;
use std::error::Error;

pub async fn run_map(cfg: &Config, start_url: &str) -> Result<(), Box<dyn Error>> {
    validate_url(start_url)?;
    if !cfg.json_output {
        print_phase("◐", "Mapping", start_url);
        println!("  {}", primary("Options:"));
        print_option("maxDepth", &cfg.max_depth.to_string());
        print_option("discoverSitemaps", &cfg.discover_sitemaps.to_string());
        println!();
    }

    let initial_mode = match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    };

    let crawl_spinner = if cfg.json_output {
        None
    } else {
        Some(Spinner::new("mapping crawl in progress"))
    };
    let (final_summary, mut final_urls) =
        crawl_and_collect_map(cfg, start_url, initial_mode).await?;
    if let Some(s) = crawl_spinner {
        s.finish(&format!(
            "initial map crawl complete (pages={})",
            final_summary.pages_seen
        ));
    }

    if cfg.discover_sitemaps {
        let sitemap_spinner = if cfg.json_output {
            None
        } else {
            Some(Spinner::new("discovering sitemap URLs"))
        };
        let mut sitemap = discover_sitemap_urls_with_robots(cfg, start_url)
            .await?
            .urls;
        final_urls.append(&mut sitemap);
        final_urls.sort();
        final_urls.dedup();
        if let Some(s) = sitemap_spinner {
            s.finish("sitemap/robots discovery complete");
        }
    }

    let sitemap_url_count = final_urls
        .len()
        .saturating_sub(final_summary.pages_seen as usize);

    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({
                "url": start_url,
                "mapped_urls": final_urls.len(),
                "sitemap_urls": sitemap_url_count,
                "pages_seen": final_summary.pages_seen,
                "thin_pages": final_summary.thin_pages,
                "elapsed_ms": final_summary.elapsed_ms,
                "urls": final_urls,
            })
        );
    } else {
        println!("{}", primary(&format!("Map Results for {start_url}")));
        println!("{} {}", muted("Showing"), final_urls.len());
        println!();
        for url in &final_urls {
            println!("  • {url}");
        }
    }

    log_done(&format!(
        "command=map mapped_urls={} sitemap_urls={} pages_seen={} thin_pages={} elapsed_ms={}",
        final_urls.len(),
        sitemap_url_count,
        final_summary.pages_seen,
        final_summary.thin_pages,
        final_summary.elapsed_ms
    ));

    Ok(())
}
