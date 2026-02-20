use crate::axon_cli::crates::core::config::Config;
use spider::url::Url;
use std::error::Error;

/// Parse a WebVTT transcript string into clean plain text.
///
/// Strips the WEBVTT header, timestamp lines, position cues, and deduplicates
/// consecutive identical lines that arise from overlapping subtitle windows.
pub fn parse_vtt_to_text(vtt: &str) -> String {
    let mut result: Vec<String> = Vec::new();
    let mut last: Option<String> = None;

    for line in vtt.lines() {
        // Strip the WEBVTT header line
        if line.trim() == "WEBVTT" {
            continue;
        }
        // Strip blank lines
        if line.trim().is_empty() {
            continue;
        }
        // Strip timestamp lines — any line containing "-->"
        if line.contains("-->") {
            continue;
        }

        // Strip HTML tags from content lines
        let mut clean = String::new();
        let mut inside_tag = false;
        for ch in line.chars() {
            match ch {
                '<' => inside_tag = true,
                '>' => inside_tag = false,
                _ if !inside_tag => clean.push(ch),
                _ => {}
            }
        }
        let clean = clean.trim().to_string();

        if clean.is_empty() {
            continue;
        }

        // Deduplicate consecutive identical lines
        if last.as_deref() == Some(&clean) {
            continue;
        }

        last = Some(clean.clone());
        result.push(clean);
    }

    result.join("\n")
}

/// Extract a YouTube video ID from a URL or return the string as-is if already an ID.
pub fn extract_video_id(input: &str) -> Option<String> {
    // Try parsing as a URL first
    if let Ok(url) = Url::parse(input) {
        let host = url.host_str().unwrap_or("");

        // https://www.youtube.com/watch?v=<ID>
        if host == "www.youtube.com" || host == "youtube.com" {
            for (key, value) in url.query_pairs() {
                if key == "v" {
                    return Some(value.into_owned());
                }
            }
            return None;
        }

        // https://youtu.be/<ID>
        if host == "youtu.be" {
            let path = url.path().trim_start_matches('/');
            if !path.is_empty() {
                return Some(path.to_string());
            }
            return None;
        }

        return None;
    }

    // Not a URL — check if it's a bare 11-character video ID
    let trimmed = input.trim();
    if trimmed.len() == 11
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(trimmed.to_string());
    }

    None
}

/// Ingest a YouTube video, playlist, or channel URL by:
/// 1. Running yt-dlp to download VTT subtitle files
/// 2. Parsing each VTT file into clean text
/// 3. Embedding text into Qdrant via embed_text_with_metadata
pub async fn ingest_youtube(_cfg: &Config, _url: &str) -> Result<usize, Box<dyn Error>> {
    todo!("implement YouTube ingestion")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vtt_strips_header_and_timestamps() {
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello world\n\n00:00:02.000 --> 00:00:04.000\nThis is a test\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "Hello world\nThis is a test");
    }

    #[test]
    fn parse_vtt_deduplicates_overlapping_lines() {
        // VTT often repeats the same line as the window shifts
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\nHello world\n\n00:00:01.000 --> 00:00:03.000\nHello world\n\n00:00:03.000 --> 00:00:05.000\nNext sentence\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "Hello world\nNext sentence");
    }

    #[test]
    fn parse_vtt_handles_empty_input() {
        assert_eq!(parse_vtt_to_text("WEBVTT\n\n"), "");
    }

    #[test]
    fn parse_vtt_handles_position_cues() {
        // VTT cues with position/alignment metadata
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000 align:start position:0%\nPositioned line\n\n00:00:02.000 --> 00:00:04.000\nNormal line\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "Positioned line\nNormal line");
    }

    #[test]
    fn parse_vtt_strips_html_tags() {
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\n<c>Tagged</c> text\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "Tagged text");
    }

    #[test]
    fn extract_video_id_from_watch_url() {
        let id = extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_from_short_url() {
        let id = extract_video_id("https://youtu.be/dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_passthrough_for_bare_id() {
        // 11-char alphanumeric = bare video ID
        let id = extract_video_id("dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_returns_none_for_garbage() {
        assert_eq!(extract_video_id("not-a-valid-thing"), None);
    }
}
