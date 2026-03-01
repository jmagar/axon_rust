use super::common::parse_urls;
use crate::crates::core::config::Config;
use crate::crates::core::http::{normalize_url, validate_url};
use crate::crates::core::logging::{log_done, log_info};
use crate::crates::core::ui::{primary, print_option, print_phase};
use crate::crates::crawl::engine::resolve_cdp_ws_url;
use futures_util::{SinkExt, StreamExt};
use std::error::Error;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_tungstenite::tungstenite::Message;

/// Monotonically increasing CDP message ID (process-wide).
static CDP_ID: AtomicU64 = AtomicU64::new(1);

fn next_cdp_id() -> u64 {
    CDP_ID.fetch_add(1, Ordering::Relaxed)
}

/// Sanitize a URL into a safe filename component.
///
/// Strips the scheme, replaces non-alphanumeric chars with hyphens,
/// collapses runs of hyphens, trims edges, and truncates to 120 chars.
pub(crate) fn url_to_screenshot_filename(url: &str, idx: usize) -> String {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);

    let sanitized: String = stripped
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();

    // Collapse consecutive hyphens and trim leading/trailing hyphens.
    let mut collapsed = String::with_capacity(sanitized.len());
    let mut prev_hyphen = true; // Start true to trim leading hyphens.
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_hyphen {
                collapsed.push('-');
            }
            prev_hyphen = true;
        } else {
            collapsed.push(c);
            prev_hyphen = false;
        }
    }
    let collapsed = collapsed.trim_end_matches('-');

    // Truncate to a reasonable filename length.
    let max_name = 120;
    let name = if collapsed.len() > max_name {
        &collapsed[..max_name]
    } else {
        collapsed
    };

    format!("{idx:04}-{name}.png")
}

/// Validate that Chrome is configured before attempting a screenshot.
fn require_chrome(cfg: &Config) -> Result<(), Box<dyn Error>> {
    if cfg.chrome_remote_url.is_none() {
        return Err(
            "screenshot requires Chrome — set AXON_CHROME_REMOTE_URL or pass --chrome-remote-url"
                .into(),
        );
    }
    Ok(())
}

/// Format screenshot result as JSON for `--json` mode.
fn format_screenshot_json(url: &str, path: &str, size_bytes: u64) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "url": url,
        "path": path,
        "size_bytes": size_bytes,
    }))
    .unwrap_or_default()
}

/// Resolve a Chrome remote URL to a browser-level WebSocket endpoint.
///
/// Tries the engine's `resolve_cdp_ws_url` first (with timeout), then falls
/// back to a direct `/json/version` query with Docker hostname rewriting.
pub(crate) async fn resolve_browser_ws_url(remote_url: &str) -> Result<String, Box<dyn Error>> {
    // Try the engine's resolve function first (5s timeout — reqwest can hang).
    if let Ok(Some(ws_url)) = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        resolve_cdp_ws_url(remote_url),
    )
    .await
    {
        return Ok(ws_url);
    }
    // Direct fallback: query /json/version ourselves with a timeout.
    let discovery = if remote_url.ends_with("/json/version") {
        remote_url.to_string()
    } else {
        format!("{}/json/version", remote_url.trim_end_matches('/'))
    };
    let client = crate::crates::core::http::http_client()?;
    let body: serde_json::Value =
        tokio::time::timeout(tokio::time::Duration::from_secs(5), async {
            client.get(&discovery).send().await?.json().await
        })
        .await
        .map_err(|_| format!("timeout querying {discovery}"))?
        .map_err(|e| format!("failed to query {discovery}: {e}"))?;

    let ws_url = body
        .get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("no webSocketDebuggerUrl in {discovery} response"))?;

    // Rewrite Docker hostnames to localhost.
    let rewritten = ws_url
        .replace("axon-chrome", "127.0.0.1")
        .replace("axon-postgres", "127.0.0.1")
        .replace("axon-redis", "127.0.0.1");

    Ok(rewritten)
}

