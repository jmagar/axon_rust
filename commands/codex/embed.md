
# Embed Content

Call the Axon MCP tool (`axon`) with:
- `action: "embed"`
- `subaction: "start|status|cancel|list|cleanup|clear|recover"` from `$ARGUMENTS`
- map remaining `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Handle both start and lifecycle operations (`status|cancel|list|cleanup|clear|recover`).
3. Parse embedding progress, vector write counts, and failures.
4. Present final indexing summary.

## Expected Output

The command returns:
- embed job identifiers
- lifecycle status/progress
- vector/indexing results and errors
