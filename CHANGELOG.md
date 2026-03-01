# Changelog
Last Modified: 2026-03-01 (session: xterm.js terminal emulator, sidebar nav, logs TS fix)

## [Unreleased] — feat/crawl-download-pack

This section documents commits on `feat/crawl-download-pack` relative to `main` (`4777f76`).

### Commit Summary (main..HEAD)

| Commit | Type | Message |
|---|---|---|
| *(this commit)* | feat(web) | xterm.js terminal emulator at /terminal — WS integration, design system theming, sidebar nav |
| `a31a58ea` | fix(docker) | install uvx for neo4j-memory MCP, add pnpm-dev finish script |
| `2a23d860` | feat(web) | hoist PulseSidebar to AppShell — visible on all pages |
| `a5dc786c` | fix(docker) | resolve inotify watch limit, EADDRINUSE port race, and node_modules ownership |
| `4e45fb38` | fix(web) | use ExtractedSection in results-panel instead of inline file list |
| `6b0619ed` | fix(web) | restore selectedFile/selectFile in results-panel with inline file list |
| `22a96263` | fix(web) | remove unused selectedFile/selectFile from results-panel destructure |
| `9235a534` | fix(web) | remove CrawlFileExplorer from results-panel, delete stub |
| `f3ca9641` | feat(web) | Logs page - Docker compose log viewer with SSE streaming |
| `7847680d` | fix(web) | jobs-dashboard Biome lint compliance - hook deps and unused imports |
| `7f7a49fa` | feat(web) | Tasks page - task scheduler dashboard with CRUD and manual run |
| `d91167a2` | fix(security) | resolve symlink traversal and path canonicalize bypasses |
| `d36e18d7` | chore | update changelog sha 8386d55 |
| `8386d55` | feat(pulse) | remove hard borders, glow shadow separators, word wrap fix in editor |
| `b7dd29e` | fix(jobs) | spawn_heartbeat_task helper, Redis cancel timeouts, async I/O fixes, 7 new unit tests |
| `1ec5513` | feat(web) | workspace virtual dirs, Claude folder, landing editor, header normalization |
| `b2d8a74` | feat(web+docker) | PlateJS editor integration, pnpm-watcher s6 service, chrome health fix |
| `8d85538` | fix(jobs) | address all P0/P1/P2 code review issues — 8-agent team landing |
| `5dc43f1` | chore | update changelog for UI overhaul + workspace explorer; misc Rust job fixes |
| `e73906a` | feat(pages) | modal delete dialogs, MCP single save, settings typography, empty states, layout improvements |
| `7ca6184` | feat(pulse) | motion, empty state, message alignment, tool badge discoverability, mobile pane labels, divider improvements |
| `e3a0c96` | feat(omnibox) | status bar persistence, @mention discovery tip, staggered suggestions |
| `4bdee4b` | feat(ui) | button/input hover micro-interactions, branded focus rings, scrollbar contrast fix |
| `e56c72d` | feat(web) | add CodeViewer component with line numbers and copy button |
| `648010c` | feat(web) | add /workspace file explorer page with tree + viewer |
| `b585aef` | feat(design) | establish design token foundation — fonts, palette, motion, atmosphere, shadows, a11y |
| `dcb077a` | feat(web) | add CodeViewer component with line numbers and copy button |
| `074ad72` | feat(web) | add workspace (FolderOpen) nav icon to omnibox toolbar |
| `63e71ff` | feat(web) | add /api/workspace route for AXON_WORKSPACE file browsing |
| `8e1f4e1` | fix(web) | prefix unused liveToolUses prop + update changelog sha |
| `bc62851` | fix(web) | fix duplicate tool badges and raw-JSON response text in Pulse chat |
| `b20a7a3` | fix | address all 12 PR review comments from cubic-dev-ai |
| `d9823b2` | feat(web+jobs+mcp) | SSRF hardening, AMQP reconnect backoff, multi-lane workers, expanded tests |
| `ebca63c` | fix(web) | add Settings2 icon import to omnibox + changelog update |
| `d3f8047` | fix(ci) | resolve sccache and cargo audit failures |
| `03b1ef3` | fix(web) | remove dangling useRouter() call from omnibox |
| `9d98e86` | fix(web) | replace !important with :root specificity for slate placeholder CSS |
| `054e262` | feat(web) | settings redesign, MCP config/agents pages, PlateJS theming, MCP status indicators, nav icons in header, 72 tests |
| `f6e5e11` | feat(web) | settings page, session cards, workspace persistence, PWA scaffold |
| `884af14` | fix(web) | fix Pulse chat 'Claude CLI exited 1' due to root-owned .claude dirs |
| `d7ad5bb` | fix(ask) | remove brittle Gate 5/6 URL heuristics; trust LLM citation grounding |
| `c246b22` | fix(rust) | address 5 PR review comments (env_bool fallback, authoritative_ratio, touch_running_job dedup, cancel exit 130) |
| `375e737` | fix(web) | use Number.isNaN instead of global isNaN (Biome lint) |
| `04d12e0` | fix(web) | address 6 PR review comments (JSON guard, timeout ref, block immutability, NaN split, stale comment, empty vector guard) |
| `93dd150` | fix(infra+docs) | address 4 PR review comments (pnpm sentinel gate, SSH mount opt-in, SERVE.md cleanup, crawl.md subcommands) |
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

