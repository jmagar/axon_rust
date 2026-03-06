#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
cd "$ROOT_DIR"

MODE="${1:-prune}" # status|prune

# Guardrails (override via env vars if needed).
AXON_TARGET_MAX_GB="${AXON_TARGET_MAX_GB:-30}"
AXON_BUILDKIT_MAX_GB="${AXON_BUILDKIT_MAX_GB:-120}"

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

fmt_gb() {
  local bytes="$1"
  awk -v b="$bytes" 'BEGIN { printf "%.2fGB", b / 1024 / 1024 / 1024 }'
}

target_bytes() {
  if [[ -d target ]]; then
    # du -sb is Linux-specific; macOS uses du -sk (kilobytes)
    if du -sb target &>/dev/null 2>&1; then
      du -sb target | awk '{print $1}'
    else
      du -sk target | awk '{print $1 * 1024}'
    fi
  else
    echo 0
  fi
}

buildkit_bytes() {
  local size
  # Capture docker output without triggering set -e if Docker is unavailable
  size="$(docker system df --format '{{.Type}}|{{.Size}}' 2>/dev/null | awk -F'|' '$1=="Build Cache"{print $2; exit}')" || true
  if [[ -z "${size:-}" ]]; then
    echo 0
    return
  fi
  to_bytes "$size"
}

print_status() {
  local t b
  t="$(target_bytes)"
  b="$(buildkit_bytes)"
  echo "target:   $(fmt_gb "$t") (limit ${AXON_TARGET_MAX_GB}GB)"
  echo "buildkit: $(fmt_gb "$b") (limit ${AXON_BUILDKIT_MAX_GB}GB)"
}

prune_target_if_needed() {
  local limit_bytes current
  limit_bytes="$((AXON_TARGET_MAX_GB * 1024 * 1024 * 1024))"
  current="$(target_bytes)"
  if (( current <= limit_bytes )); then
    return
  fi

  echo "[cache-guard] target exceeds limit ($(fmt_gb "$current") > ${AXON_TARGET_MAX_GB}GB)"
  if [[ -d target/debug/incremental ]]; then
    echo "[cache-guard] removing target/debug/incremental"
    rm -rf target/debug/incremental
  fi
  current="$(target_bytes)"
  if (( current > limit_bytes )); then
    echo "[cache-guard] target still high ($(fmt_gb "$current")); running cargo clean"
    cargo clean
  fi
}

prune_buildkit_if_needed() {
  local limit_bytes current
  limit_bytes="$((AXON_BUILDKIT_MAX_GB * 1024 * 1024 * 1024))"
  current="$(buildkit_bytes)"
  if (( current <= limit_bytes )); then
    return
  fi

  echo "[cache-guard] build cache exceeds limit ($(fmt_gb "$current") > ${AXON_BUILDKIT_MAX_GB}GB)"
  echo "[cache-guard] running docker builder prune -af"
  docker builder prune -af >/dev/null
}

case "$MODE" in
  status)
    print_status
    ;;
  prune)
    echo "[cache-guard] before:"
    print_status
    prune_target_if_needed
    prune_buildkit_if_needed
    echo "[cache-guard] after:"
    print_status
    ;;
  *)
    echo "usage: scripts/cache-guard.sh [status|prune]" >&2
    exit 2
    ;;
esac
