use axon::axon_cli::crates::vector::ops_v2::ranking::{
    get_meaningful_snippet, rerank_ask_candidates, select_diverse_candidates, tokenize_path_set,
    tokenize_query, tokenize_text_set, AskCandidate,
};
use std::collections::HashSet;

#[test]
fn rerank_candidates_boosts_docs_token_match_like_legacy() {
    let candidates = vec![
        AskCandidate {
            score: 0.40,
            url: "https://example.com/blog/overview".to_string(),
            path: "/blog/overview".to_string(),
            chunk_text: "general notes".to_string(),
            url_tokens: tokenize_path_set("/blog/overview"),
            chunk_tokens: tokenize_text_set("general notes"),
            rerank_score: 0.40,
        },
        AskCandidate {
            score: 0.39,
            url: "https://example.com/docs/install".to_string(),
            path: "/docs/install".to_string(),
            chunk_text: "installation steps and setup".to_string(),
            url_tokens: tokenize_path_set("/docs/install"),
            chunk_tokens: tokenize_text_set("installation steps and setup"),
            rerank_score: 0.39,
        },
    ];

    let reranked = rerank_ask_candidates(&candidates, &tokenize_query("install setup"));
    assert_eq!(reranked[0].url, "https://example.com/docs/install");
    assert!(reranked[0].rerank_score > reranked[1].rerank_score);
}

#[test]
fn select_diverse_candidates_respects_max_per_url_like_legacy() {
    let make_candidate = |score: f64, url: &str| AskCandidate {
        score,
        url: url.to_string(),
        path: "/docs".to_string(),
        chunk_text: "chunk".to_string(),
        url_tokens: tokenize_path_set("/docs"),
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
    let urls: HashSet<String> = selected
        .into_iter()
        .map(|idx| candidates[idx].url.clone())
        .collect();
    assert_eq!(urls.len(), 3);
}

#[test]
fn tokenize_query_drops_stop_words_and_short_tokens() {
    let tokens = tokenize_query("How to install the API in v2 docs");
    assert_eq!(tokens, vec!["install", "api", "docs"]);
}

// ── phrase_boost path ──────────────────────────────────────────────────────

#[test]
fn rerank_boosts_candidate_with_consecutive_query_phrase() {
    // Candidate A: phrase "install package" appears consecutively in chunk_text
    // Candidate B: slightly higher base score but phrase is absent
    let a = AskCandidate {
        url: "https://docs.example.com/install".to_string(),
        path: "/install".to_string(),
        // Contains the exact joined token string "install package" consecutively
        chunk_text: "Run npm install package to add the dependency.".to_string(),
        score: 0.50,
        url_tokens: tokenize_path_set("/install"),
        chunk_tokens: tokenize_text_set("Run npm install package to add the dependency."),
        rerank_score: 0.0,
    };
    let b = AskCandidate {
        url: "https://docs.example.com/overview".to_string(),
        path: "/overview".to_string(),
        chunk_text: "An overview of the library and its capabilities.".to_string(),
        score: 0.54,
        url_tokens: tokenize_path_set("/overview"),
        chunk_tokens: tokenize_text_set("An overview of the library and its capabilities."),
        rerank_score: 0.0,
    };
    let candidates = vec![b.clone(), a.clone()];
    let tokens = tokenize_query("install package");
    let reranked = rerank_ask_candidates(&candidates, &tokens);
    // A should rank first due to phrase_boost even though B has a higher vector score
    assert_eq!(
        reranked[0].url, a.url,
        "phrase match should outrank higher-score candidate without phrase"
    );
}

// ── stop-word preservation regression ─────────────────────────────────────

#[test]
fn tokenize_query_preserves_intent_verbs_create_and_make() {
    let tokens = tokenize_query("how to create a new component");
    assert!(
        tokens.contains(&"create".to_string()),
        "'create' must be preserved — it encodes user intent"
    );
    assert!(
        tokens.contains(&"component".to_string()),
        "'component' must be preserved"
    );
    assert!(
        !tokens.contains(&"how".to_string()),
        "'how' is a stop word and must be dropped"
    );

    let tokens2 = tokenize_query("make a widget");
    assert!(
        tokens2.contains(&"make".to_string()),
        "'make' must be preserved — it encodes user intent"
    );
}

// ── docs_boost path-vs-url boundary ───────────────────────────────────────

#[test]
fn rerank_docs_boost_uses_path_not_url_domain() {
    // URL domain contains "docs" but path does NOT — should NOT get docs_boost
    let no_boost = AskCandidate {
        url: "https://my-docs-host.com/blog/post".to_string(),
        path: "/blog/post".to_string(),
        chunk_text: "some content about blogging".to_string(),
        score: 0.60,
        url_tokens: tokenize_path_set("/blog/post"),
        chunk_tokens: tokenize_text_set("some content about blogging"),
        rerank_score: 0.0,
    };
    // Path contains "/docs/" — should get docs_boost
    let with_boost = AskCandidate {
        url: "https://example.com/docs/guide".to_string(),
        path: "/docs/guide".to_string(),
        chunk_text: "documentation content for the guide".to_string(),
        score: 0.55,
        url_tokens: tokenize_path_set("/docs/guide"),
        chunk_tokens: tokenize_text_set("documentation content for the guide"),
        rerank_score: 0.0,
    };
    let candidates = vec![no_boost.clone(), with_boost.clone()];
    let tokens = tokenize_query("docs guide");
    let reranked = rerank_ask_candidates(&candidates, &tokens);
    // with_boost should win despite lower vector score due to docs_boost + url_tokens match
    assert_eq!(
        reranked[0].url, with_boost.url,
        "docs_boost must fire on path, not URL domain"
    );
}

// ── get_meaningful_snippet edge cases ──────────────────────────────────────

#[test]
fn get_meaningful_snippet_returns_non_empty_for_relevant_content() {
    let text = "Tokio is an async runtime for Rust. \
                It provides async I/O, timers, and task scheduling. \
                You can install tokio by adding it to Cargo.toml.";
    let tokens = tokenize_query("tokio async runtime");
    let snippet = get_meaningful_snippet(text, &tokens);
    assert!(
        !snippet.is_empty(),
        "should return a snippet for relevant content"
    );
    assert!(snippet.len() <= 800, "snippet should be reasonably sized");
}

#[test]
fn get_meaningful_snippet_handles_empty_input() {
    let snippet = get_meaningful_snippet("", &tokenize_query("anything"));
    // Should not panic; empty or minimal output is acceptable
    assert!(snippet.len() < 100);
}

#[test]
fn get_meaningful_snippet_handles_no_query_tokens() {
    let text = "First sentence with enough words here. \
                Second sentence with enough words here. \
                Third sentence with enough words here. \
                Fourth sentence with enough words here. \
                Fifth sentence with enough words here.";
    let snippet = get_meaningful_snippet(text, &[]);
    // With no query tokens, should fall back to first few sentences
    assert!(
        !snippet.is_empty(),
        "should return first sentences when no tokens provided"
    );
}
