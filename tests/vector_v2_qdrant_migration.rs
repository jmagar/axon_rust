use axon::axon_cli::crates::vector::ops_v2::qdrant::{
    query_snippet, render_full_doc_from_points, QdrantPayload, QdrantPoint,
};

#[test]
fn render_full_doc_orders_by_chunk_index_and_trims_like_legacy() {
    let points = vec![
        QdrantPoint {
            payload: QdrantPayload {
                chunk_index: Some(1),
                chunk_text: "second".to_string(),
                ..QdrantPayload::default()
            },
        },
        QdrantPoint {
            payload: QdrantPayload {
                chunk_index: Some(0),
                chunk_text: "first".to_string(),
                ..QdrantPayload::default()
            },
        },
        QdrantPoint {
            payload: QdrantPayload {
                chunk_index: Some(2),
                chunk_text: String::new(),
                ..QdrantPayload::default()
            },
        },
    ];

    let text = render_full_doc_from_points(points);
    assert_eq!(text, "first\nsecond");
}

#[test]
fn query_snippet_truncates_and_flattens_newlines_like_legacy() {
    let payload = QdrantPayload {
        chunk_text: format!("line1\n{}", "x".repeat(200)),
        ..QdrantPayload::default()
    };

    let snippet = query_snippet(&payload);
    assert!(!snippet.is_empty());
    assert!(snippet.starts_with("line1 "));
    assert!(!snippet.contains('\n'));
    assert!(snippet.chars().count() <= 140);
}
