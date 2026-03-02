//! SSRF protection: URL validation and IP range blocking.

use spider::url::Url;
use std::net::IpAddr;

use super::error::HttpError;
use super::normalize::normalize_url;

/// Reject URLs that would allow SSRF attacks.
///
/// Blocks:
/// - Non-http/https schemes
/// - Loopback addresses (127.0.0.0/8, ::1)
/// - Link-local addresses (169.254.0.0/16, fe80::/10)
/// - RFC-1918 private ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
/// - `.internal` and `.local` TLDs
///
/// # Errors
///
/// Returns `Err` if the URL is malformed, uses a non-HTTP(S) scheme, or resolves
/// to a blocked address range.
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
pub fn validate_url(url: &str) -> Result<(), HttpError> {
    let normalized = normalize_url(url);
    let parsed = Url::parse(&normalized).map_err(|_| HttpError::InvalidUrl(url.to_string()))?;

    match parsed.scheme() {
        "http" | "https" => {}
        s => return Err(HttpError::BlockedScheme(s.to_string())),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| HttpError::InvalidUrl(url.to_string()))?;

    // Block localhost and .internal/.local TLDs
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower.ends_with(".localhost") {
        return Err(HttpError::BlockedHost(host.to_string()));
    }
    if lower.ends_with(".internal") || lower.ends_with(".local") {
        return Err(HttpError::BlockedHost(host.to_string()));
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
fn check_ip(ip: IpAddr) -> Result<(), HttpError> {
    if ip.is_loopback() {
        return Err(HttpError::BlockedIpRange(ip));
    }
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, ..] = v4.octets();
            let octets = v4.octets();
            let is_link_local = octets[0] == 169 && octets[1] == 254;
            let is_private = octets[0] == 10
                || (a == 172 && (16..=31).contains(&b))
                || octets[0..2] == [192, 168];
            if is_link_local || is_private {
                return Err(HttpError::BlockedIpRange(IpAddr::V4(v4)));
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
                return Err(HttpError::BlockedIpRange(IpAddr::V6(v6)));
            }
        }
    }
    Ok(())
}

/// SSRF defence-in-depth patterns for spider.rs `with_blacklist_url()`.
///
/// Covers RFC-1918 private ranges, loopback, link-local, and IPv6 private addresses.
/// Use alongside `validate_url()` on the seed URL so discovered URLs are also blocked.
pub(crate) fn ssrf_blacklist_patterns() -> &'static [&'static str] {
    &[
        r"^https?://127\.",
        r"^https?://10\.",
        r"^https?://192\.168\.",
        r"^https?://172\.(1[6-9]|2[0-9]|3[01])\.",
        r"^https?://169\.254\.",
        r"^https?://0\.",
        r"^https?://localhost([^a-zA-Z0-9]|$)",
        r"^https?://\[::1\]",
        r"^https?://\[::ffff:",
        r"^https?://\[fe80:",
        r"^https?://\[fc[0-9a-f]{2}:",
        r"^https?://\[fd[0-9a-f]{2}:",
    ]
}
