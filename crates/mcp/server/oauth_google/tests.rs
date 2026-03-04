#![cfg(test)]

use super::helpers::{is_allowed_redirect_uri, normalize_loopback_redirect_uri};
use super::types::RedirectPolicy;

#[test]
fn normalize_loopback_redirect_uri_prefers_localhost_http() {
    let uri = normalize_loopback_redirect_uri("https://127.0.0.1:34543/callback")
        .expect("loopback uri should normalize");
    assert_eq!(uri, "http://localhost:34543/callback");
}

#[test]
fn redirect_policy_loopback_only_rejects_non_loopback() {
    assert!(is_allowed_redirect_uri(
        "http://localhost:5555/callback",
        RedirectPolicy::LoopbackOnly
    ));
    assert!(!is_allowed_redirect_uri(
        "https://axon.tootie.tv/callback",
        RedirectPolicy::LoopbackOnly
    ));
}

#[test]
fn redirect_policy_any_allows_non_loopback() {
    assert!(is_allowed_redirect_uri(
        "https://axon.tootie.tv/callback",
        RedirectPolicy::Any
    ));
}
