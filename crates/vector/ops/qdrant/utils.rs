use super::types::{QdrantPayload, QdrantPoint, RETRIEVE_MAX_POINTS_CEILING};
use crate::crates::core::config::Config;
use spider::url::Url;
use std::env;

pub fn qdrant_base(cfg: &Config) -> &str {
    cfg.qdrant_url.trim_end_matches('/')
}

pub(crate) fn env_usize_clamped(key: &str, default: usize, min: usize, max: usize) -> usize {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v >= min)
        .unwrap_or(default)
        .clamp(min, max)
}

pub fn payload_text_typed(payload: &QdrantPayload) -> &str {
    if !payload.chunk_text.is_empty() {
        payload.chunk_text.as_str()
    } else {
        payload.text.as_str()
    }
}

pub fn payload_url_typed(payload: &QdrantPayload) -> &str {
    payload.url.as_str()
}

pub(crate) fn payload_url(payload: &serde_json::Value) -> String {
    payload
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

pub(crate) fn payload_domain(payload: &serde_json::Value) -> String {
    payload
        .get("domain")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string()
}

pub fn base_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    let mut out = format!("{}://{host}", parsed.scheme());
    if let Some(port) = parsed.port() {
        out.push(':');
        out.push_str(&port.to_string());
    }
    Some(out)
}

pub fn render_full_doc_from_points(mut points: Vec<QdrantPoint>) -> String {
    points.sort_by_key(|p| p.payload.chunk_index.unwrap_or(i64::MAX));
    let capacity = points
        .iter()
        .map(|point| payload_text_typed(&point.payload).len())
        .sum::<usize>()
        + points.len();
    let mut text = String::with_capacity(capacity);
    for point in points {
        let chunk = payload_text_typed(&point.payload);
        if chunk.is_empty() {
            continue;
        }
        text.push_str(chunk);
        text.push('\n');
    }
    text.trim().to_string()
}

pub fn query_snippet(payload: &QdrantPayload) -> String {
    let text = payload_text_typed(payload).replace('\n', " ");
    let end = text
        .char_indices()
        .nth(140)
        .map(|(idx, _)| idx)
        .unwrap_or(text.len());
    text[..end].to_string()
}

pub(crate) fn retrieve_max_points(max_points: Option<usize>) -> usize {
    max_points
        .unwrap_or(RETRIEVE_MAX_POINTS_CEILING)
        .min(RETRIEVE_MAX_POINTS_CEILING)
}

#[cfg(test)]
mod tests {
    use super::{RETRIEVE_MAX_POINTS_CEILING, retrieve_max_points};

    #[test]
    fn retrieve_max_points_defaults_to_ceiling() {
        assert_eq!(retrieve_max_points(None), RETRIEVE_MAX_POINTS_CEILING);
    }

    #[test]
    fn retrieve_max_points_caps_values_above_ceiling() {
        assert_eq!(
            retrieve_max_points(Some(RETRIEVE_MAX_POINTS_CEILING + 250)),
            RETRIEVE_MAX_POINTS_CEILING
        );
    }

    #[test]
    fn retrieve_max_points_preserves_lower_values() {
        assert_eq!(retrieve_max_points(Some(128)), 128);
    }
}
