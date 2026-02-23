use crate::crates::jobs::batch_jobs::InjectionCandidate;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub url: String,
    pub relative_path: String,
    pub markdown_chars: usize,
    #[serde(default)]
    pub content_hash: Option<String>,
    #[serde(default = "default_true")]
    pub changed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrawlAuditDiff {
    pub start_url: String,
    pub previous_count: usize,
    pub current_count: usize,
    pub added_count: usize,
    pub removed_count: usize,
    pub unchanged_count: usize,
    pub cache_hit: bool,
    pub cache_source: Option<String>,
}

fn default_true() -> bool {
    true
}

use std::time::SystemTime;

pub async fn manifest_cache_is_stale(manifest_path: &Path, ttl_secs: u64) -> bool {
    tokio::fs::metadata(manifest_path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
        .is_some_and(|age| age.as_secs() > ttl_secs)
}

pub async fn write_audit_diff(
    output_dir: &Path,
    start_url: &str,
    previous: &HashSet<String>,
    current: &HashSet<String>,
    cache_hit: bool,
    cache_source: Option<String>,
) -> Result<(PathBuf, CrawlAuditDiff), std::io::Error> {
    let unchanged_count = previous.intersection(current).count();
    let added_count = current.difference(previous).count();
    let removed_count = previous.difference(current).count();
    let report = CrawlAuditDiff {
        start_url: start_url.to_string(),
        previous_count: previous.len(),
        current_count: current.len(),
        added_count,
        removed_count,
        unchanged_count,
        cache_hit,
        cache_source,
    };

    let audit_dir = output_dir.join("audit");
    tokio::fs::create_dir_all(&audit_dir).await?;
    let report_path = audit_dir.join("diff-report.json");
    let payload = serde_json::to_string_pretty(&report).map_err(std::io::Error::other)?;
    tokio::fs::write(&report_path, payload).await?;
    Ok((report_path, report))
}

pub async fn read_manifest_urls(path: &Path) -> Result<HashSet<String>, std::io::Error> {
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = HashSet::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        out.insert(url.to_string());
    }
    Ok(out)
}

pub async fn read_manifest_data(
    path: &Path,
) -> Result<HashMap<String, ManifestEntry>, std::io::Error> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = HashMap::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(entry) = serde_json::from_str::<ManifestEntry>(line) else {
            continue;
        };
        out.insert(entry.url.clone(), entry);
    }
    Ok(out)
}

pub async fn read_manifest_candidates(
    path: &Path,
) -> Result<Vec<InjectionCandidate>, std::io::Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = tokio::fs::read_to_string(path).await?;
    let mut out = Vec::new();
    for line in content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(url) = json.get("url").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(markdown_chars) = json.get("markdown_chars").and_then(|value| value.as_u64())
        else {
            continue;
        };
        out.push(InjectionCandidate {
            url: url.to_string(),
            markdown_chars: markdown_chars as usize,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{read_manifest_candidates, read_manifest_urls};
    use std::collections::HashSet;

    #[tokio::test]
    async fn read_manifest_urls_returns_expected_set() {
        let fixture = tempfile::NamedTempFile::new().expect("create tempfile");
        tokio::fs::write(
            fixture.path(),
            "\nnot-json\n{\"url\":\"https://a.test\"}\n{\"url\":\"https://a.test\"}\n{\"other\":1}\n{\"url\":\"https://b.test\"}\n",
        )
        .await
        .expect("write fixture");

        let result = read_manifest_urls(fixture.path())
            .await
            .expect("parse manifest");
        let expected = HashSet::from(["https://a.test".to_string(), "https://b.test".to_string()]);
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn read_manifest_candidates_returns_expected_values_in_order() {
        let fixture = tempfile::NamedTempFile::new().expect("create tempfile");
        tokio::fs::write(
            fixture.path(),
            "\n{\"url\":\"https://a.test\",\"markdown_chars\":12}\n{\"url\":\"https://b.test\"}\n{\"url\":\"https://c.test\",\"markdown_chars\":0}\nnot-json\n{\"url\":\"https://d.test\",\"markdown_chars\":9}\n",
        )
        .await
        .expect("write fixture");

        let candidates = read_manifest_candidates(fixture.path())
            .await
            .expect("parse candidates");
        let tuples: Vec<(String, usize)> = candidates
            .into_iter()
            .map(|candidate| (candidate.url, candidate.markdown_chars))
            .collect();
        let expected = vec![
            ("https://a.test".to_string(), 12usize),
            ("https://c.test".to_string(), 0usize),
            ("https://d.test".to_string(), 9usize),
        ];
        assert_eq!(tuples, expected);
    }
}
