use axon_core::config::{Config, McpTransport};
use axon_core::hardening::enforce_core_dump_disabled_for_ask_cache;
use std::error::Error;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
struct McpRuntimePlan {
    context_count: usize,
    share_between_transports: bool,
}

fn runtime_plan(transport: McpTransport) -> McpRuntimePlan {
    McpRuntimePlan {
        context_count: 1,
        share_between_transports: matches!(transport, McpTransport::Both),
    }
}

async fn run_stdio_server(
    cfg: Config,
    context: Arc<axon_services::context::ServiceContext>,
) -> Result<(), Box<dyn Error>> {
    axon_mcp::run_stdio_server_with_context(cfg, context).await
}

pub async fn run_mcp(cfg: &Config) -> Result<(), Box<dyn Error>> {
    enforce_core_dump_disabled_for_ask_cache(cfg).map_err(|e| -> Box<dyn Error> { e.into() })?;
    let plan = runtime_plan(cfg.mcp_transport);
    debug_assert_eq!(plan.context_count, 1);
    let context = Arc::new(
        axon_services::context::ServiceContext::new_with_workers_and_schedulers(Arc::new(
            cfg.clone(),
        ))
        .await
        .map_err(|e| -> Box<dyn Error> { e })?,
    );
    let result = match cfg.mcp_transport {
        McpTransport::Stdio => run_stdio_server(cfg.clone(), Arc::clone(&context)).await,
        McpTransport::Http => {
            crate::commands::run_unified_server(
                cfg.clone(),
                &cfg.mcp_http_host,
                cfg.mcp_http_port,
                Arc::clone(&context),
            )
            .await
        }
        McpTransport::Both => {
            debug_assert!(plan.share_between_transports);
            let host = cfg.mcp_http_host.clone();
            let port = cfg.mcp_http_port;
            tokio::try_join!(
                run_stdio_server(cfg.clone(), Arc::clone(&context)),
                crate::commands::run_unified_server(
                    cfg.clone(),
                    &host,
                    port,
                    Arc::clone(&context),
                ),
            )
            .map(|_| ())
        }
    };
    context.shutdown_background_tasks().await;
    result
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
