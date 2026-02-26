mod metrics;

use crate::crates::core::config::Config;
use crate::crates::core::ui::{
    accent, metric, muted, primary, status_label, subtle, symbol_for_status,
};
use crate::crates::jobs::crawl::{CrawlJob, list_jobs};
use crate::crates::jobs::embed::{EmbedJob, list_embed_jobs};
use crate::crates::jobs::extract::{ExtractJob, list_extract_jobs};
use crate::crates::jobs::ingest::{IngestJob, list_ingest_jobs};
use chrono::{DateTime, Utc};
use metrics::{
    collection_from_config, display_embed_input, embed_metrics_suffix, extract_metrics_suffix,
    format_error, ingest_metrics_suffix, job_age, section_symbol, summarize_urls,
};
use std::error::Error;

pub async fn run_status(cfg: &Config) -> Result<(), Box<dyn Error>> {
    run_status_impl(cfg).await
}

pub async fn status_snapshot(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    Ok(serde_json::json!({
        "local_crawl_jobs": jobs.crawl,
        "local_extract_jobs": jobs.extract,
        "local_embed_jobs": jobs.embed,
        "local_ingest_jobs": jobs.ingest,
    }))
}

pub async fn status_text(cfg: &Config) -> Result<String, Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;
    let crawl_total = jobs.crawl.len();
    let extract_total = jobs.extract.len();
    let embed_total = jobs.embed.len();
    let ingest_total = jobs.ingest.len();
    let mut lines = Vec::new();
    lines.push("Axon Status".to_string());
    lines.push(format!("crawl jobs:   {crawl_total}"));
    lines.push(format!("extract jobs: {extract_total}"));
    lines.push(format!("embed jobs:   {embed_total}"));
    lines.push(format!("ingest jobs:  {ingest_total}"));
    Ok(lines.join("\n"))
}

struct StatusJobs {
    crawl: Vec<CrawlJob>,
    extract: Vec<ExtractJob>,
    embed: Vec<EmbedJob>,
    ingest: Vec<IngestJob>,
}

async fn run_status_impl(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let jobs = load_status_jobs(cfg).await?;

    if cfg.json_output {
        emit_status_json(&jobs.crawl, &jobs.extract, &jobs.embed, &jobs.ingest)?;
    } else {
        emit_status_human(&jobs.crawl, &jobs.extract, &jobs.embed, &jobs.ingest);
    }
    Ok(())
}

