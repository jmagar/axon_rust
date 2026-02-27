# Changelog
Last Modified: 2026-02-26

## [Unreleased] — feat/crawl-download-pack

This section documents commits on `feat/crawl-download-pack` relative to `main` (`4777f76`).

### Commit Summary (main..HEAD)

| Commit | Type | Message |
|---|---|---|
| `7be0ba0` | refactor(web+pulse+ask) | pulse module splits + ask gates + omnibox/toolbar polish |
| `ddc19a0` | feat(web+docker+pulse) | pulse thinking blocks + empty bubble fix + claude hot-reload s6 + sccache |
| `aea1c5c` | fix(web+jobs+ci) | land review fixes, test env alignment, and changelog/session plumbing |
| `d6b01b2` | fix(pulse) | ensure Qdrant collection exists before upsert |
| `75d4ee7` | fix(pulse) | default save collection to AXON_COLLECTION / cortex instead of `pulse` |
| `ab79a0c` | docs(changelog) | update ccbccfd TBD sha references and session doc |
| `ccbccfd` | fix(docker+web) | dereference claude symlink for node user + path-traversal hardening in download.rs |
| `6f8f7c7` | feat(docker) | install AI CLIs in web image, non-root node user, AXON_WORKSPACE + ~/.ssh mounts |
| `f5eb415` | fix(docker) | pin codex cli package in web image |
| `93f51e8` | chore(docker+docs) | align web CLI mounts and refresh changelog |
| `4756caa` | feat(pulse+docker) | conversation memory fallback + claude binary mount |
| `4e4a9d2` | docs(changelog) | fix TBD sha → a3b3b76 |
| `a3b3b76` | fix(docker+test) | expose axon-web on 0.0.0.0, fix test pg_url normalization, update TS snapshots |
| `cec02a8` | docs(changelog) | fix a3b3b76 sha → 167ccb3 |
| `167ccb3` | feat(docker) | axon-web service + chrome Dockerfile move + web-server s6 worker |
| `6a65ead` | docs(changelog) | update unreleased section with 10 commits since last entry |
| `d1f20a4` | feat(web+crawl) | pulse workspace overhaul + refresh schedules + crawl download pack |
| `115e264` | feat(refresh) | add refresh job pipeline and command manifests |
| `3d547dd` | fix(ci) | disable strict predelete for fresh Qdrant in mcp-smoke |
| `0e4b3f2` | fix(ci) | create .env for docker compose in mcp-smoke job |
| `7b9d9ba` | fix(ci) | resolve remaining test failures for schema, ask, and web |
| `234989b` | feat(ask) | citation-quality gates + diagnostics enrichment |
| `c1d65e8` | fix(ci) | resolve all three failing CI checks |
| `d3e0c7f` | feat | harden crawl/mcp flows and resolve PR review threads |
| `9d2c182` | feat(status) | improve CLI diagnostics and refresh web accent mapping |
| `7b4c898` | feat(mcp) | hard-cutover actions and add mcporter CI smoke tests |
| `9ad2e24` | feat(mcp) | align status action parity and refresh docs |
| `6bdfa36` | feat(mcp) | add path-first artifact contract, schema resource, and smoke coverage |
| `2724a2a` | fix | Fix CI failures for websocket v2 tests and cargo-deny config. |
| `54a543b` | chore/fix | Finalize PR feedback fixes and docs updates. |
| `9d5cdd4` | fix(web) | address remaining PR review threads comprehensively |
| `6a02ad3` | feat(web) | refresh pulse UI styling and architecture docs |
| `3863d7c` | fix | address PR API review threads batch 1 |
| `4de7d94` | feat(web) | add omnibox file mentions and root env fallback for pulse APIs |
| `4ac2b46` | fix(web) | resolve pulse UI lint warnings and align renderer changes |
| `241e7ff` | feat(web) | ship Pulse workspace foundation with RAG and copilot API |
| `d15dede` | feat(web) | doctor report renderer, options reorder, result panel polish |
| `1dd74f2` | feat(web) | crawl download routes — pack, zip, and per-file downloads |

