mod deterministic;
mod engine;

#[cfg(test)]
mod tests;

pub use deterministic::{
    DeterministicExtractionEngine, DeterministicParser, ExtractRun, ExtractionMetrics,
    PageExtraction,
};
pub use engine::{ExtractWebConfig, run_extract_with_engine};

use spider::url::Url;
use spider_transformations::transformation::content::{
    ReturnFormat, TransformConfig, TransformInput, transform_content_input,
};
use std::sync::LazyLock;

static TRANSFORM_CONFIG: LazyLock<TransformConfig> = LazyLock::new(|| TransformConfig {
    return_format: ReturnFormat::Markdown,
    // Readability (Mozilla-style article scoring) discards documentation pages
    // that lack <article> structure — doc sites with sidebar + nested divs score
    // too low and get stripped to just the title. main_content=true already
    // extracts <main>/<article>/role=main structurally without the scoring penalty.
    readability: false,
    // clean_html uses [class*='ad'] which matches Tailwind `shadow-*` classes
    // (sh**ad**ow contains "ad"). This wipes all shadow-styled elements from
    // Tailwind CSS sites (react.dev, shadcn.com, etc.), leaving only the title.
    // html2md ignores script/style content natively, so clean_html buys nothing.
    clean_html: false,
    main_content: true,
    filter_images: true,
    filter_svg: true,
});

pub fn url_to_domain(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string())
        .replace(['[', ']', ':'], "_")
}

pub fn build_transform_config() -> &'static TransformConfig {
    &TRANSFORM_CONFIG
}

pub fn to_markdown(html: &str) -> String {
    let input = TransformInput {
        url: None,
        content: html.as_bytes(),
        screenshot_bytes: None,
        encoding: None,
        selector_config: None,
        ignore_tags: None,
    };
    transform_content_input(input, &TRANSFORM_CONFIG)
        .trim()
        .to_string()
}

/// Redact credentials from a URL, replacing username and password with `***`.
/// Returns `"***redacted***"` if the URL cannot be parsed.
pub fn redact_url(url: &str) -> String {
    match Url::parse(url) {
        Ok(mut parsed) => {
            if !parsed.username().is_empty() || parsed.password().is_some() {
                let _ = parsed.set_username("***");
                let _ = parsed.set_password(Some("***"));
            }
            parsed.to_string()
        }
        Err(_) => "***redacted***".to_string(),
    }
}

pub fn url_to_filename(url: &str, idx: u32) -> String {
    let parsed = Url::parse(url).ok();
    let host = parsed
        .as_ref()
        .and_then(|u| u.host_str())
        .unwrap_or("unknown-host");
    let path = parsed.as_ref().map(|u| u.path()).unwrap_or("/unknown-path");

    let stem_raw = format!("{host}{path}");
    let stem: String = stem_raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .take(80)
        .collect();

    format!("{:04}-{stem}.md", idx)
}

pub fn find_between<'a>(haystack: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = haystack.find(start)? + start.len();
    let e = haystack[s..].find(end)? + s;
    Some(haystack[s..e].trim())
}

pub fn extract_meta_description(html: &str) -> Option<String> {
    // Limit search to <head> (≤8 KB) to avoid cloning the full document.
    let head_end = html
        .find("</head>")
        .or_else(|| html.find("</HEAD>"))
        .unwrap_or(html.len().min(8192));
    // Use .get() instead of direct index to avoid a panic when head_end falls
    // on a UTF-8 multi-byte boundary (possible when the 8192-byte default is used).
    let head = html.get(..head_end).unwrap_or(html);
    let lower = head.to_ascii_lowercase();
    let marker = "name=\"description\"";
    let idx = lower.find(marker)?;
    let content_idx = lower[idx..].find("content=\"")? + idx + "content=\"".len();
    let rest = head.get(content_idx..)?;
    let end = rest.find('"')?;
    Some(rest.get(..end)?.to_string())
}

pub fn extract_links(html: &str, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while let Some(rel) = html[pos..].find("href=\"") {
        let start = pos + rel + 6;
        let remain = &html[start..];
        let Some(end_rel) = remain.find('"') else {
            break;
        };
        let link = remain[..end_rel].trim();
        if (link.starts_with("http://") || link.starts_with("https://"))
            && !out.iter().any(|x| x == link)
        {
            out.push(link.to_string());
            if out.len() >= limit {
                break;
            }
        }
        pos = start + end_rel + 1;
    }
    out
}

pub fn extract_loc_values(xml: &str) -> Vec<String> {
    // Case-insensitive search without cloning the full document (which can be 1–5 MB).
    // The sitemap spec mandates lowercase, but real-world feeds sometimes use <LOC>.
    const OPEN: &[u8] = b"<loc>";
    const CLOSE: &[u8] = b"</loc>";
    let bytes = xml.as_bytes();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor + OPEN.len() <= bytes.len() {
        let Some(rel) = bytes[cursor..]
            .windows(OPEN.len())
            .position(|w| w.eq_ignore_ascii_case(OPEN))
        else {
            break;
        };
        let start_idx = cursor + rel + OPEN.len();
        let Some(end_rel) = bytes[start_idx..]
            .windows(CLOSE.len())
            .position(|w| w.eq_ignore_ascii_case(CLOSE))
        else {
            break;
        };
        let end_idx = start_idx + end_rel;
        let value = xml[start_idx..end_idx].trim();
        if !value.is_empty() {
            out.push(value.replace("&amp;", "&"));
        }
        cursor = end_idx + CLOSE.len();
    }
    out
}

