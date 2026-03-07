use axon::crates::services::crawl::{map_crawl_job_result, map_crawl_start_result};
use axon::crates::services::embed::{map_embed_job_result, map_embed_start_result};
use axon::crates::services::extract::{map_extract_job_result, map_extract_start_result};
use axon::crates::services::ingest::map_ingest_result;
use axon::crates::services::screenshot::map_screenshot_result;

// --- crawl mapping helpers ---

#[test]
fn maps_crawl_start_job_ids() {
    let ids = vec![
        "550e8400-e29b-41d4-a716-446655440000".to_string(),
        "6ba7b810-9dad-11d1-80b4-00c04fd430c8".to_string(),
    ];
    let result = map_crawl_start_result(ids.clone());
    assert_eq!(result.job_ids, ids);
}

#[test]
fn maps_crawl_start_empty_ids() {
    let result = map_crawl_start_result(vec![]);
    assert!(result.job_ids.is_empty());
}

#[test]
fn maps_crawl_job_result_preserves_payload() {
    let payload = serde_json::json!({
        "job_id": "550e8400-e29b-41d4-a716-446655440000",
        "status": "completed",
        "pages_crawled": 42,
        "url": "https://docs.example.com"
    });
    let result = map_crawl_job_result(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_crawl_job_result_null_payload() {
    let payload = serde_json::Value::Null;
    let result = map_crawl_job_result(payload.clone());
    assert_eq!(result.payload, serde_json::Value::Null);
}

// --- embed mapping helpers ---

#[test]
fn maps_embed_start_result_with_job_id() {
    let job_id = "550e8400-e29b-41d4-a716-446655440001".to_string();
    let result = map_embed_start_result(job_id.clone());
    assert_eq!(result.job_id, job_id);
}

#[test]
fn maps_embed_job_result_preserves_payload() {
    let payload = serde_json::json!({
        "job_id": "abc",
        "status": "running",
        "points_embedded": 100
    });
    let result = map_embed_job_result(payload.clone());
    assert_eq!(result.payload, payload);
}

// --- extract mapping helpers ---

#[test]
fn maps_extract_start_result_with_job_id() {
    let job_id = "550e8400-e29b-41d4-a716-446655440002".to_string();
    let result = map_extract_start_result(job_id.clone());
    assert_eq!(result.job_id, job_id);
}

#[test]
fn maps_extract_job_result_preserves_payload() {
    let payload = serde_json::json!({
        "job_id": "xyz",
        "status": "pending",
        "urls": ["https://example.com"],
        "total_items": 0
    });
    let result = map_extract_job_result(payload.clone());
    assert_eq!(result.payload, payload);
}

// --- ingest mapping helpers ---

#[test]
fn maps_ingest_result_github() {
    let payload = serde_json::json!({
        "source": "github",
        "repo": "owner/name",
        "chunks": 87
    });
    let result = map_ingest_result(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_ingest_result_reddit() {
    let payload = serde_json::json!({
        "source": "reddit",
        "target": "rust",
        "chunks": 23
    });
    let result = map_ingest_result(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_ingest_result_youtube() {
    let payload = serde_json::json!({
        "source": "youtube",
        "url": "https://youtube.com/watch?v=abc",
        "chunks": 5
    });
    let result = map_ingest_result(payload.clone());
    assert_eq!(result.payload["source"], "youtube");
    assert_eq!(result.payload["chunks"], 5);
}

#[test]
fn maps_ingest_result_sessions() {
    let payload = serde_json::json!({
        "source": "sessions",
        "chunks": 12
    });
    let result = map_ingest_result(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_ingest_result_null_payload() {
    let result = map_ingest_result(serde_json::Value::Null);
    assert_eq!(result.payload, serde_json::Value::Null);
}

// --- screenshot mapping helper ---

#[test]
fn maps_screenshot_result_preserves_payload() {
    let payload = serde_json::json!({
        "url": "https://example.com",
        "path": "/output/screenshots/0001-example-com.png",
        "size_bytes": 98765
    });
    let result = map_screenshot_result(payload.clone());
    assert_eq!(result.payload, payload);
}

#[test]
fn maps_screenshot_result_null_payload() {
    let result = map_screenshot_result(serde_json::Value::Null);
    assert_eq!(result.payload, serde_json::Value::Null);
}

#[test]
fn maps_screenshot_result_url_field() {
    let payload = serde_json::json!({
        "url": "https://docs.rust-lang.org",
        "path": "/screenshots/0001-docs-rust-lang-org.png",
        "size_bytes": 42000
    });
    let result = map_screenshot_result(payload.clone());
    assert_eq!(result.payload["url"], "https://docs.rust-lang.org");
    assert_eq!(result.payload["size_bytes"], 42000);
}
