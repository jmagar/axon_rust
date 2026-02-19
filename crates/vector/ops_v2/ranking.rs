use spider::url::Url;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "the", "and", "for", "with", "that", "this", "from", "into", "how", "what", "where",
        "when", "you", "your", "are", "can", "does", "create", "make",
    ]
    .into_iter()
    .collect()
});

#[derive(Debug, Clone)]
pub struct AskCandidate {
    pub score: f64,
    pub url: String,
    pub path: String,
    pub chunk_text: String,
    pub url_tokens: HashSet<String>,
    pub chunk_tokens: HashSet<String>,
    pub rerank_score: f64,
}

pub fn tokenize_query(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOP_WORDS.contains(*t))
        .map(str::to_string)
        .collect()
}

pub fn tokenize_text_set(text: &str) -> HashSet<String> {
    tokenize_query(text).into_iter().collect()
}

pub fn extract_path_from_url(path_or_url: &str) -> String {
    Url::parse(path_or_url)
        .ok()
        .map(|u| u.path().to_string())
        .unwrap_or_else(|| path_or_url.to_string())
}

pub fn tokenize_path_set(path_or_url: &str) -> HashSet<String> {
    path_or_url
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

pub fn rerank_ask_candidates(
    candidates: &[AskCandidate],
    query_tokens: &[String],
) -> Vec<AskCandidate> {
    if query_tokens.is_empty() {
        return candidates.to_vec();
    }

    let mut reranked = candidates
        .iter()
        .cloned()
        .map(|mut candidate| {
            let mut lexical_boost = 0.0f64;
            for token in query_tokens {
                if candidate.url_tokens.contains(token) {
                    lexical_boost += 0.045;
                }
                if candidate.chunk_tokens.contains(token) {
                    lexical_boost += 0.015;
                }
            }
            lexical_boost = lexical_boost.min(0.30);

            let docs_boost = if candidate.path.contains("/docs/")
                || candidate.path.contains("/guides/")
                || candidate.path.contains("/api/")
                || candidate.path.contains("/reference/")
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
) -> Vec<usize> {
    let all_indices = (0..candidates.len()).collect::<Vec<_>>();
    select_diverse_candidates_from_indices(candidates, &all_indices, target_count, max_per_url)
}

pub fn select_diverse_candidates_from_indices(
    candidates: &[AskCandidate],
    candidate_indices: &[usize],
    target_count: usize,
    max_per_url: usize,
) -> Vec<usize> {
    if candidate_indices.len() <= target_count {
        return candidate_indices.to_vec();
    }

    let mut selected: Vec<usize> = Vec::new();
    let mut per_url_count: HashMap<String, usize> = HashMap::new();

    for &candidate_idx in candidate_indices {
        if selected.len() >= target_count {
            break;
        }
        let candidate = &candidates[candidate_idx];
        if per_url_count.contains_key(&candidate.url) {
            continue;
        }
        selected.push(candidate_idx);
        per_url_count.insert(candidate.url.clone(), 1);
    }

    for &candidate_idx in candidate_indices {
        if selected.len() >= target_count {
            break;
        }
        let candidate = &candidates[candidate_idx];
        let used = *per_url_count.get(&candidate.url).unwrap_or(&0);
        if used >= max_per_url {
            continue;
        }
        selected.push(candidate_idx);
        per_url_count.insert(candidate.url.clone(), used + 1);
    }

    selected
}

pub fn implementation_label() -> &'static str {
    "v2"
}