### Highlights

#### Pulse Module Splits (TBD)
- Broke three over-limit files into 13 focused modules — no behavioral changes, zero re-exports:
  - `route.ts` (562→388 lines) split into `replay-cache.ts`, `claude-stream-types.ts`, `stream-parser.ts`
  - `pulse-workspace.tsx` (1093→342 lines) split into `hooks/use-pulse-chat.ts`, `use-pulse-persistence.ts`, `use-split-pane.ts`, `use-pulse-autosave.ts`, `lib/pulse/workspace-persistence.ts`, `lib/pulse/chat-api.ts`
  - `pulse-chat-pane.tsx` (952→450 lines) split into `components/pulse/tool-badge.tsx`, `doc-op-badge.tsx`, `message-content.tsx`, `chat-utils.ts`
- `ChatMessage` interface relocated from `pulse-workspace.tsx` to `lib/pulse/workspace-persistence.ts` (canonical location); all consumers updated in place.
- `computeMessageVirtualWindow` relocated to `chat-utils.ts`; test import updated directly (no shim).
- All 110 tests pass, TSC clean, Biome clean.

#### Ask / Strict Gates (TBD)
- Added `ask_strict_procedural` and `ask_strict_config_schema` config fields (both default `true`) — allow disabling Gate 5 (official-docs source check) and Gate 6 (exact-page-citation check) via env vars `AXON_ASK_STRICT_PROCEDURAL` / `AXON_ASK_STRICT_CONFIG_SCHEMA` without code changes.
- `crates/vector/ops/commands/ask.rs` extended with corresponding gate logic.

#### Pulse / Thinking Blocks + Empty Bubble Fix (ddc19a0)
- Wired Claude extended thinking (`type: 'thinking'` stream blocks) end-to-end through all four layers: `route.ts` captures them and emits `thinking_content` stream events; `chat-stream.ts` adds the event type; `types.ts` adds `PulseMessageBlock` thinking variant; `pulse-workspace.tsx` handles events and builds thinking blocks in real-time; `pulse-chat-pane.tsx` renders a collapsible `ThinkingBlock` component (violet-themed, shows char count, expands to monospace reasoning text).
- Fixed empty bubble bug: the assistant draft message was added to `chatHistory` eagerly (before any content arrived), creating a blank bubble above the "Claude thinking…" indicator. Now uses a `draftAdded` flag + `ensureDraftAdded()` helper — the bubble only appears when the first real content event (`thinking_content`, `assistant_delta`, or `tool_use`) fires.
- `groupBlocksForRender` updated to handle `thinking` blocks alongside `tool_use` and `text`; `MessageContent` now fires the structured-block render path for both `tool_use` and `thinking` blocks.

#### Docker / Hot Reload (ddc19a0)
- `axon-web` now runs three s6-overlay services: `pnpm-dev` (Next.js), `claude-session` (persistent Claude REPL with `--continue --fork-session`), and `claude-watcher` (inotifywait loop). When agents, skills, hooks, commands, or settings change on the host, `claude-watcher` restarts `claude-session` so the web app always loads the latest config without a container restart.
- `claude-session` uses `script -q -e /dev/null` to allocate a pseudo-TTY (required for interactive mode without a real terminal) and `--dangerously-skip-permissions` (container sandbox). Workspace trust dialog bypassed via `cont-init.d/10-trust-workspace` which patches `~/.claude.json` at boot.
- Watcher uses an explicit path whitelist (agents, commands, hooks, plugins, skills, settings, CLAUDE.md, .mcp.json) — runtime-written paths (`~/.claude/projects/`, `~/.claude/statsig/`, `~/.claude.json`) intentionally excluded to prevent restart loops.
- `docker/Dockerfile` builder stage now installs sccache prebuilt binary (arch-aware: `x86_64-unknown-linux-musl` / `aarch64-unknown-linux-musl`) so `.cargo/config.toml`'s `rustc-wrapper = "sccache"` resolves correctly during `cargo build`.
- `docs/CLAUDE-HOT-RELOAD.md` added: architecture diagram, watched paths table, setup instructions, verification commands, troubleshooting section, design decisions table.

