use super::{
    canonicalize_url_for_dedupe, is_excluded_url_path, CrawlSummary, SitemapBackfillStats,
};
use crate::crates::core::config::Config;
use crate::crates::core::content::{extract_loc_values, to_markdown, url_to_filename};
use crate::crates::core::http::validate_url;
use crate::crates::core::logging::log_info;
use spider::tokio;
use spider::url::Url;
use std::collections::{HashSet, VecDeque};
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

async fn fetch_text_with_retry(
    client: &reqwest::Client,
    url: &str,
    retries: usize,
    backoff_ms: u64,
) -> Option<String> {
    if validate_url(url).is_err() {
        return None;
    }
    let mut attempt = 0usize;
    loop {
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return resp.text().await.ok();
                }
                if attempt >= retries || !should_retry_status(status) {
                    return None;
                }
            }
            Err(_) if attempt >= retries => return None,
            Err(_) => {}
        }

        attempt = attempt.saturating_add(1);
        let exp = attempt.saturating_sub(1).min(20) as u32;
        let multiplier = 1u64 << exp;
        let delay_ms = backoff_ms.saturating_mul(multiplier).max(1);
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
    }
}

fn sitemap_seed_queue(scheme: &str, host: &str) -> VecDeque<String> {
    let mut queue = VecDeque::new();
    queue.push_back(format!("{scheme}://{host}/sitemap.xml"));
    queue.push_back(format!("{scheme}://{host}/sitemap_index.xml"));
    queue.push_back(format!("{scheme}://{host}/sitemap-index.xml"));
    queue
}

fn sitemap_loc_in_scope(
    cfg: &Config,
    loc: &str,
    start_host: &str,
    start_path: &str,
    scoped_to_root: bool,
) -> Option<String> {
    let u = Url::parse(loc).ok()?;
    let h = u.host_str()?;
    let in_scope = if cfg.include_subdomains {
        h == start_host
            || h.strip_suffix(start_host)
                .is_some_and(|rest| rest.ends_with('.'))
    } else {
        h == start_host
    };
    if !in_scope || is_excluded_url_path(loc, &cfg.exclude_path_prefix) {
        return None;
    }
    if !scoped_to_root {
        let p = u.path();
        let exact = p == start_path;
        let nested = p.starts_with(&(start_path.to_string() + "/"));
        if !exact && !nested {
            return None;
        }
    }
    canonicalize_url_for_dedupe(loc)
}

async fn process_sitemap_batch(
    cfg: &Config,
    client: &reqwest::Client,
    batch: Vec<String>,
    scope: &SitemapScope<'_>,
    output: &mut SitemapBatchOutput<'_>,
) -> usize {
    let mut joins = tokio::task::JoinSet::new();
    for sitemap_url in batch {
        let http = client.clone();
        let retries = cfg.fetch_retries;
        let backoff = cfg.retry_backoff_ms;
        joins.spawn(async move {
            fetch_text_with_retry(&http, &sitemap_url, retries, backoff)
                .await
                .map(|xml| (sitemap_url, xml))
        });
    }

    let mut parsed = 0usize;
    while let Some(joined) = joins.join_next().await {
        let Ok(Some((_sitemap_url, xml))) = joined else {
            continue;
        };
        parsed += 1;
        let is_index = xml
            .as_bytes()
            .windows(b"<sitemapindex".len())
            .any(|w| w.eq_ignore_ascii_case(b"<sitemapindex"));
        for loc in extract_loc_values(&xml) {
            if let Some(canonical_loc) = sitemap_loc_in_scope(
                cfg,
                &loc,
                scope.start_host,
                scope.start_path,
                scope.scoped_to_root,
            ) {
                if is_index && !output.seen_sitemaps.contains(&canonical_loc) {
                    output.queue.push_back(canonical_loc);
                } else if !is_index {
                    output.out.insert(canonical_loc);
                }
            }
        }
    }
    parsed
}

