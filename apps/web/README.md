# Axon Web (Next.js)
Last Modified: 2026-03-03

`apps/web` is the Next.js interface for Axon. It provides a command omnibox, workspace flows, and live command execution over WebSocket.

## Run

```bash
pnpm --dir apps/web dev
```

Open `http://localhost:3000`.

## API + Shell Security

All `app/api/*` routes are now protected by `apps/web/middleware.ts`.

- Auth headers accepted:
  - `Authorization: Bearer <token>`
  - `x-api-key: <token>`
- Required server env:
  - `AXON_WEB_API_TOKEN`
- Origin enforcement:
  - `AXON_WEB_ALLOWED_ORIGINS` (comma-separated), or same-origin fallback if unset
- Local-only bypass (development only):
  - `AXON_WEB_ALLOW_INSECURE_DEV=true`

The terminal shell websocket (`/ws/shell`) now enforces auth and origin checks in `shell-server.mjs`.

- Preferred shell token: `AXON_SHELL_WS_TOKEN` (falls back to `AXON_WEB_API_TOKEN`)
- Optional shell-specific origin allowlist: `AXON_SHELL_ALLOWED_ORIGINS`
- Client token wiring:
  - `NEXT_PUBLIC_SHELL_WS_TOKEN` (preferred)
  - `NEXT_PUBLIC_AXON_API_TOKEN` (fallback)

Next.js response hardening is configured in `next.config.ts` with CSP, `X-Frame-Options`, `Referrer-Policy`, and HSTS (non-dev).
`/api/cortex/*` responses are cache-tuned with `s-maxage=30, stale-while-revalidate=60`.

## API Contracts

- `GET /api/jobs` validates `type` (`crawl|extract|embed|github|reddit|youtube`) and `status` (`pending|running|completed|failed|canceled`); invalid filters return `400`.
- `GET /api/pulse/source` enforces URL SSRF protections and returns `code: "ssrf_blocked"` when blocked.
- Shared error envelope: `{ error, code?, errorId?, detail? }`.

## Omnibox Behavior

The omnibox supports keyboard-first operation with explicit visual state feedback.

### Focus Shortcut

- Press `/` to focus the omnibox when focus is not already inside an editable field.
- Shortcut is ignored for `input`, `textarea`, `select`, and content-editable elements.

### Mode Mentions

- Start input with `@` to enter mode mention selection.
- Example: `@c` suggests up to 3 matching modes (`crawl`, etc.).
- `Tab` or `Enter` applies the selected mode.
- After mode selection, the mention is removed and the omnibox is cleared for the next input.
- The UI shows:
  - active mention suggestions
  - selected/hovered mention state
  - transient `Mode selected: <label>` confirmation

### File Mentions

- Use `@` mentions in normal text to attach local context files.
- Suggestions are fuzzy-ranked (exact/prefix/contains/subsequence) and include recency bias from recent picks.
- Suggestion list is capped at 3 entries.
- Sources:
  - `docs/**` (`.md`, `.mdx`, `.txt`, `.rst`)
  - `.cache/pulse/**` entries from Pulse docs storage
- Selected files are shown as removable context chips under the omnibox.

### Keyboard Controls

- `ArrowUp`/`ArrowDown`: move mention selection.
- `Tab`/`Enter`: apply selected mention.
- `Escape`: close dropdown/options and clear active mention suggestions.
- `Enter` (without mention selection): execute current command.

## File Context Injection

Before command execution, mentioned files are resolved and appended to the input as a context section.

- Up to 3 files are loaded.
- Each file contributes a capped excerpt (up to 2400 chars).
- Execution flags include:
  - `context_files=<comma-separated labels>`

## Omnibox Local File API

### `GET /api/omnibox/files`

Returns mention candidates:

```json
{
  "files": [
    {
      "id": "docs:guide/setup.md",
      "label": "setup",
      "path": "docs/guide/setup.md",
      "source": "docs"
    }
  ]
}
```

### `GET /api/omnibox/files?id=<id>`

Returns file payload for context injection:

```json
{
  "file": {
    "id": "docs:guide/setup.md",
    "label": "setup",
    "path": "docs/guide/setup.md",
    "source": "docs",
    "content": "..."
  }
}
```

Route safety:

- `id` must be prefixed by `docs:` or `pulse:`.
- path traversal (`..`) is rejected.
- resolved paths must stay under source roots.

## Key Files

- `components/omnibox.tsx`: omnibox interaction/state UI
- `lib/omnibox.ts`: mention parsing, ranking, phase derivation helpers
- `app/api/omnibox/files/route.ts`: local docs listing + content fetch for mentions
- `hooks/use-ws-messages.ts`: split execution/workspace/action contexts + compatibility hook
- `middleware.ts`: API authentication + origin enforcement
- `shell-server.mjs`: authenticated node-pty websocket bridge with restricted child env
- `lib/command-options.ts`: shared omnibox command-option type (no component-layer coupling)
- `__tests__/omnibox.test.ts`: omnibox helper unit tests
