#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    axon::run().await
}
