
# Ask AI-Grounded Questions

Call the Axon MCP tool (`axon`) with:
- `action: "ask"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. Monitor streaming answer output and citations.
3. Parse answer, sources, and retrieved-doc counts.
4. Present the answer first, then supporting sources.

## Expected Output

The command returns:
- synthesized answer
- cited sources with relevance
- retrieval/context metrics