#### CI / Test Env (aea1c5c)
- Review fixes: test env alignment across `common/tests.rs`, `crawl/runtime/tests.rs`, `embed/tests.rs`, `extract/tests.rs`; changelog and session doc plumbing.

#### Pulse / Runtime
- Fixed Pulse persistence path to ensure the target Qdrant collection exists before upserts, eliminating first-write failures when collection bootstrap lagged (`d6b01b2`).
- Fixed Pulse save default collection selection to use `AXON_COLLECTION` (fallback `cortex`) instead of hardcoded `pulse` (`75d4ee7`).
- Changelog hygiene pass replaced leftover TBD SHA references from prior branch notes and refreshed linked session metadata (`ab79a0c`).
- Fixed: `spawn claude EACCES` in Pulse chat — `docker/web/Dockerfile` now dereferences the symlink (`readlink -f`) when copying the claude binary so `node` user can execute it without traversing `/root/.local/` (700 perms) (`ccbccfd`).
- `AXON_SERVE_HOST=0.0.0.0` moved to `.env`/`.env.example` (removed from inline docker-compose env) per single-source-of-truth policy (`ccbccfd`).
- Security: `download.rs` hardened with `is_safe_relative_manifest_path()` + `canonicalize()`-based path traversal prevention (`ccbccfd`).
- `axon-web` now runs as non-root `node` user; Claude, Codex, Gemini CLIs installed from official sources inside the image (`6f8f7c7`).
- `AXON_WORKSPACE` env var mounts host workspace dir at `/workspace` inside the container (`6f8f7c7`).
- `~/.ssh` and `~/.claude.json` bind-mounted into `axon-web` for key-based git ops and Claude auth (`6f8f7c7`).
- `docker/web/Dockerfile` switched to `node:24-slim`; legacy static web UI files removed (`6f8f7c7`).
- Fixed: pinned `@openai/codex` to `0.105.0` to avoid broken `@latest` tarball (`f5eb415`).
- Aligned web runtime mounts to `/home/node/.claude*` and refreshed commit-driven changelog coverage for branch history (`93f51e8`).
- Added conversation-memory fallback for favorite-color recall in Pulse chat when upstream Claude CLI path fails, ensuring turn continuity for the common “what is my favorite color?” follow-up (`4756caa`).
- Updated Docker web image/runtime to include `claude` binary mount behavior used by the Pulse chat API subprocess path (`4756caa`).

#### Pulse Workspace (latest pass)
- Pulse workspace full overhaul: streaming tool-use blocks, model selector, source management (`d1f20a4`).
- Pulse chat pane: multi-block messages, citations, op-confirmations (`d1f20a4`).
- Pulse toolbar: model picker, permission controls, editor toggle (`d1f20a4`).
- New primitives: `pulse-markdown.tsx`, `claude-response.ts`, `prompt-intent.ts`, `/api/pulse/source` route (`d1f20a4`).
- WS protocol: `PulseSourceResponse`, `PulseToolUse`, `PulseMessageBlock` types (`d1f20a4`).
- Hooks: `use-axon-ws` additions, `use-ws-messages` streaming improvements (`d1f20a4`).

#### Refresh / Schedules
- Refresh job pipeline: `RefreshSchedule` table + schedule-claim lease (300s) (`115e264`, `d1f20a4`).
- Refresh command: full schedule CRUD — list/add/remove/enable/disable/run (`d1f20a4`).
- Command artifact manifests for axon, codex, and gemini workflows (`115e264`).
- `docs/commands/refresh.md` reference added (`d1f20a4`).

