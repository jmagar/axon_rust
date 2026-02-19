#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HASH_FILE="$REPO_ROOT/docs/legacy-file-hashes.txt"

if [[ ! -f "$HASH_FILE" ]]; then
  echo "missing hash file: $HASH_FILE" >&2
  exit 1
fi

failures=0
while IFS= read -r line; do
  [[ -z "$line" || "$line" =~ ^# ]] && continue

  expected="${line%%[[:space:]]*}"
  path="${line#${expected}}"
  path="${path#${path%%[![:space:]]*}}"

  if [[ ! -f "$REPO_ROOT/$path" ]]; then
    echo "missing file: $path" >&2
    failures=$((failures + 1))
    continue
  fi

  actual="$(sha256sum "$REPO_ROOT/$path" | awk '{print $1}')"
  if [[ "$actual" != "$expected" ]]; then
    echo "hash mismatch: $path" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    failures=$((failures + 1))
  fi
done < "$HASH_FILE"

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

echo "legacy file hashes verified"
