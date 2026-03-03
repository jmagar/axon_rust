# Code Quality Review: apps/web (feat/sidebar branch)

**Date:** 2026-03-03 (updated)
**Reviewer:** Code Quality Audit
**Scope:** `apps/web/` -- Next.js 16 App Router frontend, ~313 source files
**Branch:** `feat/sidebar` (vs `main`)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Critical Findings](#critical-findings)
3. [High Findings](#high-findings)
4. [Medium Findings](#medium-findings)
5. [Low Findings](#low-findings)
6. [Architecture Observations](#architecture-observations)

---

## Executive Summary

The `apps/web` codebase demonstrates solid engineering fundamentals: consistent error envelopes (`apiError`), proper Zod validation at API boundaries, well-structured WebSocket context splitting to minimize re-renders, SSRF protection, and disciplined ref-based closure management in hooks. The security posture is above average for a self-hosted app -- CSP headers, auth middleware, Docker socket scoping, and shell environment allowlisting are all present.

The issues below focus on areas where the code either introduces real risk (security, data loss, resource leaks) or accumulates unnecessary complexity that will slow future development. Findings are ordered by severity.

**Counts:** 4 Critical, 7 High, 9 Medium, 6 Low

---

## Critical Findings

### C-1. SQL Injection via String Interpolation in Jobs Route

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/route.ts` lines 52-68, 260-278
**Severity:** Critical

The `statusWhere()` function returns raw SQL fragments that are interpolated directly into query strings via template literals:

```typescript
// line 52
function statusWhere(filter: StatusFilter): string {
  switch (filter) {
    case 'active':
      return `status IN ('pending','running')`
    // ...
  }
}

// line 78 -- interpolated directly
`SELECT ... FROM axon_crawl_jobs WHERE ${where} ORDER BY ...`
```

While the `StatusFilter` type and the `VALID_STATUSES` Set guard mean the current code path is safe (only allowlisted literals reach `statusWhere`), this pattern is one refactor away from a SQL injection. If someone adds a new filter source or changes the validation, the interpolation silently accepts it.

The same pattern is repeated in `getStatusCounts()` line 197:

```typescript
`SELECT ... FROM ${table}` // table name interpolation
```

**Fix:** Use parameterized queries. For the status filter, pass a parameter array and use `ANY($1)`. For the table-name case, use an explicit allowlist map instead of string interpolation:

```typescript
const TABLE_ALLOWLIST = {
  crawl: 'axon_crawl_jobs',
  extract: 'axon_extract_jobs',
  embed: 'axon_embed_jobs',
  ingest: 'axon_ingest_jobs',
} as const

// For status filtering, parameterize:
async function queryCrawl(statuses: string[], limit: number, offset: number) {
  const rows = await getJobsPgPool().query(
    `SELECT id, url, status, created_at, started_at, finished_at, error_text,
            config_json->>'collection' AS collection,
            COUNT(*) OVER() AS total
     FROM axon_crawl_jobs
     WHERE status = ANY($3)
     ORDER BY created_at DESC
     LIMIT $1 OFFSET $2`,
    [limit, offset, statuses],
  )
  // ...
}
```

### C-2. Subprocess Environment Leak in Source Route

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/source/route.ts` line 29
**Severity:** Critical

The `runAxonScrape` function passes the entire `process.env` to the child process:

```typescript
const child = spawn(commandPath, args, {
  cwd: repoRoot,
  env: process.env,  // <-- leaks ALL environment variables
  stdio: ['ignore', 'pipe', 'pipe'],
})
```

This includes `AXON_WEB_API_TOKEN`, `AI_GATEWAY_API_KEY`, `OPENAI_API_KEY`, `REDDIT_CLIENT_SECRET`, and any other secrets in the server environment. Compare this with `shell-server.mjs` which correctly uses a `SAFE_ENV_KEYS` allowlist (line 28-40) and `pulse/chat/route.ts` which at least strips `CLAUDECODE`.

**Fix:** Build a minimal environment for the subprocess, similar to shell-server:

```typescript
const AXON_SAFE_ENV_KEYS = [
  'PATH', 'HOME', 'LANG', 'TZ',
  'AXON_PG_URL', 'AXON_REDIS_URL', 'AXON_AMQP_URL',
  'QDRANT_URL', 'TEI_URL', 'AXON_COLLECTION',
  'OPENAI_BASE_URL', 'OPENAI_API_KEY', 'OPENAI_MODEL',
  'AXON_CHROME_REMOTE_URL',
]

function buildAxonChildEnv(): Record<string, string> {
  const env: Record<string, string> = {}
  for (const key of AXON_SAFE_ENV_KEYS) {
    const val = process.env[key]
    if (val) env[key] = val
  }
  return env
}
```

### C-3. Unbounded stderr Accumulation in Source Route

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/source/route.ts` lines 41-43
**Severity:** Critical

```typescript
child.stderr.on('data', (chunk: Buffer) => {
  stderr += chunk.toString()
})
```

Unlike the chat route which caps stderr at 16KB (`if (stderr.length < 16_384)`), the source route accumulates stderr without limit. A subprocess that writes megabytes to stderr (e.g., verbose logging on a large crawl) will consume unbounded server memory. With `SOURCE_INDEX_TIMEOUT_MS` set to 8 minutes, this is a realistic DoS vector.

**Fix:** Cap the accumulation:

```typescript
child.stderr.on('data', (chunk: Buffer) => {
  if (stderr.length < 16_384) stderr += chunk.toString()
})
```

### C-4. Duplicate CSP Headers (next.config.ts vs middleware.ts)

**File:** `/home/jmagar/workspace/axon_rust/apps/web/next.config.ts` lines 7-30 and `/home/jmagar/workspace/axon_rust/apps/web/middleware.ts` lines 12-31
**Severity:** Critical (security correctness)

Security headers including CSP are defined in two independent places with divergent values. The `next.config.ts` CSP includes `form-action 'self'` and `img-src ... https:` while the `middleware.ts` CSP omits `form-action` and uses `img-src 'self' data: blob:` (no `https:`). Both are applied to every request, meaning the browser receives duplicate headers.

Duplicate `Content-Security-Policy` headers are **not merged** by browsers -- the most restrictive one wins per-directive. This creates a false sense of security: a developer may add a permissive directive in one location thinking it takes effect, when the other location's more restrictive version silently overrides it.

**Fix:** Define security headers in exactly one place. Since the middleware already runs on all `/api` routes and can conditionally apply to pages, consolidate there and remove the `headers()` function from `next.config.ts` (except for the cortex cache-control and SW headers).

---

## High Findings

### H-1. Sequential Job Lookups in /api/jobs/[id]

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/[id]/route.ts` lines 232-237
**Severity:** High (performance)

```typescript
const job =
  (await findCrawlJob(id)) ??
  (await findEmbedJob(id)) ??
  (await findExtractJob(id)) ??
  (await findIngestJob(id))
```

Four sequential database queries. In the worst case (ingest job or not found), all four tables are queried serially. Each query hits a separate table with a UUID primary key lookup, but the latency adds up under load.

**Fix:** Run all four queries concurrently with `Promise.all` and take the first non-null result:

```typescript
const [crawl, embed, extract, ingest] = await Promise.all([
  findCrawlJob(id),
  findEmbedJob(id),
  findExtractJob(id),
  findIngestJob(id),
])
const job = crawl ?? embed ?? extract ?? ingest
```

### H-2. PgPool Created Without Connection Limits or Timeouts

**File:** `/home/jmagar/workspace/axon_rust/apps/web/lib/server/pg-pool.ts` lines 11-14
**Severity:** High (reliability)

```typescript
function createPool(): Pool {
  return new Pool({
    connectionString: process.env.AXON_PG_URL ?? DEFAULT_AXON_PG_URL,
  })
}
```

The pool uses `pg` defaults: 10 max connections, no idle timeout, no connection timeout. In a serverless-like environment (Next.js API routes with concurrent requests), this can exhaust connections under load or leave idle connections consuming Postgres slots indefinitely.

**Fix:**

```typescript
function createPool(): Pool {
  return new Pool({
    connectionString: process.env.AXON_PG_URL ?? DEFAULT_AXON_PG_URL,
    max: 5,
    idleTimeoutMillis: 30_000,
    connectionTimeoutMillis: 5_000,
  })
}
```

### H-3. useWsMessagesProvider Has 30+ useState Calls -- God Hook

**File:** `/home/jmagar/workspace/axon_rust/apps/web/hooks/use-ws-messages.ts` lines 82-487
**Severity:** High (maintainability)

This single hook manages 30+ state variables, 6+ tracked-setter wrappers, 7 localStorage effects, and delegates to a message handler. It is the single largest behavioral unit in the codebase at 405 lines. The context split into `WsMessagesExecutionContext`, `WsMessagesWorkspaceContext`, and `WsMessagesActionsContext` is a good optimization, but the hook itself is still a monolith.

The "tracked setter" pattern (lines 123-189) -- wrapping `setState` to keep a ref in sync -- is repeated 5 times with identical boilerplate:

```typescript
const setCrawlFilesTracked = useCallback((action: React.SetStateAction<CrawlFile[]>) => {
  if (typeof action === 'function') {
    setCrawlFiles((prev) => {
      const next = action(prev)
      crawlFilesRef.current = next
      return next
    })
  } else {
    crawlFilesRef.current = action
    setCrawlFiles(action)
  }
}, [])
```

**Fix:** Extract a generic `useTrackedState` hook:

```typescript
function useTrackedState<T>(initial: T) {
  const [state, setState] = useState(initial)
  const ref = useRef(initial)
  const setTracked = useCallback((action: SetStateAction<T>) => {
    if (typeof action === 'function') {
      setState((prev) => {
        const next = (action as (prev: T) => T)(prev)
        ref.current = next
        return next
      })
    } else {
      ref.current = action
      setState(action)
    }
  }, [])
  return [state, setTracked, ref] as const
}
```

This would eliminate ~70 lines of boilerplate and reduce the cognitive load of the hook.

### H-4. Replay Cache Serializes Every Event on Every Emit

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/chat/route.ts` lines 232-243
**Severity:** High (performance)

```typescript
const emit = (event: ...) => {
  const normalized = createPulseChatStreamEvent(event)
  replayBuffer.push(normalized)
  // ...
  persistReplay()  // <-- called on EVERY emit
  enqueueEvent(normalized)
}
```

`persistReplay()` calls `upsertReplayEntry()` which calls `estimateBufferBytes()`, serializing every event in the buffer to JSON on **every single streaming delta**. For a Claude response with 200 deltas, this means ~200 calls to `JSON.stringify` over a growing array. At 100 events, each call serializes all 100 events -- O(n^2) total serialization.

**Fix:** Debounce the persist call. The replay buffer is only needed for client reconnection, so a 500ms debounce is more than sufficient:

```typescript
let persistTimer: ReturnType<typeof setTimeout> | null = null
const debouncedPersistReplay = () => {
  if (persistTimer) clearTimeout(persistTimer)
  persistTimer = setTimeout(() => {
    upsertReplayEntry(replayKey, replayBuffer, Date.now())
  }, 500)
}

// In cleanup:
if (persistTimer) clearTimeout(persistTimer)
persistReplay() // final flush
```

### H-5. New TextEncoder on Every SSE Chunk in AI Chat

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/ai/chat/route.ts` lines 79-86
**Severity:** High (performance)

```typescript
for (const delta of parsed.deltas) {
  controller.enqueue(
    new TextEncoder().encode(
      `data: ${JSON.stringify({ choices: [{ delta: { content: delta } }] })}\n\n`,
    ),
  )
}
```

A new `TextEncoder` is instantiated for every SSE chunk inside a tight streaming loop. `TextEncoder` construction allocates internal buffers each time.

**Fix:** Hoist the encoder outside the stream:

```typescript
const encoder = new TextEncoder()
// ... later, inside the loop:
controller.enqueue(
  encoder.encode(`data: ${JSON.stringify(...)}\n\n`),
)
```

### H-6. JSON.parse Without Validation in AI Command Tools

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/ai/command/route.ts` lines 226, 273
**Severity:** High (reliability)

```typescript
const comments = JSON.parse(text) as Array<{ blockId: string; comment: string; content: string }>
```

The LLM response is `JSON.parse`d and then cast directly without validation. If the LLM returns malformed JSON or a shape that doesn't match the expected type (e.g., an object instead of an array, or missing fields), the code will either throw at `JSON.parse` (caught by the try/catch), or silently produce `undefined` values when accessing missing fields. The same pattern appears at line 273 for table updates.

**Fix:** Validate with Zod after parsing:

```typescript
const CommentSchema = z.array(
  z.object({
    blockId: z.string(),
    comment: z.string(),
    content: z.string(),
  }),
)

const parsed = CommentSchema.safeParse(JSON.parse(text))
if (!parsed.success) {
  console.error('[ai/command] malformed comment response:', parsed.error)
  return
}
const comments = parsed.data
```

### H-7. Copilot Route Returns Full AI SDK Result Object

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/ai/copilot/route.ts` line 98
**Severity:** High (security)

```typescript
return NextResponse.json(result)
```

When `streamNdjson` is not set, the entire `generateText` result object is returned to the client. This object may contain internal metadata, token counts, provider details, or other information that should not be exposed to the frontend.

**Fix:** Return only the fields the client needs:

```typescript
return NextResponse.json({
  text: result.text,
  finishReason: result.finishReason,
})
```

---

## Medium Findings

### M-1. useWsMessages `activeSources` Memo Is a No-op

**File:** `/home/jmagar/workspace/axon_rust/apps/web/components/pulse/pulse-chat-pane.tsx` line 62
**Severity:** Medium (performance)

```typescript
const activeSources = useMemo(() => activeThreadSources, [activeThreadSources])
```

This `useMemo` returns its input unchanged. It provides zero memoization benefit -- `activeThreadSources` is already referentially stable from the parent. This is dead code that adds noise.

**Fix:** Remove the memo and use `activeThreadSources` directly.

### M-2. handleCopyError Creates New Map on Every State Update

**File:** `/home/jmagar/workspace/axon_rust/apps/web/components/pulse/pulse-chat-pane.tsx` lines 176-197
**Severity:** Medium (performance)

```typescript
setCopyStatuses((prev) => new Map(prev).set(messageId, 'copied'))
setTimeout(() => {
  setCopyStatuses((prev) => {
    const next = new Map(prev)
    next.delete(messageId)
    return next
  })
}, 1200)
```

Each copy operation creates 2-4 new Map instances. The timeout closures are not cleaned up on unmount, so they can fire after the component unmounts (React state update on unmounted component).

**Fix:** Use a ref for the timeout and clean up on unmount. Consider using a simple object instead of Map since the keys are strings:

```typescript
const copyTimers = useRef(new Map<string, ReturnType<typeof setTimeout>>())

useEffect(() => {
  return () => {
    for (const timer of copyTimers.current.values()) clearTimeout(timer)
  }
}, [])
```

### M-3. localStorage Read in useEffect with [messages.length] Dependency

**File:** `/home/jmagar/workspace/axon_rust/apps/web/components/pulse/pulse-chat-pane.tsx` lines 117-131
**Severity:** Medium (correctness)

```typescript
useEffect(() => {
  const node = scrollRef.current
  if (!node) return
  try {
    const saved = Number(window.localStorage.getItem(CHAT_SCROLL_STORAGE_KEY) ?? 0)
    if (Number.isFinite(saved) && saved > 0) {
      node.scrollTop = saved
      // ...
    }
  } catch { /* ... */ }
}, [messages.length])  // <-- fires on every message
```

This effect re-reads the scroll position from localStorage and resets `scrollTop` on every new message. The intent was likely to restore scroll position on mount only, but the dependency on `messages.length` means it fires during streaming, fighting the auto-scroll-to-bottom behavior on line 168-173.

**Fix:** Use an empty dependency array (or a `useRef` guard) to run the restore only once on mount:

```typescript
const hasRestoredScroll = useRef(false)
useEffect(() => {
  if (hasRestoredScroll.current) return
  hasRestoredScroll.current = true
  // ... restore logic
}, [])
```

### M-4. JobDetail Interface Has 18 Nullable Fields

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/[id]/route.ts` lines 11-42
**Severity:** Medium (maintainability)

The `JobDetail` interface contains fields for all four job types (`pagesCrawled`, `docsEmbedded`, `urls`, etc.), most of which are `null` for any given job type. This wide, flat interface means every consumer must handle nullable fields for data that structurally cannot exist.

**Fix:** Use a discriminated union:

```typescript
interface BaseJobDetail {
  id: string
  status: JobStatus
  createdAt: string
  startedAt: string | null
  finishedAt: string | null
  errorText: string | null
  resultJson: Record<string, unknown> | null
  configJson: Record<string, unknown> | null
}

interface CrawlJobDetail extends BaseJobDetail {
  type: 'crawl'
  target: string
  collection: string | null
  pagesCrawled: number | null
  // ...only crawl-specific fields
}

type JobDetail = CrawlJobDetail | EmbedJobDetail | ExtractJobDetail | IngestJobDetail
```

### M-5. findCrawl/Embed/Extract/IngestJob -- 90% Identical Boilerplate

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/[id]/route.ts` lines 44-219
**Severity:** Medium (maintainability/DRY)

Four nearly identical functions each query a different table and map rows to `JobDetail`. The mapping logic (converting dates, parsing JSON, null-coalescing) is duplicated across all four. Each function is ~45 lines, and ~30 lines of each are identical structural scaffolding.

**Fix:** Extract a generic query-and-map function:

```typescript
async function findJobInTable(
  table: string,
  id: string,
  type: JobType,
  mapTarget: (row: Record<string, unknown>) => string,
  mapExtras: (row: Record<string, unknown>, res: Record<string, unknown>, cfg: Record<string, unknown>) => Partial<JobDetail>,
): Promise<JobDetail | null> {
  // shared query + base mapping logic
}
```

### M-6. Stale Closure Risk in useSplitPane Drag Effect

**File:** `/home/jmagar/workspace/axon_rust/apps/web/hooks/use-split-pane.ts` lines 77-125
**Severity:** Medium (correctness)

The `pointermove` and `pointerup` event listeners are registered once (empty dependency array `[]`). They read from `showEditorRef` and `showChatRef` which are kept in sync manually. However, `setDesktopSplitPercentTracked` and `setShowEditorTracked` are used inside the effect but are not in the dependency array. The effect relies on the assumption that `useCallback` with `[]` deps produces stable references -- which is true, but the pattern is fragile: if any dependency of those callbacks changes, the effect won't pick up the new version.

**Fix:** Add the tracked setters to the dependency array, or convert to a single `useCallback` that captures everything via refs:

```typescript
useEffect(() => {
  // ...
}, [setDesktopSplitPercentTracked, setShowEditorTracked])
```

### M-7. Timer Leak in handleCopyError (PulseChatPane)

**File:** `/home/jmagar/workspace/axon_rust/apps/web/components/pulse/pulse-chat-pane.tsx` lines 180-196
**Severity:** Medium (resource leak)

The `setTimeout` calls inside `handleCopyError` are not tracked or cleaned up. If the component unmounts before the timeout fires, the `setCopyStatuses` call targets an unmounted component. While React 19 suppresses the warning, the callback still runs unnecessarily.

**Fix:** Track timers in a ref and clear on unmount (see M-2 fix above).

### M-8. `toolsRestrict` Passed Unsanitized to Claude CLI

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/chat/claude-stream-types.ts` lines 206-208
**Severity:** Medium (security)

```typescript
if (extra?.toolsRestrict) {
  args.push('--tools', extra.toolsRestrict)
}
```

While `allowedTools` and `disallowedTools` are validated against `TOOL_ENTRY_RE`, `toolsRestrict` is passed through without any sanitization. This allows arbitrary string values to be injected as a Claude CLI `--tools` argument.

**Fix:** Apply the same `TOOL_ENTRY_RE` filter:

```typescript
if (extra?.toolsRestrict) {
  const filtered = extra.toolsRestrict
    .split(',')
    .map((t) => t.trim())
    .filter((t) => TOOL_ENTRY_RE.test(t))
    .join(',')
  if (filtered) args.push('--tools', filtered)
}
```

### M-9. `connect-src` CSP Allows All HTTP/HTTPS/WS/WSS

**File:** `/home/jmagar/workspace/axon_rust/apps/web/middleware.ts` line 28 and `/home/jmagar/workspace/axon_rust/apps/web/next.config.ts` line 24
**Severity:** Medium (security)

```typescript
"connect-src 'self' ws: wss: http: https:"
```

This CSP directive allows the frontend to make network requests to any origin, which defeats the purpose of CSP's connect-src directive. An XSS payload could exfiltrate data to any external server.

**Fix:** Restrict to known backends:

```typescript
`connect-src 'self' ws://localhost:* wss://localhost:* ${process.env.AXON_BACKEND_URL ?? ''}`
```

---

## Low Findings

### L-1. `safeStatus` Duplicated Across Two Route Files

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/route.ts` line 38 and `/home/jmagar/workspace/axon_rust/apps/web/app/api/jobs/[id]/route.ts` line 6
**Severity:** Low (DRY)

Identical `safeStatus` function defined in both files. Extract to a shared module (`lib/server/job-types.ts`).

### L-2. Inconsistent Error Response Format in Logs Route

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/logs/route.ts` lines 76, 80
**Severity:** Low (consistency)

```typescript
return new Response('Invalid service', { status: 400 })
return new Response('Invalid tail value', { status: 400 })
```

All other API routes use the `apiError()` helper for consistent JSON error envelopes. The logs route returns plain text.

**Fix:** Use `apiError(400, 'Invalid service')`.

### L-3. Hardcoded Model Strings in Multiple Locations

**File:** Multiple files
**Severity:** Low (maintainability)

The string literals `'sonnet'`, `'opus'`, `'haiku'` appear in at least 4 files:
- `hooks/use-ws-messages.ts` line 215
- `hooks/ws-messages/types.ts`
- `app/api/pulse/chat/claude-stream-types.ts` line 18
- `components/omnibox/omnibox-input-bar.tsx` line 339

**Fix:** Define a single `PULSE_MODELS` constant in `lib/pulse/types.ts` and import everywhere.

### L-4. Dead `_placeholderVisible` Prop in OmniboxInputBar

**File:** `/home/jmagar/workspace/axon_rust/apps/web/components/omnibox/omnibox-input-bar.tsx` line 89
**Severity:** Low (dead code)

```typescript
placeholderVisible: _placeholderVisible,
```

The prop is destructured and immediately discarded. Either the component should use it or the prop should be removed from the interface.

### L-5. `isLoopbackHost` Inconsistency Between middleware.ts and shell-server.mjs

**File:** `/home/jmagar/workspace/axon_rust/apps/web/middleware.ts` line 43 vs `/home/jmagar/workspace/axon_rust/apps/web/shell-server.mjs` line 42
**Severity:** Low (consistency)

The middleware's `isLoopbackHost` checks `localhost`, `127.0.0.1`, `::1`, `[::1]`. The shell server's version additionally checks `0.0.0.0`. Since `0.0.0.0` is a valid loopback bind address that browsers can connect to, this inconsistency means the middleware rejects what the shell server allows.

**Fix:** Align both to include `0.0.0.0`, or better, extract a shared utility.

### L-6. No Rate Limiting on Subprocess-Spawning Routes

**File:** `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/chat/route.ts`, `/home/jmagar/workspace/axon_rust/apps/web/app/api/pulse/source/route.ts`
**Severity:** Low (for self-hosted; would be High for public-facing)

Both routes spawn OS-level subprocesses (`claude`, `axon scrape`) on every request. There is no rate limiting. A burst of requests can exhaust system resources. The auth middleware provides some protection, but a compromised or misconfigured client with a valid token could DoS the server.

**Fix:** Add a simple in-memory concurrency semaphore:

```typescript
let activeSpawns = 0
const MAX_CONCURRENT_SPAWNS = 3

// At start of handler:
if (activeSpawns >= MAX_CONCURRENT_SPAWNS) {
  return apiError(429, 'Too many concurrent requests')
}
activeSpawns++
// In cleanup: activeSpawns--
```

---

## Architecture Observations

These are not defects but structural observations worth tracking.

### A-1. Context Provider Nesting Depth

`providers.tsx` nests 6 context providers deep (AxonWsContext > WsMessagesExecutionContext > WsMessagesWorkspaceContext > WsMessagesActionsContext > WsMessagesContext > TooltipProvider). The split contexts are a good optimization, but the "combined" `WsMessagesContext` that spreads all three sub-contexts back together (line 477-484) means any consumer using `useWsMessages()` re-renders on any change to any of the three contexts, defeating the split. Consumers should use the specific sub-context hooks (`useWsExecutionState()`, `useWsWorkspaceState()`, `useWsMessageActions()`) and the combined context should be deprecated.

### A-2. Dual WebSocket Pattern

The app maintains two independent WebSocket connections: `use-axon-ws.ts` for the Axon backend, and `use-shell-session.ts` for the PTY. Both implement identical reconnection logic (exponential backoff, `connectRef` pattern, visibility/online listeners). This is a reasonable design choice -- the PTY session has different lifecycle semantics -- but the reconnect logic should be extracted into a shared `createReconnectingWebSocket` utility to avoid the duplication.

### A-3. State Persistence Strategy

The Pulse workspace persists ~15 fields to localStorage via `use-pulse-persistence.ts`, including full chat history and document markdown. For large documents and long conversations, this can easily exceed 5MB (the localStorage quota in most browsers), at which point writes silently fail. The catch blocks suppress these errors. Consider switching to IndexedDB for large payloads (chat history, document markdown) while keeping small values in localStorage.

### A-4. Handler/Reducer Divergence Risk

`ws-messages/handlers.ts` and `ws-messages/runtime.ts` both handle WebSocket messages but through different patterns (imperative setState calls vs. pure reducer). The comment on line 101 of `runtime.ts` explicitly warns about this: "When updating message handling in handlers.ts, update the matching cases here to prevent divergence." This is a maintenance trap. Consider making the handler use the reducer internally.

---

## Confirmed Strengths

1. **Auth middleware exists and works** -- `middleware.ts` with token validation, origin checks, insecure-dev toggle, and HSTS in production.
2. **Shell server environment allowlisting** -- `SAFE_ENV_KEYS` prevents secret leakage from PTY sessions.
3. **SSRF validation** -- `url-validation.ts` covers IPv4 private ranges, IPv6 mapped/ULA/link-local/multicast, blocked hostnames, and scheme filtering.
4. **Provider chain architecture** -- Clean dependency direction from layout through providers to leaf components.
5. **Hook composition in Pulse** -- `usePulseChat`, `usePulsePersistence`, `usePulseAutosave`, `usePulseSettings` are well-separated.
6. **WS bridge pattern** -- API routes proxy through Rust backend via WS, avoiding binary coupling.
7. **SQL parameterization** -- All user values use `$1`, `$2` params. Only hardcoded strings interpolated.
8. **Subprocess spawn safety** -- All `spawn()` calls use array args, not shell strings.
9. **Zod validation on critical routes** -- PulseChatRequest, SaveRequest, McpConfig, PulseSourceRequest.
10. **Claude CLI arg sanitization** -- `TOOL_ENTRY_RE`, `validateAddDir()` with realpath resolution, `sanitizeBetas()` with allowlist.
