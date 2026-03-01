#!/usr/bin/env bash
# warn_new_unwraps.sh
# Warns (never blocks) when staged Rust files outside test modules introduce
# new .unwrap() or .expect( calls. Keeps the existing 125+ unwrap count from
# silently growing without blocking legitimate test usage.
#
# Heuristic: excludes files whose path contains /test, /tests, _test.rs, tests.rs

set -uo pipefail

# Staged non-test Rust files
STAGED_RUST=$(git diff --cached --name-only --diff-filter=ACMR -- '*.rs' 2>/dev/null \
    | grep -vE '(^|/)tests?(/|\.rs$)|_tests?\.rs$' \
    || true)

if [[ -z "$STAGED_RUST" ]]; then
    exit 0
fi

TOTAL=0
DETAIL=""

while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    count=$(git diff --cached -- "$file" 2>/dev/null \
        | grep '^+' \
        | grep -v '^+++' \
        | grep -cE '\.unwrap\(\)|\.expect\(' \
        || true)
    if [[ "${count:-0}" -gt 0 ]]; then
        TOTAL=$((TOTAL + count))
        DETAIL="${DETAIL}  +${count}  ${file}\n"
    fi
done <<< "$STAGED_RUST"

if [[ "$TOTAL" -gt 0 ]]; then
    echo ""
    echo "[unwrap-warn] WARNING: ${TOTAL} new .unwrap()/.expect() call(s) in staged non-test Rust"
    printf "%b" "$DETAIL"
    echo "[unwrap-warn] Prefer '?' propagation or explicit error handling in production code."
    echo "[unwrap-warn] (Warning only — commit proceeds)"
fi

exit 0
