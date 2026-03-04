# YouTube Ingest
Last Modified: 2026-03-03

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

> CLI reference (flags, subcommands, examples): [`docs/commands/youtube.md`](../commands/youtube.md)

Ingests a single YouTube video transcript into Qdrant via `yt-dlp`. No API key required.

## What Gets Indexed

- Auto-generated English subtitles (VTT format)
- Transcript stripped of timestamps, position cues, HTML tags, and deduplicated overlapping lines
- Embedded with the canonical `https://www.youtube.com/watch?v=<ID>` URL as source metadata

## URL Handling

`extract_video_id()` accepts:
- Full watch URLs: `https://www.youtube.com/watch?v=<ID>`
- Short URLs: `https://youtu.be/<ID>`
- Embed/shorts/v path patterns: `/embed/<ID>`, `/shorts/<ID>`, `/v/<ID>`
- Bare 11-character video IDs

**Playlist/channel URLs are not supported.** The implementation extracts the `v=` parameter and rebuilds a clean `watch?v=<ID>` URL — the `list=` parameter is discarded. Pure playlist/channel URLs (no `v=` parameter) fail with "URL does not appear to be a YouTube video URL".

## Prerequisites: yt-dlp

`yt-dlp` must be installed and on `$PATH`.

### Docker (axon-workers container)

Installed automatically in the Dockerfile runtime stage using the standalone binary (no Python required), with arch detection for amd64 and arm64:

```dockerfile
RUN curl -fsSL -o /usr/local/bin/yt-dlp \
  "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux" \
  && chmod +x /usr/local/bin/yt-dlp
```

### Local development

```bash
# Linux standalone binary (no Python required)
curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux \
  -o ~/.local/bin/yt-dlp && chmod +x ~/.local/bin/yt-dlp

# macOS
brew install yt-dlp

# pip (cross-platform, requires Python)
pip install yt-dlp

# Verify
yt-dlp --version
```

## How It Works

The exact yt-dlp invocation:

```bash
yt-dlp --write-auto-sub --skip-download --sub-format vtt --convert-subs vtt \
  --sub-langs en --no-exec -o "<tmp>/<%(id)s>" -- https://www.youtube.com/watch?v=<ID>
```

Note: manual-caption fallback (`--write-subs`) is not currently implemented.

1. URL is SSRF-validated (private IP ranges blocked)
2. Video ID extracted and URL reconstructed as canonical `watch?v=<ID>`
3. `yt-dlp` downloads the `.vtt` subtitle file to a temp directory
4. `parse_vtt_to_text()` processes the VTT:
   - Strips the `WEBVTT` header
   - Strips timestamp lines (containing `-->`)
   - Strips numeric cue identifiers
   - Strips HTML tags (e.g. `<c>`, `<b>`)
   - Deduplicates consecutive identical lines (common in overlapping subtitle windows)
5. Cleaned transcript embedded via `embed_text_with_metadata()` → TEI → Qdrant
6. Temp directory cleaned up automatically on drop

## Known Limitations

| Limitation | Detail |
|-----------|--------|
| **Single video only** | Pure playlist/channel URLs fail; `list=` parameter is stripped from watch URLs |
| **English captions required** | Only `--sub-langs en` is requested. Fails if no English captions exist |
| **Age-restricted / private videos** | `yt-dlp` exits non-zero; error surfaces as job failure |
| **`yt-dlp` version drift** | YouTube format changes periodically require `yt-dlp` updates: `pip install -U yt-dlp` or re-pull the Docker image |

## Troubleshooting

**`yt-dlp not found or failed to start`**

Binary not on `$PATH`. See Prerequisites above.

**`yt-dlp produced no VTT subtitle files`**

No English captions on this video. Run `yt-dlp --list-subs <url>` to see available languages.

**`yt-dlp exited non-zero`**

Run the yt-dlp command manually to see the raw error:
```bash
yt-dlp --write-auto-sub --skip-download --sub-format vtt --sub-langs en "https://www.youtube.com/watch?v=<ID>"
```
