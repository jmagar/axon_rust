//! Typed HTTP validation errors for SSRF protection and URL validation.

use std::fmt;
use std::net::IpAddr;

/// Typed HTTP validation errors for SSRF protection and URL validation.
#[derive(Debug)]
pub enum HttpError {
    /// URL could not be parsed.
    InvalidUrl(String),
    /// URL uses a non-http/https scheme (e.g. ftp://, file://).
    BlockedScheme(String),
    /// Hostname is blocked (localhost, .internal, .local).
    BlockedHost(String),
    /// IP address falls in a blocked range (loopback, link-local, RFC-1918).
    BlockedIpRange(IpAddr),
    /// Network-level error from reqwest.
    Network(reqwest::Error),
    /// DNS resolution would target a blocked host.
    Dns(String),
}

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUrl(url) => write!(f, "invalid URL: {url}"),
            Self::BlockedScheme(scheme) => {
                write!(f, "blocked URL scheme '{scheme}': only http/https allowed")
            }
            Self::BlockedHost(host) => write!(f, "blocked host '{host}'"),
            Self::BlockedIpRange(ip) => write!(f, "blocked IP '{ip}': private/reserved range"),
            Self::Network(err) => write!(f, "network error: {err}"),
            Self::Dns(msg) => write!(f, "DNS error: {msg}"),
        }
    }
}

impl std::error::Error for HttpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Network(err) => Some(err),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for HttpError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err)
    }
}
