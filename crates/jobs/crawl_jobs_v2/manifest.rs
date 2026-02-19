#[allow(dead_code)]
pub(crate) const STAGE_NAME: &str = "manifest";

use crate::axon_cli::crates::jobs::batch_jobs::InjectionCandidate;
use std::collections::HashSet;
use std::path::Path;

#[allow(dead_code)]
pub(crate) async fn read_manifest_urls(path: &Path) -> Result<HashSet<String>, std::io::Error> {
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

#[allow(dead_code)]
pub(crate) async fn read_manifest_candidates(
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

        let v2 = read_manifest_urls(fixture.path()).await.expect("v2 parse");
        let expected = HashSet::from(["https://a.test".to_string(), "https://b.test".to_string()]);
        assert_eq!(v2, expected);
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

        let v2 = read_manifest_candidates(fixture.path())
            .await
            .expect("v2 parse");
        let tuples: Vec<(String, usize)> = v2
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
