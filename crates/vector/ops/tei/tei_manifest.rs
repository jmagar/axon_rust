use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

pub(super) fn read_manifest_url_map(markdown_dir: &Path) -> HashMap<PathBuf, String> {
    let Some(parent) = markdown_dir.parent() else {
        return HashMap::new();
    };
    let manifest = parent.join("manifest.jsonl");
    let file = match std::fs::File::open(&manifest) {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };
    let mut out = HashMap::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let parsed: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let file_path = match parsed.get("file_path").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v,
            _ => continue,
        };
        let url = match parsed.get("url").and_then(|v| v.as_str()) {
            Some(v) if !v.is_empty() => v.to_string(),
            _ => continue,
        };
        let normalized =
            std::fs::canonicalize(file_path).unwrap_or_else(|_| PathBuf::from(file_path));
        out.insert(normalized, url);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::read_manifest_url_map;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn read_manifest_url_map_maps_markdown_file_to_url() {
        let root = std::env::temp_dir().join(format!("axon-tei-manifest-test-{}", Uuid::new_v4()));
        let markdown_dir = root.join("markdown");
        fs::create_dir_all(&markdown_dir).expect("create markdown dir");

        let markdown_file = markdown_dir.join("001-example.md");
        fs::write(&markdown_file, "# test").expect("write markdown file");

        let manifest_path = root.join("manifest.jsonl");
        let line = serde_json::json!({
            "url": "https://example.com/docs",
            "file_path": markdown_file.to_string_lossy(),
            "markdown_chars": 42
        });
        fs::write(&manifest_path, format!("{line}\n")).expect("write manifest");

        let mapped = read_manifest_url_map(&markdown_dir);
        let key =
            fs::canonicalize(&markdown_file).unwrap_or_else(|_| PathBuf::from(&markdown_file));
        assert_eq!(
            mapped.get(&key).map(String::as_str),
            Some("https://example.com/docs")
        );

        let _ = fs::remove_dir_all(&root);
    }
}
