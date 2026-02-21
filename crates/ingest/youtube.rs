use crate::crates::core::config::Config;
use crate::crates::core::logging::log_warn;
use crate::crates::vector::ops::embed_text_with_metadata;
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
        // Strip numeric-only cue identifiers (VTT sequence numbers like "1", "2", etc.)
        if line.trim().chars().all(|c| c.is_ascii_digit()) && !line.trim().is_empty() {
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

        // https://www.youtube.com/watch?v=<ID> (also m.youtube.com)
        if host == "www.youtube.com" || host == "youtube.com" || host == "m.youtube.com" {
            for (key, value) in url.query_pairs() {
                if key == "v" {
                    return Some(value.into_owned());
                }
            }
            // Handle /embed/<ID>, /shorts/<ID>, /v/<ID> path patterns
            if let Some(id) = url.path_segments().and_then(|mut segs| {
                let first = segs.next()?;
                if matches!(first, "embed" | "shorts" | "v") {
                    segs.next().map(|s| s.to_string())
                } else {
                    None
                }
            }) {
                if !id.is_empty() {
                    return Some(id);
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
/// 1. Running yt-dlp to download VTT subtitle files into a temp directory
/// 2. Parsing each VTT file into clean text via parse_vtt_to_text
/// 3. Embedding each transcript into Qdrant via embed_text_with_metadata
///
/// Requires `yt-dlp` to be installed and on PATH.
pub async fn ingest_youtube(cfg: &Config, url: &str) -> Result<usize, Box<dyn Error>> {
    // Create a temp directory; cleaned up automatically when `tmp` is dropped
    let tmp = tempfile::tempdir()?;
    let tmp_path = tmp.path().to_string_lossy().to_string();

    // Run yt-dlp: download English auto-generated subtitles only, skip video download
    let output = tokio::process::Command::new("yt-dlp")
        .args([
            "--write-auto-sub",
            "--skip-download",
            "--sub-format",
            "vtt",
            "--convert-subs",
            "vtt",
            "--sub-langs",
            "en",
            "-o",
            &format!("{tmp_path}/%(id)s"),
            url,
        ])
        .output()
        .await
        .map_err(|e| format!("yt-dlp not found or failed to start: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp exited non-zero: {stderr}").into());
    }

    // Collect all .vtt files produced by yt-dlp
    let mut vtt_files: Vec<std::path::PathBuf> = Vec::new();
    let mut dir = tokio::fs::read_dir(&tmp_path).await?;
    while let Some(entry) = dir.next_entry().await? {
        let path: std::path::PathBuf = entry.path();
        if path.extension().is_some_and(|e| e == "vtt") {
            vtt_files.push(path);
        }
    }

    if vtt_files.is_empty() {
        return Err(
            "yt-dlp produced no VTT subtitle files — video may have no captions, \
             or yt-dlp needs updating"
                .into(),
        );
    }

    let mut count = 0usize;

    for vtt_path in &vtt_files {
        let vtt_text = tokio::fs::read_to_string(vtt_path).await?;
        let text = parse_vtt_to_text(&vtt_text);

        if text.trim().is_empty() {
            continue;
        }

        // yt-dlp output template is "%(id)s" so the stem before the first "." is the video ID
        let stem = vtt_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        let video_id = stem.split('.').next().unwrap_or(stem);

        let source_url = format!("https://www.youtube.com/watch?v={video_id}");
        let title = format!("YouTube: {video_id}");

        match embed_text_with_metadata(cfg, &text, &source_url, "youtube", Some(&title)).await {
            Ok(n) => count += n,
            Err(e) => log_warn(&format!(
                "command=ingest_youtube embed_failed video_id={video_id} err={e}"
            )),
        }
    }

    Ok(count)
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

    #[test]
    fn extract_video_id_from_embed_url() {
        let id = extract_video_id("https://www.youtube.com/embed/dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_from_shorts_url() {
        let id = extract_video_id("https://www.youtube.com/shorts/dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_from_v_path_url() {
        let id = extract_video_id("https://www.youtube.com/v/dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn extract_video_id_from_mobile_url() {
        let id = extract_video_id("https://m.youtube.com/watch?v=dQw4w9WgXcQ");
        assert_eq!(id, Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn parse_vtt_strips_numeric_cue_ids() {
        let vtt = "WEBVTT\n\n1\n00:00:00.000 --> 00:00:02.000\nHello world\n\n2\n00:00:02.000 --> 00:00:04.000\nSecond line\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "Hello world\nSecond line");
    }

    #[test]
    fn parse_vtt_keeps_lines_with_digits_and_text() {
        // A legitimate line containing digits mixed with text should NOT be stripped
        let vtt = "WEBVTT\n\n00:00:00.000 --> 00:00:02.000\n3 blind mice\n";
        let text = parse_vtt_to_text(vtt);
        assert_eq!(text, "3 blind mice");
    }
}
