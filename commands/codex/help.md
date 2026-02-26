
# Axon Help

Call the Axon MCP tool (`axon`) with:
- `action: "help"`
- map `$ARGUMENTS` to schema fields

## Instructions

1. Execute using the Axon MCP tool (`axon`) with action/subaction routing and mapped arguments.
2. If no arguments are provided, use top-level help.
3. Parse the output to highlight available commands and usage examples.
4. Present the most relevant next command for the user’s goal.

## Expected Output

The command returns CLI help text including:
- available commands
- option flags
- usage patterns