#### UI Design System Overhaul — 7-Agent Parallel Implementation (b585aef..e73906a)
33 design review issues addressed across 6 commits using a parallel agent team with zero file conflicts.

- **Design token foundation (`b585aef`):** Space_Mono (display) + Sora (body) fonts replace Outfit; 30+ CSS custom properties (`--axon-primary/secondary`, `--surface-*`, `--border-*`, `--shadow-sm/md/lg/xl`, `--focus-ring-color`, `--text-*`); 8 new `@keyframes` + 7 `@utility` Tailwind animation aliases; 3-radial + linear gradient body background with grain overlay via `body::before`; WCAG contrast fixes (`--axon-text-dim` 3.2:1 → 5.1:1, scrollbar pink 0.15 → blue 0.35).
- **UI primitives (`4bdee4b`):** Button hover scale (1.03/0.98) + primary glow; branded `--focus-ring-color` outline on all interactive elements (button, input, tabs, dropdown); scrollbar thumb WCAG fix; hardcoded rgba audit across `ui/` components.
- **Omnibox (`e3a0c96`):** Status bar persists 4 s post-completion with CheckCircle2/XCircle icons; dismissible `@mention` discovery tip backed by localStorage; staggered 35 ms suggestion reveals via `animate-fade-in-up`.
- **Neural canvas (`e3a0c96`):** New `zen` profile (brightness 0.3, density 0.4, 20 particles, high burstThreshold) for low-CPU focused-work mode; `useNeuralCanvasProfile` hook with localStorage persistence exported for parent consumers.
- **Pulse chat (`7ca6184`):** Asymmetric message alignment (user right 72%, assistant left 80%); ThinkingBlock word count + `animate-fade-in` reveal; radial-glow empty state with scale-in animation; 3-dot breathing loading indicator; labeled mobile pane switcher with `role="tablist"` ARIA; drag-handle divider with grip dots; unsaved title indicator dot.
- **Results panel (`e73906a`):** Virtual scrolling via `@tanstack/react-virtual` (threshold: 200 rows); top-N toggle for 1000+ row tables; failure-first service grouping in doctor report; asymmetric 2:1 metric grid; `animate-fade-in-up` stagger on table rows; focus rings on crawl-file-explorer and command-options-panel; copy button success state with `animate-check-bounce`.
- **Pages (`e73906a`):** Modal overlay delete confirmation (MCP + settings reset) replaces inline toggle; unified MCP save button (single sticky footer, dispatches to form/JSON tab handler); `font-display` section headers with icon container; improved empty states with contextual guidance; settings sidebar `border-r` accent, gradient `SectionDivider`, `border-l-2` left accent bars on sections, `max-w-[780px]`.

