
# Ingest External Sources

Call the Axon MCP tool (`axon`) with:
- `action: "ingest"`
- `subaction: "start|status|cancel|list|cleanup|clear|recover"` from `$ARGUMENTS`
- map remaining `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Handle both start and lifecycle operations (`status|cancel|list|cleanup|clear|recover`).
3. Parse source metadata, job state, and ingest results.
4. Present ingest summary and follow-up indexing status.

## Expected Output

The command returns:
- ingest job identifiers
- lifecycle status/progress
- ingestion results and errors
