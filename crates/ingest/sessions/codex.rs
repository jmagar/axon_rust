use super::{resolve_collection, IngestResult, SessionStateTracker};
use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
use futures_util::stream::{FuturesUnordered, StreamExt};
use indicatif::MultiProgress;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;

pub(super) async fn ingest_codex_sessions(
    cfg: &Config,
    state: &SessionStateTracker,
    multi: &MultiProgress,
) -> IngestResult<usize> {
    let root = super::expand_home("~/.codex/sessions");
    if !root.exists() {
        return Ok(0);
    }

    let pb = multi.add(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.yellow} Codex: {msg}")
            .unwrap(),
    );
    pb.enable_steady_tick(Duration::from_millis(100));

    let mut total = 0;
    let mut dir_entries = vec![root];
    let mut futures = FuturesUnordered::new();

    while let Some(current_dir) = dir_entries.pop() {
        let mut read_dir = fs::read_dir(current_dir)
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            let path = entry.path();
            if path.is_dir() {
                dir_entries.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "jsonl") {
                continue;
            }
            let meta = fs::metadata(&path)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let mtime = meta
                .modified()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            if state.should_skip(&path, mtime, meta.len()).await {
                continue;
            }

            let collection = resolve_collection(cfg, "codex");
            let cfg_clone = cfg.clone();
            let size = meta.len();
            futures.push(tokio::spawn(async move {
                let res = process_codex_file(&cfg_clone, path.clone(), collection).await;
                (path, mtime, size, res)
            }));

            if futures.len() >= 32 {
                if let Some(res) = futures.next().await {
                    let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
                    match r {
                        Ok(count) => {
                            total += count;
                            state.mark_indexed(&p, m, s).await;
                        }
                        Err(e) => log_warn(&format!("Codex file {}: {e}", p.display())),
                    }
                }
            }
        }
    }

    while let Some(res) = futures.next().await {
        let (p, m, s, r) = res.map_err(|e| anyhow::anyhow!(e.to_string()))?;
        match r {
            Ok(count) => {
                total += count;
                state.mark_indexed(&p, m, s).await;
            }
            Err(e) => log_warn(&format!("Codex file {}: {e}", p.display())),
        }
    }

    pb.finish_with_message(format!("indexed {} chunks", total));
    Ok(total)
}

async fn process_codex_file(
    cfg: &Config,
    path: PathBuf,
    collection: String,
) -> IngestResult<usize> {
    let content = fs::read_to_string(&path)
        .await
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let mut session_text = String::new();

    let mut session_cfg = cfg.clone();
    session_cfg.collection = collection;

    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val["type"] != "response_item" {
            continue;
        }
        let role = val["payload"]["role"].as_str().unwrap_or("unknown");
        if let Some(arr) = val["payload"]["content"].as_array() {
            let mut combined = String::new();
            for item in arr {
                if let Some(t) = item["text"].as_str() {
                    combined.push_str(t);
                    combined.push('\n');
                } else if let Some(t) = item["input_text"].as_str() {
                    combined.push_str(t);
                    combined.push('\n');
                }
            }
            if !combined.trim().is_empty() {
                session_text.push_str(&format!("\n\n### {}:\n{}", role.to_uppercase(), combined));
            }
        }
    }

    if session_text.trim().is_empty() {
        return Ok(0);
    }

    let url = format!("file://{}", path.display());
    let title = path.file_name().and_then(|n| n.to_str());

    let mut attempt = 0;
    loop {
        let res =
            embed_text_with_metadata(&session_cfg, &session_text, &url, "codex_session", title)
                .await
                .map_err(|e| anyhow::anyhow!(e.to_string()));
        match res {
            Ok(n) => return Ok(n),
            Err(e) => {
                if attempt < 3 {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(attempt * 500)).await;
                    log_warn(&format!("retry {} for {}: {}", attempt, url, e));
                } else {
                    return Err(e);
                }
            }
        }
    }
}

