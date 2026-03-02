use crate::crates::core::config::Config;
use std::error::Error;

pub async fn run_mcp(_cfg: &Config) -> Result<(), Box<dyn Error>> {
    crate::crates::mcp::run_stdio_server().await
}
