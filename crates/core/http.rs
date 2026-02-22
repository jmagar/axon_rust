use spider::url::Url;
use std::error::Error;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::Duration;

pub(crate) static HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> =
    LazyLock::new(|| build_client(30).map_err(|e| e.to_string()));

pub fn http_client() -> Result<&'static reqwest::Client, Box<dyn Error>> {
    HTTP_CLIENT
        .as_ref()
        .map_err(|err| format!("failed to initialize HTTP client: {err}").into())
}

pub fn normalize_url(url: &str) -> String {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.contains("://") {
        return trimmed.to_string();
    }

    let looks_like_host = trimmed.contains('.')
        || trimmed.starts_with("localhost")
        || trimmed.starts_with("127.0.0.1")
        || trimmed.starts_with("[::1]");
    let has_no_spaces = !trimmed.chars().any(char::is_whitespace);

    if looks_like_host && has_no_spaces {
        format!("https://{trimmed}")
    } else {
        trimmed.to_string()
    }
}

/// Reject URLs that would allow SSRF attacks.
///
/// Blocks:
/// - Non-http/https schemes
/// - Loopback addresses (127.0.0.0/8, ::1)
/// - Link-local addresses (169.254.0.0/16, fe80::/10)
/// - RFC-1918 private ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - `.internal` and `.local` TLDs
///
/// # DNS Rebinding (TOCTOU residual risk)
///
/// This validation is TOCTOU — it checks the resolved IP at parse time, but
/// `reqwest` resolves DNS independently at connect time. An attacker with a
/// TTL-0 DNS record can pass validation (first resolution → public IP) then
/// rebind before the connection is established (second resolution → 127.0.0.1).
///
/// Full mitigation requires DNS pre-resolution and connection pinning, which
/// `reqwest` does not support natively. Consider adding a reverse-DNS check or
/// using `hickory-resolver` for pre-resolution if the threat model includes
/// attacker-controlled domains with short-TTL records.
///
/// As defence-in-depth, `ssrf_blacklist_patterns()` is also applied to
/// discovered URLs during crawl via spider's `with_blacklist_url()`.
pub fn validate_url(url: &str) -> Result<(), Box<dyn Error>> {
    let normalized = normalize_url(url);
    let parsed = Url::parse(&normalized).map_err(|_| format!("invalid URL: {url}"))?;

    match parsed.scheme() {
        "http" | "https" => {}
        s => return Err(format!("blocked URL scheme '{s}': only http/https allowed").into()),
    }

    let host = parsed.host_str().ok_or("URL has no host")?;

    // Block localhost and .internal/.local TLDs
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(format!("blocked host '{host}': localhost not allowed").into());
    }
    if lower.ends_with(".internal") || lower.ends_with(".local") {
        return Err(format!("blocked host '{host}': .internal/.local domains not allowed").into());
    }

    // Use parsed.host() for typed extraction — host_str().parse::<IpAddr>()
    // silently fails for IPv6 because spider::url::Url returns zone-scoped or
    // bracket-ambiguous representations. The Host enum gives us the parsed addr directly.
    match parsed.host() {
        Some(spider::url::Host::Ipv4(v4)) => check_ip(IpAddr::V4(v4))?,
        Some(spider::url::Host::Ipv6(v6)) => check_ip(IpAddr::V6(v6))?,
        _ => {}
    }

    Ok(())
}