pub async fn crawl_sitemap_urls(
    cfg: &Config,
    start_url: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    let parsed = Url::parse(start_url)?;
    let scheme = parsed.scheme().to_string();
    let host = parsed.host_str().ok_or("missing host")?.to_string();

    let mut queue = sitemap_seed_queue(&scheme, &host);

    let mut seen_sitemaps = HashSet::new();
    let mut out = HashSet::new();
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let start_host = host;
    let start_path = parsed.path().trim_end_matches('/').to_string();
    let scoped_to_root = start_path.is_empty();
    let scope = SitemapScope {
        start_host: &start_host,
        start_path: &start_path,
        scoped_to_root,
    };
    let worker_limit = cfg.sitemap_concurrency_limit.unwrap_or(64).clamp(1, 1024);
    let max_sitemaps = cfg.max_sitemaps.max(1);
    let mut parsed_sitemaps = 0usize;

    while !queue.is_empty() && parsed_sitemaps < max_sitemaps {
        let mut batch = Vec::new();
        while batch.len() < worker_limit && parsed_sitemaps + batch.len() < max_sitemaps {
            let Some(url) = queue.pop_front() else {
                break;
            };
            if seen_sitemaps.insert(url.clone()) {
                batch.push(url);
            }
        }
        if batch.is_empty() {
            break;
        }

        let mut output = SitemapBatchOutput {
            seen_sitemaps: &seen_sitemaps,
            queue: &mut queue,
            out: &mut out,
        };
        parsed_sitemaps += process_sitemap_batch(cfg, &client, batch, &scope, &mut output).await;
        if parsed_sitemaps.is_multiple_of(64) {
            log_info(&format!(
                "command=sitemap parsed={} discovered_urls={} queue={}",
                parsed_sitemaps,
                out.len(),
                queue.len()
            ));
        }
    }

    let mut urls: Vec<String> = out.into_iter().collect();
    urls.sort();
    Ok(urls)
}

struct SitemapScope<'a> {
    start_host: &'a str,
    start_path: &'a str,
    scoped_to_root: bool,
}

struct SitemapBatchOutput<'a> {
    seen_sitemaps: &'a HashSet<String>,
    queue: &'a mut VecDeque<String>,
    out: &'a mut HashSet<String>,
}

fn spawn_backfill_task(
    set: &mut tokio::task::JoinSet<(String, Result<String, String>)>,
    url: String,
    client: reqwest::Client,
    retries: usize,
    backoff_ms: u64,
) {
    set.spawn(async move {
        let result = fetch_text_with_retry(&client, &url, retries, backoff_ms)
            .await
            .ok_or_else(|| format!("fetch failed for {url}"));
        (url, result)
    });
}

async fn handle_backfill_result(
    joined: Result<(String, Result<String, String>), tokio::task::JoinError>,
    markdown_dir: &Path,
    manifest: &mut tokio::io::BufWriter<tokio::fs::File>,
    cfg: &Config,
    idx: &mut u32,
    summary: &mut CrawlSummary,
) -> Result<(usize, usize, usize), Box<dyn Error>> {
    let mut fetched_ok = 0usize;
    let mut written = 0usize;
    let mut failed = 0usize;
    match joined {
        Ok((url, Ok(html))) => {
            fetched_ok += 1;
            let md = to_markdown(&html);
            let chars = md.chars().count();
            if chars < cfg.min_markdown_chars {
                summary.thin_pages += 1;
            }
            if chars >= cfg.min_markdown_chars || !cfg.drop_thin_markdown {
                *idx += 1;
                let file = markdown_dir.join(url_to_filename(&url, *idx));
                tokio::fs::write(&file, md).await?;
                let rec = serde_json::json!({"url": url, "file_path": file.to_string_lossy(), "markdown_chars": chars, "source": "sitemap_backfill"});
                let mut line = rec.to_string();
                line.push('\n');
                manifest.write_all(line.as_bytes()).await?;
                summary.markdown_files += 1;
                written += 1;
            }
        }
        Ok((url, Err(err))) => {
            log_info(&format!(
                "command=sitemap_backfill fetch_failed url={url} err={err}"
            ));
            failed += 1;
        }
        Err(_) => failed += 1,
    }
    Ok((fetched_ok, written, failed))
}

