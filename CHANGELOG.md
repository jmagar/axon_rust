# Changelog
Last Modified: 2026-03-06 (session: v0.7.4 — ACP comprehensive review fixes)

## [Unreleased] — feat/services-layer-refactor

This section documents commits on `feat/services-layer-refactor` relative to `main` (`51a2c9c8`).

### Highlights

- **ACP comprehensive review fixes (v0.7.4)** — 30 unique findings fixed across security, performance, and code quality: model argument injection guard (`validate_model_string`), env allowlist in `spawn_adapter` (env_clear + 12 vars), 5-minute adapter lifecycle timeout, `LogLevel` enum replacing raw strings (30+ call sites), `try_send` event loss logging, double mutex → single lock, `std::fs` → `tokio::fs`, dead code removal, duplicate function merge, `Serialize` derives on all ACP types with serde rename, hand-rolled JSON → `serde_json::to_value`, channel capacity 32→256, `toolsRestrict` regex tightened to match backend `TOOL_ENTRY_RE`, `--dangerously-skip-permissions` gated behind `AXON_ALLOW_SKIP_PERMISSIONS`, `response.body!` null guard, localStorage Zod validation, `handlePrompt` split 268→155 lines, dual config state unified, config probe caching (60s TTL), 5 localStorage effects consolidated to 2

- **Regression tests for ACP env isolation (v0.7.3)** — `tests/services_acp_spawn_env.rs` (3 tests) locks in `spawn_adapter()` env stripping: `CLAUDECODE`, `OPENAI_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_MODEL` must never leak to child process; uses process-level `Mutex` to serialize env mutations; `#![allow(unsafe_code)]` at file scope with `#[allow(clippy::await_holding_lock)]` per test; credentials staged into `axon-web` via `16-materialize-agent-credentials` cont-init.d

- **Pulse Chat local dev fixed (v0.7.2)** — two root causes identified and fixed: (1) `CLAUDECODE` env var inherited from parent Claude Code session blocked `claude-agent-acp` from spawning the `claude` CLI ("Claude Code cannot be launched inside another Claude Code session") — fixed by `command.env_remove("CLAUDECODE")` in `spawn_adapter()`; (2) `acp.rs` was double-wrapping `assistant_text` in a JSON object before passing it as `AcpTurnResultEvent.result`, causing `parseClaudeAssistantPayload` to extract raw JSON instead of the assistant's text — fixed by passing `assistant_text` directly; added `17-materialize-claude-credentials` cont-init.d for Docker credential staging; `docker-compose.yaml` mounts host Claude credentials read-only into workers container; `constants.rs` updated with Pulse Chat WS mode constant

