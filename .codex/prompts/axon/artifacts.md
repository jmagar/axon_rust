
# Inspect Artifact Files

Call the Axon MCP tool (`axon`) with:
- `action: "artifacts"`
- `subaction: "head|grep|wc|read"` from `$ARGUMENTS`
- map remaining `$ARGUMENTS` to schema fields (`path`, `pattern`, `limit`, `offset`)

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Route operations by subaction (`head|grep|wc|read`).
3. For `grep`, require a search pattern.
4. Present concise file insights and preserve exact match lines for grep.

## Expected Output

The command returns:
- file inspection output
- line/word/byte counts (wc)
- filtered lines (grep)
- sampled or full content (head/read)
