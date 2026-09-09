use super::*;
use axon_core::config::{Config, McpTransport};

#[test]
fn config_defaults_to_stdio_transport() {
    let cfg = Config::default();
    assert_eq!(cfg.mcp_transport, McpTransport::Stdio);
    assert_eq!(cfg.mcp_http_host, "127.0.0.1");
    assert_eq!(cfg.mcp_http_port, 8001);
}

#[tokio::test]
async fn transports_receive_the_same_prebuilt_service_context() {
    let temp = tempfile::tempdir().unwrap();
    let cfg = Config {
        sqlite_path: temp.path().join("jobs.db"),
        qdrant_url: String::new(),
        tei_url: String::new(),
        ..Config::default()
    };
    let context = Arc::new(ServiceContext::new(Arc::new(cfg.clone())).await.unwrap());
    for (transport, expected) in [
        (McpTransport::Stdio, vec!["stdio"]),
        (McpTransport::Http, vec!["http"]),
        (McpTransport::Both, vec!["stdio", "http"]),
    ] {
        let cfg = Config {
            mcp_transport: transport,
            ..cfg.clone()
        };
        let seen = std::sync::Mutex::new(Vec::new());
        run_transports(
            &cfg,
            Arc::clone(&context),
            |_, received| {
                assert!(Arc::ptr_eq(&received, &context));
                seen.lock().unwrap().push("stdio");
                std::future::ready(Ok(()))
            },
            |_, received| {
                assert!(Arc::ptr_eq(&received, &context));
                seen.lock().unwrap().push("http");
                std::future::ready(Ok(()))
            },
        )
        .await
        .unwrap();
        assert_eq!(*seen.lock().unwrap(), expected);
    }
    context.shutdown_background_tasks().await;
}
