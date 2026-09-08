use axon_core::config::{Config, McpTransport};
use axon_core::hardening::enforce_core_dump_disabled_for_ask_cache;
use axon_services::context::ServiceContext;
use std::error::Error;
use std::future::Future;
use std::sync::Arc;

pub async fn run_mcp(cfg: &Config) -> Result<(), Box<dyn Error>> {
    enforce_core_dump_disabled_for_ask_cache(cfg).map_err(|e| -> Box<dyn Error> { e.into() })?;
    let context = Arc::new(
        ServiceContext::new_with_workers_and_schedulers(Arc::new(cfg.clone()))
            .await
            .map_err(|e| -> Box<dyn Error> { e })?,
    );
    let result = run_transports(
        cfg,
        Arc::clone(&context),
        axon_mcp::run_stdio_server_with_context,
        |cfg, context| async move {
            crate::commands::run_unified_server(
                cfg.clone(),
                &cfg.mcp_http_host,
                cfg.mcp_http_port,
                context,
            )
            .await
        },
    )
    .await;
    context.shutdown_background_tasks().await;
    result
}

async fn run_transports<S, H>(
    cfg: &Config,
    context: Arc<ServiceContext>,
    stdio: impl FnOnce(Config, Arc<ServiceContext>) -> S,
    http: impl FnOnce(Config, Arc<ServiceContext>) -> H,
) -> Result<(), Box<dyn Error>>
where
    S: Future<Output = Result<(), Box<dyn Error>>>,
    H: Future<Output = Result<(), Box<dyn Error>>>,
{
    match cfg.mcp_transport {
        McpTransport::Stdio => stdio(cfg.clone(), context).await,
        McpTransport::Http => http(cfg.clone(), context).await,
        McpTransport::Both => tokio::try_join!(
            stdio(cfg.clone(), Arc::clone(&context)),
            http(cfg.clone(), context),
        )
        .map(|_| ()),
    }
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
