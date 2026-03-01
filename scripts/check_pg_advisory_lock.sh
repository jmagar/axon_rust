#!/usr/bin/env bash
# check_pg_advisory_lock.sh
# Blocks commits that introduce session-scoped pg_advisory_lock / pg_advisory_unlock
# calls in Rust source. Mirrors the CI advisory-lock-policy job so the violation is
# caught before push rather than after.
#
# Allowed alternative: pg_advisory_xact_lock via begin_schema_migration_tx (transaction-
# scoped — released automatically on commit/rollback, no leaked lock risk).

set -euo pipefail

VIOLATIONS=$(git diff --cached -- '*.rs' 2>/dev/null \
    | grep '^+' \
    | grep -v '^+++' \
    | grep -E 'pg_advisory_lock[[:space:]]*\(|pg_advisory_unlock[[:space:]]*\(' \
    || true)

if [[ -n "$VIOLATIONS" ]]; then
    echo "[pg-lock-ban] BLOCKED — session-scoped advisory lock in staged diff:"
    echo "$VIOLATIONS" | sed 's/^+/  +/'
    echo ""
    echo "[pg-lock-ban] Use pg_advisory_xact_lock via begin_schema_migration_tx instead."
    echo "[pg-lock-ban] Session locks leak if the connection is pooled or the process crashes."
    exit 1
fi

exit 0
