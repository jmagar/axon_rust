
# Extract Structured Data

Call the Axon MCP tool (`axon`) with:
- `action: "extract"`
- `subaction: "start|status|cancel|list|cleanup|clear|recover"` from `$ARGUMENTS`
- map remaining `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Handle both start and lifecycle operations (`status|cancel|list|cleanup|clear|recover`).
3. Parse extraction outputs, progress, and failures.
4. Present extracted fields and per-URL quality issues.

## Expected Output

The command returns:
- extract job identifiers
- lifecycle status/progress
- structured extraction results and errors