async fn load_status_jobs(cfg: &Config) -> Result<StatusJobs, Box<dyn Error>> {
    let (crawl, extract, embed, ingest) = spider::tokio::try_join!(
        async {
            list_jobs(cfg, 20)
                .await
                .map_err(|e| format!("crawl status lookup failed: {e}"))
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
        async {
            list_ingest_jobs(cfg, 20)
                .await
                .map_err(|e| format!("ingest status lookup failed: {e}"))
        },
    )?;
    Ok(StatusJobs {
        crawl,
        extract,
        embed,
        ingest,
    })
}

fn emit_status_json(
    crawl_jobs: &[CrawlJob],
    extract_jobs: &[ExtractJob],
    embed_jobs: &[EmbedJob],
    ingest_jobs: &[IngestJob],
) -> Result<(), Box<dyn Error>> {
    let payload = serde_json::json!({
        "local_crawl_jobs": crawl_jobs,
        "local_extract_jobs": extract_jobs,
        "local_embed_jobs": embed_jobs,
        "local_ingest_jobs": ingest_jobs,
    });
    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn emit_status_human(
    crawl_jobs: &[CrawlJob],
    extract_jobs: &[ExtractJob],
    embed_jobs: &[EmbedJob],
    ingest_jobs: &[IngestJob],
) {
    print_totals(crawl_jobs, extract_jobs, embed_jobs, ingest_jobs);
    print_crawls(crawl_jobs);
    print_embeds(embed_jobs, crawl_jobs);
    print_ingests(ingest_jobs);
    print_extracts(extract_jobs);
}

fn status_breakdown(statuses: &[&str]) -> String {
    let done = statuses.iter().filter(|s| **s == "completed").count();
    let active = statuses
        .iter()
        .filter(|s| matches!(**s, "pending" | "running" | "processing" | "scraping"))
        .count();
    let failed = statuses
        .iter()
        .filter(|s| matches!(**s, "failed" | "error"))
        .count();
    let canceled = statuses.iter().filter(|s| **s == "canceled").count();
    let mut parts = Vec::new();
    if done > 0 {
        parts.push(format!("{} {}", symbol_for_status("completed"), done));
    }
    if active > 0 {
        parts.push(format!("{} {}", symbol_for_status("pending"), active));
    }
    if failed > 0 {
        parts.push(format!("{} {}", symbol_for_status("failed"), failed));
    }
    if canceled > 0 {
        parts.push(format!("{} {}", symbol_for_status("canceled"), canceled));
    }
    if parts.is_empty() {
        "0".to_string()
    } else {
        parts.join(" ")
    }
}

fn print_totals(
    crawl_jobs: &[CrawlJob],
    extract_jobs: &[ExtractJob],
    embed_jobs: &[EmbedJob],
    ingest_jobs: &[IngestJob],
) {
    let crawl_statuses: Vec<&str> = crawl_jobs.iter().map(|j| j.status.as_str()).collect();
    let extract_statuses: Vec<&str> = extract_jobs.iter().map(|j| j.status.as_str()).collect();
    let embed_statuses: Vec<&str> = embed_jobs.iter().map(|j| j.status.as_str()).collect();
    let ingest_statuses: Vec<&str> = ingest_jobs.iter().map(|j| j.status.as_str()).collect();

    println!("{}", primary("Job Status"));
    println!(
        "  {}  {}    {}  {}    {}  {}    {}  {}",
        muted("Crawl"),
        status_breakdown(&crawl_statuses),
        muted("Embed"),
        status_breakdown(&embed_statuses),
        muted("Ingest"),
        status_breakdown(&ingest_statuses),
        muted("Extract"),
        status_breakdown(&extract_statuses),
    );
    println!();
}

fn print_crawls(crawl_jobs: &[CrawlJob]) {
    let statuses: Vec<&str> = crawl_jobs.iter().map(|j| j.status.as_str()).collect();
    let header_sym = if crawl_jobs.is_empty() {
        symbol_for_status("completed")
    } else {
        section_symbol(&statuses)
    };
    println!("{}", primary(&format!("{header_sym} Crawls")));
    if crawl_jobs.is_empty() {
        println!("  {}", muted("None."));
        println!();
        return;
    }
    for job in crawl_jobs.iter().take(5) {
        let metrics_suffix = job
            .result_json
            .as_ref()
            .map(|metrics| crawl_metrics_suffix(&job.status, metrics))
            .unwrap_or_default();
        let age_text = job_age(&job.status, job.finished_at.as_ref(), &job.updated_at);
        let age = format!("{}{}{}", subtle(" | ("), accent(&age_text), subtle(")"));
        let label = status_label(&job.status);
        let prefix = if label.is_empty() {
            format!("  {} ", symbol_for_status(&job.status))
        } else {
            format!("  {} {} ", symbol_for_status(&job.status), label)
        };
        println!(
            "{}{}{}{} {} {}",
            prefix,
            accent(&job.url),
            metrics_suffix,
            age,
            subtle("|"),
            subtle(&job.id.to_string()),
        );
        if let Some(err) = format_error(job.error_text.as_deref()) {
            println!("       {}", muted(&format!("↳ {err}")));
        }
    }
    println!();
}

fn crawl_metrics_suffix(status: &str, metrics: &serde_json::Value) -> String {
    if status == "completed" {
        let md_created = metrics
            .get("md_created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let thin_md = metrics.get("thin_md").and_then(|v| v.as_u64()).unwrap_or(0);
        let filtered_urls = metrics
            .get("filtered_urls")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pages_discovered = metrics
            .get("pages_discovered")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let pages_target = pages_discovered.saturating_sub(filtered_urls);
        let thin_pct = if pages_target > 0 {
            (thin_md as f64 / pages_target as f64) * 100.0
        } else {
            0.0
        };
        let sep = subtle(" | ");
        let thin_str = format!("{:.1}%", thin_pct);
        return format!(
            "{sep}{}{}{}{sep}{}{sep}{}",
            primary(&md_created.to_string()),
            subtle("/"),
            metric(pages_target, "pages"),
            metric(filtered_urls, "filtered"),
            metric(&thin_str as &str, "thin"),
        );
    }
    if matches!(status, "pending" | "running" | "processing" | "scraping") {
        let md_created = metrics
            .get("md_created")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let filtered_urls = metrics
            .get("filtered_urls")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if md_created > 0 || filtered_urls > 0 {
            let sep = subtle(" | ");
            return format!(
                "{sep}{}{sep}{}",
                metric(md_created, "crawled"),
                metric(filtered_urls, "filtered"),
            );
        }
    }
    String::new()
}

struct JobRow<'a> {
    status: &'a str,
    id: &'a uuid::Uuid,
    target: &'a str,
    metrics_suffix: &'a str,
    collection: Option<&'a str>,
    finished_at: Option<&'a DateTime<Utc>>,
    updated_at: &'a DateTime<Utc>,
    error_text: Option<&'a str>,
}

fn print_job_row(row: &JobRow<'_>) {
    let collection_suffix = row
        .collection
        .map(|c| format!("{}{}", subtle(" | "), primary(c)))
        .unwrap_or_default();
    let age_text = job_age(row.status, row.finished_at, row.updated_at);
    let age = format!("{}{}{}", subtle(" | ("), accent(&age_text), subtle(")"));
    let label = status_label(row.status);
    let prefix = if label.is_empty() {
        format!("  {} ", symbol_for_status(row.status))
    } else {
        format!("  {} {} ", symbol_for_status(row.status), label)
    };
    println!(
        "{}{}{}{}{} {} {}",
        prefix,
        accent(row.target),
        row.metrics_suffix,
        collection_suffix,
        age,
        subtle("|"),
        subtle(&row.id.to_string()),
    );
    if let Some(err) = format_error(row.error_text) {
        println!("       {}", muted(&format!("↳ {err}")));
    }
}

fn print_extracts(extract_jobs: &[ExtractJob]) {
    let statuses: Vec<&str> = extract_jobs.iter().map(|j| j.status.as_str()).collect();
    let header_sym = if extract_jobs.is_empty() {
        symbol_for_status("completed")
    } else {
        section_symbol(&statuses)
    };
    println!("{}", primary(&format!("{header_sym} Extracts")));
    if extract_jobs.is_empty() {
        println!("  {}", muted("None."));
        println!();
        return;
    }
    for job in extract_jobs.iter().take(5) {
        let (target, url_count) = summarize_urls(&job.urls_json);
        let metrics_suffix = extract_metrics_suffix(job.result_json.as_ref(), url_count);
        print_job_row(&JobRow {
            status: &job.status,
            id: &job.id,
            target: &target,
            metrics_suffix: &metrics_suffix,
            collection: None,
            finished_at: job.finished_at.as_ref(),
            updated_at: &job.updated_at,
            error_text: job.error_text.as_deref(),
        });
    }
    println!();
}

fn print_ingests(ingest_jobs: &[IngestJob]) {
    let statuses: Vec<&str> = ingest_jobs.iter().map(|j| j.status.as_str()).collect();
    let header_sym = if ingest_jobs.is_empty() {
        symbol_for_status("completed")
    } else {
        section_symbol(&statuses)
    };
    println!("{}", primary(&format!("{header_sym} Ingests")));
    if ingest_jobs.is_empty() {
        println!("  {}", muted("None."));
        println!();
        return;
    }
    for job in ingest_jobs.iter().take(5) {
        let target = format!("{}: {}", job.source_type, job.target);
        let metrics_suffix = ingest_metrics_suffix(&job.status, job.result_json.as_ref());
        let collection = collection_from_config(&job.config_json);
        print_job_row(&JobRow {
            status: &job.status,
            id: &job.id,
            target: &target,
            metrics_suffix: &metrics_suffix,
            collection,
            finished_at: job.finished_at.as_ref(),
            updated_at: &job.updated_at,
            error_text: job.error_text.as_deref(),
        });
    }
    println!();
}

fn print_embeds(embed_jobs: &[EmbedJob], crawl_jobs: &[CrawlJob]) {
    let crawl_url_map: std::collections::HashMap<uuid::Uuid, &str> =
        crawl_jobs.iter().map(|j| (j.id, j.url.as_str())).collect();

    let statuses: Vec<&str> = embed_jobs.iter().map(|j| j.status.as_str()).collect();
    let header_sym = if embed_jobs.is_empty() {
        symbol_for_status("completed")
    } else {
        section_symbol(&statuses)
    };
    println!("{}", primary(&format!("{header_sym} Embeds")));
    if embed_jobs.is_empty() {
        println!("  {}", muted("None."));
        println!();
        return;
    }
    for job in embed_jobs.iter().take(5) {
        let metrics_suffix = embed_metrics_suffix(&job.status, job.result_json.as_ref());
        let target = display_embed_input(&job.input_text, &crawl_url_map);
        let collection = collection_from_config(&job.config_json);
        print_job_row(&JobRow {
            status: &job.status,
            id: &job.id,
            target: &target,
            metrics_suffix: &metrics_suffix,
            collection,
            finished_at: job.finished_at.as_ref(),
            updated_at: &job.updated_at,
            error_text: job.error_text.as_deref(),
        });
    }
    println!();
}
