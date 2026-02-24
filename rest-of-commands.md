# Web UI: Complete Command Support

> **Implementation plan:** see `.claude/plans/linear-coalescing-plum.md`

## Problem

Only `scrape` and `crawl` render results in the web UI. All other commands execute silently — `execute.rs:318` discards stdout for sync commands, `--json` is only injected for async modes.

## Phases

| Phase | Goal | Key Deliverable |
|-------|------|-----------------|
| **A** | Core unlock | Stream stdout, inject `--json`, raw fallback renderer |
| **B** | Typed renderers | Table, cards, report, status renderers + normalizer pipeline |
| **C** | Job lifecycle | Async command status/cancel UI, complete mode picker |
| **D** | Polish | Per-mode options, recent runs, result history cache |

## Status

- [ ] Phase A: Core Unlock
- [ ] Phase B: Typed Renderers
- [ ] Phase C: Job Lifecycle
- [ ] Phase D: Polish