#### Workspace File Explorer (63e71ff..e56c72d)
- **`/api/workspace` route (`63e71ff`):** Serves AXON_WORKSPACE directory tree over HTTP; SSRF-guarded path traversal prevention.
- **Workspace nav icon (`074ad72`):** FolderOpen icon added to omnibox toolbar linking to `/workspace`.
- **`/workspace` page (`648010c`):** Full-page file explorer with tree sidebar + content viewer; directory navigation.
- **CodeViewer component (`dcb077a`, `e56c72d`):** Syntax-highlighted code viewer with line numbers and one-click copy.

#### Security Hardening + Worker Resilience (ebca63c..HEAD)
- **SSRF guards (web):** `validateAddDir()` in `buildClaudeArgs` checks `--add-dir` paths against `ALLOWED_DIR_ROOTS` (`/home/node`, `/tmp`, `/workspace`); `validateStatusUrl()` in `/api/mcp/status` blocks `localhost`, `127.x`, `10.x`, `192.168.x`, `172.16-31.x`, and IPv6 loopback/ULA ranges before probing MCP HTTP servers.
- **Input sanitisation (web):** `--allowedTools` / `--disallowedTools` values now filtered through `TOOL_ENTRY_RE` (`/^[a-zA-Z][a-zA-Z0-9_*(),:]*$/`) — malformed entries silently dropped. `PULSE_SKIP_PERMISSIONS` env var makes `--dangerously-skip-permissions` opt-out instead of hardcoded.
- **AMQP reconnect backoff (Rust):** `worker_lane.rs` adds exponential backoff (2 s → 60 s) on consecutive AMQP failures; resets on successful reconnect. Prevents thundering-herd against RabbitMQ on restart.
- **Dynamic multi-lane workers (Rust):** `loops.rs` replaces hardcoded `tokio::join!(lane1, lane2)` with `join_all(1..=WORKER_CONCURRENCY)` — lane count is now driven by config, not compile-time constants.
- **`claim_delivery()` helper (Rust):** extracts semaphore-acquire + DB claim + ack/nack into a single unit; prevents job leaks on ack failure.
- **MCP response cleanup (Rust):** `respond_with_mode` removed from crawl `status`/`list` and `domains` handlers — always inline; `#[allow(dead_code)]` + comment on `response_mode` struct fields clarify intent.
- **New test coverage:** sessions scanner/parser tests (`__tests__/sessions/`), expanded `build-claude-args.test.ts`, `mcp/route.test.ts`, `agents/parser.test.ts`.
- **New helpers:** `error-boundary.tsx`, `lib/agents/parser.ts`, `scripts/axon-mcp` launcher.

#### PR Review Batch (93dd150..c246b22)
- **Rust (5 fixes):** `env_bool()` now falls back to `default` for unknown/typo env values (not `false`); `authoritative_ratio` returns 0.0 when domain list is empty; `touch_running_extract_job` / `touch_running_ingest_job` removed — replaced with shared `common::job_ops::touch_running_job`; `handle_cancel` emits exit code 130 (SIGINT convention) instead of 0 so UI doesn't log canceled jobs as successful.
- **TypeScript (7 fixes):** `tool-badge.tsx` guards `JSON.stringify` undefined before `.slice`; `use-pulse-autosave` clears `setTimeout` ref on unmount; `use-pulse-chat` block update is now immutable (spread instead of mutation); `workspace-persistence` NaN-safe `parseSplit()` helper; pulse/chat route stale comment removed; pulse/save route guards empty embedding response before `ensureCollection`.
- **Infra / Docs (4 fixes):** `20-pnpm-install` sentinel touch gated on successful install (exits 1 on failure); `docker-compose.yaml` SSH mount commented out (opt-in); `docs/SERVE.md` legacy browser-UI instructions removed; `commands/axon/crawl.md` `errors`/`worker` subcommands added to argument-hint.