/// Extract session text from Codex JSONL (pure, no I/O) for unit tests.
#[cfg(test)]
fn parse_codex_jsonl(content: &str) -> String {
    let mut session_text = String::new();
    for line in content.lines() {
        let Ok(val) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if val["type"] != "response_item" {
            continue;
        }
        let role = val["payload"]["role"].as_str().unwrap_or("unknown");
        if let Some(arr) = val["payload"]["content"].as_array() {
            let mut combined = String::new();
            for item in arr {
                if let Some(t) = item["text"].as_str() {
                    combined.push_str(t);
                    combined.push('\n');
                } else if let Some(t) = item["input_text"].as_str() {
                    combined.push_str(t);
                    combined.push('\n');
                }
            }
            if !combined.trim().is_empty() {
                session_text.push_str(&format!("\n\n### {}:\n{}", role.to_uppercase(), combined));
            }
        }
    }
    session_text
}

#[cfg(test)]
mod tests {
    use super::parse_codex_jsonl;

    // --- parse_codex_jsonl ---

    #[test]
    fn parse_valid_codex_jsonl_text_field() {
        let jsonl = "{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"How do I use async/await?\"}]}}\n\
                     {\"type\":\"response_item\",\"payload\":{\"role\":\"assistant\",\"content\":[{\"text\":\"Use the async keyword.\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(result.contains("### USER:"));
        assert!(result.contains("How do I use async/await?"));
        assert!(result.contains("### ASSISTANT:"));
        assert!(result.contains("Use the async keyword."));
    }

    #[test]
    fn parse_valid_codex_jsonl_input_text_field() {
        // input_text is the alternate key name for user input blocks
        let jsonl = "{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"input_text\":\"Explain ownership\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(result.contains("Explain ownership"));
    }

    #[test]
    fn parse_codex_jsonl_skips_non_response_item_types() {
        let jsonl = "{\"type\":\"session_start\",\"payload\":{\"id\":\"sess-abc\"}}\n\
                     {\"type\":\"response_item\",\"payload\":{\"role\":\"assistant\",\"content\":[{\"text\":\"Hello!\"}]}}\n\
                     {\"type\":\"session_end\",\"payload\":{}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(!result.contains("sess-abc"));
        assert!(result.contains("Hello!"));
    }

    #[test]
    fn parse_codex_jsonl_malformed_lines_no_panic() {
        let jsonl = "this is not json\n\
                     {\"incomplete\":\n\
                     {\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"Valid\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(result.contains("Valid"));
    }

    #[test]
    fn parse_codex_jsonl_empty_input_returns_empty() {
        assert!(parse_codex_jsonl("").trim().is_empty());
    }

    #[test]
    fn parse_codex_jsonl_multiple_blocks_concatenated() {
        let jsonl = "{\"type\":\"response_item\",\"payload\":{\"role\":\"assistant\",\"content\":[{\"text\":\"Part A. \"},{\"text\":\"Part B.\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(result.contains("Part A."));
        assert!(result.contains("Part B."));
    }

    #[test]
    fn parse_codex_jsonl_whitespace_only_content_skipped() {
        let jsonl = "{\"type\":\"response_item\",\"payload\":{\"role\":\"user\",\"content\":[{\"text\":\"   \"}]}}\n\
                     {\"type\":\"response_item\",\"payload\":{\"role\":\"assistant\",\"content\":[{\"text\":\"Answer\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(!result.contains("### USER:"));
        assert!(result.contains("Answer"));
    }

    #[test]
    fn parse_codex_jsonl_unknown_role_falls_back_to_unknown() {
        let jsonl =
            "{\"type\":\"response_item\",\"payload\":{\"content\":[{\"text\":\"Mystery\"}]}}";
        let result = parse_codex_jsonl(jsonl);
        assert!(result.contains("### UNKNOWN:"));
        assert!(result.contains("Mystery"));
    }
}