/// Find the value between `open` and `close` tags (case-insensitive) within `xml`.
/// Returns `None` if either tag is absent or the content is empty after trimming.
fn extract_between_tags(xml: &str, open: &[u8], close: &[u8]) -> Option<String> {
    let bytes = xml.as_bytes();
    let start = bytes
        .windows(open.len())
        .position(|w| w.eq_ignore_ascii_case(open))?
        + open.len();
    let end = bytes[start..]
        .windows(close.len())
        .position(|w| w.eq_ignore_ascii_case(close))?
        + start;
    let val = xml[start..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.replace("&amp;", "&"))
    }
}

/// Extract `(loc, optional lastmod)` pairs from sitemap XML `<url>` blocks.
/// `lastmod` is `None` when the tag is absent — callers should treat absent dates as "recent"
/// (i.e. do not filter out URLs whose age is unknown).
pub fn extract_loc_with_lastmod(xml: &str) -> Vec<(String, Option<String>)> {
    const URL_OPEN: &[u8] = b"<url>";
    const URL_CLOSE: &[u8] = b"</url>";
    let bytes = xml.as_bytes();
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor + URL_OPEN.len() <= bytes.len() {
        let Some(rel) = bytes[cursor..]
            .windows(URL_OPEN.len())
            .position(|w| w.eq_ignore_ascii_case(URL_OPEN))
        else {
            break;
        };
        let block_start = cursor + rel + URL_OPEN.len();
        let block_end = bytes[block_start..]
            .windows(URL_CLOSE.len())
            .position(|w| w.eq_ignore_ascii_case(URL_CLOSE))
            .map(|r| block_start + r)
            .unwrap_or(bytes.len());
        let block = &xml[block_start..block_end];
        if let Some(loc) = extract_between_tags(block, b"<loc>", b"</loc>") {
            let lastmod = extract_between_tags(block, b"<lastmod>", b"</lastmod>");
            out.push((loc, lastmod));
        }
        cursor = block_end + URL_CLOSE.len();
    }
    out
}

pub fn normalize_prefix(prefix: &str) -> Option<String> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }
    let mut value = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    if value.len() > 1 && value.ends_with('/') {
        value.truncate(value.len() - 1);
    }
    Some(value)
}

pub fn is_excluded_url_path(url: &str, prefixes: &[String]) -> bool {
    let Ok(parsed) = Url::parse(url) else {
        return false;
    };
    let path = parsed.path();
    prefixes.iter().any(|raw| {
        // Inline normalize_prefix logic without allocating — prefixes are pre-validated
        // at config time so the hot path is the common case (already has leading slash).
        let p = raw.trim().trim_end_matches('/');
        if p.is_empty() || p == "/" {
            return false;
        }
        // Common case: prefix already has leading slash (no allocation needed).
        // Treat `/` and `-` as word boundaries to match engine.rs locale logic
        // (e.g. `/ja` blocks `/ja/docs` AND `/ja-jp/docs`).
        if p.starts_with('/') {
            return path == p
                || (path.starts_with(p)
                    && matches!(
                        path.as_bytes().get(p.len()),
                        Some(&b'/') | Some(&b'-') | None
                    ));
        }
        // Rare case: prefix lacks leading slash — compare with implicit "/".
        path == format!("/{p}")
            || path.starts_with('/')
                && path[1..].starts_with(p)
                && matches!(
                    path.as_bytes().get(p.len() + 1),
                    Some(&b'/') | Some(&b'-') | None
                )
    })
}

pub fn canonicalize_url(url: &str) -> Option<String> {
    let mut parsed = Url::parse(url).ok()?;
    // Strip fragment
    parsed.set_fragment(None);
    // Strip default ports to prevent duplicate entries
    // (http://x:80/p and http://x/p must deduplicate)
    match (parsed.scheme(), parsed.port()) {
        ("http", Some(80)) | ("https", Some(443)) => {
            let _ = parsed.set_port(None);
        }
        _ => {}
    }
    // Strip trailing slashes from all paths (not just root)
    let path = parsed.path().to_string();
    if path.len() > 1 && path.ends_with('/') {
        parsed.set_path(path.trim_end_matches('/'));
    }
    Some(parsed.to_string())
}

pub fn extract_robots_sitemaps(robots_txt: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in robots_txt.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if !key.trim().eq_ignore_ascii_case("sitemap") {
            continue;
        }
        let url = value.trim();
        if !url.is_empty() {
            out.push(url.to_string());
        }
    }
    out.sort();
    out.dedup();
    out
}
