#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT_DIR"

PROBE_TIMEOUT_SECS="${AXON_CONTEXT_PROBE_TIMEOUT_SECS:-30}"
WORKERS_LIMIT_MB="${AXON_WORKERS_CONTEXT_MAX_MB:-500}"
WEB_LIMIT_MB="${AXON_WEB_CONTEXT_MAX_MB:-100}"

to_bytes() {
  local value="$1"
  value="$(echo "$value" | tr -d '[:space:]')"
  if [[ -z "$value" || "$value" == "0" || "$value" == "0B" ]]; then
    echo 0
    return
  fi

  local num unit mul
  num="$(echo "$value" | sed -E 's/^([0-9]+(\.[0-9]+)?).*/\1/')"
  unit="$(echo "$value" | sed -E 's/^[0-9]+(\.[0-9]+)?([A-Za-z]+)$/\2/' | tr '[:lower:]' '[:upper:]')"

  case "$unit" in
    B) mul=1 ;;
    KB) mul=1024 ;;
    MB) mul=$((1024 * 1024)) ;;
    GB) mul=$((1024 * 1024 * 1024)) ;;
    TB) mul=$((1024 * 1024 * 1024 * 1024)) ;;
    *) mul=1 ;;
  esac

  awk -v n="$num" -v m="$mul" 'BEGIN { printf "%.0f\n", n * m }'
}

fmt_mb() {
  local bytes="$1"
  awk -v b="$bytes" 'BEGIN { printf "%.2fMB", b / 1024 / 1024 }'
}

probe_service() {
  local service="$1"
  local limit_mb="$2"
  local limit_bytes
  limit_bytes=$((limit_mb * 1024 * 1024))

  echo "[context-probe] probing ${service} (limit ${limit_mb}MB, timeout ${PROBE_TIMEOUT_SECS}s)"

  # We only need early build logs for context transfer sizes.
  # timeout exit code 124 is acceptable for this probe.
  set +e
  local out
  out="$(timeout "${PROBE_TIMEOUT_SECS}s" docker compose --progress=plain build "$service" 2>&1)"
  local rc=$?
  set -e
  if [[ $rc -ne 0 && $rc -ne 124 ]]; then
    echo "$out"
    echo "[context-probe] ERROR: docker compose build failed for ${service} (rc=${rc})"
    exit 1
  fi

  local context_lines
  context_lines="$(echo "$out" | rg "transferring context:" || true)"
  if [[ -z "$context_lines" ]]; then
    echo "$out" | rg "load build context|transferring context|context spider" || true
    echo "[context-probe] ERROR: no context transfer lines found for ${service}"
    exit 1
  fi

  local max_bytes=0
  local max_label="0B"
  while IFS= read -r line; do
    local size
    size="$(echo "$line" | sed -E 's/.*transferring context: ([0-9.]+[A-Za-z]+).*/\1/')"
    if [[ -z "$size" || "$size" == "$line" ]]; then
      continue
    fi
    local bytes
    bytes="$(to_bytes "$size")"
    if (( bytes > max_bytes )); then
      max_bytes="$bytes"
      max_label="$size"
    fi
  done <<< "$context_lines"

  echo "[context-probe] ${service} max observed context: ${max_label} ($(fmt_mb "$max_bytes"))"
  echo "$out" | rg "load build context|transferring context:|context spider" | tail -n 20 || true

  if (( max_bytes > limit_bytes )); then
    echo "[context-probe] ERROR: ${service} context exceeded limit (${max_label} > ${limit_mb}MB)"
    exit 1
  fi
}

probe_service "axon-workers" "$WORKERS_LIMIT_MB"
probe_service "axon-web" "$WEB_LIMIT_MB"

echo "[context-probe] context size checks passed."
