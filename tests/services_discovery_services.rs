use axon::crates::services::scrape::map_scrape_payload;
use axon::crates::services::search::{map_research_payload, map_search_results};
use axon::crates::services::types::MapResult;

// ---------------------------------------------------------------------------
// scrape service — map_scrape_payload
// ---------------------------------------------------------------------------

#[test]
fn maps_scrape_payload_to_scrape_result() {
    let payload = serde_json::json!({
        "url": "https://example.com",
        "status_code": 200,
        "markdown": "# Hello\n\nWorld",
        "title": "Hello",
        "description": "A page",
    });
    let result = map_scrape_payload(payload.clone()).expect("valid scrape payload");
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_scrape_payload_preserves_arbitrary_fields() {
    let payload = serde_json::json!({
        "url": "https://example.com/docs",
        "status_code": 200,
        "markdown": "content",
        "title": "Docs",
        "description": "",
        "extra_field": "should be preserved",
    });
    let result = map_scrape_payload(payload.clone()).expect("valid scrape payload");
    assert_eq!(result.payload["extra_field"], "should be preserved");
}

#[test]
fn maps_scrape_payload_with_non_200_status() {
    let payload = serde_json::json!({
        "url": "https://example.com/missing",
        "status_code": 404,
        "markdown": "",
        "title": "Not Found",
        "description": "",
    });
    let result =
        map_scrape_payload(payload.clone()).expect("non-200 scrape payload is still mappable");
    assert_eq!(result.payload["status_code"], 404);
}

#[test]
fn maps_scrape_payload_wraps_json_value_verbatim() {
    let payload = serde_json::json!({"a": 1, "b": [1, 2, 3]});
    let result = map_scrape_payload(payload.clone()).expect("any JSON value is mappable");
    assert_eq!(result.payload, payload);
}

// ---------------------------------------------------------------------------
// map service — MapResult
// ---------------------------------------------------------------------------

#[test]
fn maps_map_payload_to_map_result() {
    let payload = serde_json::json!({
        "url": "https://example.com",
        "mapped_urls": 3,
        "sitemap_urls": 2,
        "pages_seen": 3,
        "thin_pages": 0,
        "elapsed_ms": 450,
        "urls": ["https://example.com/a", "https://example.com/b", "https://example.com/c"],
    });
    let result = MapResult {
        payload: payload.clone(),
    };
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_map_payload_preserves_urls_array() {
    let payload = serde_json::json!({
        "url": "https://example.com",
        "mapped_urls": 2,
        "sitemap_urls": 0,
        "pages_seen": 2,
        "thin_pages": 0,
        "elapsed_ms": 200,
        "urls": ["https://example.com/one", "https://example.com/two"],
    });
    let result = MapResult {
        payload: payload.clone(),
    };
    let urls = result.payload["urls"]
        .as_array()
        .expect("urls must be array");
    assert_eq!(urls.len(), 2);
    assert_eq!(urls[0], "https://example.com/one");
    assert_eq!(urls[1], "https://example.com/two");
}

#[test]
fn maps_map_payload_with_empty_urls() {
    let payload = serde_json::json!({
        "url": "https://example.com",
        "mapped_urls": 0,
        "sitemap_urls": 0,
        "pages_seen": 0,
        "thin_pages": 0,
        "elapsed_ms": 50,
        "urls": [],
    });
    let result = MapResult {
        payload: payload.clone(),
    };
    assert_eq!(result.payload["mapped_urls"], 0);
    let urls = result.payload["urls"]
        .as_array()
        .expect("urls must be array");
    assert!(urls.is_empty());
}

#[test]
fn maps_map_payload_wraps_json_value_verbatim() {
    let payload = serde_json::json!({"x": true});
    let result = MapResult {
        payload: payload.clone(),
    };
    assert_eq!(result.payload, payload);
}

// ---------------------------------------------------------------------------
// search service — map_search_results
// ---------------------------------------------------------------------------

#[test]
fn maps_search_results_to_search_result() {
    let results = vec![
        serde_json::json!({"position": 1, "title": "Result One", "url": "https://a.com", "snippet": "snippet one"}),
        serde_json::json!({"position": 2, "title": "Result Two", "url": "https://b.com", "snippet": "snippet two"}),
    ];
    let result = map_search_results(results.clone());
    assert_eq!(result.results.len(), 2);
    assert_eq!(result.results[0]["title"], "Result One");
    assert_eq!(result.results[1]["url"], "https://b.com");
}

#[test]
fn maps_empty_search_results() {
    let results: Vec<serde_json::Value> = vec![];
    let result = map_search_results(results);
    assert!(result.results.is_empty());
}

#[test]
fn maps_search_result_preserves_all_fields() {
    let item = serde_json::json!({
        "position": 1,
        "title": "Test",
        "url": "https://test.com",
        "snippet": "A snippet",
    });
    let result = map_search_results(vec![item.clone()]);
    assert_eq!(result.results[0], item);
}

#[test]
fn maps_single_search_result() {
    let results = vec![
        serde_json::json!({"position": 1, "title": "Solo", "url": "https://solo.com", "snippet": null}),
    ];
    let result = map_search_results(results);
    assert_eq!(result.results.len(), 1);
    assert_eq!(result.results[0]["title"], "Solo");
}

// ---------------------------------------------------------------------------
// research service — map_research_payload
// ---------------------------------------------------------------------------

#[test]
fn maps_research_payload_to_research_result() {
    let payload = serde_json::json!({
        "query": "rust async patterns",
        "limit": 5,
        "offset": 0,
        "search_results": [],
        "extractions": [],
        "summary": "A comprehensive summary.",
        "usage": {"prompt_tokens": 100, "completion_tokens": 50, "total_tokens": 150},
        "timing_ms": {"total": 1200},
    });
    let result = map_research_payload(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_research_payload_preserves_summary() {
    let payload = serde_json::json!({
        "query": "test",
        "summary": "The summary goes here.",
    });
    let result = map_research_payload(payload.clone());
    assert_eq!(result.payload["summary"], "The summary goes here.");
}

#[test]
fn maps_research_payload_with_null_summary() {
    let payload = serde_json::json!({
        "query": "test",
        "summary": null,
    });
    let result = map_research_payload(payload.clone());
    assert!(result.payload["summary"].is_null());
}

#[test]
fn maps_research_payload_wraps_json_value_verbatim() {
    let payload = serde_json::json!({"anything": [1, 2, 3]});
    let result = map_research_payload(payload.clone());
    assert_eq!(result.payload, payload);
}
