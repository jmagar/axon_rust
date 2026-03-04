use axon::crates::services::system::{
    map_doctor_payload, map_domains_payload, map_sources_payload, map_stats_payload,
};

#[test]
fn maps_source_facets_to_sources_result() {
    let payload = serde_json::json!({
        "count": 3,
        "limit": 2,
        "offset": 1,
        "urls": [
            {"url": "https://a", "chunks": 5},
            {"url": "https://b", "chunks": 12}
        ]
    });

    let result = map_sources_payload(&payload).expect("valid sources payload");
    assert_eq!(result.count, 3);
    assert_eq!(result.limit, 2);
    assert_eq!(result.offset, 1);
    assert_eq!(
        result.urls,
        vec![("https://a".to_string(), 5), ("https://b".to_string(), 12)]
    );
}

#[test]
fn maps_domain_facets_to_domains_result() {
    let payload = serde_json::json!({
        "domains": [
            {"domain": "example.com", "vectors": 4},
            {"domain": "docs.example.com", "vectors": 2}
        ],
        "limit": 10,
        "offset": 0
    });

    let result = map_domains_payload(&payload).expect("valid domains payload");
    assert_eq!(result.limit, 10);
    assert_eq!(result.offset, 0);
    assert_eq!(result.domains.len(), 2);
    assert_eq!(result.domains[0].domain, "example.com");
    assert_eq!(result.domains[0].vectors, 4);
}

#[test]
fn maps_stats_payload_shape() {
    let payload = serde_json::json!({"collection": "cortex", "points_count": 12});
    let result = map_stats_payload(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_doctor_payload_shape() {
    let payload = serde_json::json!({"all_ok": true});
    let result = map_doctor_payload(payload.clone());
    assert_eq!(result.payload, payload);
}
