use axon::crates::core::config::RenderMode;
use axon::crates::web::execute::overrides::ws_request_to_overrides;

// ── render_mode mapping ───────────────────────────────────────────────────────

#[test]
fn render_mode_http_maps_correctly() {
    let ov = ws_request_to_overrides(Some("http"), None, None, None).unwrap();
    let mode = ov.overrides.render_mode.unwrap();
    assert!(
        matches!(mode, RenderMode::Http),
        "expected Http, got {mode:?}"
    );
}

#[test]
fn render_mode_chrome_maps_correctly() {
    let ov = ws_request_to_overrides(Some("chrome"), None, None, None).unwrap();
    let mode = ov.overrides.render_mode.unwrap();
    assert!(matches!(mode, RenderMode::Chrome));
}

#[test]
fn render_mode_auto_switch_maps_correctly() {
    let ov = ws_request_to_overrides(Some("auto-switch"), None, None, None).unwrap();
    let mode = ov.overrides.render_mode.unwrap();
    assert!(matches!(mode, RenderMode::AutoSwitch));
}

#[test]
fn render_mode_invalid_is_rejected() {
    let result = ws_request_to_overrides(Some("ftp"), None, None, None);
    assert!(
        result.is_err(),
        "expected Err for invalid render_mode 'ftp'"
    );
    let msg = result.unwrap_err();
    assert!(
        msg.contains("render_mode"),
        "error should mention render_mode, got: {msg}"
    );
}

#[test]
fn render_mode_empty_string_is_rejected() {
    let result = ws_request_to_overrides(Some(""), None, None, None);
    assert!(result.is_err(), "expected Err for empty render_mode");
}

#[test]
fn render_mode_none_leaves_field_unset() {
    let ov = ws_request_to_overrides(None, None, None, None).unwrap();
    assert!(
        ov.overrides.render_mode.is_none(),
        "render_mode should be None when not provided"
    );
}

// ── max_pages mapping ─────────────────────────────────────────────────────────

#[test]
fn max_pages_maps_correctly() {
    let ov = ws_request_to_overrides(None, Some(50), None, None).unwrap();
    assert_eq!(ov.overrides.max_pages, Some(50));
}

#[test]
fn max_pages_zero_maps_as_uncapped() {
    let ov = ws_request_to_overrides(None, Some(0), None, None).unwrap();
    assert_eq!(ov.overrides.max_pages, Some(0));
}

#[test]
fn max_pages_none_leaves_field_unset() {
    let ov = ws_request_to_overrides(None, None, None, None).unwrap();
    assert!(ov.overrides.max_pages.is_none());
}

// ── wait mapping ──────────────────────────────────────────────────────────────

#[test]
fn wait_true_maps_correctly() {
    let ov = ws_request_to_overrides(None, None, Some(true), None).unwrap();
    assert_eq!(ov.overrides.wait, Some(true));
}

#[test]
fn wait_false_maps_correctly() {
    let ov = ws_request_to_overrides(None, None, Some(false), None).unwrap();
    assert_eq!(ov.overrides.wait, Some(false));
}

#[test]
fn wait_none_leaves_field_unset() {
    let ov = ws_request_to_overrides(None, None, None, None).unwrap();
    assert!(ov.overrides.wait.is_none());
}

// ── response_mode mapping ─────────────────────────────────────────────────────

#[test]
fn response_mode_inline_maps_correctly() {
    let ov = ws_request_to_overrides(None, None, None, Some("inline")).unwrap();
    assert_eq!(ov.response_mode.as_deref(), Some("inline"));
}

#[test]
fn response_mode_path_maps_correctly() {
    let ov = ws_request_to_overrides(None, None, None, Some("path")).unwrap();
    assert_eq!(ov.response_mode.as_deref(), Some("path"));
}

#[test]
fn response_mode_artifact_maps_correctly() {
    let ov = ws_request_to_overrides(None, None, None, Some("artifact")).unwrap();
    assert_eq!(ov.response_mode.as_deref(), Some("artifact"));
}

#[test]
fn response_mode_invalid_is_rejected() {
    let result = ws_request_to_overrides(None, None, None, Some("stream"));
    assert!(
        result.is_err(),
        "expected Err for invalid response_mode 'stream'"
    );
    let msg = result.unwrap_err();
    assert!(
        msg.contains("response_mode"),
        "error should mention response_mode, got: {msg}"
    );
}

#[test]
fn response_mode_none_leaves_field_unset() {
    let ov = ws_request_to_overrides(None, None, None, None).unwrap();
    assert!(ov.response_mode.is_none());
}

// ── all-None produces empty overrides without panicking ───────────────────────

#[test]
fn all_none_returns_empty_overrides() {
    let ov = ws_request_to_overrides(None, None, None, None).unwrap();
    assert!(ov.overrides.render_mode.is_none());
    assert!(ov.overrides.max_pages.is_none());
    assert!(ov.overrides.wait.is_none());
    assert!(ov.response_mode.is_none());
}

// ── combined valid fields ─────────────────────────────────────────────────────

#[test]
fn combined_valid_fields_all_set() {
    let ov =
        ws_request_to_overrides(Some("chrome"), Some(100), Some(true), Some("inline")).unwrap();
    assert!(matches!(
        ov.overrides.render_mode.unwrap(),
        RenderMode::Chrome
    ));
    assert_eq!(ov.overrides.max_pages, Some(100));
    assert_eq!(ov.overrides.wait, Some(true));
    assert_eq!(ov.response_mode.as_deref(), Some("inline"));
}
