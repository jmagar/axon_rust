use super::super::*;

use chrono::{DateTime, Duration};

fn watchdog_json(observed: DateTime<Utc>, first_seen: &str) -> Value {
    serde_json::json!({
        "_watchdog": {
            "observed_updated_at": observed.to_rfc3339(),
            "first_seen_stale_at": first_seen
        }
    })
}

#[test]
fn stale_watchdog_payload_adds_metadata_and_normalizes_shape() {
    let observed = Utc::now() - Duration::seconds(45);
    let payload = stale_watchdog_payload(serde_json::json!("not-an-object"), observed);
    let watchdog = payload.get("_watchdog").expect("missing _watchdog");
    let observed_value = watchdog
        .get("observed_updated_at")
        .and_then(|v| v.as_str())
        .expect("missing observed_updated_at");
    assert_eq!(observed_value, observed.to_rfc3339());
    let first_seen = watchdog
        .get("first_seen_stale_at")
        .and_then(|v| v.as_str())
        .expect("missing first_seen_stale_at");
    assert!(DateTime::parse_from_rfc3339(first_seen).is_ok());
}

#[test]
fn stale_watchdog_confirmed_requires_watchdog_metadata() {
    let observed = Utc::now() - Duration::seconds(10);
    assert!(!stale_watchdog_confirmed(
        &serde_json::json!({}),
        observed,
        30
    ));
}

#[test]
fn stale_watchdog_confirmed_rejects_observed_timestamp_mismatch() {
    let observed = Utc::now() - Duration::seconds(90);
    let mismatched = observed + Duration::seconds(1);
    let payload = watchdog_json(
        observed,
        &(Utc::now() - Duration::seconds(120)).to_rfc3339(),
    );
    assert!(!stale_watchdog_confirmed(&payload, mismatched, 60));
}

#[test]
fn stale_watchdog_confirmed_rejects_malformed_first_seen() {
    let observed = Utc::now() - Duration::seconds(120);
    let payload = watchdog_json(observed, "not-a-timestamp");
    assert!(!stale_watchdog_confirmed(&payload, observed, 60));
}

#[test]
fn stale_watchdog_confirmed_requires_confirmation_window() {
    let observed = Utc::now() - Duration::seconds(10);
    let payload = watchdog_json(observed, &Utc::now().to_rfc3339());
    assert!(!stale_watchdog_confirmed(&payload, observed, 60));
}

#[test]
fn stale_watchdog_confirmed_true_after_confirmation_window_elapsed() {
    let observed = Utc::now() - Duration::seconds(120);
    let payload = watchdog_json(
        observed,
        &(Utc::now() - Duration::seconds(180)).to_rfc3339(),
    );
    assert!(stale_watchdog_confirmed(&payload, observed, 60));
}

// ── T-H-4: Watchdog RFC3339 timestamp round-trip ────────────────────────────

#[test]
fn watchdog_rfc3339_timestamp_round_trips() {
    let ts = Utc::now();
    let rfc = ts.to_rfc3339();
    let parsed = DateTime::parse_from_rfc3339(&rfc).unwrap();
    assert_eq!(ts.timestamp(), parsed.timestamp());
}

// ── T-M-4: Watchdog two-pass payload preservation ───────────────────────────

#[test]
fn watchdog_payload_preserves_first_seen_on_same_observed() {
    let observed = Utc::now() - Duration::seconds(120);

    // First pass: creates the _watchdog metadata
    let first = stale_watchdog_payload(serde_json::json!({}), observed);
    let first_seen = first["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Simulate time passing, then second pass with same observed_updated_at
    let second = stale_watchdog_payload(first, observed);
    let second_first_seen = second["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // first_seen_stale_at should be preserved (not reset)
    assert_eq!(first_seen, second_first_seen);
}

#[test]
fn watchdog_payload_resets_first_seen_on_different_observed() {
    let observed_old = Utc::now() - Duration::seconds(120);
    let observed_new = Utc::now() - Duration::seconds(60);

    let first = stale_watchdog_payload(serde_json::json!({}), observed_old);
    let first_seen = first["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // Heartbeat arrived, observed_updated_at changed — first_seen resets
    let second = stale_watchdog_payload(first, observed_new);
    let second_first_seen = second["_watchdog"]["first_seen_stale_at"]
        .as_str()
        .unwrap()
        .to_string();

    // first_seen_stale_at should be different (reset due to new observed timestamp)
    assert_ne!(first_seen, second_first_seen);
}
