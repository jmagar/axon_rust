use crate::crates::cli::commands::job_contracts::{
    JobCancelResponse, JobErrorsResponse, JobStatusResponse, JobSummaryEntry,
};
use crate::crates::core::config::{CommandKind, Config};
use crate::crates::core::http::normalize_url;
use crate::crates::core::logging::log_warn;
use crate::crates::core::ui::{accent, muted, primary, status_text, symbol_for_status};
use crate::crates::services::types::ServiceTimeRange;
use std::collections::HashSet;

/// Convert a CLI time-range string to the services-layer [`ServiceTimeRange`] enum.
///
/// Shared by `search` and `research` commands.
pub fn parse_service_time_range(value: Option<&str>) -> Option<ServiceTimeRange> {
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some("day") => Some(ServiceTimeRange::Day),
        Some("week") => Some(ServiceTimeRange::Week),
        Some("month") => Some(ServiceTimeRange::Month),
        Some("year") => Some(ServiceTimeRange::Year),
        _ => None,
    }
}

/// Truncate a string to at most `max_chars` characters, slicing on a char
/// boundary so multi-byte UTF-8 sequences never panic.
pub fn truncate_chars(s: &str, max_chars: usize) -> &str {
    s.char_indices().nth(max_chars).map_or(s, |(i, _)| &s[..i])
}

fn expand_numeric_range(start: i64, end: i64, step: i64) -> Vec<String> {
    let mut out = Vec::new();
    if step == 0 {
        return out;
    }
    let mut current = start;
    if start <= end && step > 0 {
        while current <= end {
            out.push(current.to_string());
            current += step;
        }
    } else if start >= end && step < 0 {
        while current >= end {
            out.push(current.to_string());
            current += step;
        }
    }
    out
}

fn expand_brace_token(token: &str) -> Vec<String> {
    let trimmed = token.trim();
    if let Some((lhs, rhs)) = trimmed.split_once("..") {
        let lhs = lhs.trim();
        let rhs = rhs.trim();
        if let (Ok(start), Ok(end)) = (lhs.parse::<i64>(), rhs.parse::<i64>()) {
            let step = if start <= end { 1 } else { -1 };
            let values = expand_numeric_range(start, end, step);
            if !values.is_empty() {
                return values;
            }
        }
    }
    trimmed
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

const MAX_EXPANSION_DEPTH: usize = 10;

fn expand_url_glob_seed(seed: &str) -> Vec<String> {
    expand_url_glob_seed_inner(seed, 0)
}

fn expand_url_glob_seed_inner(seed: &str, depth: usize) -> Vec<String> {
    if depth >= MAX_EXPANSION_DEPTH {
        log_warn(&format!(
            "URL glob expansion reached MAX_EXPANSION_DEPTH ({MAX_EXPANSION_DEPTH}) for seed: {seed}. Truncating."
        ));
        return vec![seed.to_string()];
    }
    let Some(open_idx) = seed.find('{') else {
        return vec![seed.to_string()];
    };
    let Some(close_rel) = seed[open_idx + 1..].find('}') else {
        return vec![seed.to_string()];
    };
    let close_idx = open_idx + 1 + close_rel;
    let prefix = &seed[..open_idx];
    let token = &seed[open_idx + 1..close_idx];
    let suffix = &seed[close_idx + 1..];
    let choices = expand_brace_token(token);
    if choices.is_empty() {
        return vec![seed.to_string()];
    }

    let mut out = Vec::new();
    for choice in choices {
        let next = format!("{prefix}{choice}{suffix}");
        out.extend(expand_url_glob_seed_inner(&next, depth + 1));
    }
    out
}

pub fn parse_urls(cfg: &Config) -> Vec<String> {
    let mut out = Vec::new();
    let mut raw = Vec::new();
    if let Some(csv) = &cfg.urls_csv {
        raw.extend(
            csv.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
        );
    }
    raw.extend(
        cfg.url_glob
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    );
    raw.extend(
        cfg.positional
            .iter()
            .flat_map(|s| s.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    );
    let mut seen = HashSet::new();
    for seed in raw {
        for expanded in expand_url_glob_seed(&seed) {
            let normalized = normalize_url(&expanded);
            if seen.insert(normalized.clone()) {
                out.push(normalized);
            }
        }
    }
    out
}

pub fn start_url_from_cfg(cfg: &Config) -> String {
    if matches!(
        cfg.command,
        CommandKind::Crawl | CommandKind::Refresh | CommandKind::Extract | CommandKind::Embed
    ) && matches!(
        cfg.positional.first().map(|s| s.as_str()),
        Some("status" | "cancel" | "errors" | "list" | "cleanup" | "clear" | "worker" | "doctor")
    ) {
        return cfg.start_url.clone();
    }

    if matches!(
        cfg.command,
        CommandKind::Scrape
            | CommandKind::Map
            | CommandKind::Crawl
            | CommandKind::Refresh
            | CommandKind::Extract
            | CommandKind::Embed
            | CommandKind::Screenshot
    ) {
        let selected = cfg
            .positional
            .first()
            .cloned()
            .unwrap_or_else(|| cfg.start_url.clone());
        return normalize_url(&selected);
    }

    cfg.start_url.clone()
}

pub trait JobStatus {
    fn id(&self) -> uuid::Uuid;
    fn status(&self) -> &str;
    fn created_at(&self) -> chrono::DateTime<chrono::Utc>;
    fn updated_at(&self) -> chrono::DateTime<chrono::Utc>;
    fn error_text(&self) -> Option<&str>;
    fn to_status_response_json(&self) -> serde_json::Value;
    fn to_summary_entry_json(&self) -> serde_json::Value;
    fn to_errors_response_json(&self) -> serde_json::Value;
}

impl JobStatus for crate::crates::jobs::crawl::CrawlJob {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn status(&self) -> &str {
        &self.status
    }
    fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }
    fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.updated_at
    }
    fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }
    fn to_status_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobStatusResponse::from_crawl(self)).unwrap_or_default()
    }
    fn to_summary_entry_json(&self) -> serde_json::Value {
        serde_json::to_value(JobSummaryEntry::from_crawl(self)).unwrap_or_default()
    }
    fn to_errors_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobErrorsResponse::from_job(
            self.id,
            self.status.clone(),
            self.error_text.clone(),
        ))
        .unwrap_or_default()
    }
}

