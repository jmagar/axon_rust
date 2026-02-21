use crate::crates::core::config::{Config, RenderMode};
use spider::url::Url;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ChromeRuntimeMode {
    Chrome,
    WebDriverFallback,
}

#[derive(Debug, Clone)]
pub(super) struct ChromeBootstrapOutcome {
    pub mode: ChromeRuntimeMode,
    pub remote_ready: bool,
    pub warnings: Vec<String>,
}

pub(super) fn chrome_runtime_requested(cfg: &Config) -> bool {
    !cfg.cache_skip_browser
        && matches!(cfg.render_mode, RenderMode::Chrome | RenderMode::AutoSwitch)
}

fn to_devtools_probe_url(remote_url: &str) -> Option<String> {
    let parsed = Url::parse(remote_url).ok()?;
    let host = parsed.host_str()?;
    let port = parsed.port_or_known_default()?;
    let scheme = match parsed.scheme() {
        "ws" => "http",
        "wss" => "https",
        "http" | "https" => parsed.scheme(),
        _ => return None,
    };
    Some(format!("{scheme}://{host}:{port}/json/version"))
}

async fn remote_chrome_ready(probe_url: &str, timeout_ms: u64) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(timeout_ms.max(250)))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    match client.get(probe_url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

pub(super) async fn bootstrap_chrome_runtime(cfg: &Config) -> ChromeBootstrapOutcome {
    let mut outcome = ChromeBootstrapOutcome {
        mode: ChromeRuntimeMode::Chrome,
        remote_ready: false,
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

    let Some(probe_url) = to_devtools_probe_url(remote_url) else {
        outcome.warnings.push(format!(
            "unable to parse --chrome-remote-url `{remote_url}`; proceeding with local launcher"
        ));
        return outcome;
    };

    for attempt in 0..=cfg.chrome_bootstrap_retries {
        if remote_chrome_ready(&probe_url, cfg.chrome_bootstrap_timeout_ms).await {
            outcome.remote_ready = true;
            return outcome;
        }
        if attempt < cfg.chrome_bootstrap_retries {
            tokio::time::sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
        }
    }

    if cfg.webdriver_url.is_some() {
        outcome.mode = ChromeRuntimeMode::WebDriverFallback;
        outcome.warnings.push(
            "remote chrome probe failed; WebDriver fallback selected for engine handoff"
                .to_string(),
        );
    } else {
        outcome
            .warnings
            .push("remote chrome probe failed; falling back to local Chrome launcher".to_string());
    }

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
