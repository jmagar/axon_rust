use spider::url::Url;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static STOP_WORDS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    // Structural/syntactic words only. Content verbs like "make", "create", "build"
    // encode user intent and must NOT be stripped — they distinguish "how to USE a
    // library" from "how to IMPLEMENT an interface."
    //
    // Extended from TS counterpart: high-frequency doc words that add noise without
    // distinguishing what a page is actually about.
    [
        "the", "and", "for", "with", "that", "this", "from", "into", "how", "what", "where",
        "when", "you", "your", "are", "can", "does",
        // Extended set (ported from axon/src/utils/deduplication.ts)
        "use", "using", "used", "get", "set", "via", "not", "all", "any", "but", "too", "out",
        "our", "their", "them", "they", "its", "then", "than", "also", "have", "has", "had", "was",
        "were", "who", "why",
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

    // Reconstruct joined phrase for verbatim phrase-match boost.
    // Tokens are already lowercased so this matches case-insensitively.
    let phrase = query_tokens.join(" ");
    let phrase_threshold = phrase.len() >= 6 && query_tokens.len() >= 2;

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

            // Verbatim phrase boost: +0.06 when the joined query tokens appear
            // consecutively in the chunk text (ported from TS deduplication.ts).
            let phrase_boost = if phrase_threshold
                && candidate
                    .chunk_text
                    .to_ascii_lowercase()
                    .contains(phrase.as_str())
            {
                0.06
            } else {
                0.0
            };

            candidate.rerank_score = candidate.score + lexical_boost + docs_boost + phrase_boost;
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

/// Score a chunk for display preview quality: query relevance × sentence richness.
///
/// Formula (ported from TS `scoreChunkForPreview`):
///   `relevance_score * 10 + richness`
/// where richness = `min(sentence_count, 5) * 2 + min(char_count, 500) / 100`.
///
/// This is intentionally independent of the vector score so a dense code block
/// doesn't win the preview slot over a chunk with rich explanatory prose.
fn score_chunk_for_preview(text: &str, query_tokens: &[String]) -> f32 {
    let phrase = query_tokens.join(" ");
    let cleaned = clean_snippet_source(text);
    let sentences: Vec<&str> = cleaned
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| is_relevant_sentence(s))
        .collect();
    let relevance: usize = sentences
        .iter()
        .map(|s| score_sentence(s, query_tokens, &phrase))
        .sum();
    let richness = sentences.len().min(5) as f32 * 2.0 + (cleaned.len().min(500) as f32) / 100.0;
    relevance as f32 * 10.0 + richness
}

/// From all chunks belonging to `url` in `candidates`, return the index of the
/// chunk with the highest preview score (prose richness × query relevance).
/// Scans up to 8 candidates per URL. Falls back to `fallback_idx` if none match.
///
/// Ported from TS `selectBestPreviewItem` in `utils/snippet.ts`.
pub fn select_best_preview_chunk(
    candidates: &[AskCandidate],
    url: &str,
    query_tokens: &[String],
    fallback_idx: usize,
) -> usize {
    candidates
        .iter()
        .enumerate()
        .filter(|(_, c)| c.url == url)
        .take(8)
        .map(|(i, c)| (i, score_chunk_for_preview(&c.chunk_text, query_tokens)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(fallback_idx)
}

// ─── Snippet extraction (ported from axon/src/utils/snippet.ts) ──────────────

/// Strip markdown inline formatting from a single line of text.
/// - Images `![alt](url)` are removed entirely.
/// - Links `[text](url)` are replaced with just the text.
fn strip_markdown_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        // Image: ![...](url) — drop entirely
        if chars[i] == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            i += 2;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            if i < chars.len() {
                i += 1; // skip ]
            }
            if i < chars.len() && chars[i] == '(' {
                i += 1;
                while i < chars.len() && chars[i] != ')' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // skip )
                }
            }
            continue;
        }
        // Link: [text](url) — keep text only
        if chars[i] == '[' {
            let text_start = i + 1;
            i += 1;
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            let text_end = i;
            if i < chars.len() {
                i += 1; // skip ]
            }
            if i < chars.len() && chars[i] == '(' {
                i += 1;
                while i < chars.len() && chars[i] != ')' {
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // skip )
                }
                for &c in &chars[text_start..text_end] {
                    out.push(c);
                }
            } else {
                // Not a valid link; emit literal [
                out.push('[');
                for &c in &chars[text_start..i] {
                    out.push(c);
                }
            }
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Clean chunk text for snippet display: strip markdown structure, navigation
/// boilerplate, and bare URLs so sentence extraction sees prose only.
fn clean_snippet_source(text: &str) -> String {
    const NAV: &[&str] = &[
        "prev",
        "next",
        "home",
        "menu",
        "read more",
        "copy",
        "subscribe",
        "was this page helpful",
        "table of contents",
        "on this page",
        "developer docs",
    ];

    let mut parts: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Horizontal rules
        let no_spaces: String = line.chars().filter(|c| !c.is_whitespace()).collect();
        if no_spaces.len() >= 3
            && (no_spaces.chars().all(|c| c == '-')
                || no_spaces.chars().all(|c| c == '*')
                || no_spaces.chars().all(|c| c == '_'))
        {
            continue;
        }
        // Short-line nav boilerplate
        if line.len() < 50 {
            let lower = line.to_ascii_lowercase();
            if NAV.iter().any(|p| lower.trim() == *p) {
                continue;
            }
        }
        // Strip leading list markers
        let line = line
            .strip_prefix("- ")
            .or_else(|| line.strip_prefix("* "))
            .or_else(|| line.strip_prefix("• "))
            .unwrap_or(line);
        // Strip inline markdown then bare URLs
        let stripped = strip_markdown_inline(line);
        let filtered: String = stripped
            .split_whitespace()
            .filter(|w| !w.starts_with("http://") && !w.starts_with("https://"))
            .collect::<Vec<_>>()
            .join(" ");
        if !filtered.is_empty() {
            parts.push(filtered);
        }
    }
    parts.join(" ")
}

