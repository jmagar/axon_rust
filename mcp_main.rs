#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Install aws-lc-rs as the process-level rustls crypto provider. See main.rs comment.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // No .env file — expected in production/CI
        }
        Err(e) => {
            eprintln!("warning: failed to load .env: {e}");
        }
    }

    axon::crates::mcp::run_stdio_server().await
}
