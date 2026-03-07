#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
DOCKERIGNORE="$ROOT_DIR/.dockerignore"

if [[ ! -f "$DOCKERIGNORE" ]]; then
  echo "ERROR: .dockerignore not found at $DOCKERIGNORE"
  exit 1
fi

required_patterns=(
  "/.claude"
  "/.codex"
  "/.gemini"
  "/target"
  "/.cache"
  "/.git"
)

missing=()
for pattern in "${required_patterns[@]}"; do
  # Use grep -xF (exact line match, fixed strings) to avoid regex metacharacter
  # false positives — e.g. '/.claude' as a regex would match '/xclaude'.
  if ! grep -qxF "$pattern" "$DOCKERIGNORE"; then
    missing+=("$pattern")
  fi
done

if [[ ${#missing[@]} -gt 0 ]]; then
  echo "ERROR: .dockerignore is missing required context guard entries:"
  for m in "${missing[@]}"; do
    echo "  - $m"
  done
  echo
  echo "Add the missing entries to prevent massive Docker build contexts."
  exit 1
fi

echo ".dockerignore guard check passed."
