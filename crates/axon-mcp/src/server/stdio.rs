use crate::auth::AuthPolicy;
use axon_core::config::Config;
use axon_services::context::ServiceContext;
use rmcp::{ServiceExt, transport::stdio};
use std::sync::Arc;
use tokio::sync::OnceCell;

use super::AxonMcpServer;

pub async fn run_stdio_server(cfg: Config) -> Result<(), Box<dyn std::error::Error>> {
    let context = Arc::new(
        ServiceContext::new_with_workers_and_schedulers(Arc::new(cfg.clone()))
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.to_string().into() })?,
    );
    let result = run_stdio_server_with_context(cfg, Arc::clone(&context)).await;
    context.shutdown_background_tasks().await;
    result
}

pub async fn run_stdio_server_with_context(
    cfg: Config,
    context: Arc<ServiceContext>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Stdio always uses LoopbackDev: process isolation is the trust boundary.
    let service_context = Arc::new(OnceCell::new());
    service_context
        .set(context)
        .map_err(|_| "stdio: failed to initialize service context")?;
    let server = AxonMcpServer::new_with_service_context_cell(
        cfg,
        service_context,
        Arc::new(OnceCell::new()),
    )
    .with_auth_policy(AuthPolicy::LoopbackDev);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
