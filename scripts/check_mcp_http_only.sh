#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

TARGET="crates/cli/commands/mcp.rs"

if [ ! -f "$TARGET" ]; then
  echo "ERROR: missing $TARGET"
  exit 1
fi

if ! rg -q 'run_http_server\(' "$TARGET"; then
  echo "ERROR: MCP CLI must call run_http_server(...) in $TARGET"
  exit 1
fi

if rg -q 'run_stdio_server\(' "$TARGET"; then
  echo "ERROR: MCP CLI regressed to stdio transport in $TARGET"
  exit 1
fi

if ! rg -q 'AXON_MCP_HTTP_HOST' "$TARGET"; then
  echo "ERROR: MCP CLI must read AXON_MCP_HTTP_HOST in $TARGET"
  exit 1
fi

if ! rg -q 'AXON_MCP_HTTP_PORT' "$TARGET"; then
  echo "ERROR: MCP CLI must read AXON_MCP_HTTP_PORT in $TARGET"
  exit 1
fi

echo "OK: MCP CLI transport is HTTP-only."
