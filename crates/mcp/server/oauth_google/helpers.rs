use axum::{
    Json,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use reqwest::Url;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::types::{
    GoogleOAuthState, OAUTH_SESSION_COOKIE, OAUTH_SESSION_TTL_SECS, RedirectPolicy,
    TokenErrorResponse,
};

pub(crate) fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
}

pub(crate) fn append_query_pairs(base: &str, params: &[(&str, String)]) -> Result<String, String> {
    let mut url = Url::parse(base).map_err(|e| format!("invalid redirect uri: {e}"))?;
    {
        let mut qp = url.query_pairs_mut();
        for (key, value) in params {
            qp.append_pair(key, value);
        }
    }
    Ok(url.to_string())
}

pub(crate) fn normalize_loopback_redirect_uri(uri: &str) -> Option<String> {
    let mut parsed = Url::parse(uri).ok()?;
    let host = parsed.host_str()?.to_ascii_lowercase();
    if host == "127.0.0.1" || host == "localhost" {
        let _ = parsed.set_scheme("http");
        let _ = parsed.set_host(Some("localhost"));
    }
    Some(parsed.to_string())
}

pub(crate) fn is_allowed_redirect_uri(uri: &str, policy: RedirectPolicy) -> bool {
    let parsed = match Url::parse(uri) {
        Ok(v) => v,
        Err(_) => return false,
    };

    match policy {
        RedirectPolicy::Any => true,
        RedirectPolicy::LoopbackOnly => {
            let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
            parsed.scheme() == "http" && (host == "localhost" || host == "127.0.0.1")
        }
    }
}

pub(crate) fn request_identity(req: &axum::extract::Request) -> String {
    request_identity_from_headers(req.headers())
}

pub(crate) fn request_identity_from_headers(headers: &axum::http::HeaderMap) -> String {
    if let Some(v) = headers.get("cf-connecting-ip")
        && let Ok(s) = v.to_str()
    {
        return s.to_string();
    }
    if let Some(v) = headers.get("x-forwarded-for")
        && let Ok(s) = v.to_str()
        && let Some(first) = s.split(',').next()
    {
        return first.trim().to_string();
    }
    if let Some(v) = headers.get("x-real-ip")
        && let Ok(s) = v.to_str()
    {
        return s.to_string();
    }
    format!("anon-{:x}", {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        for (name, value) in headers.iter() {
            if name == "user-agent" || name == "accept-language" || name == "accept-encoding" {
                name.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    })
}

pub(crate) fn bearer_token_from_headers(headers: &axum::http::HeaderMap) -> Option<String> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix("Bearer ").map(|s| s.to_string())
}

pub(crate) fn unauthorized_response(state: &GoogleOAuthState, body: serde_json::Value) -> Response {
    let mut response = (StatusCode::UNAUTHORIZED, Json(body)).into_response();
    if let Some(cfg) = state.inner.config.as_ref() {
        let value = format!("Bearer resource_metadata=\"{}\"", cfg.resource_metadata_url);
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, header_value);
        }
    }
    response
}

pub(crate) fn is_secure_cookie(state: &GoogleOAuthState) -> bool {
    state
        .inner
        .config
        .as_ref()
        .map(|cfg| cfg.broker_issuer.starts_with("https://"))
        .unwrap_or(false)
}

pub(crate) fn build_session_set_cookie(
    state: &GoogleOAuthState,
    session_id: &str,
) -> Option<HeaderValue> {
    let mut cookie = format!(
        "{name}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax",
        name = OAUTH_SESSION_COOKIE,
        value = session_id,
        max_age = OAUTH_SESSION_TTL_SECS
    );
    if is_secure_cookie(state) {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).ok()
}

pub(crate) fn build_session_clear_cookie(state: &GoogleOAuthState) -> Option<HeaderValue> {
    let mut cookie = format!(
        "{name}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
        name = OAUTH_SESSION_COOKIE
    );
    if is_secure_cookie(state) {
        cookie.push_str("; Secure");
    }
    HeaderValue::from_str(&cookie).ok()
}

pub(crate) fn extract_cookie_value(req: &axum::extract::Request, name: &str) -> Option<String> {
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|part| {
        let mut split = part.trim().splitn(2, '=');
        let key = split.next()?.trim();
        let value = split.next()?.trim();
        if key == name {
            Some(value.to_string())
        } else {
            None
        }
    })
}

