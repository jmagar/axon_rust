use spider::url::Url;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct AskCandidate {
    pub score: f64,
    pub url: String,
    pub chunk_text: String,
    pub url_tokens: HashSet<String>,
    pub chunk_tokens: HashSet<String>,
    pub rerank_score: f64,
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    let stop = [
        "the", "and", "for", "with", "that", "this", "from", "into", "how", "what", "where",
        "when", "you", "your", "are", "can", "does", "create", "make",
    ];
    let stop_words: HashSet<&str> = stop.into_iter().collect();
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3 && !stop_words.contains(*t))
        .map(str::to_string)
        .collect()
}

pub fn tokenize_text_set(text: &str) -> HashSet<String> {
    tokenize_query(text).into_iter().collect()
}

pub fn tokenize_path_set(path_or_url: &str) -> HashSet<String> {
    let path = Url::parse(path_or_url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| path_or_url.to_string());
    path.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

pub fn rerank_ask_candidates(candidates: &[AskCandidate], query: &str) -> Vec<AskCandidate> {
    let tokens: Vec<String> = tokenize_query(query);
    if tokens.is_empty() {
        return candidates.to_vec();
    }

    let mut reranked = candidates
        .iter()
        .cloned()
        .map(|mut candidate| {
            let mut lexical_boost = 0.0f64;
            for token in &tokens {
                if candidate.url_tokens.contains(token) {
                    lexical_boost += 0.045;
                }
                if candidate.chunk_tokens.contains(token) {
                    lexical_boost += 0.015;
                }
            }
            lexical_boost = lexical_boost.min(0.30);

            let docs_boost = if candidate.url.contains("/docs/")
                || candidate.url.contains("/guides/")
                || candidate.url.contains("/api/")
                || candidate.url.contains("/reference/")
            {
                0.04
            } else {
                0.0
            };
            candidate.rerank_score = candidate.score + lexical_boost + docs_boost;
            candidate
        })
        .collect::<Vec<_>>();
    reranked.sort_by(|a, b| {
        b.rerank_score
            .partial_cmp(&a.rerank_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    reranked
}

pub fn select_diverse_candidates(
    candidates: &[AskCandidate],
    target_count: usize,
    max_per_url: usize,
) -> Vec<AskCandidate> {
    if candidates.len() <= target_count {
        return candidates.to_vec();
    }

    let mut selected: Vec<AskCandidate> = Vec::new();
    let mut per_url_count: HashMap<String, usize> = HashMap::new();

    for candidate in candidates {
        if selected.len() >= target_count {
            break;
        }
        if per_url_count.contains_key(&candidate.url) {
            continue;
        }
        selected.push(candidate.clone());
        per_url_count.insert(candidate.url.clone(), 1);
    }

    for candidate in candidates {
        if selected.len() >= target_count {
            break;
        }
        let used = *per_url_count.get(&candidate.url).unwrap_or(&0);
        if used >= max_per_url {
            continue;
        }
        selected.push(candidate.clone());
        per_url_count.insert(candidate.url.clone(), used + 1);
    }

    selected
}

pub fn implementation_label() -> &'static str {
    "v2"
}