#### MCP Config, Agents, Status Indicators, Nav Icons (`054e262`, `9d98e86`)
- **MCP configuration page** (`/mcp`): full CRUD for `~/.claude/mcp.json` — form-based (stdio command+args / HTTP URL) and raw JSON editor tab, delete confirmation, glass-morphic design. Accessible directly from the omnibox Network icon.
- **MCP server status indicators**: `/api/mcp/status` probes each server on page load — HTTP via `AbortSignal.timeout(4s)` fetch, stdio via `which <command>`. Cards show animated status dot (green glow = online, red = offline, yellow pulse = checking).
- **Agents listing page** (`/agents`): parses `claude agents` CLI output into grouped card grid with source badges (Built-in/Project/Global). Shimmer skeleton loading and empty state with actionable message.
- **Omnibox nav buttons**: Network (→ `/mcp`), Bot (→ `/agents`), Settings2 (→ `/settings`) icons in every omnibox instance. Previously only Settings was one-click accessible.
- **Settings redesign**: NeuralCanvas background bleeds through glass-morphic panels; all 3-option card selectors replaced with `<select>` dropdowns; 3 new CLI flags wired end-to-end (`--add-dir`, `--betas`, `--tools`).
- **PlateJS Axon theme**: `.axon-editor` CSS scope, `axon` CVA variants, toolbar hover/active/tooltip colors aligned to design system.
- **72 new tests**: `build-claude-args.test.ts` (49), `agents/parser.test.ts` (11), `mcp/route.test.ts` (12).

#### Pulse Settings Page + Session Cards (f6e5e11)
- **Settings full page** (`/settings`): replaced popup panel with a proper Next.js route — sticky header with back button and "Reset to defaults", sidebar nav on lg+, 8 sections: Model, Permission Mode, Reasoning Effort, Limits, Custom Instructions, Tools & Permissions, Session Behavior, Keyboard Shortcuts.
- **5 new CLI flags** wired end-to-end through the entire settings → API stack: `--allowedTools`, `--disallowedTools`, `--disable-slash-commands`, `--no-session-persistence`, `--fallback-model`. Each passes from `usePulseSettings` → `usePulseChat` → `chat-api.ts` → `route.ts` → `buildClaudeArgs`.
- **Session cards**: `extractPreview()` in `session-scanner.ts` reads the first 4 KB of each JSONL file to extract the first real user message (≤80 chars) as a preview. "tmp" project label hidden; UUID filename capped at 20 chars as fallback. Limited to 4 cards.
- **Workspace persistence**: `workspaceMode` now lazy-initializes from `localStorage('axon.web.workspace-mode')` and syncs on every change. Workspace restores correctly after page reload.
- **New Session button**: "New" button (Plus icon) in `PulseToolbar` clears all chat/doc state and wipes the localStorage persistence key so blank state survives reload.
- **Handoff message chip**: session handoff messages (`I'm loading a previous Claude Code session…`) now render as a compact inline chip ("Loaded session: project · N turns") instead of the raw multi-line dump.
- **Omnibox**: settings gear always visible and navigates to `/settings` via `router.push`; controlled `input` cleared when leaving Pulse workspace.
- `settings-panel.tsx` deleted (no remaining consumers).

#### Pulse Module Splits (7be0ba0)
- Broke three over-limit files into 13 focused modules — no behavioral changes, zero re-exports:
  - `route.ts` (562→388 lines) split into `replay-cache.ts`, `claude-stream-types.ts`, `stream-parser.ts`
  - `pulse-workspace.tsx` (1093→342 lines) split into `hooks/use-pulse-chat.ts`, `use-pulse-persistence.ts`, `use-split-pane.ts`, `use-pulse-autosave.ts`, `lib/pulse/workspace-persistence.ts`, `lib/pulse/chat-api.ts`
  - `pulse-chat-pane.tsx` (952→450 lines) split into `components/pulse/tool-badge.tsx`, `doc-op-badge.tsx`, `message-content.tsx`, `chat-utils.ts`
- `ChatMessage` interface relocated from `pulse-workspace.tsx` to `lib/pulse/workspace-persistence.ts` (canonical location); all consumers updated in place.
- `computeMessageVirtualWindow` relocated to `chat-utils.ts`; test import updated directly (no shim).
- All 110 tests pass, TSC clean, Biome clean.

#### Ask / Strict Gates (d7ad5bb)
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
