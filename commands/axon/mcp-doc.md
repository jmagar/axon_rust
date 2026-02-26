---
description: Update docs/MCP.md with MCP server contract changes
argument-hint: [change-request]
allowed-tools: Read, Edit, Grep, Glob, Bash(cargo check --bin axon-mcp)
---

Apply this change request to `docs/MCP.md`: $ARGUMENTS

Use these files as required references before editing:
- @docs/MCP.md
- @docs/MCP-TOOL-SCHEMA.md
- @crates/mcp/README.md
- @crates/mcp/schema.rs
- @crates/mcp/server.rs

Workflow:
1. Verify whether the requested change is documentation-only or requires code/schema updates.
2. Update `docs/MCP.md` to keep action/subaction names, parser rules, and response semantics accurate.
3. Keep terminology and examples aligned with the schema and crate README.
4. If behavior changed, run `cargo check --bin axon-mcp` and report pass/fail.
5. Summarize exactly what changed and any follow-up required in related docs.
