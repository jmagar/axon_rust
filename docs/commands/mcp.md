# axon mcp
Last Modified: 2026-03-03

Start Axon's MCP HTTP server exposing a single unified tool: `axon`.

## Synopsis

```bash
axon mcp [FLAGS]
```

## Runtime Binding

`axon mcp` uses environment variables for bind address:

| Variable | Default | Description |
|----------|---------|-------------|
| `AXON_MCP_HTTP_HOST` | `0.0.0.0` | MCP server bind host |
| `AXON_MCP_HTTP_PORT` | `8001` | MCP server bind port |

Primary MCP endpoint is mounted at `/mcp`.

## Tool Contract

- Tool count: 1
- Tool name: `axon`
- Routing: `action` + `subaction` (for lifecycle families)

Supported top-level action families include: `status`, `help`, `crawl`, `extract`, `embed`, `ingest`, `refresh`, `query`, `retrieve`, `search`, `map`, `doctor`, `domains`, `sources`, `stats`, `artifacts`, `scrape`, `research`, `ask`, `screenshot`.

## Examples

```bash
# Default bind 0.0.0.0:8001
axon mcp

# Custom bind
AXON_MCP_HTTP_HOST=127.0.0.1 AXON_MCP_HTTP_PORT=8900 axon mcp
```

## Notes

- If `AXON_MCP_HTTP_PORT` is not a valid `u16`, startup fails immediately.
- Server also mounts OAuth-related endpoints (for configured auth flows).
- See `docs/MCP.md` and `docs/MCP-TOOL-SCHEMA.md` for full request/response contract details.
