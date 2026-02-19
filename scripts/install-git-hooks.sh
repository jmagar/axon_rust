#!/usr/bin/env bash
set -euo pipefail

if ! command -v lefthook >/dev/null 2>&1; then
  echo "lefthook is not installed."
  echo "Install with one of:"
  echo "  brew install lefthook"
  echo "  cargo install --locked lefthook"
  exit 1
fi

# Remove legacy custom hooks path so git uses standard .git/hooks again.
git config --unset core.hooksPath >/dev/null 2>&1 || true

lefthook install
echo "Installed lefthook git hooks."