- **Services layer refactor complete (v0.5.0)** — `crates/services/` is now the single source of business logic; CLI/MCP/WS are thin transport adapters; `crawl`/`extract`/`embed` modes use fire-and-forget direct service enqueue (no subprocess); `github`/`reddit`/`youtube` remain on subprocess fallback due to `!Send` constraint; `polling.rs` deleted; 971 tests passing
- **PR review threads fully resolved (v0.7.1)** — all 154 review threads on `feat/services-layer-refactor` addressed across 10 batches; fixes cover security hardening (env mutation serialization, port binding to localhost), stale React ref cleanup (`isBackgroundRef` on background error), `AbortController` dedup via `tabsRef`, trivial wrapper removal (`map_map_payload` inlined), and a range of typed errors, fail-fast mappers, probe uniqueness, MCP error sanitization, and flag validation
- **Pulse ACP agent selection + routing (v0.7.0)** — Pulse UI now supports selecting `claude`/`codex`; selection persists in workspace state/localStorage; `/api/pulse/chat` forwards `agent` to ws flags; `pulse_chat` sync mode resolves per-agent ACP adapter env overrides (`AXON_ACP_CLAUDE_ADAPTER_*`, `AXON_ACP_CODEX_ADAPTER_*`) with fallback to shared `AXON_ACP_ADAPTER_*`; replay cache key now includes `agent` to prevent cross-agent replay collisions
- **Scrape/embed stabilization** — fixed scrape page selection and constrained embed operations to the current run for deterministic indexing behavior
- **Release v0.6.0** — web workspace/sidebar updates landed with TEI retry behavior hardening and release/documentation refresh
- **Editor tab bar + tabs hook** — new `apps/web/components/editor-tab-bar.tsx`, `apps/web/hooks/use-tabs.ts`, `apps/web/lib/pending-tab.ts`, `apps/web/lib/result-to-markdown.ts` for multi-tab editor UX
- **CmdK palette improvements** — `CmdKOutput`, `CmdKPalette`, `cmdk-palette-dialog.tsx`, `cmdk-palette-types.ts` updated for better JSON/output display
- **MCP common.rs expansion** — `crates/mcp/server/common.rs` (+99 lines) with shared helpers; `handlers_system.rs` updated
- **Scripts + docker hardening** — `scripts/cache-guard.sh`, `scripts/check_docker_context_size.sh`, `scripts/check_dockerignore_guards.sh` added; `docker-compose.yaml`, `.dockerignore`, `scripts/rebuild-fresh.sh`, `lefthook.yml`, `Justfile` updated
- **Docs updated** — `docs/MCP-TOOL-SCHEMA.md`, `docs/OPERATIONS.md`, `docs/TESTING.md`, `README.md`, `.env.example` refreshed
- **Post-v0.4.0 stabilization** — fixed MCP OAuth smoke env handling and serialized crawl DB tests to reduce flakes; fixed 4 failing CI checks; pinned Vitest timezone (`TZ=UTC`) and refreshed snapshots for deterministic test output
- **Release prep + execution hardening (v0.4.1)** — updated web/container/docs env wiring and token guidance (`AXON_WEB_API_TOKEN`/`NEXT_PUBLIC_AXON_API_TOKEN`), refreshed Docker/compose defaults, and fully hardened the services-layer refactor execution plan with strict preflight, safety rails, and parallel-worker dispatch protocol
- **Full codebase security & quality review (v0.4.0)** — comprehensive 5-phase review covering 244 Rust + 424 TypeScript files; 40 Phase 1 findings (3 Critical, 7 High, 17 Medium, 13 Low) + 17 CodeRabbit findings all addressed; WS OAuth bearer token gating added; all `format!` SQL → parameterized queries (H-03); `Secret<T>` wrapper with `[REDACTED]` debug; `ConfigOverrides` + sub-config scaffolding (A-H-01); `Config::test_default()` (CR-Q); ANTHROPIC_API_KEY + CLAUDE_* passthrough in child env allowlist (H-02/CR-D); `spawn_blocking` replaces `block_in_place` in MCP ask handler (CR-E); token rotation race fixed (CR-F); OAuth state capacity caps (H-05/CR-K); `apply_overrides` returns new `Config` (CR-M); `ServiceUrls` Debug redacts secrets (CR-L); migration table for `axon_session_ingest_state` (CR-B); arch docs for A-H-01/A-M-01/A-M-04/A-M-08
- **Evaluate page + cortex suggest API** — new `/app/evaluate/page.tsx` for RAG evaluation UI; new `/api/cortex/suggest/route.ts` server route; `apps/web/lib/api-fetch.ts` typed fetch utility; v0.3.0 (minor bump)
- **Image SHA verification** — `docker/s6/cont-init.d/00-verify-image-sha` and `docker/web/cont-init.d/00-verify-image-sha` added to both worker and web containers; `scripts/check-container-revisions.sh` for CI; `scripts/rebuild-fresh.sh` and `scripts/test-mcp-oauth-protection.sh` added
- **CLI help contract test** — `tests/cli_help_contract.rs` verifies `axon --help` exit code and output structure; `scripts/check_mcp_http_only.sh` ensures HTTP transport is correctly gated
- **Sidebar simplification** — `SidebarSectionId` pruned to `'extracted' | 'workspace'`; `recents-section`, `starred-section`, `templates-section` removed; `workspace-section.tsx` and `file-tree.tsx` updated
- **Docs reorganization** — `commands/axon/`, `commands/codex/`, `commands/gemini/` skill command stubs deleted; 20+ `docs/commands/*.md` reference files added covering all CLI subcommands; new `docs/CONTEXT-INJECTION.md`, `docs/schema.md` added; `scripts/check_no_mod_rs.sh` and `scripts/check_no_next_middleware.sh` added for CI
- **Module consolidation** — `mod.rs` indirection pattern replaced with single-file modules across `crates/core/config/cli.rs`, `crates/core/config/types.rs`, `crates/core/http.rs`, `crates/jobs/common.rs`, `crates/jobs/ingest.rs`, `crates/jobs/refresh.rs`, `crates/jobs/worker_lane.rs`, `crates/web/execute.rs`, `crates/web/download.rs`, `crates/ingest/reddit.rs`; deleted corresponding `mod.rs` files
- **Map migration tests** — `crates/cli/commands/map_migration_tests.rs` added (TDD red phase): `map_payload_returns_unique_urls_without_cli_side_dedup`, `map_payload_reports_sitemap_url_count_consistently`, `map_autoswitch_only_falls_back_when_no_pages_seen`; wired via `#[cfg(test)] mod map_migration_tests` in `map.rs`
- **CLI/config refactor** — `crates/cli/commands/crawl.rs`, `map.rs`, `mcp.rs`, `research.rs`, `search.rs`, `youtube.rs` updated; `crates/core/config.rs`, `config/parse/build_config.rs`, `config/parse/helpers.rs`, `config/types/config.rs`, `config/types/config_impls.rs`, `config/types/enums.rs` updated; `crates/cli/commands/crawl/runtime.rs` updated
- **Web/Docker updates** — `apps/web/lib/axon-ws-exec.ts` updated; `apps/web/middleware.ts` deleted; `docker-compose.yaml`, `docker/Dockerfile`, `docker/web/Dockerfile` updated; image SHA verification scripts added to s6 cont-init
- **CI improvements** — `.github/workflows/ci.yml` updated; `lefthook.yml` refined; `Justfile` updated
- **MCP HTTP transport + Google OAuth** — `rmcp` upgraded 0.16→0.17 with `transport-streamable-http-server` feature; `run_http_server()` added alongside existing `run_stdio_server()`; new `crates/mcp/server/oauth_google/` module (8 files: config, handlers_broker, handlers_google, handlers_protected, helpers, state, tests, types) implements Google OAuth2 flow with PKCE, session management, and MCP-native auth middleware; s6 `mcp-http` service for Docker; `crates/mcp.rs` replaces `crates/mcp/mod.rs` with `#[path]` attributes
- **Screenshot CDP→Spider migration** — hand-rolled CDP WebSocket screenshot client deleted; replaced with Spider's `screenshot()` API; contract tests verify full-page capture behavior; scrape migration coverage added
- **Engine-level sitemap backfill** — `append_sitemap_backfill()` moved from CLI robots loop into `engine.rs`; fires automatically after every crawl; `discover_sitemap_urls_with_robots()` characterization tests; SSRF-safe `build_client` enforced; CLI robots backfill loop removed
- **API middleware + server-side extraction** — new Next.js `middleware.ts` (125L) with Bearer token auth (`AXON_WEB_API_TOKEN`), origin allowlist (`AXON_WEB_ALLOWED_ORIGINS`), and insecure dev bypass; `lib/server/url-validation.ts` (212L) extracts SSRF guards + URL sanitization from inline route code; `lib/server/api-error.ts` standardizes error responses; `lib/server/pg-pool.ts` centralizes Postgres pool creation; all API routes refactored to use shared server utilities
- **Omnibox hook extraction** — monolithic `omnibox-hooks.ts` (506→~200L) split into 3 focused hooks: `use-omnibox-execution.ts` (command dispatch), `use-omnibox-keyboard.ts` (key handlers), `use-omnibox-mentions.ts` (@ mentions); `omnibox-types.ts` relocated from component dir to `lib/`
- **Pulse workspace hook** — new `use-pulse-workspace.ts` (336L) consolidates workspace state management from `pulse-workspace.tsx`; `pulse-error-boundary.tsx` adds React error boundary; `use-timed-notice.ts` hook for auto-dismissing UI notices
- **Utility extractions** — `lib/debounce.ts`, `lib/storage.ts` (typed localStorage wrapper), `lib/command-options.ts` centralize shared logic previously duplicated across components
- **10 new test suites** (1250L) — `api-error.test.ts`, `axon-ws-logic.test.ts`, `jobs-route.test.ts`, `pg-pool.test.ts`, `pulse-op-confirmation.test.ts`, `replay-cache-eviction.test.ts`, `url-validation.test.ts`, `use-timed-notice.test.ts`, `workspace-persistence.test.ts`, `ws-messages-handlers.test.ts`
- **Existing test updates** — connection-buckets, terminal-history, omnibox-snapshot, replay-cache, ws-messages-runtime, ws-protocol tests updated for module extraction imports
- **Inline Chrome thin-page recovery** — new `cdp_render.rs` module renders thin pages inline via raw CDP WebSocket (`Page.setContent()` — no second HTTP request) while the HTTP crawl continues; `thin_refetch.rs` provides both inline (concurrent semaphore-gated) and batch fallback (spider-based post-crawl) re-fetch paths; `CollectorConfig` gains `chrome_ws_url`, `chrome_timeout_secs`, `output_dir`; `process_page()` extracted as pure function returning `PageOutcome` enum; collector spawns `JoinSet` of Chrome render tasks capped at `THIN_REFETCH_CONCURRENCY=4`
- **Custom HTTP headers (`--header`)** — new `--header "Key: Value"` repeatable CLI flag; `Config.custom_headers: Vec<String>` threaded through crawl/scrape/extract/Chrome re-fetch paths; headers applied to spider `Website` config and to standalone reqwest calls
- **Streaming sources dedup** — `check_sources_repetition()` in `streaming.rs` detects and truncates duplicate `## Sources` sections in LLM streaming responses; tracks first occurrence position and truncates at the second
- **Spider feature flags documentation** — new `docs/spider-feature-flags.md` inventorying all spider/spider_agent feature flags with observable behavior notes
- **Monolith enforcer improvements** — `enforce_monoliths_helpers.py` and `enforce_monoliths_impl.py` refined; `.monolith-allowlist` updated
- **CI enhancements** — `.github/workflows/ci.yml` updated with additional service container config
- **Web test improvements** — new/updated vitest tests for pulse mobile pane switcher; vitest config updates; 14 new web test files for various utilities
- **Integration/proptest test suite** — new integration tests for AMQP channel/queue (`amqp_integration.rs`), Redis pool (`redis_integration.rs`), heartbeat (`heartbeat.rs`), Postgres pool (`pool_integration.rs`), refresh job scheduling (`schedule_integration_tests.rs`), and WS protocol/allowlist/ANSI stripping (`ws_protocol_tests.rs`); proptest suites for `is_junk_discovered_url` (`url_utils_proptest.rs`), HTTP SSRF validators (`proptest_tests.rs`), and vector input chunking (`input_proptest.rs`); CI adds Redis 8.2, RabbitMQ 4.0, and Qdrant 1.13.1 service containers with health checks + `AXON_TEST_REDIS_URL` / `AXON_TEST_AMQP_URL` / `AXON_TEST_QDRANT_URL` env vars
- **MCP typed schema** — `crates/mcp/schema.rs` introduces fully-typed `AxonRequest` enum (tagged union, `snake_case`, `schemars::JsonSchema`) covering all 22+ actions (status/crawl/extract/embed/ingest/query/retrieve/search/map/doctor/domains/sources/stats/help/artifacts/scrape/research/ask/screenshot/refresh and more) with per-action request structs
- **Ask context heuristics module** — budget helpers and supplemental-injection logic extracted to `crates/vector/ops/commands/ask/context/heuristics.rs`; `push_context_entry` respects `max_chars` budget; `should_inject_supplemental` gates domain-boost on coverage gaps; `SUPPLEMENTAL_CONTEXT_BUDGET_PCT` / `SUPPLEMENTAL_MIN_TOP_CHUNKS_FOR_COVERAGE` / `SUPPLEMENTAL_RELEVANCE_BONUS` constants
- **Qdrant utils + tests expanded** — `crates/vector/ops/qdrant/utils.rs` (+229 lines) and `crates/vector/ops/qdrant/tests.rs` (+366 lines): test helpers, scroll utilities, source display improvements, additional coverage for search and facet paths
- **Sidebar simplified** — removed `recents-section.tsx`, `starred-section.tsx`, `templates-section.tsx`; `SidebarSectionId` reduced to `'extracted' | 'workspace'`; `StarredItem`, `RecentItem`, `TagDef`, `TaggedItem` types removed from `types.ts`
- **Web deprecation cleanup** — deleted creator dashboard + route (`/api/creator`, `/creator`), tasks dashboard + route (`/api/tasks`, `/tasks`), and all associated components (`task-form.tsx`, `tasks-dashboard.tsx`, `tasks-list.tsx`, `creator-dashboard.tsx`)
- **CmdK palette — no raw JSON** — `CmdKPalette` tracks `jsonCount` separately; `command.output.json` events increment the counter instead of `JSON.stringify`-ing into the log lines array; `CmdKOutput` shows a "N data objects received — see results panel" badge; `classifyLine` drops the `json` case; `formatToolArg` in `tool-badge.tsx` renders tool call inputs as human-readable labels (arrays as `[N items]`, objects as `{key, key, …}`) instead of raw `JSON.stringify`
- **Integration tests: vector + cancel** — `resolve_test_redis_url` + `resolve_test_qdrant_url` helpers added to `common/mod.rs` (skip-not-fail if env var unset); `poll_cancel_key` integration test in `process.rs`; `ensure_collection` idempotency test in `qdrant_store.rs`; new `crates/vector/ops/qdrant/tests.rs` (search + url_facets); new `crates/vector/ops/tei/tests.rs` (empty-input short-circuit + 429 retry via httpmock); `resolve_test_pg_url` no longer falls through to `AXON_PG_URL` production DB
- **`--include-subdomains` default changed to `false`** — was accidentally `true`; default is now documented and matches the CLAUDE.md gotcha note
- **MCP as `axon mcp` subcommand** — `mcp_main.rs` and `scripts/axon-mcp` deleted; `crates/cli/commands/mcp.rs` added; `CommandKind::Mcp` wired through config stack; MCP server is now a first-class CLI subcommand rather than a separate binary entry point
- **CLI `common.rs` expansion** — shared `JobStatus` trait + status display helpers extracted from crawl/extract/ingest subcommands, reducing duplication; URL glob expansion now logs a warning at `MAX_EXPANSION_DEPTH`
- **Smart dotenv loading** — `main.rs` discovers `.env` by walking ancestors from exe path and CWD; `AXON_ENV_FILE` env var for explicit override; graceful fallback chain with per-error warnings
- **Mobile omnibox fix** — three-bug root-cause chain: (1) sidebar auto-collapses on mobile viewports (<768px) when no stored preference, preventing it from consuming 260px of a 390px screen; (2) textarea auto-resize uses `height: '1px'` instead of `'auto'` before reading `scrollHeight` — `'auto'` in a flex layout returns the stretched layout height rather than intrinsic content height; (3) `ResizeObserver` added so height recalculates after sidebar collapse reflows the layout (the `[input]`-dep effect fired once on mount while sidebar was still 260px and never re-ran)
- **CmdK palette** — new `apps/web/components/cmdk-palette/` component with `CmdKPalette` and `CmdKOutput`; wired into `AppShell`
- **xterm.js terminal enhancements** — WebGL GPU renderer (`@xterm/addon-webgl`) with context-loss fallback; search decorations (amber highlights + active-match blue) via `allowProposedApi: true`; overview ruler lane (`overviewRulerWidth: 8`) shows match positions in scrollbar; copy-on-select via `onSelectionChange`; visual bell via `onBell` opacity flash; `attachCustomKeyEventHandler` for Ctrl+Shift+C (copy) / Ctrl+Shift+V (paste); all clipboard calls guarded with `?.` for HTTP contexts
- **Cortex layout refactor** — `app/cortex/layout.tsx` rewritten with proper sidebar integration; Cortex API routes standardised; doctor/status/stats/sources/domains dashboards updated for new layout
- **Plate.js editor enhancements** — slash commands (`/`), block drag-and-drop, callout blocks, collapsible toggles, table of contents, multi-block selection, block context menu, AI menu, inline comments, suggestion mode, export (HTML/PDF/image/markdown); 15 new plugin kit files wired into `copilot-kit.tsx`; mobile-responsive compact toolbar; `@ai-sdk/gateway@1.0.15` pinned for `ai@5` compatibility; `@platejs/ai` command route rewired with `generateText` for `ai@5` breaking changes (`Output.choice`, `partialOutputStream` removed); `useSearchParams` Suspense guard on `/cortex/sources`
- **Plate.js editor expansion** — 15 additional `@platejs/*` plugins (callout, caption, combobox, comment, date, emoji, indent, layout, math, mention, resizable, selection, suggestion, toc, basic-styles), supporting packages (`@ai-sdk/react`, `ai`, `@ariakit/react`, `date-fns`, `cmdk`, `lowlight`, etc.), `tailwind-scrollbar-hide` plugin, and new shadcn/ui components (dialog, popover, cursor-overlay)
- **Cortex dashboard review fixes** — AbortController on all polling dashboards (status/doctor/stats) cancels in-flight fetches on unmount and before each new poll; `disabled={loading || spinning}` on all 5 Refresh buttons; `Object.keys(data).length` badge fix in sources-dashboard; `useSearchParams` seeds filter from `?q=` param so domain drill-down links work; `local_ingest_jobs ?? []` guard in SummaryBar; `AXON_BIN` env var wires the pre-built binary path for Docker (routes were silently broken without it); missing `--sidebar-w` CSS update in `handleNavClick`; `aria-label` + `aria-current="page"` on Cortex sub-links; `target?: string` added to `JobEntry` interface
- **Cortex virtual folder in sidebar** — collapsible "Cortex" folder appended after PAGE_LINKS with Brain icon; 5 sub-links (Status, Doctor, Sources, Domains, Stats); open/closed state persists to `localStorage`; clicking Brain icon while collapsed auto-expands sidebar; active route highlighting on `/cortex/*` paths; 5 API routes (`/api/cortex/*`) spawn the axon binary with `--json`; 5 server component pages under `/app/cortex/`; 5 client dashboard components with loading skeletons, error banners, and refresh buttons; Status polls every 5s with collapsible job cards, Doctor polls every 15s with service health grid + pipeline chips, Sources uses `@tanstack/react-virtual` for virtualized URL table with search filter, Domains renders relative CSS bar chart with clickable domain→sources links, Stats polls every 30s with 6 large metric cards + payload fields + command count table
- **Jobs dashboard UX overhaul** — color-coded type badges (crawl=sky, embed=amber, extract=violet, ingest=rose), stats summary bar with live counts per status, sortable column headers (type/target/collection/status/started), relative timestamps ("5m ago") with absolute on hover, smart URL truncation (last 2 path segments), row hover actions (cancel/retry/view), animated ping ring + shimmer progress bar for active jobs; API extended with `StatusCounts` from parallel DB queries
- **Pulse 3-panel collapsible layout** — chat panel left, editor right, chevron strips to collapse/expand; `showChat`/`showEditor` booleans replace `DesktopViewMode`/`DesktopPaneOrder`; `use-split-pane` rewritten for 3-panel chevron layout
- **Pulse autosave optimization** — `updatePulseDoc` skips file read when client caches `createdAt`/`tags`/`collections` from last save response; pre-deletes stale Qdrant vectors before re-embed; save response now includes `createdAt`, `tags`, `collections`
- **Editor UX** — `loadedDocRef` tracks loaded doc param so re-navigation to a different `?doc=` reloads content; `SaveStatusBadge` wrapped in `memo`; `Suspense` fallback skeleton added
- **Z-index fix** — sidebar `z-[2]`, main content `z-[1]` — prevents NeuralCanvas/floating elements from bleeding over the sidebar
- **Job Detail Pages (`/jobs/[id]`)** — clickable job rows on `/jobs` now navigate to a dedicated detail page showing status, pages crawled/discovered, markdown created, timing, config, and raw result JSON; live-polls every 3s for running jobs
- **Knowledge Base (`/docs`)** — new page listing every scraped/crawled page from the axon output directory, grouped by domain, with markdown content viewer; backed by filesystem manifest.jsonl reads (no Qdrant calls)
- **PTY Shell** — real interactive shell at `/terminal` via `portable-pty` + dedicated `/ws/shell` WebSocket
- **Sidebar nav** — "Files" replaced with "Docs" → `/docs`; AXON logo made a home link; section-tab architecture with extracted/starred/recents/templates/workspace content panels

