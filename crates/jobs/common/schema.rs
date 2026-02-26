//! Shared schema migration helpers for job tables.

use sqlx::{PgPool, Postgres, Transaction};

const SCHEMA_LOCK_TIMEOUT_MS: i64 = 5_000;
const SCHEMA_STATEMENT_TIMEOUT_MS: i64 = 60_000;

/// Open a migration transaction with session-local timeouts and a transaction-scoped advisory lock.
///
/// - `lock_timeout`: prevents deadlock-like hangs when another migration holds conflicting locks.
/// - `statement_timeout`: bounds DDL execution time under contention.
/// - `pg_advisory_xact_lock`: serializes schema changes and auto-releases on commit/rollback.
pub(crate) async fn begin_schema_migration_tx<'a>(
    pool: &'a PgPool,
    lock_key: i64,
) -> Result<Transaction<'a, Postgres>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout = ($1::bigint || 'ms')::interval")
        .bind(SCHEMA_LOCK_TIMEOUT_MS)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout = ($1::bigint || 'ms')::interval")
        .bind(SCHEMA_STATEMENT_TIMEOUT_MS)
        .execute(&mut *tx)
        .await?;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
