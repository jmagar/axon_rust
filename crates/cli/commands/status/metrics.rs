use crate::crates::core::ui::{metric, muted, primary, symbol_for_status};
use chrono::{DateTime, Utc};
use serde_json::Value;

/// Human-readable relative age: "3s ago", "12m ago", "2h ago", "4d ago".
pub(super) fn format_age(ts: &DateTime<Utc>) -> String {
    let secs = (Utc::now() - *ts).num_seconds().max(0) as u64;
    if secs < 60 {
        format!("{secs}s ago")
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Best timestamp to show: finished_at for terminal jobs, updated_at for active ones.
pub(super) fn job_age(
    status: &str,
    finished_at: Option<&DateTime<Utc>>,
    updated_at: &DateTime<Utc>,
) -> String {
    match status {
        "completed" | "failed" | "canceled" => finished_at
            .map(format_age)
            .unwrap_or_else(|| format_age(updated_at)),
        _ => format_age(updated_at),
    }
}

/// First line of error_text, truncated to 60 chars.
pub(super) fn format_error(error_text: Option<&str>) -> Option<String> {
    let text = error_text?.trim();
    if text.is_empty() {
        return None;
    }
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.len() > 60 {
        Some(format!("{}…", &first_line[..60]))
    } else {
        Some(first_line.to_string())
    }
}

/// Section header symbol: ✗ if any failed, ◐ if any active, ✓ if all terminal.
pub(super) fn section_symbol(statuses: &[&str]) -> String {
    if statuses.iter().any(|s| matches!(*s, "failed" | "error")) {
        symbol_for_status("failed")
    } else if statuses
        .iter()
        .any(|s| matches!(*s, "pending" | "running" | "processing" | "scraping"))
    {
        symbol_for_status("pending")
    } else {
        symbol_for_status("completed")
    }
}

pub(super) fn extract_metrics_suffix(result_json: Option<&Value>, url_count: usize) -> String {
    let sep = muted(" | ");
    let mut parts = vec![metric(url_count, "urls")];
    if let Some(total_items) = result_json
        .and_then(|r| r.get("total_items"))
        .and_then(|v| v.as_u64())
    {
        parts.push(metric(total_items, "items"));
    }
    if let Some(pages) = result_json
        .and_then(|r| r.get("pages_visited"))
        .and_then(|v| v.as_u64())
    {
        parts.push(metric(pages, "pages"));
    }
    format!("{sep}{}", parts.join(&sep))
}

pub(super) fn embed_metrics_suffix(status: &str, result_json: Option<&Value>) -> String {
    let sep = muted(" | ");
    if matches!(status, "pending" | "running" | "processing") {
        if let (Some(done), Some(total)) = (
            result_json
                .and_then(|r| r.get("docs_completed"))
                .and_then(|v| v.as_u64()),
            result_json
                .and_then(|r| r.get("docs_total"))
                .and_then(|v| v.as_u64()),
        ) {
            return format!(
                "{sep}{}{}{} {}",
                primary(&done.to_string()),
                muted("/"),
                primary(&total.to_string()),
                muted("docs")
            );
        }
        return String::new();
    }
    let docs = result_json
        .and_then(|r| r.get("docs_embedded"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let chunks = result_json
        .and_then(|r| r.get("chunks_embedded"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if docs == 0 && chunks == 0 {
        return String::new();
    }
    format!(
        "{sep}{}{sep}{}",
        metric(docs, "docs"),
        metric(chunks, "chunks")
    )
}

pub(super) fn ingest_metrics_suffix(status: &str, result_json: Option<&Value>) -> String {
    if matches!(status, "pending" | "running" | "processing") {
        return String::new();
    }
    let chunks = result_json
        .and_then(|r| r.get("chunks_embedded"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    if chunks == 0 {
        return String::new();
    }
    format!("{}{}", muted(" | "), metric(chunks, "chunks"))
}

pub(super) fn summarize_urls(urls_json: &Value) -> (String, usize) {
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

/// Extract crawl job UUID from an embed input path like
/// `.cache/axon-rust/output/jobs/<UUID>/markdown`.
pub(super) fn crawl_uuid_from_embed_input(input: &str) -> Option<uuid::Uuid> {
    let after_jobs = input.split("/jobs/").nth(1)?;
    let candidate = after_jobs.split('/').next()?;
    candidate.parse::<uuid::Uuid>().ok()
}

/// Resolve a human-readable label for an embed job's input_text.
/// Priority: crawl URL lookup → URL passthrough → pretty path.
pub(super) fn display_embed_input<'a>(
    input: &'a str,
    crawl_url_map: &std::collections::HashMap<uuid::Uuid, &'a str>,
) -> std::borrow::Cow<'a, str> {
    if let Some(url) =
        crawl_uuid_from_embed_input(input).and_then(|uid| crawl_url_map.get(&uid).copied())
    {
        return std::borrow::Cow::Borrowed(url);
    }
    if input.starts_with("http://") || input.starts_with("https://") {
        return std::borrow::Cow::Borrowed(input);
    }
    let path = std::path::Path::new(input);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(input);
    if name == "markdown" {
        return std::borrow::Cow::Owned(
            path.parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|parent| format!("{parent}/markdown"))
                .unwrap_or_else(|| "output/markdown".to_string()),
        );
    }
    std::borrow::Cow::Borrowed(path.to_str().unwrap_or(input))
}
