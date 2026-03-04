# axon debug
Last Modified: 2026-03-03

Run `doctor`, then ask the configured LLM for prioritized troubleshooting steps.

## Synopsis

```bash
axon debug [context text ...] [FLAGS]
```

## Arguments

| Argument | Description |
|----------|-------------|
| `[context text ...]` | Optional operator context appended to the LLM prompt. |

## Required Environment Variables

| Variable | Description |
|----------|-------------|
| `OPENAI_BASE_URL` | OpenAI-compatible base URL (for example `http://host/v1`). |
| `OPENAI_MODEL` | Model used to generate troubleshooting guidance. |

`OPENAI_API_KEY` is optional unless your endpoint requires auth.

## Flags

All global flags apply. Key flag:

| Flag | Default | Description |
|------|---------|-------------|
| `--json` | `false` | Include both `doctor_report` and `llm_debug` in JSON. |

## Examples

```bash
# Basic debug workflow
axon debug

# Include symptom context for better guidance
axon debug "crawl jobs stuck in pending for 30m"

# Structured output
axon debug "qdrant timeout after restart" --json
```

## Notes

- Fails fast if `OPENAI_BASE_URL` or `OPENAI_MODEL` is missing.
- Requests are sent to `{OPENAI_BASE_URL}/chat/completions`.
- Keep `OPENAI_BASE_URL` as the API base (typically including `/v1`), not a full chat-completions URL.
