use super::*;

#[test]
fn extracted_fetch_provider_redirects_do_not_exempt_other_helpers() {
    let root = tempfile::tempdir().unwrap();
    let helpers = root
        .path()
        .join("crates/axon-adapters/src/providers/http_fetch");
    std::fs::create_dir_all(&helpers).unwrap();
    std::fs::write(
        helpers.join("redirects.rs"),
        "let client = self.build_client()?;\n",
    )
    .unwrap();
    check(root.path()).expect("the existing provider's extracted redirect boundary is permitted");
    std::fs::write(
        helpers.join("other.rs"),
        "let client = self.build_client()?;\n",
    )
    .unwrap();
    let error = check(root.path()).unwrap_err().to_string();
    assert!(error.contains("http_fetch/other.rs:1"), "{error}");
}

#[test]
fn test_files_are_ignored() {
    assert!(is_ignored(
        "crates/axon-adapters/src/web_engine/scrape_tests.rs"
    ));
    assert!(is_ignored("crates/axon-adapters/tests/foo.rs"));
    assert!(is_ignored("crates/axon-adapters/src/testing.rs"));
    assert!(!is_ignored("crates/axon-adapters/src/web_engine/scrape.rs"));
}

#[test]
fn every_approved_exception_carries_a_real_reason() {
    for (path, reason) in APPROVED_EXCEPTIONS {
        assert!(!path.is_empty(), "empty exception path");
        assert!(
            reason.len() > 40,
            "exception for {path} needs a reason a reviewer can evaluate, got: {reason:?}"
        );
    }
}

#[test]
fn approved_exceptions_are_unique() {
    let mut seen = std::collections::HashSet::new();
    for (path, _) in APPROVED_EXCEPTIONS {
        assert!(seen.insert(*path), "duplicate exception entry: {path}");
    }
}

#[test]
fn exception_lookup_matches_exact_paths_only() {
    assert!(is_exception("crates/axon-adapters/src/web_engine/scrape.rs").is_some());
    // A near-miss must NOT inherit an exception.
    assert!(is_exception("crates/axon-adapters/src/web_engine/scrape2.rs").is_none());
    assert!(is_exception("crates/axon-extract/src/verticals/nonexistent.rs").is_none());
}

#[test]
fn tracked_fetchers_are_listed_but_still_flagged_as_tracked() {
    // These are known divergences, not blessed ones: they resolve to the
    // TRACKED reason so the message says migration is outstanding.
    let reason = is_exception("crates/axon-extract/src/verticals/reddit.rs")
        .expect("tracked vertical must resolve to a reason");
    assert!(reason.contains("TRACKED"), "{reason}");
}

#[test]
fn tracked_and_settled_lists_do_not_overlap() {
    for path in TRACKED_SHARED_CLIENT_FETCHERS {
        assert!(
            !APPROVED_EXCEPTIONS.iter().any(|(p, _)| p == path),
            "{path} is in both lists; its status would be ambiguous"
        );
    }
}