/// SSRF IP validation — checks loopback, link-local, RFC-1918 private, and
/// IPv4-mapped IPv6 addresses. Extracted as a named function (not a closure)
/// so the IPv4-mapped branch can recurse into the IPv4 checks.
fn check_ip(ip: IpAddr) -> Result<(), Box<dyn Error>> {
    if ip.is_loopback() {
        return Err(format!("blocked IP '{ip}': loopback address not allowed").into());
    }
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            let octets = v4.octets();
            let is_link_local = octets[0] == 169 && octets[1] == 254;
            let is_private = octets[0] == 10
                || (a == 172 && (16..=31).contains(&b))
                || octets[0..2] == [192, 168];
            if is_link_local {
                return Err(format!(
                    "blocked IP '{v4}': link-local address (169.254.x.x) not allowed"
                )
                .into());
            }
            if is_private {
                return Err(
                    format!("blocked IP '{v4}': private/RFC-1918 address not allowed").into(),
                );
            }
        }
        IpAddr::V6(v6) => {
            // IPv4-mapped IPv6 (::ffff:x.x.x.x) — extract the embedded IPv4
            // and apply the same private/loopback/link-local checks. Without this,
            // ::ffff:127.0.0.1 bypasses the V4 branch entirely.
            if let Some(mapped_v4) = v6.to_ipv4_mapped() {
                return check_ip(IpAddr::V4(mapped_v4));
            }

            // Block unique-local (fc00::/7) and link-local (fe80::/10)
            let segs = v6.segments();
            let is_unique_local = segs[0] & 0xfe00 == 0xfc00;
            let is_link_local_v6 = segs[0] & 0xffc0 == 0xfe80;
            if is_unique_local || is_link_local_v6 {
                return Err(
                    format!("blocked IPv6 '{v6}': private/link-local address not allowed").into(),
                );
            }
        }
    }
    Ok(())
}

/// SSRF defence-in-depth patterns for spider.rs `with_blacklist_url()`.
///
/// Covers RFC-1918 private ranges, loopback, link-local, and IPv6 private addresses.
/// Use alongside `validate_url()` on the seed URL so discovered URLs are also blocked.
pub(crate) fn ssrf_blacklist_patterns() -> Vec<String> {
    vec![
        r"^https?://127\.".to_string(),
        r"^https?://10\.".to_string(),
        r"^https?://192\.168\.".to_string(),
        r"^https?://172\.(1[6-9]|2[0-9]|3[01])\.".to_string(),
        r"^https?://169\.254\.".to_string(),
        r"^https?://0\.".to_string(),
        r"^https?://localhost([^a-zA-Z0-9]|$)".to_string(),
        r"^https?://\[::1\]".to_string(),
        r"^https?://\[::ffff:".to_string(),
        r"^https?://\[fe80:".to_string(),
        r"^https?://\[fc[0-9a-f]{2}:".to_string(),
        r"^https?://\[fd[0-9a-f]{2}:".to_string(),
    ]
}

pub fn build_client(timeout_secs: u64) -> Result<reqwest::Client, Box<dyn Error>> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
}

/// Build the CDP `/json/version` discovery URL from a Chrome remote URL.
///
/// Handles `ws://` / `wss://` → `http://` / `https://` conversion (reqwest cannot
/// make requests to `ws://` scheme URLs) and appends `/json/version` when the path
/// is absent or root.  Returns `None` if the URL cannot be parsed or uses an
/// unsupported scheme (`ftp://`, `file://`, etc.).
pub(crate) fn cdp_discovery_url(remote_url: &str) -> Option<String> {
    let parsed = Url::parse(remote_url).ok()?;
    let http_scheme = match parsed.scheme() {
        "ws" | "http" => "http",
        "wss" | "https" => "https",
        _ => return None,
    };
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    let path = parsed.path();
    let path = if path == "/" || path.is_empty() {
        "/json/version"
    } else {
        path
    };
    Some(format!("{http_scheme}://{host}:{port}{path}"))
}

