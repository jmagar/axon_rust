use crate::crates::core::config::{Config, RenderMode};
use crate::crates::crawl::engine::resolve_cdp_ws_url;
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

    for attempt in 0..=cfg.chrome_bootstrap_retries {
        if let Some(ws_url) = resolve_cdp_ws_url(remote_url).await {
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
