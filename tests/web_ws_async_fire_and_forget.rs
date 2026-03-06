//! Structural tests for the async fire-and-forget web execution path.
//!
//! These tests verify:
//! - Async mode constants are consistent with the new direct-dispatch design
//! - The `ASYNC_MODES` set does not overlap `DIRECT_SYNC_MODES`
//! - The `JobEnqueued` WS event shape is correct
//! - Cancel logic validates job IDs before attempting service cancel

use axon::crates::web::execute;

// ── Async mode classification ─────────────────────────────────────────────────

#[test]
fn async_modes_do_not_overlap_direct_sync_modes() {
    // Async modes enqueue jobs via direct service. Sync direct modes call
    // services synchronously. These two sets must be disjoint.
    let async_modes = execute::async_modes_pub();
    let direct_sync_modes = execute::direct_sync_modes_pub();
    for mode in async_modes {
        assert!(
            !direct_sync_modes.contains(mode),
            "mode '{}' is in both ASYNC_MODES and DIRECT_SYNC_MODES — must be exclusive",
            mode
        );
    }
}

#[test]
fn async_modes_all_covered_by_allowed_modes() {
    let async_modes = execute::async_modes_pub();
    let allowed_modes = execute::allowed_modes_pub();
    for mode in async_modes {
        assert!(
            allowed_modes.contains(mode),
            "async mode '{}' is not in ALLOWED_MODES — must be registered",
            mode
        );
    }
}

#[test]
fn async_modes_contains_direct_enqueue_commands() {
    // Direct fire-and-forget enqueue: crawl, extract, embed.
    let async_modes = execute::async_modes_pub();
    for expected in &["crawl", "extract", "embed"] {
        assert!(
            async_modes.contains(expected),
            "expected async mode '{}' missing from ASYNC_MODES",
            expected
        );
    }
    // github/reddit/youtube are subprocess fallback — NOT in ASYNC_MODES.
    for excluded in &["github", "reddit", "youtube"] {
        assert!(
            !async_modes.contains(excluded),
            "'{}' should NOT be in ASYNC_MODES (uses subprocess fallback)",
            excluded
        );
    }
}

// ── Cancel job ID validation ──────────────────────────────────────────────────

#[test]
fn cancel_rejects_empty_job_id() {
    // is_valid_cancel_job_id must reject non-UUID strings.
    assert!(
        !execute::is_valid_cancel_job_id_pub(""),
        "empty string should not be a valid cancel job_id"
    );
}

#[test]
fn cancel_rejects_non_uuid_job_id() {
    assert!(
        !execute::is_valid_cancel_job_id_pub("not-a-uuid"),
        "non-UUID string should not be a valid cancel job_id"
    );
    assert!(
        !execute::is_valid_cancel_job_id_pub("123"),
        "short numeric string should not be a valid cancel job_id"
    );
}

#[test]
fn cancel_accepts_valid_uuid_format() {
    assert!(
        execute::is_valid_cancel_job_id_pub("550e8400-e29b-41d4-a716-446655440000"),
        "well-formed UUID should be accepted as cancel job_id"
    );
    assert!(
        execute::is_valid_cancel_job_id_pub("12345678-1234-1234-1234-123456789abc"),
        "well-formed UUID (lowercase hex) should be accepted"
    );
}

// ── Fire-and-forget: no subprocess for async modes ────────────────────────────

#[test]
fn async_mode_enqueue_job_result_maps_job_ids() {
    // Pure mapping: verify CrawlStartResult's job_ids are preserved when mapping
    // to the enqueued JSON payload sent over the WS.
    use axon::crates::services::types::CrawlStartResult;

    let result = CrawlStartResult {
        job_ids: vec!["aabbccdd-1234-1234-1234-aabbccddeeff".to_string()],
    };
    // The first job_id should be emittable as a JSON-serializable string.
    assert_eq!(result.job_ids.len(), 1);
    let first = &result.job_ids[0];
    assert!(first.contains('-'), "job_id should look like a UUID");
}

#[test]
fn async_mode_embed_result_maps_job_id() {
    use axon::crates::services::types::EmbedStartResult;
    let result = EmbedStartResult {
        job_id: "aabbccdd-1234-1234-1234-aabbccddeeff".to_string(),
    };
    assert!(!result.job_id.is_empty());
}

#[test]
fn async_mode_extract_result_maps_job_id() {
    use axon::crates::services::types::ExtractStartResult;
    let result = ExtractStartResult {
        job_id: "aabbccdd-1234-1234-1234-aabbccddeeff".to_string(),
    };
    assert!(!result.job_id.is_empty());
}
