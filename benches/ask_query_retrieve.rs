use axon::crates::vector::ops::{chunk_text, url_lookup_candidates};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone)]
struct CandidateOld {
    score: f64,
    url: String,
    chunk_text: String,
}

#[derive(Clone)]
struct CandidateNew {
    score: f64,
    url: String,
    url_tokens: HashSet<String>,
    chunk_tokens: HashSet<String>,
    rerank_score: f64,
}

fn tokenize_query(text: &str) -> Vec<String> {
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

fn tokenize_text_set(text: &str) -> HashSet<String> {
    tokenize_query(text).into_iter().collect()
}

fn tokenize_path_set(path_or_url: &str) -> HashSet<String> {
    path_or_url
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 3)
        .map(str::to_string)
        .collect()
}

fn rerank_old(candidates: &[CandidateOld], query: &str) -> Vec<CandidateOld> {
    let tokens: Vec<String> = tokenize_query(query);
    let mut reranked = candidates.to_vec();
    reranked.sort_by(|a, b| {
        let adjusted = |candidate: &CandidateOld| -> f64 {
            let mut lexical_boost = 0.0f64;
            let url_tokens = tokenize_path_set(&candidate.url);
            let chunk_tokens = tokenize_text_set(&candidate.chunk_text);
            for token in &tokens {
                if url_tokens.contains(token) {
                    lexical_boost += 0.045;
                }
                if chunk_tokens.contains(token) {
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
            candidate.score + lexical_boost + docs_boost
        };
        adjusted(b)
            .partial_cmp(&adjusted(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    reranked
}

fn rerank_new(candidates: &[CandidateNew], query: &str) -> Vec<CandidateNew> {
    let tokens: Vec<String> = tokenize_query(query);
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

fn build_old_context(
    full_docs_context: Vec<String>,
    supplemental_context: Vec<String>,
    separator: &str,
) -> String {
    [
        "Answer only from the provided sources.",
        "Cite supporting sources inline using [S#] labels.",
        "If the sources are incomplete, say so explicitly.",
        "",
        "Sources:",
        &(full_docs_context
            .into_iter()
            .chain(supplemental_context.into_iter())
            .collect::<Vec<_>>()
            .join(separator)),
    ]
    .join("\n")
}

fn push_context_entry(
    entries: &mut Vec<String>,
    context_char_count: &mut usize,
    entry: String,
    separator: &str,
    max_chars: usize,
) -> bool {
    let projected = if entries.is_empty() {
        entry.len()
    } else {
        *context_char_count + separator.len() + entry.len()
    };
    if projected > max_chars {
        return false;
    }
    entries.push(entry);
    *context_char_count = projected;
    true
}

fn build_new_context(all_entries: Vec<String>, separator: &str, max_chars: usize) -> String {
    let mut entries = Vec::new();
    let mut count = 0usize;
    for e in all_entries {
        if !push_context_entry(&mut entries, &mut count, e, separator, max_chars) {
            break;
        }
    }
    format!(
        "Answer only from the provided sources.\nCite supporting sources inline using [S#] labels.\nIf the sources are incomplete, say so explicitly.\n\nSources:\n{}",
        entries.join(separator)
    )
}

fn bench_rerank_old_vs_new(c: &mut Criterion) {
    let query = "install api auth token configuration with docs";
    let old = (0..256)
        .map(|i| CandidateOld {
            score: 0.2 + ((i % 100) as f64 / 500.0),
            url: format!("https://example.dev/docs/{i}/install/api/reference"),
            chunk_text: format!(
                "installation guide {i} with token auth api setup and configuration {}",
                "x".repeat(256)
            ),
        })
        .collect::<Vec<_>>();
    let new = old
        .iter()
        .map(|c| CandidateNew {
            score: c.score,
            url: c.url.clone(),
            url_tokens: tokenize_path_set(&c.url),
            chunk_tokens: tokenize_text_set(&c.chunk_text),
            rerank_score: c.score,
        })
        .collect::<Vec<_>>();

    c.bench_function("rerank_old_256", |b| {
        b.iter(|| {
            let out = rerank_old(black_box(&old), black_box(query));
            black_box(out);
        })
    });

    c.bench_function("rerank_new_256", |b| {
        b.iter(|| {
            let out = rerank_new(black_box(&new), black_box(query));
            black_box(out);
        })
    });
}

fn bench_context_old_vs_new(c: &mut Criterion) {
    let separator = "\n\n---\n\n";
    let full_docs = (0..6)
        .map(|i| {
            format!(
                "## Source Document [S{}]: https://example.dev/{}\n\n{}",
                i + 1,
                i,
                "a".repeat(12_000)
            )
        })
        .collect::<Vec<_>>();
    let supplemental = (0..6)
        .map(|i| {
            format!(
                "## Supplemental Chunk [S{}]: https://example.dev/chunk/{}\n\n{}",
                i + 7,
                i,
                "b".repeat(2_000)
            )
        })
        .collect::<Vec<_>>();
    let combined = full_docs
        .iter()
        .cloned()
        .chain(supplemental.iter().cloned())
        .collect::<Vec<_>>();

    c.bench_function("context_old_join_strategy", |b| {
        b.iter(|| {
            let out = build_old_context(
                black_box(full_docs.clone()),
                black_box(supplemental.clone()),
                black_box(separator),
            );
            black_box(out);
        })
    });

    c.bench_function("context_new_single_pass", |b| {
        b.iter(|| {
            let out = build_new_context(
                black_box(combined.clone()),
                black_box(separator),
                black_box(120_000),
            );
            black_box(out);
        })
    });
}

fn bench_retrieve_url_normalization(c: &mut Criterion) {
    c.bench_function("url_lookup_candidates", |b| {
        b.iter(|| {
            let out = url_lookup_candidates(black_box("example.com/docs/install"));
            black_box(out);
        })
    });
}

fn bench_ask_chunking(c: &mut Criterion) {
    let text = "x".repeat(128_000);
    c.bench_function("chunk_text_128k", |b| {
        b.iter(|| {
            let out = chunk_text(black_box(&text));
            black_box(out);
        })
    });
}

#[derive(Debug, Clone, Default, Deserialize)]
struct BenchPayload {
    #[serde(default)]
    url: String,
    #[serde(default)]
    chunk_text: String,
}

#[derive(Debug, Clone, Deserialize)]
struct BenchHit {
    score: f64,
    #[serde(default)]
    payload: BenchPayload,
}

#[derive(Debug, Clone, Deserialize)]
struct BenchResponse {
    #[serde(default)]
    result: Vec<BenchHit>,
}

fn bench_qdrant_extract_old_vs_typed(c: &mut Criterion) {
    let wire = serde_json::json!({
        "result": (0..256).map(|i| serde_json::json!({
            "score": 0.4 + (i as f64 / 1000.0),
            "payload": {
                "url": format!("https://example.dev/docs/{i}/install"),
                "chunk_text": format!("install guide {i} {}", "x".repeat(128))
            }
        })).collect::<Vec<_>>()
    });
    let wire_bytes = serde_json::to_vec(&wire).expect("serialize bench payload");

    c.bench_function("qdrant_extract_old_value_path", |b| {
        b.iter(|| {
            let parsed: serde_json::Value =
                serde_json::from_slice(black_box(&wire_bytes)).expect("parse value");
            let out = parsed["result"]
                .as_array()
                .expect("result array")
                .iter()
                .map(|hit| {
                    let score = hit["score"].as_f64().unwrap_or(0.0);
                    let payload = hit.get("payload").cloned().unwrap_or_default();
                    let url = payload
                        .get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let chunk = payload
                        .get("chunk_text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    (score, url, chunk)
                })
                .collect::<Vec<_>>();
            black_box(out);
        })
    });

    c.bench_function("qdrant_extract_typed_path", |b| {
        b.iter(|| {
            let parsed: BenchResponse =
                serde_json::from_slice(black_box(&wire_bytes)).expect("parse typed");
            let out = parsed
                .result
                .iter()
                .map(|hit| {
                    (
                        hit.score,
                        hit.payload.url.clone(),
                        hit.payload.chunk_text.clone(),
                    )
                })
                .collect::<Vec<_>>();
            black_box(out);
        })
    });
}

criterion_group!(
    benches,
    bench_rerank_old_vs_new,
    bench_context_old_vs_new,
    bench_qdrant_extract_old_vs_typed,
    bench_retrieve_url_normalization,
    bench_ask_chunking
);
criterion_main!(benches);