impl JobStatus for crate::crates::jobs::extract::ExtractJob {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn status(&self) -> &str {
        &self.status
    }
    fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }
    fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.updated_at
    }
    fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }
    fn to_status_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobStatusResponse::from_extract(self)).unwrap_or_default()
    }
    fn to_summary_entry_json(&self) -> serde_json::Value {
        serde_json::to_value(JobSummaryEntry::from_extract(self)).unwrap_or_default()
    }
    fn to_errors_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobErrorsResponse::from_job(
            self.id,
            self.status.clone(),
            self.error_text.clone(),
        ))
        .unwrap_or_default()
    }
}

impl JobStatus for crate::crates::jobs::ingest::IngestJob {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn status(&self) -> &str {
        &self.status
    }
    fn created_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.created_at
    }
    fn updated_at(&self) -> chrono::DateTime<chrono::Utc> {
        self.updated_at
    }
    fn error_text(&self) -> Option<&str> {
        self.error_text.as_deref()
    }
    fn to_status_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobStatusResponse::from_ingest(self)).unwrap_or_default()
    }
    fn to_summary_entry_json(&self) -> serde_json::Value {
        serde_json::to_value(JobSummaryEntry::from_ingest(self)).unwrap_or_default()
    }
    fn to_errors_response_json(&self) -> serde_json::Value {
        serde_json::to_value(JobErrorsResponse::from_job(
            self.id,
            self.status.clone(),
            self.error_text.clone(),
        ))
        .unwrap_or_default()
    }
}

pub fn handle_job_status<T: JobStatus + serde::Serialize>(
    cfg: &Config,
    job: Option<T>,
    job_id: uuid::Uuid,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match job {
        Some(job) => {
            if cfg.json_output {
                let json = job.to_status_response_json();
                println!("{}", serde_json::to_string_pretty(&json)?);
            } else {
                println!(
                    "{} {}",
                    primary(&format!("{command_name} Status for")),
                    accent(&job.id().to_string())
                );
                println!(
                    "  {} {}",
                    symbol_for_status(job.status()),
                    status_text(job.status())
                );
                println!("  {} {}", muted("Created:"), job.created_at());
                println!("  {} {}", muted("Updated:"), job.updated_at());
                if let Some(err) = job.error_text() {
                    println!("  {} {}", muted("Error:"), err);
                }
                println!("Job ID: {}", job.id());
            }
        }
        None => {
            if cfg.json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": format!("job not found: {job_id}"),
                        "job_id": job_id
                    })
                );
            } else {
                println!(
                    "{} {}",
                    symbol_for_status("error"),
                    muted(&format!("job not found: {job_id}"))
                );
            }
        }
    }
    Ok(())
}

