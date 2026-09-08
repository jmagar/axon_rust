use axon_core::config::Config;
use axon_core::hardening::enforce_core_dump_disabled_for_ask_cache;
use std::error::Error;
use std::sync::Arc;

pub async fn run_serve(cfg: &Config) -> Result<(), Box<dyn Error>> {
    enforce_core_dump_disabled_for_ask_cache(cfg).map_err(|e| -> Box<dyn Error> { e.into() })?;
    let context = Arc::new(
        axon_services::context::ServiceContext::new_with_workers_and_schedulers(Arc::new(
            cfg.clone(),
        ))
        .await
        .map_err(|e| -> Box<dyn Error> { e })?,
    );
    let result = crate::commands::run_unified_server(
        cfg.clone(),
        &cfg.mcp_http_host,
        cfg.mcp_http_port,
        Arc::clone(&context),
    )
    .await;
    context.shutdown_background_tasks().await;
    result
}
