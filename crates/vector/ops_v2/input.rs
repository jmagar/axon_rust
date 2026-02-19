use crate::axon_cli::crates::core::http::normalize_url;
use std::collections::HashSet;

pub fn chunk_text(text: &str) -> Vec<String> {
    const MAX: usize = 2000;
    const OVERLAP: usize = 200;

    let offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
    if offsets.len() <= MAX {
        return vec![text.to_string()];
    }

    let char_count = offsets.len();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < char_count {
        let end = (i + MAX).min(char_count);
        let byte_start = offsets[i];
        let byte_end = if end < char_count {
            offsets[end]
        } else {
            text.len()
        };
        out.push(text[byte_start..byte_end].to_string());
        if end == char_count {
            break;
        }
        i = end.saturating_sub(OVERLAP);
    }
    out
}

pub fn url_lookup_candidates(target: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    let normalized = normalize_url(target);
    let variants = [
        target.to_string(),
        normalized.clone(),
        normalized.trim_end_matches('/').to_string(),
        format!("{}/", normalized.trim_end_matches('/')),
    ];
    for variant in variants {
        if variant.is_empty() {
            continue;
        }
        if seen.insert(variant.clone()) {
            out.push(variant);
        }
    }
    out
}
