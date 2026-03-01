# Sessions Ingest
Last Modified: 2026-02-25

Version: 1.0.0
Last Updated: 01:26:53 | 02/25/2026 EST

> CLI reference (flags, subcommands, examples): [`docs/commands/sessions.md`](../commands/sessions.md)

Ingests exported AI conversation files (Claude, Codex, Gemini) into Qdrant. Uses a Postgres-backed state tracker to avoid re-processing unchanged files on subsequent runs.

## Supported Formats

| Provider | Scan path | File format |
|----------|-----------|-------------|
| **Claude** | `~/.claude/projects/` | `.jsonl` per conversation |
| **Codex** | `~/.codex/sessions/` | `.jsonl` per session |
| **Gemini** | `~/.gemini/history/`, `~/.gemini/tmp/` | `.json` per conversation |

Each parser (`claude.rs`, `codex.rs`, `gemini.rs`) extracts message pairs (human + assistant turns) into flat text chunks, stripping internal metadata to keep only the conversational content.

## What Gets Indexed

- All human and assistant message turns
- Session metadata embedded as Qdrant point payload: source path, provider, project name
- Each changed session file produces one or more Qdrant points

## Incremental Indexing (State Tracker)

Processed files are tracked in the `axon_session_ingest_state` Postgres table (auto-created on first run):

| Column | Description |
|--------|-------------|
| `file_path` | Absolute path to the session file (primary key) |
| `last_modified` | File mtime at index time |
| `file_size` | File size at index time |
| `indexed_at` | Timestamp of last successful index |

On each run, a file is skipped if its mtime **and** size match the tracked values. Only new or modified session files are re-embedded.

## How It Works

1. Discovers all session files under each provider's scan path
2. For each file, checks the state tracker — skips unchanged files
3. Dispatches to the matching parser based on path/provider
4. Parser extracts message turns and formats them as text chunks
5. Chunks embedded via `embed_text_with_metadata()` → TEI → Qdrant
6. State tracker updated with new mtime/size

Sessions runs **synchronously** (no AMQP queue, no job ID) — same as `axon ask`/`axon query`.

## Adding a New Session Format

1. Create `crates/ingest/sessions/<provider>.rs`
2. Implement `ingest_<provider>_sessions(cfg, state, multi)` following the pattern in `claude.rs`
3. Register it in `crates/ingest/sessions/mod.rs` dispatch (add a `cfg.sessions_<provider>` flag check)
4. Add the `--sessions-<provider>` flag to `config/cli.rs` and `Config` struct
5. Add a unit test with a minimal sample file in `#[cfg(test)]`

## Troubleshooting

**No files processed / `0 chunks indexed`**

Session export files don't exist at the scanned paths. Export conversations from the respective app:
- Claude: Settings → Export Data
- Codex: `codex export` or check `~/.codex/sessions/` after running sessions
- Gemini: Check `~/.gemini/history/` after using Gemini CLI

**File skipped on re-run**

State tracker has seen this file before with the same mtime/size. Touch the file to force re-index:
```bash
touch ~/.claude/projects/<project>/<session>.jsonl
axon sessions
```

**Parse errors on a `.jsonl` / `.json` file**

The export schema may have changed. Open the file and verify the structure matches what the parser expects, or check `crates/ingest/sessions/<provider>.rs` for the expected fields.
