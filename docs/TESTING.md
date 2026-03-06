# Testing Guide
Last Modified: 2026-02-27

This document defines how to run tests locally and in CI for `axon`.

## Goals
- Keep the default local loop fast.
- Keep infra-backed tests explicit and reproducible.
- Ensure CI and local workflows stay aligned.

## Test Lanes

### Fast local lane (default)
Use this for most edits:

```bash
just test
```

Behavior:
- Uses `cargo nextest` when available.
- Falls back to `cargo test` if `cargo-nextest` is not installed.
- Skips `worker_e2e` tests.
- Enforces lockfile reproducibility (`--locked`).

### Fastest inner loop (lib-focused)

```bash
just test-fast
```

Use while iterating on library logic; excludes `worker_e2e`.

### Infra lane (explicit)
Use this when touching queue/worker/DB/integration behavior:

```bash
just test-infra
```

Behavior:
- Runs ignored `worker_e2e` tests explicitly.
- Requires local infra dependencies to be reachable.

## Validation Commands

### Compile checks
```bash
just check
just check-tests
```

### Full pre-push gate
```bash
just verify
```

`just verify` runs:
- `./scripts/check_dockerignore_guards.sh`
- `fmt-check`
- `clippy`
- `check`
- `test`

## CI Mapping

- `test` job: standard Rust test lane (`cargo test --all --locked`).
- `test-infra` job: manual-only lane, triggered via `workflow_dispatch` input `run_infra_tests=true`.
- `security` job: explicit `cargo audit --deny warnings` and `cargo deny check` with pinned tool versions.
- `msrv` job: validates declared MSRV separately.

## MCP Tooling Tests (mcporter)

Use the existing smoke script to quickly validate MCP tool contract coverage (tools/actions/subactions/resources):

```bash
# quick smoke set (just wrapper)
just mcp-smoke

# equivalent direct script call
./scripts/test-mcp-tools-mcporter.sh

# extended set (includes heavier actions)
./scripts/test-mcp-tools-mcporter.sh --full
```

Prerequisites:
- `mcporter` installed (`npm install -g mcporter@0.7.3`).
- MCP config available at `config/mcporter.json`.

Useful direct checks:

```bash
mcporter list axon --schema
mcporter call axon.axon action:help response_mode:inline --output json
mcporter call axon.axon action:crawl subaction:list limit:5 offset:0 --output json
```

Notes:
- Script artifacts/logs are written under `.cache/mcporter-test/`.
- CI parity: the `mcp-smoke` workflow job runs this same script in GitHub Actions.
- Canonical MCP runtime/testing reference: `docs/MCP.md`.

## Recommended Local Setup

```bash
just nextest-install
just llvm-cov-install
```

Optional performance helpers already auto-detected by `just` recipes:
- `sccache`
- `mold`

## Coverage (branch-level)

Run once per branch before merge:

```bash
just coverage-branch
```

## Common Failure Modes

### `worker_e2e` tests not running
- Cause: They are intentionally `#[ignore]` in default test lane.
- Fix: Run `just test-infra`.

### Lockfile errors in CI/local commands
- Cause: dependency graph changed but lockfile not updated.
- Fix: run a lockfile-refreshing command locally, then rerun `just verify`.

### DB test connection/auth failures
- Check `AXON_TEST_PG_URL` first.
- If unset, test resolver falls back to `.env` and then defaults.
- Ensure credentials in local `.env` match running Postgres.

## Pull Request Checklist (Testing)
- Ran `just test` after code changes.
- Ran `just test-infra` when changing worker/queue/DB integration paths.
- Ran `just verify` before opening/updating PR.
