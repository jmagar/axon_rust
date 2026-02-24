use crate::crates::core::config::parse::is_docker_service_host;
use crate::crates::core::config::{Config, RenderMode};
use crate::crates::core::http::cdp_discovery_url;
use spider::url::Url;
use std::time::Duration;

#[derive(Debug, Clone)]
pub(super) struct ChromeBootstrapOutcome {
    pub remote_ready: bool,
    /// Pre-resolved CDP WebSocket URL (`ws://host:port/devtools/browser/UUID`).
    ///
    /// Set by `bootstrap_chrome_runtime` when the probe succeeds.  Pass as
    /// `chrome_remote_url` to the crawl config so `configure_website` can
    /// connect directly without a second `/json/version` fetch.
    pub resolved_ws_url: Option<String>,
    pub warnings: Vec<String>,
}

pub(super) fn chrome_runtime_requested(cfg: &Config) -> bool {
    !cfg.cache_skip_browser
        && matches!(cfg.render_mode, RenderMode::Chrome | RenderMode::AutoSwitch)
}

/// Probe the CDP `/json/version` endpoint and return the resolved WebSocket URL.
///
/// Returns `None` if the endpoint is unreachable or returns unexpected JSON.
/// Outside Docker, rewrites any known Docker service hostname in the WebSocket
/// URL to `127.0.0.1` so the host CLI can connect directly.
async fn probe_cdp_connection(client: &reqwest::Client, probe_url: &str) -> Option<String> {
    let body: serde_json::Value = client.get(probe_url).send().await.ok()?.json().await.ok()?;

    let ws_url = body.get("webSocketDebuggerUrl")?.as_str()?;
    let mut parsed = Url::parse(ws_url).ok()?;

    // Outside Docker, rewrite known Docker service hostnames to 127.0.0.1.
    if !std::path::Path::new("/.dockerenv").exists() {
        if let Some(host) = parsed.host_str() {
            let host = host.to_string();
            if is_docker_service_host(&host) {
                let _ = parsed.set_host(Some("127.0.0.1"));
            }
        }
    }

    Some(parsed.to_string())
}

pub(super) async fn bootstrap_chrome_runtime(cfg: &Config) -> ChromeBootstrapOutcome {
    let mut outcome = ChromeBootstrapOutcome {
        remote_ready: false,
        resolved_ws_url: None,
        warnings: Vec::new(),
    };

    if !chrome_runtime_requested(cfg) {
        return outcome;
    }
    if !cfg.chrome_bootstrap {
        return outcome;
    }

    let Some(remote_url) = cfg.chrome_remote_url.as_deref() else {
        outcome.warnings.push(
            "no --chrome-remote-url provided; using Spider local Chrome launcher".to_string(),
        );
        return outcome;
    };

    let Some(probe_url) = cdp_discovery_url(remote_url) else {
        outcome.warnings.push(format!(
            "unable to parse --chrome-remote-url `{remote_url}`; proceeding with local launcher"
        ));
        return outcome;
    };

    // Build the client once and reuse across all retry attempts.
    // Custom millisecond-precision timeout from config — build_client() only supports whole seconds.
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(
            cfg.chrome_bootstrap_timeout_ms.max(250),
        ))
        .build()
    {
        Ok(c) => c,
        Err(_) => return outcome,
    };

    for attempt in 0..=cfg.chrome_bootstrap_retries {
        if let Some(ws_url) = probe_cdp_connection(&client, &probe_url).await {
            outcome.remote_ready = true;
            outcome.resolved_ws_url = Some(ws_url);
            return outcome;
        }
        if attempt < cfg.chrome_bootstrap_retries {
            tokio::time::sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
        }
    }

    outcome
        .warnings
        .push("remote chrome probe failed; falling back to local Chrome launcher".to_string());

    outcome
}

pub(super) fn resolve_initial_mode(cfg: &Config) -> RenderMode {
    if cfg.cache_skip_browser {
        return RenderMode::Http;
    }
    match cfg.render_mode {
        RenderMode::AutoSwitch => RenderMode::Http,
        m => m,
    }
}