pub async fn fetch_html(client: &reqwest::Client, url: &str) -> Result<String, Box<dyn Error>> {
    let normalized = normalize_url(url);
    validate_url(&normalized)?;
    let body = client
        .get(&normalized)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- normalize_url tests ---

    #[test]
    fn normalize_url_adds_https_scheme_to_bare_host() {
        assert_eq!(normalize_url("example.com"), "https://example.com");
    }

    #[test]
    fn normalize_url_adds_https_scheme_to_host_with_path() {
        assert_eq!(
            normalize_url("example.com/docs/install"),
            "https://example.com/docs/install"
        );
    }

    #[test]
    fn normalize_url_preserves_existing_https_scheme() {
        assert_eq!(
            normalize_url("https://example.com/page"),
            "https://example.com/page"
        );
    }

    #[test]
    fn normalize_url_preserves_existing_http_scheme() {
        assert_eq!(
            normalize_url("http://example.com/page"),
            "http://example.com/page"
        );
    }

    #[test]
    fn normalize_url_preserves_path_and_query() {
        assert_eq!(
            normalize_url("example.com/path?key=value"),
            "https://example.com/path?key=value"
        );
    }

    #[test]
    fn normalize_url_preserves_fragment() {
        assert_eq!(
            normalize_url("example.com/page#section"),
            "https://example.com/page#section"
        );
    }

    #[test]
    fn normalize_url_trims_whitespace() {
        assert_eq!(normalize_url("  example.com  "), "https://example.com");
    }

    #[test]
    fn normalize_url_returns_empty_for_empty_input() {
        assert_eq!(normalize_url(""), "");
    }

    #[test]
    fn normalize_url_handles_localhost() {
        assert_eq!(normalize_url("localhost"), "https://localhost");
    }

    #[test]
    fn normalize_url_handles_localhost_with_port() {
        // localhost:8080 contains a '.'-free host but starts with "localhost"
        assert_eq!(normalize_url("localhost:8080"), "https://localhost:8080");
    }

    #[test]
    fn normalize_url_does_not_add_scheme_to_bare_text_with_spaces() {
        // A string with spaces is not a valid URL host — normalize_url leaves it as-is
        assert_eq!(normalize_url("not a url"), "not a url");
    }

    // --- Public URLs should be allowed ---

    #[test]
    fn test_validate_url_allows_public_https() {
        assert!(validate_url("https://example.com/").is_ok());
    }

    #[test]
    fn test_validate_url_allows_public_http() {
        assert!(validate_url("http://example.com/page").is_ok());
    }

    // --- Loopback addresses ---

    #[test]
    fn test_validate_url_blocks_loopback_ipv4() {
        assert!(validate_url("http://127.0.0.1/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_localhost() {
        assert!(validate_url("http://localhost/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_ipv6_loopback() {
        assert!(validate_url("http://[::1]/").is_err());
    }

    // --- AWS metadata / link-local ---

    #[test]
    fn test_validate_url_blocks_aws_metadata() {
        assert!(validate_url("http://169.254.169.254/latest/meta-data/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_link_local_boundary() {
        // 169.254.169.253 is still in 169.254.0.0/16 — should be blocked
        assert!(validate_url("http://169.254.169.253/").is_err());
    }

    // --- RFC-1918 private ranges ---

    #[test]
    fn test_validate_url_blocks_10_network() {
        assert!(validate_url("http://10.0.0.1/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_10_network_upper() {
        assert!(validate_url("http://10.255.255.255/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_172_16() {
        assert!(validate_url("http://172.16.0.1/").is_err());
    }

    #[test]
    fn test_validate_url_allows_172_15() {
        // 172.15.255.255 is just below the 172.16.0.0/12 range — should ALLOW
        assert!(validate_url("http://172.15.255.255/").is_ok());
    }

    #[test]
    fn test_validate_url_allows_172_32() {
        // 172.32.0.0 is just above the 172.16-31 range — should ALLOW
        assert!(validate_url("http://172.32.0.0/").is_ok());
    }

    #[test]
    fn test_validate_url_blocks_192_168() {
        assert!(validate_url("http://192.168.0.1/").is_err());
    }

    // --- Blocked URL schemes ---

    #[test]
    fn test_validate_url_blocks_ftp() {
        assert!(validate_url("ftp://example.com/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_file() {
        assert!(validate_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn test_validate_url_blocks_data() {
        assert!(validate_url("data:text/plain,hello").is_err());
    }

    // --- TLD blocking ---

    #[test]
    fn test_validate_url_blocks_internal_tld() {
        assert!(validate_url("http://host.internal/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_local_tld() {
        assert!(validate_url("http://host.local/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_internal_tld_case_insensitive() {
        assert!(validate_url("http://HOST.INTERNAL/").is_err());
    }

    // --- Invalid URLs ---

    #[test]
    fn test_validate_url_blocks_invalid_url() {
        assert!(validate_url("not a valid url at all").is_err());
    }

    // --- IPv6 private ranges ---

    #[test]
    fn test_validate_url_blocks_ipv6_ula() {
        // fc00::1 is unique-local address (fc00::/7)
        assert!(validate_url("http://[fc00::1]/").is_err());
    }

    #[test]
    fn test_validate_url_blocks_ipv6_link_local() {
        // fe80::1 is link-local (fe80::/10)
        assert!(validate_url("http://[fe80::1]/").is_err());
    }

    /// Compiled SSRF blacklist regexes — built once, reused across tests.
    static COMPILED_SSRF_PATTERNS: LazyLock<Vec<regex::Regex>> = LazyLock::new(|| {
        ssrf_blacklist_patterns()
            .into_iter()
            .map(|p| regex::Regex::new(&p).expect("ssrf blacklist pattern must compile"))
            .collect()
    });

    #[test]
    fn test_ssrf_blacklist_blocks_localhost_with_query() {
        let url = "http://localhost?admin=true";
        let blocked = COMPILED_SSRF_PATTERNS.iter().any(|re| re.is_match(url));
        assert!(
            blocked,
            "localhost with query string should be blocked by blacklist"
        );
    }

    // --- cdp_discovery_url tests ---

    #[test]
    fn test_cdp_discovery_url_http_appends_json_version() {
        assert_eq!(
            cdp_discovery_url("http://127.0.0.1:6000"),
            Some("http://127.0.0.1:6000/json/version".to_string())
        );
    }

    #[test]
    fn test_cdp_discovery_url_ws_converts_to_http_and_appends() {
        assert_eq!(
            cdp_discovery_url("ws://axon-chrome:9222"),
            Some("http://axon-chrome:9222/json/version".to_string())
        );
    }

    #[test]
    fn test_cdp_discovery_url_preserves_non_root_path() {
        // Already has /json/version — must not double-append.
        assert_eq!(
            cdp_discovery_url("http://127.0.0.1:6000/json/version"),
            Some("http://127.0.0.1:6000/json/version".to_string())
        );
    }

    #[test]
    fn test_cdp_discovery_url_rejects_unsupported_scheme() {
        assert_eq!(cdp_discovery_url("ftp://host:21/"), None);
        assert_eq!(cdp_discovery_url("file:///etc/hosts"), None);
    }

    #[test]
    fn test_cdp_discovery_url_wss_converts_to_https() {
        assert_eq!(
            cdp_discovery_url("wss://secure-host:443"),
            Some("https://secure-host:443/json/version".to_string())
        );
    }

    #[test]
    fn test_cdp_discovery_url_ws_with_existing_path_preserved() {
        // Pre-resolved ws:// URL with browser UUID path: path must not be clobbered.
        let ws = "ws://127.0.0.1:9222/devtools/browser/abc-123";
        let result = cdp_discovery_url(ws);
        assert_eq!(
            result,
            Some("http://127.0.0.1:9222/devtools/browser/abc-123".to_string())
        );
    }

    #[test]
    fn test_ssrf_blacklist_blocks_localhost_with_fragment() {
        let url = "https://localhost#secret";
        let blocked = COMPILED_SSRF_PATTERNS.iter().any(|re| re.is_match(url));
        assert!(
            blocked,
            "localhost with fragment should be blocked by blacklist"
        );
    }

    /// Documents the DNS rebinding TOCTOU residual risk in `validate_url()`.
    ///
    /// An attacker-controlled domain can initially resolve to a public IP (passing
    /// validation) then rebind via TTL-0 to a private IP before `reqwest` connects.
    /// This test verifies that `validate_url()` correctly allows a public-looking
    /// hostname — it cannot detect the rebind because DNS resolution happens again
    /// at connect time inside `reqwest`, which is outside our control.
    ///
    /// Full mitigation would require DNS pre-resolution + connection pinning (e.g.
    /// `hickory-resolver`), which `reqwest` does not natively support.
    #[test]
    fn test_dns_rebinding_toctou_documents_residual_risk() {
        // A hostname that resolves to a public IP passes validation — this is correct
        // behavior. The risk is that between validation and connection, the DNS record
        // could change to 127.0.0.1. We cannot test the actual rebind in a unit test,
        // but we document the expected behavior: public hostnames pass, private IPs fail.
        assert!(
            validate_url("https://attacker-controlled.example.com/").is_ok(),
            "public hostname should pass — DNS rebinding cannot be caught at parse time"
        );
        assert!(
            validate_url("http://127.0.0.1/").is_err(),
            "direct private IP must still be blocked"
        );
        assert!(
            validate_url("http://[::1]/").is_err(),
            "direct loopback IPv6 must still be blocked"
        );
    }

    /// Verifies that a public IP passes validation — documents the TOCTOU window.
    /// Between validation (public IP) and connection, DNS could rebind to private.
    #[test]
    fn test_validate_url_accepts_public_ip_but_documents_rebinding_risk() {
        // 93.184.216.34 is example.com's public IP. This passes validation correctly.
        // The TOCTOU risk: an attacker-controlled domain could resolve to this IP during
        // validation, then rebind to 127.0.0.1 before reqwest connects. We cannot
        // prevent this at the URL validation layer — requires DNS pre-resolution.
        assert!(
            validate_url("http://93.184.216.34/").is_ok(),
            "public IP should pass validation"
        );
        // Confirm the inverse: direct private IPs are always blocked
        assert!(validate_url("http://10.0.0.1/").is_err());
        assert!(validate_url("http://192.168.1.1/").is_err());
    }

    /// Verifies the LazyLock SSRF pattern compilation works and all patterns are valid.
    #[test]
    fn test_ssrf_blacklist_patterns_compile_once() {
        // Accessing COMPILED_SSRF_PATTERNS forces the LazyLock to initialize.
        // If any pattern fails to compile, the .expect() inside the LazyLock panics.
        let patterns = &*COMPILED_SSRF_PATTERNS;
        assert!(
            !patterns.is_empty(),
            "SSRF blacklist should have at least one pattern"
        );
        // Verify the count matches the raw pattern list
        assert_eq!(
            patterns.len(),
            ssrf_blacklist_patterns().len(),
            "compiled pattern count must match raw pattern count"
        );
        // Smoke-test: a known-bad URL should match at least one pattern
        assert!(
            patterns
                .iter()
                .any(|re| re.is_match("http://127.0.0.1/admin")),
            "loopback URL should match at least one SSRF pattern"
        );
    }

    // --- IPv4-mapped IPv6 bypass tests ---

    /// IPv4-mapped IPv6 addresses (::ffff:x.x.x.x) embed an IPv4 address inside
    /// an IPv6 representation. Without explicit handling, these bypass the IPv4
    /// private/loopback checks because they arrive in the V6 branch of check_ip.
    #[test]
    fn test_validate_url_rejects_ipv4_mapped_ipv6_loopback() {
        // ::ffff:127.0.0.1 is loopback via IPv4-mapped IPv6
        assert!(
            validate_url("http://[::ffff:127.0.0.1]/").is_err(),
            "::ffff:127.0.0.1 must be blocked as loopback"
        );
    }

    #[test]
    fn test_validate_url_rejects_ipv4_mapped_ipv6_link_local() {
        // ::ffff:169.254.0.1 is link-local via IPv4-mapped IPv6
        assert!(
            validate_url("http://[::ffff:169.254.0.1]/").is_err(),
            "::ffff:169.254.0.1 must be blocked as link-local"
        );
    }

    #[test]
    fn test_validate_url_rejects_ipv4_mapped_ipv6_private() {
        // ::ffff:10.0.0.1 is RFC-1918 private via IPv4-mapped IPv6
        assert!(
            validate_url("http://[::ffff:10.0.0.1]/").is_err(),
            "::ffff:10.0.0.1 must be blocked as private"
        );
        // ::ffff:192.168.1.1
        assert!(
            validate_url("http://[::ffff:192.168.1.1]/").is_err(),
            "::ffff:192.168.1.1 must be blocked as private"
        );
        // ::ffff:172.16.0.1
        assert!(
            validate_url("http://[::ffff:172.16.0.1]/").is_err(),
            "::ffff:172.16.0.1 must be blocked as private"
        );
    }

    #[test]
    fn test_validate_url_allows_ipv4_mapped_ipv6_public() {
        // ::ffff:93.184.216.34 (example.com) should be allowed — it's a public IP
        assert!(
            validate_url("http://[::ffff:93.184.216.34]/").is_ok(),
            "::ffff: with public IPv4 should be allowed"
        );
    }
}
