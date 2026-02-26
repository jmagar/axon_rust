
# Scrape URLs

Call the Axon MCP tool (`axon`) with:
- `action: "scrape"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Parse scraped output and per-URL status.
3. Report skipped/failed URLs with reasons.
4. Confirm embedding behavior when enabled.

## Expected Output

The command returns:
- per-URL scrape results
- output file/artifact paths
- embed/queue status details
