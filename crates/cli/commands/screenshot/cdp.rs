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

/// Send a session-scoped CDP command (includes sessionId in every message).
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

/// Connect to a browser WebSocket endpoint via raw TCP.
async fn connect_browser_ws(
    browser_ws: &str,
) -> Result<
    (
        impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    ),
    Box<dyn Error>,
> {
    use tokio_tungstenite::tungstenite::handshake::client::generate_key;
    use tokio_tungstenite::tungstenite::http;

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
    Ok((bw, br))
}

/// Navigate to a URL and wait for both navigation response and Page.loadEventFired.
async fn navigate_and_wait(
    browser: &mut (
        impl SinkExt<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
        impl StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
    ),
    session_id: &str,
    url: &str,
    timeout_secs: u64,
) -> Result<(), Box<dyn Error>> {
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

    // Brief settle time for rendering to complete.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    Ok(())
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

    let mut browser = connect_browser_ws(browser_ws).await?;

    // Create a new target (tab).
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

    // Attach to the target to get a session.
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

    // Enable Page events so we can wait for load.
    session_cmd(
        &mut browser,
        &session_id,
        "Page.enable",
        serde_json::json!({}),
    )
    .await?;

    // Set viewport.
    session_cmd(
        &mut browser,
        &session_id,
        "Emulation.setDeviceMetricsOverride",
        serde_json::json!({
            "width": width, "height": height,
            "deviceScaleFactor": 1, "mobile": false,
        }),
    )
    .await?;

    // Navigate and wait for load event.
    navigate_and_wait(&mut browser, &session_id, url, timeout_secs).await?;

    // If full_page, get the full document dimensions and resize.
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
                "width": content_width, "height": content_height,
                "deviceScaleFactor": 1, "mobile": false,
            }),
        )
        .await?;

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    // Capture screenshot.
    let screenshot_result = session_cmd(
        &mut browser,
        &session_id,
        "Page.captureScreenshot",
        serde_json::json!({ "format": "png", "captureBeyondViewport": full_page }),
    )
    .await?;

    let b64_data = screenshot_result["data"]
        .as_str()
        .ok_or("CDP captureScreenshot did not return data")?;
    let png_bytes = base64::engine::general_purpose::STANDARD.decode(b64_data)?;

    // Close the target (tab).
    let _ = cdp_send(
        &mut browser,
        "Target.closeTarget",
        serde_json::json!({ "targetId": target_id }),
    )
    .await;

    Ok(png_bytes)
}
