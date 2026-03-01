#!/usr/bin/env bash
# check_biome_staged.sh
# Runs biome check on staged TypeScript/TSX/JS/JSX/CSS files under apps/web/.
# Closes the pre-commit TypeScript quality gap — biome errors blocked at commit, not CI.

set -euo pipefail

WEB_DIR="$(git rev-parse --show-toplevel)/apps/web"

if [[ ! -d "$WEB_DIR" ]]; then
    exit 0
fi

# Collect staged files under apps/web that biome covers
STAGED=$(git diff --cached --name-only --diff-filter=ACMR 2>/dev/null \
    | grep -E '^apps/web/.*\.(ts|tsx|js|jsx|css)$' \
    | sed 's|^apps/web/||' \
    || true)

if [[ -z "$STAGED" ]]; then
    exit 0
fi

if ! command -v pnpm &>/dev/null; then
    echo "[biome] pnpm not found — skipping TypeScript check"
    exit 0
fi

cd "$WEB_DIR"

if ! pnpm exec biome --version &>/dev/null 2>&1; then
    echo "[biome] biome not installed in apps/web — run: pnpm install"
    exit 0
fi

# Pass files as individual arguments to biome
# shellcheck disable=SC2086
pnpm exec biome check --no-errors-on-unmatched $STAGED
