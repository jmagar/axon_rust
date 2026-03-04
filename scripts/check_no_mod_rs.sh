#!/usr/bin/env bash
set -euo pipefail

ROOT="${1:-.}"
cd "$ROOT"

mapfile -t offenders < <(rg --files | rg '(^|/)mod\.rs$' || true)

if [ "${#offenders[@]}" -gt 0 ]; then
  echo "ERROR: legacy Rust module roots detected (mod.rs is disallowed):"
  printf '  %s\n' "${offenders[@]}"
  echo
  echo "Use modern module style:"
  echo "  foo.rs + foo/*.rs"
  exit 1
fi

echo "OK: no mod.rs files found."
