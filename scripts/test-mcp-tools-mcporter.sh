#!/usr/bin/env bash
set -euo pipefail

# MCP smoke test runner via mcporter for Axon MCP server.
# - Verifies tool schema visibility
# - Verifies resource exposure via action:help response
# - Exercises top-level actions with minimal arguments
#
# Usage:
#   ./scripts/test-mcp-tools-mcporter.sh
#   MCP_SERVER=axon ./scripts/test-mcp-tools-mcporter.sh
#   ./scripts/test-mcp-tools-mcporter.sh --full
#
# Notes:
# - mcporter currently exposes list/call; resource checks are performed via action:help
#   and list --schema output.
# - --full includes network-heavy/side-effect actions.

SERVER="${MCP_SERVER:-axon}"
SELECTOR="${SERVER}.axon"
FULL=0

if [[ "${1:-}" == "--full" ]]; then
  FULL=1
fi

if ! command -v mcporter >/dev/null 2>&1; then
  echo "FAIL: mcporter not found in PATH" >&2
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "FAIL: jq not found in PATH" >&2
  exit 2
fi

OUTDIR=".cache/mcporter-test"
mkdir -p "$OUTDIR"
SUMMARY="$OUTDIR/summary.txt"
: > "$SUMMARY"

pass=0
fail=0

run_case() {
  local name="$1"
  shift
  local logfile="$OUTDIR/${name}.log"
  if "$@" >"$logfile" 2>&1; then
    echo "PASS $name" | tee -a "$SUMMARY"
    pass=$((pass + 1))
  else
    echo "FAIL $name (see $logfile)" | tee -a "$SUMMARY"
    fail=$((fail + 1))
  fi
}

run_pipe_case() {
  local name="$1"
  local script="$2"
  local logfile="$OUTDIR/${name}.log"
  if bash -lc "$script" >"$logfile" 2>&1; then
    echo "PASS $name" | tee -a "$SUMMARY"
    pass=$((pass + 1))
  else
    echo "FAIL $name (see $logfile)" | tee -a "$SUMMARY"
    fail=$((fail + 1))
  fi
}

echo "Server: $SERVER" | tee -a "$SUMMARY"

echo "== Schema checks ==" | tee -a "$SUMMARY"
run_case list_server mcporter list "$SERVER"
run_case list_schema mcporter list "$SERVER" --schema

echo "== Resource checks ==" | tee -a "$SUMMARY"
run_pipe_case help_inline "mcporter call '$SELECTOR' action:help response_mode:inline --output json | jq -e '.ok == true' >/dev/null"
run_pipe_case help_has_resource_uri "mcporter call '$SELECTOR' action:help response_mode:inline --output json | jq -e '.data.inline.resources | index(\"axon://schema/mcp-tool\") != null' >/dev/null"
run_pipe_case schema_mentions_resource "mcporter list '$SERVER' --schema | grep -q 'axon://schema/mcp-tool'"

echo "== Core action checks ==" | tee -a "$SUMMARY"
run_pipe_case action_status "mcporter call '$SELECTOR' action:status --output json | jq -e '.ok == true and .action == \"status\"' >/dev/null"
run_pipe_case action_help "mcporter call '$SELECTOR' action:help --output json | jq -e '.ok == true and .action == \"help\"' >/dev/null"
run_pipe_case action_doctor "mcporter call '$SELECTOR' action:doctor --output json | jq -e '.ok == true and .action == \"doctor\"' >/dev/null"
run_pipe_case action_stats "mcporter call '$SELECTOR' action:stats --output json | jq -e '.ok == true and .action == \"stats\"' >/dev/null"
run_pipe_case action_domains "mcporter call '$SELECTOR' action:domains limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"domains\"' >/dev/null"
run_pipe_case action_sources "mcporter call '$SELECTOR' action:sources limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"sources\"' >/dev/null"
run_pipe_case action_query "mcporter call '$SELECTOR' action:query query:'rust mcp sdk' limit:3 offset:0 --output json | jq -e '.ok == true and .action == \"query\"' >/dev/null"
run_pipe_case action_retrieve "mcporter call '$SELECTOR' action:retrieve url:"$PWD/docs/MCP.md" --output json | jq -e '.ok == true and .action == \"retrieve\"' >/dev/null"
run_pipe_case action_map "mcporter call '$SELECTOR' action:map url:'https://example.com' limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"map\"' >/dev/null"
run_pipe_case action_scrape "mcporter call '$SELECTOR' action:scrape url:'https://example.com' --output json | jq -e '.ok == true and .action == \"scrape\"' >/dev/null"
run_pipe_case action_crawl_list "mcporter call '$SELECTOR' action:crawl subaction:list limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"crawl\" and .subaction == \"list\"' >/dev/null"
run_pipe_case action_extract_list "mcporter call '$SELECTOR' action:extract subaction:list limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"extract\" and .subaction == \"list\"' >/dev/null"
run_pipe_case action_embed_list "mcporter call '$SELECTOR' action:embed subaction:list limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"embed\" and .subaction == \"list\"' >/dev/null"
run_pipe_case action_ingest_list "mcporter call '$SELECTOR' action:ingest subaction:list limit:5 offset:0 --output json | jq -e '.ok == true and .action == \"ingest\" and .subaction == \"list\"' >/dev/null"

run_pipe_case action_artifacts_head "mcporter call '$SELECTOR' action:artifacts subaction:head path:'.cache/axon-mcp/help-actions.json' limit:10 --output json | jq -e '.ok == true and .action == \"head\" and .subaction == \"head\"' >/dev/null"
run_pipe_case action_artifacts_wc "mcporter call '$SELECTOR' action:artifacts subaction:wc path:'.cache/axon-mcp/help-actions.json' --output json | jq -e '.ok == true and .action == \"wc\" and .subaction == \"wc\"' >/dev/null"
run_pipe_case action_artifacts_read "mcporter call '$SELECTOR' action:artifacts subaction:read path:'.cache/axon-mcp/help-actions.json' limit:20 offset:0 --output json | jq -e '.ok == true and .action == \"read\" and .subaction == \"read\"' >/dev/null"
run_pipe_case action_artifacts_grep "mcporter call '$SELECTOR' action:artifacts subaction:grep path:'.cache/axon-mcp/help-actions.json' pattern:'action' limit:10 offset:0 --output json | jq -e '.ok == true and .action == \"grep\" and .subaction == \"grep\"' >/dev/null"

if [[ "$FULL" -eq 1 ]]; then
  echo "== Full/side-effect checks ==" | tee -a "$SUMMARY"
  run_pipe_case action_search "mcporter call '$SELECTOR' action:search query:'rust programming language' limit:3 offset:0 --output json | jq -e '.ok == true and .action == \"search\"' >/dev/null"
  run_pipe_case action_research "mcporter call '$SELECTOR' action:research query:'rust async best practices' limit:3 offset:0 --output json | jq -e '.ok == true and .action == \"research\"' >/dev/null"
  run_pipe_case action_ask "mcporter call '$SELECTOR' action:ask query:'What is this repository?' --output json | jq -e '.ok == true and .action == \"ask\"' >/dev/null"
  run_pipe_case action_screenshot "mcporter call '$SELECTOR' action:screenshot url:'https://example.com' --output json | jq -e '.ok == true and .action == \"screenshot\"' >/dev/null"
fi

echo "" | tee -a "$SUMMARY"
echo "Results: PASS=$pass FAIL=$fail" | tee -a "$SUMMARY"
echo "Summary: $SUMMARY"

if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
