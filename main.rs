#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match dotenvy::dotenv() {
        Ok(_) => {}
        Err(dotenvy::Error::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => {
            // No .env file — expected in production/CI
        }
        Err(e) => {
            eprintln!("warning: failed to load .env: {e}");
        }
    }
    axon::run().await
}
