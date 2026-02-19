use axon::axon_cli::crates::vector::ops_v2::ranking::{
    rerank_ask_candidates, select_diverse_candidates, tokenize_path_set, tokenize_query,
    tokenize_text_set, AskCandidate,
};
use std::collections::HashSet;

#[test]
fn rerank_candidates_boosts_docs_token_match_like_legacy() {
    let candidates = vec![
        AskCandidate {
            score: 0.40,
            url: "https://example.com/blog/overview".to_string(),
            chunk_text: "general notes".to_string(),
            url_tokens: tokenize_path_set("https://example.com/blog/overview"),
            chunk_tokens: tokenize_text_set("general notes"),
            rerank_score: 0.40,
        },
        AskCandidate {
            score: 0.39,
            url: "https://example.com/docs/install".to_string(),
            chunk_text: "installation steps and setup".to_string(),
            url_tokens: tokenize_path_set("https://example.com/docs/install"),
            chunk_tokens: tokenize_text_set("installation steps and setup"),
            rerank_score: 0.39,
        },
    ];

    let reranked = rerank_ask_candidates(&candidates, "install setup");
    assert_eq!(reranked[0].url, "https://example.com/docs/install");
    assert!(reranked[0].rerank_score > reranked[1].rerank_score);
}

#[test]
fn select_diverse_candidates_respects_max_per_url_like_legacy() {
    let make_candidate = |score: f64, url: &str| AskCandidate {
        score,
        url: url.to_string(),
        chunk_text: "chunk".to_string(),
        url_tokens: tokenize_path_set(url),
        chunk_tokens: tokenize_text_set("chunk"),
        rerank_score: score,
    };
    let candidates = vec![
        make_candidate(0.9, "https://a.dev/docs"),
        make_candidate(0.8, "https://a.dev/docs"),
        make_candidate(0.7, "https://b.dev/docs"),
        make_candidate(0.6, "https://c.dev/docs"),
    ];

    let selected = select_diverse_candidates(&candidates, 3, 1);
    let urls: HashSet<String> = selected.into_iter().map(|c| c.url).collect();
    assert_eq!(urls.len(), 3);
}

#[test]
fn tokenize_query_drops_stop_words_and_short_tokens() {
    let tokens = tokenize_query("How to install the API in v2 docs");
    assert_eq!(tokens, vec!["install", "api", "docs"]);
}