pub async fn run_screenshot(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let urls = parse_urls(cfg);
    if urls.is_empty() {
        return Err("screenshot requires at least one URL (positional or --urls)".into());
    }
    for (idx, url) in urls.iter().enumerate() {
        screenshot_one(cfg, url, idx + 1).await?;
    }
    Ok(())
}

async fn screenshot_one(cfg: &Config, url: &str, idx: usize) -> Result<(), Box<dyn Error>> {
    require_chrome(cfg)?;

    let normalized = normalize_url(url);
    validate_url(&normalized)?;

    print_phase("◐", "Screenshot", &normalized);
    println!("  {}", primary("Options:"));
    print_option("fullPage", &cfg.screenshot_full_page.to_string());
    print_option(
        "viewport",
        &format!("{}x{}", cfg.viewport_width, cfg.viewport_height),
    );
    print_option(
        "chromeRemoteUrl",
        cfg.chrome_remote_url.as_deref().unwrap_or("none"),
    );
    println!();

    let remote_url = cfg
        .chrome_remote_url
        .as_deref()
        .expect("require_chrome already validated");
    let browser_ws = resolve_browser_ws_url(remote_url).await?;
    log_info(&format!("[Screenshot] CDP browser: {browser_ws}"));

    let bytes = cdp_screenshot(
        &browser_ws,
        &normalized,
        cfg.viewport_width,
        cfg.viewport_height,
        cfg.screenshot_full_page,
        cfg.chrome_network_idle_timeout_secs,
    )
    .await?;

    let path = if let Some(p) = &cfg.output_path {
        p.clone()
    } else {
        let dir = cfg.output_dir.join("screenshots");
        dir.join(url_to_screenshot_filename(&normalized, idx))
    };

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;

    let size = bytes.len() as u64;
    if cfg.json_output {
        println!(
            "{}",
            format_screenshot_json(&normalized, &path.to_string_lossy(), size)
        );
    } else {
        log_done(&format!("saved: {} ({} bytes)", path.display(), size));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Raw CDP screenshot implementation over WebSocket
// ---------------------------------------------------------------------------

/// Send a CDP command and wait for the matching response by `id`.
async fn cdp_send(
    ws: &mut (
        impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    ),
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn Error>> {
    let id = next_cdp_id();
    let msg = serde_json::json!({ "id": id, "method": method, "params": params });
    ws.0.send(Message::Text(msg.to_string().into())).await?;

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(15);

    loop {
        let frame = tokio::time::timeout_at(deadline, ws.1.next())
            .await
            .map_err(|_| format!("timeout waiting for CDP response to {method}"))?
            .ok_or_else(|| format!("WebSocket closed waiting for {method}"))??;

        if let Message::Text(text) = frame {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
                if let Some(err) = v.get("error") {
                    return Err(format!("CDP error on {method}: {err}").into());
                }
                return Ok(v["result"].clone());
            }
        }
    }
}

/// Take a screenshot via raw CDP protocol.
///
/// 1. Connect to the browser WS endpoint
/// 2. Create a new target (tab)
/// 3. Attach to it with a flattened session
/// 4. Set viewport via Emulation.setDeviceMetricsOverride
/// 5. Navigate + wait for Page.loadEventFired
/// 6. If full_page, measure layout and resize viewport
/// 7. Capture screenshot as PNG base64
/// 8. Close target
pub(crate) async fn cdp_screenshot(
    browser_ws: &str,
    url: &str,
    width: u32,
    height: u32,
    full_page: bool,
    timeout_secs: u64,
) -> Result<Vec<u8>, Box<dyn Error>> {
    use base64::Engine;
    use tokio_tungstenite::tungstenite::handshake::client::generate_key;
    use tokio_tungstenite::tungstenite::http;

    // 1. Connect via raw TCP + WS handshake (bypasses TLS connector for ws://).
    let parsed = reqwest::Url::parse(browser_ws)
        .map_err(|e| format!("invalid WebSocket URL {browser_ws}: {e}"))?;
    let host = parsed.host_str().ok_or("no host in WS URL")?;
    let port = parsed.port().unwrap_or(9222);
    let addr = format!("{host}:{port}");

    let tcp_stream = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    .map_err(|_| format!("timeout connecting TCP to {addr}"))?
    .map_err(|e| format!("failed TCP connect to {addr}: {e}"))?;

    let ws_key = generate_key();
    let request = http::Request::builder()
        .uri(browser_ws)
        .header("Host", &addr)
        .header("Upgrade", "websocket")
        .header("Connection", "Upgrade")
        .header("Sec-WebSocket-Key", &ws_key)
        .header("Sec-WebSocket-Version", "13")
        .body(())
        .map_err(|e| format!("failed to build WS request: {e}"))?;

    let (browser_stream, _response) = tokio::time::timeout(
        tokio::time::Duration::from_secs(5),
        tokio_tungstenite::client_async(request, tcp_stream),
    )
    .await
    .map_err(|_| format!("timeout during WebSocket handshake with {browser_ws}"))?
    .map_err(|e| format!("WebSocket handshake failed with {browser_ws}: {e}"))?;

    let (bw, br) = browser_stream.split();
    let mut browser = (bw, br);

    // 2. Create a new target (tab).
    let result = cdp_send(
        &mut browser,
        "Target.createTarget",
        serde_json::json!({ "url": "about:blank" }),
    )
    .await?;
    let target_id = result["targetId"]
        .as_str()
        .ok_or("CDP createTarget did not return targetId")?
        .to_string();

    // 3. Attach to the target to get a session.
    let result = cdp_send(
        &mut browser,
        "Target.attachToTarget",
        serde_json::json!({ "targetId": target_id, "flatten": true }),
    )
    .await?;
    let session_id = result["sessionId"]
        .as_str()
        .ok_or("CDP attachToTarget did not return sessionId")?
        .to_string();

    // Session-scoped send + receive (includes sessionId in every message).
    async fn session_cmd(
        ws: &mut (
            impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
            impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
        ),
        session_id: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, Box<dyn Error>> {
        let id = next_cdp_id();
        let msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params,
            "sessionId": session_id,
        });
        ws.0.send(Message::Text(msg.to_string().into())).await?;

        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(30);

        loop {
            let frame = tokio::time::timeout_at(deadline, ws.1.next())
                .await
                .map_err(|_| format!("timeout waiting for CDP response to {method}"))?
                .ok_or_else(|| format!("WebSocket closed waiting for {method}"))??;

            if let Message::Text(text) = frame {
                let v: serde_json::Value = serde_json::from_str(&text)?;
                if v.get("id").and_then(|i| i.as_u64()) == Some(id) {
                    if let Some(err) = v.get("error") {
                        return Err(format!("CDP error on {method}: {err}").into());
                    }
                    return Ok(v.get("result").cloned().unwrap_or(serde_json::Value::Null));
                }
            }
        }
    }

    // 4. Enable Page events so we can wait for load.
    session_cmd(
        &mut browser,
        &session_id,
        "Page.enable",
        serde_json::json!({}),
    )
    .await?;

    // 5. Set viewport.
    session_cmd(
        &mut browser,
        &session_id,
        "Emulation.setDeviceMetricsOverride",
        serde_json::json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1,
            "mobile": false,
        }),
    )
    .await?;

    // 6. Navigate and wait for load event.
    let nav_id = next_cdp_id();
    let nav_msg = serde_json::json!({
        "id": nav_id,
        "method": "Page.navigate",
        "params": { "url": url },
        "sessionId": session_id,
    });
    browser
        .0
        .send(Message::Text(nav_msg.to_string().into()))
        .await?;

    // Wait for both navigation response and Page.loadEventFired.
    let deadline =
        tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs + 15);
    let mut nav_responded = false;
    let mut load_fired = false;

    while !(nav_responded && load_fired) {
        let frame = tokio::time::timeout_at(deadline, browser.1.next())
            .await
            .map_err(|_| "timeout waiting for page load")?
            .ok_or("WebSocket closed")??;

        if let Message::Text(text) = frame {
            let v: serde_json::Value = serde_json::from_str(&text)?;
            if v.get("id").and_then(|i| i.as_u64()) == Some(nav_id) {
                if let Some(err) = v.get("error") {
                    return Err(format!("CDP navigation error: {err}").into());
                }
                nav_responded = true;
            }
            if v.get("method").and_then(|m| m.as_str()) == Some("Page.loadEventFired") {
                load_fired = true;
            }
        }
    }

    // 7. Brief settle time for rendering to complete.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // 8. If full_page, get the full document dimensions and resize.
    if full_page {
        let metrics = session_cmd(
            &mut browser,
            &session_id,
            "Page.getLayoutMetrics",
            serde_json::json!({}),
        )
        .await?;

        let content_width = metrics["contentSize"]["width"]
            .as_f64()
            .unwrap_or(width as f64) as u32;
        let content_height = metrics["contentSize"]["height"]
            .as_f64()
            .unwrap_or(height as f64) as u32;

        session_cmd(
            &mut browser,
            &session_id,
            "Emulation.setDeviceMetricsOverride",
            serde_json::json!({
                "width": content_width,
                "height": content_height,
                "deviceScaleFactor": 1,
                "mobile": false,
            }),
        )
        .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // 9. Capture screenshot.
    let screenshot_result = session_cmd(
        &mut browser,
        &session_id,
        "Page.captureScreenshot",
        serde_json::json!({
            "format": "png",
            "captureBeyondViewport": full_page,
        }),
    )
    .await?;

    let b64_data = screenshot_result["data"]
        .as_str()
        .ok_or("CDP captureScreenshot did not return data")?;
    let png_bytes = base64::engine::general_purpose::STANDARD.decode(b64_data)?;

    // 10. Close the target (tab).
    let _ = cdp_send(
        &mut browser,
        "Target.closeTarget",
        serde_json::json!({ "targetId": target_id }),
    )
    .await;

    Ok(png_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crates::core::config::Config;

    // --- url_to_screenshot_filename ---

    #[test]
    fn test_url_to_screenshot_filename_basic() {
        let name = url_to_screenshot_filename("https://example.com/docs/intro", 1);
        assert_eq!(name, "0001-example-com-docs-intro.png");
    }

    #[test]
    fn test_url_to_screenshot_filename_special_chars() {
        let name = url_to_screenshot_filename("https://foo.bar/a?b=c&d=e", 3);
        assert!(name.starts_with("0003-"));
        assert!(name.ends_with(".png"));
        // Should not contain raw special chars.
        assert!(!name.contains('?'));
        assert!(!name.contains('&'));
        assert!(!name.contains('='));
    }

    #[test]
    fn test_url_to_screenshot_filename_long_url() {
        let long = format!("https://example.com/{}", "a".repeat(200));
        let name = url_to_screenshot_filename(&long, 1);
        assert!(name.ends_with(".png"));
        // The stem (before .png) should be truncated.
        assert!(name.len() < 200, "filename should be truncated: {name}");
    }

    #[test]
    fn test_url_to_screenshot_filename_no_consecutive_hyphens() {
        let name = url_to_screenshot_filename("https://example.com/a///b..c", 1);
        assert!(!name.contains("--"), "should not have consecutive hyphens");
    }

    // --- require_chrome ---

    #[test]
    fn test_require_chrome_errors_when_missing() {
        let cfg = Config {
            chrome_remote_url: None,
            ..Config::default()
        };
        let result = require_chrome(&cfg);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("requires Chrome"),
            "error should mention Chrome requirement: {msg}"
        );
    }

    #[test]
    fn test_require_chrome_ok_when_set() {
        let cfg = Config {
            chrome_remote_url: Some("ws://localhost:9222".to_string()),
            ..Config::default()
        };
        assert!(require_chrome(&cfg).is_ok());
    }

    // --- format_screenshot_json ---

    #[test]
    fn test_json_output_format() {
        let json = format_screenshot_json("https://example.com", "/tmp/out.png", 12345);
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("output should be valid JSON");
        assert_eq!(parsed["url"], "https://example.com");
        assert_eq!(parsed["path"], "/tmp/out.png");
        assert_eq!(parsed["size_bytes"], 12345);
    }
}