### Commit Summary (main..HEAD)

| Commit | Type | Message |
|---|---|---|
| `7368ddb7` | fix | stage claude/codex credentials into axon-web container |
| `107d2a6c` | fix | remove pulse_chat direct-dispatch flags from ALLOWED_FLAGS |
| `a017bb28` | chore | v0.7.1 — address all PR review threads (batches 1-10) |
| `2ae80ede` | fix | address PR review batch 10 — thread-safety, stale ref, and cleanup |
| `98f0d817` | fix | address remaining CodeRabbit review comments (batch 9) |
| `b464c3ab` | fix | address frontend PR review comments (batch 8) |
| `cb708b2a` | fix | decouple services layer from CLI commands (screenshot + map) |
| `68ff42c9` | fix | bind infra ports to localhost, fix nginx CORS, pin TEI retry env vars in tests |
| `e2f8bd90` | fix | address PR review batch 5 — typed errors, fail-fast mappers, probe uniqueness |
| `e933160c` | fix | address PR review feedback (batch 4 - frontend) |
| `5359faba` | fix | address PR review feedback (batch 3) |
| `2ad79b93` | fix | address PR review feedback (batch 2) |
| `6fde4d77` | fix | address PR review threads — dead code, render modes, service hardening |
| `54075260` | fix(review) | arrow fns, session id, proxy headers, pulse chat, chunk fix, dispatch split |
| `b787c7ba` | fix(review) | mode ref routing, log visibility, facet limit clamps |
| `e7b3e249` | fix(review) | address PR comments — MCP error sanitization, event field names, cancel safety, flag validation |
| `477f44a0` | fix(pr) | address review comments — security, correctness, and flag propagation |
| `de90c337` | feat(release) | v0.7.0; Pulse agent selector (claude/codex), ACP adapter routing, ws/api wiring, replay-key hardening |
| `baf24e5e` | fix(scrape) | select requested page and scope embed to current run |
| `4d5b0cb5` | feat(release) | v0.6.0 — web workspace/sidebar updates + TEI retry fixes |
| `f90d123a` | feat(release) | v0.5.0 — services-layer refactor complete + editor tabs + CmdK + scripts |
| `4e5144a3` | chore(web) | remove dead code from services layer refactor |
| `14b62d49` | feat(web) | fire-and-forget async dispatch and cancel via services |
| `476ad35b` | feat(web) | replace sync subprocess execution with direct service dispatch |
| `fe83d0a9` | fix(web) | replace dead Some(other) arm with unreachable! in render_mode match |
| `ed2bd90d` | refactor(web) | plumb base Config and ws override mapping for direct service dispatch |
| `dae2b0b1` | test(mcp) | pin map_retrieve_result data contract — chunk_count in wrapper element |
| `e93df53e` | fix(mcp) | correct retrieve chunk_count and research error class |
| `fb485043` | fix(mcp) | preserve sources wire contract — urls remains string[] in MCP response |
| `03996f72` | fix(mcp) | use option mapper helpers in system and query handlers |
| `38f0a53d` | refactor(mcp) | rewire handlers to use services layer |
| `d146571f` | refactor(mcp) | add request-to-service option mappers |
| `e4f81653` | fix(services) | address quality review issues from Wave 2 |
| `7f91caf2` | refactor(cli) | route system/stats/doctor/status handlers through services |
| `196ab300` | refactor(cli) | route query scrape search lifecycle and ingest handlers through services |
| `a802ff87` | feat(services) | implement query services (query/retrieve/ask/evaluate/suggest) |
| `c76fe394` | feat(services) | implement scrape/map/search/research services |
| `5a6f0393` | feat(services) | implement system services (sources/domains/stats/doctor/status/dedupe) |
| `475aa3da` | feat(services) | scaffold services module and events/types base |
| `cd42ee57` | docs(plan) | record baseline verification for services refactor |
| `58c66e29` | fix(docker) | expose service ports and restore external MCP reachability |
| *(prev)* | chore(release) | v0.4.1; stage pending web/docker/docs updates; harden services-layer refactor execution plan and dispatch safety |
| `b71fd7fd` | test | fix mcp-oauth-smoke missing env vars and serialize crawl DB tests |
| `25e2287f` | fix(ci) | fix 4 failing CI checks |
| `05238113` | fix(web) | set TZ=UTC in vitest config and update snapshot timestamps |
| `9eddd039` | chore(release) | v0.4.0 — full codebase review complete; 40+17 findings fixed; changelog updated |
| *(this commit)* | feat+chore | v0.4.0; full codebase review — 40 + 17 CR findings fixed; WS OAuth gating; SQL parameterization; Secret<T>; ConfigOverrides; env allowlist hardening |
| `18c6e6ae` | fix(test) | add #[serial] to extract DB tests to eliminate race condition |
| `54ced213` | fix(jobs) | fix doctest annotation in status.rs |
| `79cca7ba` | fix(config) | add Config::test_default() for stable test helpers (CR-Q) |
| `cf178f6e` | docs,feat | add arch docs (A-H-01, A-M-01, A-M-04, A-M-08) and scrape/evaluate module files |
| `da712968` | fix(jobs) | H-03 SQL parameterization in ingest/ops.rs |
| `b6671081` | fix(jobs,mcp,web) | H-03 SQL parameterization (extract/ingest/crawl), spawn_blocking, ANTHROPIC_API_KEY allowlist, sitemap tests |
| `ee330e95` | fix(jobs,mcp,web) | H-03 SQL params in process.rs, spawn_blocking safety, ? operator cleanup, CLAUDE_* env passthrough |
| `d95938ce` | fix(web,mcp) | add ANTHROPIC_API_KEY to env allowlist, fix block_in_place panic risk (CR-D, CR-E) |
| `e3134ef7` | feat(security) | gate /ws with OAuth bearer token; fix cancel mode injection, shell IPv4-mapped loopback, clock sentinel |
| `61169198` | fix(config) | wire modules, fix Secret timing, align defaults, expand ConfigOverrides, fix Debug (CR-A, CR-G, CR-H, CR-I, CR-L, CR-M) |
| `57c0250e` | fix(oauth) | fix token rotation race and add pending_state capacity cap (CR-F, CR-K) |
| `09d15d26` | fix(migrations,docs) | add missing tables/indexes to migration, fix scaling.md network (CR-B, CR-C, CR-N) |
| `72e7742d` | fix(deps) | bump aws-lc-sys 0.37.1 → 0.38.0 via aws-lc-rs 1.16.1 |
| `012cdcf4` | fix(ingest) | address 3 code review findings (C-02, M-04, L-06) |
| `e7238085` | fix | use raw sitemap url count in MapResult and remove shadow test |
| `4fff3661` | docs | record map command engine unification |
| `4eea6b93` | test | lock map payload schema after engine unification |
| `0186de11` | fix(compile) | add missing log crate dependency for web execute module |
| `b2f4c124` | fix(oauth) | address 8 code review findings (C-01, C-03, H-05, M-02, M-05, M-07, M-09, L-04) |
| `ddf4e830` | fix(cli) | restore stable JSON schemas for status/cancel/list/errors |
| `f9c26621` | fix(scrape) | redact headers in debug, fix failure propagation, dedup markdown, CDP timeout, schedule tier |
| `d2ade357` | fix(omnibox) | exec_id guard, suggestion staleness, useCallback deps, isProcessing sync, empty content |
| `66fd1ed6` | fix(ssrf) | block IPv6 enum bypass, 0.0.0.0, and redirect SSRF |
| `f35ce379` | fix(pulse) | auto-scroll MAX_LINES, Enter double-fire, clipboard fallback, empty text guard, unreachable boundary, allowlist expiry |
| `e63f6473` | fix(web) | api-fetch header merge, token scope, permissionLevel default, CSP, loopback, eviction order |
| `6f172dbd` | test | add map migration coverage |
| `3466ddf0` | test | serialize DB-touching integration tests with #[serial] to prevent race conditions |
| *(this commit v0.3.0)* | feat+chore | v0.3.0; evaluate page; cortex/suggest API; image SHA verification cont-init; CLI help contract test; command docs expansion (20+ files); module consolidation; sidebar simplification; script additions |
| 7fb1100d | feat(mcp)+chore | MCP HTTP transport + Google OAuth; rmcp 0.17; screenshot CDP→Spider migration; engine sitemap backfill; cleanup |
| `62bdae5e` | test | add scrape migration contract coverage |
| `2d004e27` | docs | record screenshot migration to spider api |
| `426cac65` | test | verify full-page screenshot behavior after migration |
| `0e45780c` | chore | delete hand-rolled screenshot cdp client |
| `e6ca9ddf` | feat(screenshot) | replace CDP client with Spider screenshot capture |
| `22310087` | test(screenshot) | add migration contract tests for CDP→Spider transition |
| `370ee1af` | docs | record engine-only backfill architecture |
| `147b9ca5` | chore | remove cli robots backfill loop |
| `c38dfb5f` | refactor | remove double validate_url + add TODO for http_client singleton |
| `209b86a1` | feat(crawl) | add engine-level append_sitemap_backfill and wire into sync_crawl |
| `2862eb9d` | test(crawl) | add failing contract tests for engine-delegated sitemap backfill |
| `c9ebd58b` | fix | use SSRF-safe build_client + add max_sitemaps TODO in engine sitemap |
| `817160bd` | test(sitemap) | characterization tests for discover_sitemap_urls_with_robots |
| `04559aed` | refactor(web)+test | API middleware + server-side extraction; omnibox/pulse module splits; 10 new test suites; utility extractions |
| `84cd8d2b` | feat(crawl)+refactor | inline Chrome thin-page recovery; CDP render module; custom headers; streaming sources dedup; spider feature flags docs |
| `129eb1fa` | test(rust)+refactor(web) | integration/proptest test suite; MCP typed schema; ask context heuristics; sidebar cleanup; CI service containers |
| `9428156c` | fix(ci) | remove invalid cargo-audit --deny flag; add Qdrant keyword indexes on collection init |
| `fa8ddc29` | revert | remove redundant .cargo/config.toml — sccache already in ~/.cargo/config.toml |
| `149325f0` | fix | restore sccache config; patch minimatch ReDoS (CVE high x2) |
| `edaafabf` | fix(web)+test(rust) | suppress raw JSON in CmdK palette; add vector/cancel integration tests; fix include_subdomains default |
| `959537ac` | refactor(mcp) | deduplicate DB queries in handle_status; fix artifacts action field |
| `76356b0e` | refactor(mcp+cli) | CLI command handlers, MCP wiring, and web fixes |
| `186a6936` | refactor(mcp+cli) | MCP as axon mcp subcommand; CLI common.rs JobStatus trait; smart dotenv loading; misc fixes |
| `d022c6f5` | fix(web) | mobile omnibox sizing — sidebar auto-collapse <768px, textarea ResizeObserver + height:1px fix; CmdK palette; web improvements |
| `27fc39f6` | feat(web) | xterm.js terminal enhancements — WebGL renderer, search decorations, overview ruler, copy-on-select, visual bell, Ctrl+Shift+C/V; Cortex layout refactor |
| `72d1f651` | fix(web) | wire AIKit into CopilotKit + address open items |
| `b2e2d61d` | fix(web) | address code review findings from Plate.js editor enhancements |
| `405e0945` | feat(web) | Plate.js editor enhancements — slash, DnD, callouts, toggles, TOC, block selection, AI menu, comments, export; ai@5 compat fixes |
| `f27cc810` | chore(deps) | Plate.js editor plugin expansion + dialog/popover/cursor-overlay UI components |
| `756a081e` | chore | wire AXON_BIN env var for Cortex routes in Docker — routes now fall back to pre-built release binary via /workspace mount |
| `f5d14901` | fix(web) | address Cortex dashboard review findings — AbortController, disabled state, binary path, accessibility |
| `51a2c9c8` | merge | feat/crawl-download-pack → main |
| `928ce7ba` | feat(web) | Cortex virtual folder in sidebar — status/doctor/sources/domains/stats diagnostic pages with API routes and dashboard components |
| `e2e5ee6b` | chore + fix | mcporter plate MCP entry; crawl worker output_dir uses worker root not job-serialized path |
| `5dee20a7` | fix(web) | pulse dual-hydration race + both-collapsed restore guard |
| `4e4633d9` | fix(web) | pulse workspace quality fixes — collapse guard, editor flex, aria |
| `a941173c` | feat(web) | jobs dashboard — color badges, stats bar, sort, relative time, smart truncation, hover actions, active progress |
| `61a1696e` | fix(web) | remove unused verticalDragStartRef from pulse-workspace destructure |
| `3359e863` | feat(web) | 3-panel collapsible layout — chat left, editor right, chevron strips |
| `cf1323ce` | fix(web) | remove unused showChatRef from use-split-pane |
| `50dd9473` | feat(web) | update use-pulse-persistence for showChat/showEditor |
| `f5c13206` | feat(web) | remove view-mode toggle buttons from PulseToolbar |
| `60cd01ed` | feat(web) | rewrite use-split-pane for 3-panel chevron layout |
| `1925a5bb` | feat(web) | replace DesktopViewMode/DesktopPaneOrder with showChat/showEditor booleans |
| `8ad11100` | fix(web) | pulse autosave update-in-place + editor hardening |
| *(2d32f42e)* | fix(web) | pulse autosave: skip file read, pre-delete stale vectors, editor doc-reload fix, z-index |
| `394917d5` | feat(web) | /jobs/[id] detail page — status, stats, timing, config, live polling |
| `ac294073` | feat(web) | /docs knowledge base page — filesystem-backed manifest reader |
| `9fdf8913` | feat(web) | terminal page — real PTY shell via useShellSession |
| `d7cff203` | feat(web) | useShellSession hook — dedicated /ws/shell WebSocket |
| `d357f088` | feat(web) | add /ws/shell route for PTY shell sessions |
| `e9011060` | feat(web) | PTY shell WebSocket handler in crates/web/shell.rs |
| `e55c4e00` | chore(deps) | add portable-pty for PTY shell support |
| `ac16331b` | feat(web) | xterm.js terminal emulator at /terminal — WS integration, design system theming, sidebar nav |
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
