use spider::url::Url;
use std::error::Error;
use std::net::IpAddr;
use std::time::Duration;

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

    // Use Url::host() for typed IP extraction — avoids string-parsing edge cases
    // with IPv6 bracket notation that host_str().parse::<IpAddr>() may miss.
    let check_ip = |ip: IpAddr| -> Result<(), Box<dyn Error>> {
        if ip.is_loopback() {
            return Err(format!("blocked IP '{ip}': loopback address not allowed").into());
        }
        match ip {
            IpAddr::V4(v4) => {
                let [a, b, ..] = v4.octets();
                let octets = v4.octets();
                let is_link_local = octets[0] == 169 && octets[1] == 254;
                let is_private =
                    octets[0] == 10 || (a == 172 && (16..=31).contains(&b)) || octets[0..2] == [192, 168];
                if is_link_local {
                    return Err(format!(
                        "blocked IP '{v4}': link-local address (169.254.x.x) not allowed"
                    )
                    .into());
                }
                if is_private {
                    return Err(format!(
                        "blocked IP '{v4}': private/RFC-1918 address not allowed"
                    )
                    .into());
                }
            }
            IpAddr::V6(v6) => {
                // Block unique-local (fc00::/7) and link-local (fe80::/10)
                let segs = v6.segments();
                let is_unique_local = segs[0] & 0xfe00 == 0xfc00;
                let is_link_local_v6 = segs[0] & 0xffc0 == 0xfe80;
                if is_unique_local || is_link_local_v6 {
                    return Err(format!(
                        "blocked IPv6 '{v6}': private/link-local address not allowed"
                    )
                    .into());
                }
            }
        }
        Ok(())
    };

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

pub fn build_client(timeout_secs: u64) -> Result<reqwest::Client, Box<dyn Error>> {
    Ok(reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?)
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
}