pub(crate) fn required_scopes(state: &GoogleOAuthState) -> Vec<String> {
    if let Ok(raw) = std::env::var("GOOGLE_OAUTH_REQUIRED_SCOPES") {
        let parsed = raw
            .split_whitespace()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if !parsed.is_empty() {
            return parsed;
        }
    }
    state
        .inner
        .config
        .as_ref()
        .map(|cfg| cfg.scopes.clone())
        .unwrap_or_default()
}

pub(crate) fn token_error_response(error: &str, description: &str, status: StatusCode) -> Response {
    (
        status,
        Json(TokenErrorResponse {
            error: error.to_string(),
            error_description: description.to_string(),
        }),
    )
        .into_response()
}

pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn oauth_html_page(
    status: StatusCode,
    title: &str,
    subtitle: &str,
    detail: &str,
    primary_href: &str,
    primary_label: &str,
    secondary_href: &str,
    secondary_label: &str,
) -> Response {
    let title = escape_html(title);
    let subtitle = escape_html(subtitle);
    let detail = escape_html(detail);
    let primary_href = escape_html(primary_href);
    let primary_label = escape_html(primary_label);
    let secondary_href = escape_html(secondary_href);
    let secondary_label = escape_html(secondary_label);
    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>{title}</title>
  <style>
    :root {{
      --bg: #090e1a;
      --panel: rgba(16, 26, 46, 0.88);
      --text: #e8f0ff;
      --muted: #b8c7eb;
      --accent: #67c7ff;
      --accent-2: #2f7ee8;
      --border: rgba(141, 180, 255, 0.24);
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      min-height: 100vh;
      display: grid;
      place-items: center;
      padding: 24px;
      color: var(--text);
      background:
        radial-gradient(1100px 620px at 8% -12%, #23448a 0%, transparent 60%),
        radial-gradient(900px 560px at 100% 112%, #0f3f6e 0%, transparent 62%),
        radial-gradient(700px 360px at 65% 8%, rgba(71, 164, 255, 0.2) 0%, transparent 60%),
        var(--bg);
      font-family: "Avenir Next", "Segoe UI", -apple-system, system-ui, sans-serif;
    }}
    .card {{
      width: min(720px, 100%);
      position: relative;
      overflow: hidden;
      background: linear-gradient(180deg, rgba(20, 33, 60, 0.94) 0%, var(--panel) 100%);
      border: 1px solid var(--border);
      border-radius: 20px;
      box-shadow: 0 28px 90px rgba(0, 0, 0, 0.5);
      backdrop-filter: blur(10px);
      padding: 30px;
    }}
    .card::before {{
      content: "";
      position: absolute;
      inset: -120px auto auto -140px;
      width: 320px;
      height: 320px;
      border-radius: 999px;
      background: radial-gradient(circle, rgba(103, 199, 255, 0.36) 0%, transparent 72%);
      pointer-events: none;
    }}
    .eyebrow {{
      font-size: 12px;
      letter-spacing: 0.09em;
      text-transform: uppercase;
      color: var(--muted);
      margin: 0 0 10px;
    }}
    h1 {{
      margin: 0 0 10px;
      font-size: clamp(26px, 4vw, 34px);
      line-height: 1.15;
      max-width: 18ch;
    }}
    p {{
      margin: 0;
      color: var(--muted);
      line-height: 1.55;
    }}
    .detail {{
      margin-top: 14px;
      padding: 12px 14px;
      border: 1px solid var(--border);
      border-radius: 10px;
      background: rgba(8, 14, 27, 0.72);
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
      font-size: 13px;
      color: #d7e3ff;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
    }}
    .actions {{
      display: flex;
      gap: 12px;
      flex-wrap: wrap;
      margin-top: 20px;
    }}
    a.btn {{
      text-decoration: none;
      border-radius: 11px;
      padding: 10px 14px;
      border: 1px solid var(--border);
      color: var(--text);
      background: rgba(255, 255, 255, 0.04);
      transition: transform .15s ease, background .15s ease;
    }}
    a.btn:hover {{ transform: translateY(-1px); }}
    a.btn.primary {{
      background: linear-gradient(180deg, var(--accent), var(--accent-2));
      border-color: transparent;
      color: #fff;
      font-weight: 700;
    }}
  </style>
</head>
<body>
  <main class="card">
    <p class="eyebrow">Axon MCP OAuth</p>
    <h1>{subtitle}</h1>
    <p>{title}</p>
    <div class="detail">{detail}</div>
    <div class="actions">
      <a class="btn primary" href="{primary_href}">{primary_label}</a>
      <a class="btn" href="{secondary_href}">{secondary_label}</a>
    </div>
  </main>
</body>
</html>"#
    );
    let mut response = (status, html).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    response
}