#### Ask / RAG
- Citation-quality gates: min score threshold, per-citation diagnostic fields (`234989b`).
- Diagnostics enrichment: ask command surfaces citation metadata in structured output (`234989b`).

#### MCP
- Hard-cutover to strict action parser; added mcporter CI smoke tests with resource checks (`7b4c898`).
- Hardened crawl/MCP safety and response behavior; restored compatibility paths (`d3e0c7f`).
- Added MCP artifact contract and schema-resource support (`6bdfa36`).
- Status action parity + related docs refresh (`9ad2e24`).

#### CLI / Status
- Status command: extended job table output, improved CLI diagnostics (`9d2c182`, `d1f20a4`).
- Scrape command: `--output-file` flag added (`d1f20a4`).
- Web accent palette updated (pink/blue → new interface palette) (`9d2c182`).

#### Docker / Infrastructure (latest)
- `axon-web` port binding changed from `127.0.0.1:49010` → `0.0.0.0:49010` so reverse proxies (SWAG/Tailscale) can reach the Next.js UI (`a3b3b76`).
- Fixed `docker-compose.yaml` `dockerfile:` path for `axon-web` — was relative to context (`apps/web`), now uses `../../docker/web/Dockerfile` (`a3b3b76`).

#### Tests / Rust
- Applied `normalize_local_service_url()` to all `pg_url()` test helpers across `common/tests.rs`, `crawl/runtime/tests.rs`, `embed/tests.rs`, `extract/tests.rs`, `refresh.rs` — Docker hostnames now rewrite to `127.0.0.1:PORT` when running `cargo test` from the host (`a3b3b76`).
- Updated `.env.example` comment for `AXON_TEST_PG_URL` to document auto-normalization fallback (`a3b3b76`).

#### Web / Pulse
- Regenerated stale snapshots for `pulse-chat-pane-layout.test.ts` after component rewrite; all 85 TS tests passing (`a3b3b76`).

#### Docker / Infrastructure
- Added `axon-web` service: Next.js dev UI with hot reload on port `49010`, bind-mounted source + anonymous volumes for `node_modules`/`.next` cache.
- Moved Chrome Dockerfile from `docker/Dockerfile.chrome` → `docker/chrome/Dockerfile`; updated compose reference.
- Added `web-server` s6-overlay service in `axon-workers`; healthcheck updated to include it.
- Exposed `axon-workers` port `49000` (`axon serve` HTTP + WebSocket) on localhost.
- Added `docker/web/Dockerfile` for the Next.js container build.
- `.env.example` updated with new service env vars (`AXON_BACKEND_URL`, `NEXT_PUBLIC_AXON_PORT`, etc.).

#### Web / Pulse Workspace (earlier pass)
- Added Pulse workspace foundation with RAG and copilot API (`241e7ff`).
- Added crawl download routes for pack/zip/per-file downloads (`1dd74f2`).
- Added omnibox file mentions and root env fallback for Pulse APIs (`4de7d94`).
- Applied UI/renderer polish and lint/review follow-up fixes (`d15dede`, `4ac2b46`, `6a02ad3`, `9d5cdd4`).

#### CI Stability
- Fixed strict predelete on fresh Qdrant in mcp-smoke (`3d547dd`).
- Fixed `.env` provisioning for docker compose in CI (`0e4b3f2`).
- Resolved schema, ask, and web test failures (`7b9d9ba`).
- Resolved security, crawl schema, and mcp-smoke CI checks (`c1d65e8`).
- Fixed CI failures for websocket v2 tests and cargo-deny config (`2724a2a`).

#### Stability and Review Follow-up
- Hardened crawl/MCP flows; tightened API error handling and docs alignment (`d3e0c7f`).
- Landed multiple PR feedback batches and docs updates (`3863d7c`, `54a543b`).

### Notes
- This changelog entry is commit-driven and branch-scoped to avoid stale migration guidance from unrelated historical branches.
- For file-level detail, inspect `git log --name-status main..HEAD`.
