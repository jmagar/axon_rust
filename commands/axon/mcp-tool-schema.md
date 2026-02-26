---
description: Update docs/MCP-TOOL-SCHEMA.md source-of-truth schema doc
argument-hint: [change-request]
allowed-tools: Read, Edit, Grep, Glob, Bash(cargo check --bin axon-mcp)
---

Apply this change request to `docs/MCP-TOOL-SCHEMA.md`: $ARGUMENTS

Use these files as required references before editing:
- @docs/MCP-TOOL-SCHEMA.md
- @docs/MCP.md
- @crates/mcp/schema.rs
- @crates/mcp/server.rs
- @crates/mcp/README.md

Workflow:
1. Validate canonical request/response shapes against the Rust MCP schema/parser implementation.
2. Update action, subaction, required fields, and error semantics precisely.
3. Ensure examples and lifecycle families match the implemented routing contract.
4. Keep `docs/MCP.md` and `crates/mcp/README.md` consistency notes accurate; update if needed.
5. If behavior changed, run `cargo check --bin axon-mcp` and report pass/fail.
