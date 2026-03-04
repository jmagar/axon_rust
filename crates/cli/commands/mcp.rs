use crate::crates::core::config::Config;
use std::error::Error;

pub async fn run_mcp(_cfg: &Config) -> Result<(), Box<dyn Error>> {
    let (host, port) = resolve_http_bind_from_env()?;
    crate::crates::mcp::run_http_server(&host, port).await
}

fn resolve_http_bind_from_env() -> Result<(String, u16), Box<dyn Error>> {
    let host_env = std::env::var("AXON_MCP_HTTP_HOST").ok();
    let port_env = std::env::var("AXON_MCP_HTTP_PORT").ok();
    resolve_http_bind(host_env, port_env)
}

fn resolve_http_bind(
    host_env: Option<String>,
    port_env: Option<String>,
) -> Result<(String, u16), Box<dyn Error>> {
    let host = host_env.unwrap_or_else(|| "0.0.0.0".to_string());
    let port_raw = port_env.unwrap_or_else(|| "8001".to_string());
    let port = parse_http_port(&port_raw)?;
    Ok((host, port))
}

fn parse_http_port(port_raw: &str) -> Result<u16, Box<dyn Error>> {
    port_raw
        .parse::<u16>()
        .map_err(|e| format!("invalid AXON_MCP_HTTP_PORT '{port_raw}': {e}").into())
}

#[cfg(test)]
mod tests {
    use super::{parse_http_port, resolve_http_bind};

    #[test]
    fn parse_http_port_rejects_invalid_port() {
        let err = parse_http_port("not-a-port")
            .expect_err("non-numeric port should be rejected")
            .to_string();
        assert!(err.contains("invalid AXON_MCP_HTTP_PORT 'not-a-port'"));
    }

    #[test]
    fn resolve_http_bind_uses_defaults_when_env_missing() {
        let (host, port) = resolve_http_bind(None, None).expect("default host/port should resolve");
        assert_eq!(host, "0.0.0.0");
        assert_eq!(port, 8001);
    }

    #[test]
    fn resolve_http_bind_uses_env_values() {
        let (host, port) =
            resolve_http_bind(Some("127.0.0.1".to_string()), Some("18001".to_string()))
                .expect("env host/port should resolve");
        assert_eq!(host, "127.0.0.1");
        assert_eq!(port, 18001);
    }
}