/// True if a sentence is worth showing: has enough length, word count, and
/// contains real prose (not a bare URL or symbol line).
/// Requires ≥5 words, matching the TS reference (`split.length < 5` → reject).
fn is_relevant_sentence(s: &str) -> bool {
    let t = s.trim();
    t.len() >= 25
        && t.split_whitespace().count() >= 5
        && t.chars().any(|c| c.is_alphabetic())
        && !t.starts_with("http://")
        && !t.starts_with("https://")
        && t.chars().any(|c| c.is_alphanumeric())
}

/// Score one sentence against query tokens. +2 per matching token; +4 bonus
/// if the full query phrase appears verbatim (≥6 chars, ≥2 tokens).
fn score_sentence(sentence: &str, tokens: &[String], phrase: &str) -> usize {
    let lower = sentence.to_ascii_lowercase();
    let mut score: usize = tokens.iter().filter(|t| lower.contains(t.as_str())).count() * 2;
    if phrase.len() >= 6 && tokens.len() >= 2 && lower.contains(phrase) {
        score += 4;
    }
    score
}

/// Extract a compact, query-relevant snippet from chunk text.
///
/// Algorithm (ported from axon/src/utils/snippet.ts `getMeaningfulSnippet`):
/// 1. Clean the text (strip markdown structure, nav boilerplate, bare URLs).
/// 2. Split into sentences on `.!?` boundaries.
/// 3. Filter out navigation fragments and too-short strings.
/// 4. Score each sentence for query-token hits and phrase matches.
/// 5. Select up to 5 relevant sentences (≤700 chars), in document order.
/// 6. If fewer than 3 match, pad with adjacent sentences to fill context.
/// 7. Fallback: first 5 sentences, or first 220 chars of cleaned text.
pub fn get_meaningful_snippet(text: &str, query_tokens: &[String]) -> String {
    let phrase = query_tokens.join(" ");
    let cleaned = clean_snippet_source(text);

    let sentences: Vec<&str> = cleaned
        .split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| is_relevant_sentence(s))
        .collect();

    if sentences.is_empty() {
        // TS fallback: find the first prose line (≥20 chars, alphabetic), then truncate.
        let fallback = cleaned
            .split('\n')
            .map(str::trim)
            .find(|l| l.len() >= 20 && l.chars().any(|c| c.is_alphabetic()))
            .unwrap_or(&cleaned);
        let end = fallback
            .char_indices()
            .nth(220)
            .map(|(i, _)| i)
            .unwrap_or(fallback.len());
        return fallback[..end].to_string();
    }

    if !query_tokens.is_empty() {
        let mut scored: Vec<(usize, usize)> = sentences
            .iter()
            .enumerate()
            .map(|(i, s)| (i, score_sentence(s, query_tokens, &phrase)))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        let mut selected: Vec<usize> = Vec::new();
        let mut total_len = 0usize;
        for &(idx, score) in &scored {
            if score == 0 {
                break;
            }
            if selected.len() >= 5 {
                break;
            }
            if total_len + sentences[idx].len() > 700 && selected.len() >= 3 {
                break;
            }
            selected.push(idx);
            total_len += sentences[idx].len();
        }

        // Pad with contextually adjacent sentences if we have fewer than 3.
        if !selected.is_empty() && selected.len() < 3 {
            let anchor = *selected.iter().min().unwrap_or(&0);
            let selected_set: HashSet<usize> = selected.iter().copied().collect();
            let mut pads: Vec<usize> = (0..sentences.len())
                .filter(|i| !selected_set.contains(i))
                .collect();
            pads.sort_by_key(|&i| (i as isize - anchor as isize).unsigned_abs());
            for idx in pads {
                if selected.len() >= 3 {
                    break;
                }
                if total_len + sentences[idx].len() > 700 && selected.len() >= 2 {
                    break;
                }
                total_len += sentences[idx].len();
                selected.push(idx);
            }
        }

        if !selected.is_empty() {
            selected.sort_unstable();
            return selected
                .iter()
                .map(|&i| sentences[i])
                .collect::<Vec<_>>()
                .join(" ");
        }
    }

    // No query or no term matches: take first 3-5 sentences.
    let mut out: Vec<&str> = Vec::new();
    let mut total_len = 0usize;
    for &s in &sentences {
        if out.len() >= 5 {
            break;
        }
        if total_len + s.len() > 700 && out.len() >= 3 {
            break;
        }
        out.push(s);
        total_len += s.len();
    }
    out.join(" ")
}
