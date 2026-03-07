#[path = "mcp/config.rs"]
mod config;
#[path = "mcp/schema.rs"]
pub mod schema;
#[path = "mcp/server.rs"]
pub mod server;

pub use server::{run_http_server, run_stdio_server};
