#[path = "mcp/config.rs"]
mod config;
#[path = "mcp/schema.rs"]
mod schema;
#[path = "mcp/server.rs"]
mod server;

pub use server::{run_http_server, run_stdio_server};