pub async fn append_sitemap_backfill(
    cfg: &Config,
    start_url: &str,
    output_dir: &Path,
    seen_urls: &HashSet<String>,
    summary: &mut CrawlSummary,
) -> Result<SitemapBackfillStats, Box<dyn Error>> {
    let sitemap_urls = crawl_sitemap_urls(cfg, start_url).await?;
    let sitemap_discovered = sitemap_urls.len();
    log_info(&format!(
        "command=crawl sitemap_backfill_discovered={} concurrency={}",
        sitemap_discovered,
        cfg.backfill_concurrency_limit
            .unwrap_or(cfg.batch_concurrency)
            .max(1)
    ));
    let candidates_vec: Vec<String> = sitemap_urls
        .into_iter()
        .filter(|url| {
            !seen_urls.contains(url) && !is_excluded_url_path(url, &cfg.exclude_path_prefix)
        })
        .collect();
    let sitemap_candidates = candidates_vec.len();
    let markdown_dir = output_dir.join("markdown");
    tokio::fs::create_dir_all(&markdown_dir).await?;
    let manifest_path = output_dir.join("manifest.jsonl");
    let manifest_file = tokio::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&manifest_path)
        .await?;
    let mut manifest = tokio::io::BufWriter::new(manifest_file);
    let timeout = Duration::from_millis(cfg.request_timeout_ms.unwrap_or(30_000));
    let client = reqwest::Client::builder().timeout(timeout).build()?;
    let initial_idx = summary.markdown_files;
    let mut worker = BackfillWorker {
        cfg,
        client,
        markdown_dir: &markdown_dir,
        manifest: &mut manifest,
        summary,
        idx: initial_idx,
        processed: 0,
        fetched_ok: 0,
        written: 0,
        failed_fetches: 0,
    };
    let mut pending = tokio::task::JoinSet::new();
    let mut candidates = candidates_vec.into_iter();
    let concurrency = cfg
        .backfill_concurrency_limit
        .unwrap_or(cfg.batch_concurrency)
        .max(1);
    run_backfill_workers(&mut worker, &mut pending, &mut candidates, concurrency).await?;
    worker.manifest.flush().await?;
    let processed = worker.processed;
    let fetched_ok = worker.fetched_ok;
    let written = worker.written;
    let failed_fetches = worker.failed_fetches;
    drop(worker);
    log_info(&format!(
        "command=crawl sitemap_backfill_complete processed={} fetched_ok={} written={} failed={}",
        processed, fetched_ok, written, failed_fetches
    ));
    Ok(SitemapBackfillStats {
        sitemap_discovered,
        sitemap_candidates,
        processed,
        fetched_ok,
        written,
        failed: failed_fetches,
        filtered: processed.saturating_sub(written),
    })
}

async fn run_backfill_workers(
    worker: &mut BackfillWorker<'_>,
    pending: &mut tokio::task::JoinSet<(String, Result<String, String>)>,
    candidates: &mut std::vec::IntoIter<String>,
    concurrency: usize,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..concurrency {
        if let Some(url) = candidates.next() {
            spawn_backfill_task(
                pending,
                url,
                worker.client.clone(),
                worker.cfg.fetch_retries,
                worker.cfg.retry_backoff_ms,
            );
        }
    }
    while let Some(joined) = pending.join_next().await {
        worker.processed += 1;
        let (ok_inc, written_inc, failed_inc) = handle_backfill_result(
            joined,
            worker.markdown_dir,
            worker.manifest,
            worker.cfg,
            &mut worker.idx,
            worker.summary,
        )
        .await?;
        worker.fetched_ok += ok_inc;
        worker.written += written_inc;
        worker.failed_fetches += failed_inc;
        if worker.processed.is_multiple_of(50) {
            log_info(&format!(
                "command=crawl sitemap_backfill_progress processed={} fetched_ok={} written={} failed={}",
                worker.processed, worker.fetched_ok, worker.written, worker.failed_fetches
            ));
        }
        if let Some(url) = candidates.next() {
            spawn_backfill_task(
                pending,
                url,
                worker.client.clone(),
                worker.cfg.fetch_retries,
                worker.cfg.retry_backoff_ms,
            );
        }
    }
    Ok(())
}

struct BackfillWorker<'a> {
    cfg: &'a Config,
    client: reqwest::Client,
    markdown_dir: &'a Path,
    manifest: &'a mut tokio::io::BufWriter<tokio::fs::File>,
    summary: &'a mut CrawlSummary,
    idx: u32,
    processed: usize,
    fetched_ok: usize,
    written: usize,
    failed_fetches: usize,
}
