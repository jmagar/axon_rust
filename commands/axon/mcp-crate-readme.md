---
description: Update crates/mcp/README.md to match current MCP crate behavior
argument-hint: [change-request]
allowed-tools: Read, Edit, Grep, Glob, Bash(cargo check --bin axon-mcp)
---

Apply this change request to `crates/mcp/README.md`: $ARGUMENTS

Use these files as required references before editing:
- @crates/mcp/README.md
- @docs/MCP.md
- @docs/MCP-TOOL-SCHEMA.md
- @crates/mcp/schema.rs
- @crates/mcp/server.rs
- @crates/mcp/config.rs

Workflow:
1. Confirm the crate README reflects actual scope, contract, and runtime behavior.
2. Keep the README concise, accurate, and aligned with `docs/MCP.md` and `docs/MCP-TOOL-SCHEMA.md`.
3. Update local dev and smoke-test sections only if commands are validated.
4. If behavior changed, run `cargo check --bin axon-mcp` and report pass/fail.
5. List any cross-file updates needed to preserve the documented change rule.
