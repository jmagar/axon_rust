use crate::axon_cli::crates::core::config::Config;
use crate::axon_cli::crates::core::ui::{accent, muted, primary, status_text, symbol_for_status};
use crate::axon_cli::crates::jobs::batch_jobs::list_batch_jobs;
use crate::axon_cli::crates::jobs::crawl_jobs::list_jobs;
use crate::axon_cli::crates::jobs::embed_jobs::list_embed_jobs;
use crate::axon_cli::crates::jobs::extract_jobs::list_extract_jobs;
use console::style;
use serde_json::Value;
use std::env;
use std::error::Error;

fn styled_metric(token: String, color: &str) -> String {
    if env::var("AXON_NO_COLOR").is_ok() || env::var("CORTEX_NO_COLOR").is_ok() {
        return token;
    }
    match color {
        "green" => style(token).green().to_string(),
        "yellow" => style(token).yellow().to_string(),
        "cyan" => style(token).cyan().to_string(),
        "blue" => style(token).blue().to_string(),
        _ => token,
    }
}

fn summarize_urls(urls_json: &Value) -> (String, usize) {
    let urls = urls_json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(ToOwned::to_owned))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let count = urls.len();
    if count == 0 {
        return ("(no targets)".to_string(), 0);
    }
    let first = urls[0].clone();
    let label = if count > 1 {
        format!("{first} (+{} more)", count - 1)
    } else {
        first
    };
    (label, count)
}

pub async fn run_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let (crawl_jobs, batch_jobs, extract_jobs, embed_jobs) = spider::tokio::try_join!(
        async {
            list_jobs(cfg, 20)
                .await
                .map_err(|e| format!("crawl status lookup failed: {e}"))
        },
        async {
            list_batch_jobs(cfg, 20)
                .await
                .map_err(|e| format!("batch status lookup failed: {e}"))
        },
        async {
            list_extract_jobs(cfg, 20)
                .await
                .map_err(|e| format!("extract status lookup failed: {e}"))
        },
        async {
            list_embed_jobs(cfg, 20)
                .await
                .map_err(|e| format!("embed status lookup failed: {e}"))
        },
    )?;

    if cfg.json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "local_crawl_jobs": crawl_jobs,
                "local_batch_jobs": batch_jobs,
                "local_extract_jobs": extract_jobs,
                "local_embed_jobs": embed_jobs
            }))?
        );
    } else {
        println!("{}", primary("Job Status (all)"));
        println!(
            "  {} {} | {} {} | {} {} | {} {}",
            muted("Crawl:"),
            crawl_jobs.len(),
            muted("Batch:"),
            batch_jobs.len(),
            muted("Extract:"),
            extract_jobs.len(),
            muted("Embed:"),
            embed_jobs.len()
        );
        println!();

        println!("{}", primary("◐ Crawls"));
        if crawl_jobs.is_empty() {
            println!("  {}", muted("None."));
        } else {
            for job in crawl_jobs.iter().take(5) {
                let mut metrics_suffix = String::new();
                if let Some(metrics) = job.result_json.as_ref() {
                    if job.status == "completed" {
                        let md_created = metrics
                            .get("md_created")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let thin_md = metrics.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0);
                        let filtered_urls = metrics
                            .get("filtered_urls")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let pages_crawled = metrics
                            .get("pages_crawled")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let pages_discovered = metrics
                            .get("pages_discovered")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let skipped = thin_md + filtered_urls;
                        let thin_pct = if pages_crawled > 0 {
                            (thin_md as f64 / pages_crawled as f64) * 100.0
                        } else {
                            0.0
                        };
                        metrics_suffix = format!(
                            " | {pages_crawled}/{pages_discovered} 🕷️ | {md_created} 📄 | {skipped} ⏭️ | thin {thin_md}/{pages_crawled} ({thin_pct:.1}%)"
                        );
                    } else if matches!(
                        job.status.as_str(),
                        "pending" | "running" | "processing" | "scraping"
                    ) {
                        let pages_crawled = metrics
                            .get("pages_crawled")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        let pages_discovered = metrics
                            .get("pages_discovered")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0);
                        if pages_crawled > 0 || pages_discovered > 0 {
                            metrics_suffix = format!(" | {pages_crawled}/{pages_discovered} 🕷️");
                        }
                    }
                }
                println!(
                    "  {} {} {} {}{}",
                    symbol_for_status(&job.status),
                    accent(&job.id.to_string()),
                    status_text(&job.status),
                    muted(&job.url),
                    metrics_suffix
                );
            }
        }
        println!();

        println!("{}", primary("◐ Batches"));
        if batch_jobs.is_empty() {
            println!("  {}", muted("None."));
        } else {
            for job in batch_jobs.iter().take(5) {
                let (target, url_count) = summarize_urls(&job.urls_json);
                let mut metrics = vec![styled_metric(format!("u{url_count}"), "blue")];
                if let Some(results_len) = job
                    .result_json
                    .as_ref()
                    .and_then(|r| r.get("results"))
                    .and_then(|v| v.as_array())
                    .map(|v| v.len())
                {
                    metrics.push(styled_metric(format!("r{results_len}"), "green"));
                }
                println!(
                    "  {} {} {} {} {}",
                    symbol_for_status(&job.status),
                    accent(&job.id.to_string()),
                    status_text(&job.status),
                    muted(&target),
                    metrics.join(" ")
                );
            }
        }
        println!();

        println!("{}", primary("◐ Extracts"));
        if extract_jobs.is_empty() {
            println!("  {}", muted("None."));
        } else {
            for job in extract_jobs.iter().take(5) {
                let (target, url_count) = summarize_urls(&job.urls_json);
                let mut metrics = vec![styled_metric(format!("u{url_count}"), "blue")];
                if let Some(total_items) = job
                    .result_json
                    .as_ref()
                    .and_then(|r| r.get("total_items"))
                    .and_then(|v| v.as_u64())
                {
                    metrics.push(styled_metric(format!("i{total_items}"), "green"));
                }
                if let Some(pages) = job
                    .result_json
                    .as_ref()
                    .and_then(|r| r.get("pages_visited"))
                    .and_then(|v| v.as_u64())
                {
                    metrics.push(styled_metric(format!("p{pages}"), "cyan"));
                }
                println!(
                    "  {} {} {} {} {}",
                    symbol_for_status(&job.status),
                    accent(&job.id.to_string()),
                    status_text(&job.status),
                    muted(&target),
                    metrics.join(" ")
                );
            }
        }
        println!();

        println!("{}", primary("◐ Embeds"));
        if embed_jobs.is_empty() {
            println!("  {}", muted("None."));
        } else {
            for job in embed_jobs.iter().take(5) {
                let mut metrics = Vec::new();
                if let Some(docs) = job
                    .result_json
                    .as_ref()
                    .and_then(|r| r.get("docs_embedded"))
                    .and_then(|v| v.as_u64())
                {
                    metrics.push(styled_metric(format!("d{docs}"), "blue"));
                } else {
                    metrics.push(styled_metric("d1".to_string(), "blue"));
                }
                if let Some(chunks) = job
                    .result_json
                    .as_ref()
                    .and_then(|r| r.get("chunks_embedded"))
                    .and_then(|v| v.as_u64())
                {
                    metrics.push(styled_metric(format!("c{chunks}"), "green"));
                }
                println!(
                    "  {} {} {} {} {}",
                    symbol_for_status(&job.status),
                    accent(&job.id.to_string()),
                    status_text(&job.status),
                    muted(&job.input_text),
                    metrics.join(" ")
                );
            }
        }
    }
    Ok(())
}
