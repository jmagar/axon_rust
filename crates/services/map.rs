use crate::crates::cli::commands::map::map_payload;
use crate::crates::core::config::Config;
use crate::crates::services::events::{ServiceEvent, emit};
use crate::crates::services::types::{MapOptions, MapResult};
use std::error::Error;
use tokio::sync::mpsc;

/// Map a raw JSON payload into a [`MapResult`].
///
/// This is a pure function — no network required. Tests call it with JSON literals.
pub fn map_map_payload(payload: serde_json::Value) -> Result<MapResult, Box<dyn Error>> {
    Ok(MapResult { payload })
}

/// Discover all URLs for a site starting at `url`.
///
/// Delegates to [`map_payload`] from the CLI commands layer and wraps the raw
/// JSON into the typed [`MapResult`]. Emits log events before and after the call
/// when a `tx` sender is provided.
pub async fn discover(
    cfg: &Config,
    url: &str,
    _opts: MapOptions,
    tx: Option<mpsc::Sender<ServiceEvent>>,
) -> Result<MapResult, Box<dyn Error>> {
    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: format!("starting map: {url}"),
        },
    );

    let payload = map_payload(cfg, url).await?;

    emit(
        &tx,
        ServiceEvent::Log {
            level: "info".to_string(),
            message: format!(
                "map complete: {} urls",
                payload
                    .get("mapped_urls")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ),
        },
    );

    map_map_payload(payload)
}
