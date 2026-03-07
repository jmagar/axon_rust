use crate::crates::core::config::Config;
use std::error::Error;
use std::sync::Arc;

pub async fn run_serve(cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::crates::web::start_server(cfg.serve_port, Arc::new(cfg.clone())).await
}
