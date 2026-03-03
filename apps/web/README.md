# Axon Web (Next.js)
Last Modified: 2026-03-02

`apps/web` is the Next.js interface for Axon. It provides a command omnibox, workspace flows, and live command execution over WebSocket.

## Run

```bash
pnpm --dir apps/web dev
```

Open `http://localhost:3000`.

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
- `__tests__/omnibox.test.ts`: omnibox helper unit tests