pub fn handle_job_cancel(
    cfg: &Config,
    id: uuid::Uuid,
    canceled: bool,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.json_output {
        let resp = JobCancelResponse::new(id, canceled);
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else if canceled {
        println!(
            "{} canceled {command_name} job {}",
            symbol_for_status("canceled"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    } else {
        println!(
            "{} no cancellable {command_name} job found for {}",
            symbol_for_status("error"),
            accent(&id.to_string())
        );
        println!("Job ID: {id}");
    }
    Ok(())
}

pub fn handle_job_errors<T: JobStatus + serde::Serialize>(
    cfg: &Config,
    job: Option<T>,
    id: uuid::Uuid,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match job {
        Some(job) => {
            if cfg.json_output {
                let contract = job.to_errors_response_json();
                println!("{}", serde_json::to_string_pretty(&contract)?);
            } else {
                println!(
                    "{} {} job {} {}",
                    symbol_for_status(job.status()),
                    command_name,
                    accent(&id.to_string()),
                    status_text(job.status())
                );
                println!(
                    "  {} {}",
                    muted("Error:"),
                    job.error_text().unwrap_or("None")
                );
                println!("Job ID: {id}");
            }
        }
        None => {
            if cfg.json_output {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": format!("job not found: {id}"),
                        "job_id": id
                    })
                );
            } else {
                println!(
                    "{} {}",
                    symbol_for_status("error"),
                    muted(&format!("job not found: {id}"))
                );
            }
        }
    }
    Ok(())
}

pub fn handle_job_list<T: JobStatus + serde::Serialize>(
    cfg: &Config,
    jobs: Vec<T>,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.json_output {
        let entries: Vec<serde_json::Value> =
            jobs.iter().map(|j| j.to_summary_entry_json()).collect();
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!("{}", primary(&format!("{command_name} Jobs")));
    if jobs.is_empty() {
        println!("  {}", muted(&format!("No {command_name} jobs found.")));
        return Ok(());
    }

    for job in jobs {
        println!(
            "  {} {} {}",
            symbol_for_status(job.status()),
            accent(&job.id().to_string()),
            status_text(job.status())
        );
    }
    Ok(())
}

pub fn handle_job_cleanup(
    cfg: &Config,
    removed: u64,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.json_output {
        println!("{}", serde_json::json!({ "removed": removed }));
    } else {
        println!(
            "{} removed {} {command_name} jobs",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

pub fn handle_job_clear(
    cfg: &Config,
    removed: u64,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.json_output {
        println!(
            "{}",
            serde_json::json!({ "removed": removed, "queue_purged": true })
        );
    } else {
        println!(
            "{} cleared {} {command_name} jobs and purged queue",
            symbol_for_status("completed"),
            removed
        );
    }
    Ok(())
}

pub fn handle_job_recover(
    cfg: &Config,
    reclaimed: u64,
    command_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if cfg.json_output {
        println!("{}", serde_json::json!({ "reclaimed": reclaimed }));
    } else {
        println!(
            "{} reclaimed {} stale {command_name} jobs",
            symbol_for_status("completed"),
            reclaimed
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::expand_url_glob_seed;
    use super::truncate_chars;

    #[test]
    fn truncate_chars_multibyte() {
        // ASCII — no truncation needed
        assert_eq!(truncate_chars("hello", 5), "hello");
        // ASCII — truncation
        assert_eq!(truncate_chars("hello", 3), "hel");
        // Multi-byte char boundary
        assert_eq!(truncate_chars("héllo", 3), "hél");
        // Zero limit
        assert_eq!(truncate_chars("hello", 0), "");
        // Limit exceeds length
        assert_eq!(truncate_chars("hi", 10), "hi");
    }

    #[test]
    fn expands_url_glob_range() {
        let expanded = expand_url_glob_seed("https://example.com/page/{1..3}");
        assert_eq!(
            expanded,
            vec![
                "https://example.com/page/1".to_string(),
                "https://example.com/page/2".to_string(),
                "https://example.com/page/3".to_string()
            ]
        );
    }

    #[test]
    fn expands_url_glob_list_and_nested() {
        let expanded = expand_url_glob_seed("https://example.com/{news,docs}/{a,b}");
        assert_eq!(
            expanded,
            vec![
                "https://example.com/news/a".to_string(),
                "https://example.com/news/b".to_string(),
                "https://example.com/docs/a".to_string(),
                "https://example.com/docs/b".to_string()
            ]
        );
    }
}
