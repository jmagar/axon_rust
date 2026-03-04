# crates/mcp
Last Modified: 2026-03-03

Axon MCP server crate backing the `axon mcp` command.

## Scope
- MCP transport and server wiring (`server.rs`)
- Tool request schema and strict parser (`schema.rs`)
- Runtime config loading (`config.rs`)

## Public Contract
- Single MCP tool: `axon`
- Transport: HTTP-only (`mcp-http` runtime); no stdio transport
- Primary request shape: action-routed requests via `action` + `subaction`
- Parser is strict (no fallback action keys, no alias remapping)
- Context-safe default: `response_mode=path` (artifact-first output in `.cache/axon-mcp/`)
- Resource exposed: `axon://schema/mcp-tool`

See source-of-truth docs:
- `docs/MCP.md`
- `docs/MCP-TOOL-SCHEMA.md`

## Local Development
```bash
cargo check --bin axon
cargo run --bin axon -- mcp
```

HTTP MCP transport is managed via container runtime/s6 (`mcp-http`) and documented in `docs/MCP.md`.

## Schema Validation / Smoke Tests
Primary MCP smoke path:

```bash
./scripts/test-mcp-tools-mcporter.sh
```

```bash
mcporter list axon --schema
mcporter call axon.axon action:doctor
mcporter call axon.axon action:crawl subaction:list limit:5
mcporter call axon.axon action:refresh subaction:list limit:5
```

## Change Rule
When changing tool behavior, update in the same commit:
1. `crates/mcp/schema.rs`
2. `crates/mcp/server.rs`
3. `docs/MCP.md`
4. `docs/MCP-TOOL-SCHEMA.md`

## Related Docs
- [Repository README](../../README.md)
- [Architecture](../../docs/ARCHITECTURE.md)
- [MCP Runtime Guide](../../docs/MCP.md)
