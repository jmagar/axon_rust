use super::*;
use axon_core::config::{Config, McpTransport};

#[test]
fn config_defaults_to_stdio_transport() {
    let cfg = Config::default();
    assert_eq!(cfg.mcp_transport, McpTransport::Stdio);
    assert_eq!(cfg.mcp_http_host, "127.0.0.1");
    assert_eq!(cfg.mcp_http_port, 8001);
}

#[test]
fn every_mcp_transport_uses_one_worker_context() {
    for transport in [McpTransport::Stdio, McpTransport::Http, McpTransport::Both] {
        let plan = runtime_plan(transport);
        assert_eq!(plan.context_count, 1, "transport={transport}");
    }
}

#[test]
fn both_transports_share_the_prebuilt_context() {
    assert!(runtime_plan(McpTransport::Both).share_between_transports);
}
