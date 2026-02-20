use tracing::{info, warn};

pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // Browserless (CDP proxy) sends non-standard session management frames over
    // the same WebSocket as standard CDP traffic. chromiumoxide's Message<T> is
    // an untagged enum with only two variants — Response (needs `id`) and Event
    // (needs `method`) — so any Browserless-specific frame fails serde and the
    // chromey library logs it at ERROR. The frames are gracefully dropped and
    // crawling succeeds; the error level is a library misclassification.
    // Suppress by default; override via RUST_LOG if raw CDP debug is needed.
    const SUPPRESS_CDP_NOISE: &str = "chromiumoxide::conn::raw_ws::parse_errors=off";

    let filter = EnvFilter::try_from_default_env()
        .map(|f| {
            f.add_directive(
                SUPPRESS_CDP_NOISE
                    .parse()
                    .expect("hard-coded directive is valid"),
            )
        })
        .unwrap_or_else(|_| EnvFilter::new(format!("info,{SUPPRESS_CDP_NOISE}")));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_writer(std::io::stdout)
        .init();
}

pub fn log_info(msg: &str) {
    info!("{}", msg);
}

pub fn log_warn(msg: &str) {
    warn!("{}", msg);
}

pub fn log_done(msg: &str) {
    info!(status = "done", "{}", msg);
}
