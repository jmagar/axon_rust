use super::*;

#[tokio::test]
async fn pending_job_counter_waits_for_existing_writer_before_migration_reads() {
    let directory = tempfile::tempdir().expect("private database directory");
    let path = directory.path().join("jobs.db");
    let pool = open_sqlite_pool(path.to_str().expect("database path"))
        .await
        .expect("initialized database");
    let mut writer = axon_core::sqlite::ImmediateTx::begin(&pool)
        .await
        .expect("independent writer");
    sqlx::query("UPDATE axon_applied_migrations SET applied_at = applied_at")
        .execute(&mut *writer)
        .await
        .expect("hold a write transaction");

    let mut count = Box::pin(count_pending_jobs(&path));
    let early = tokio::time::timeout(std::time::Duration::from_millis(200), count.as_mut()).await;
    writer.commit().await.expect("release independent writer");
    let result = match early {
        Ok(result) => result,
        Err(_) => count.await,
    };
    pool.close().await;
    assert_eq!(
        result.expect("counter survives concurrent migration writer"),
        0
    );
}

#[tokio::test]
async fn pending_job_counter_reads_the_canonical_job_table() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("jobs.db");
    let pool = open_sqlite_pool(path.to_string_lossy().as_ref())
        .await
        .expect("job store");
    sqlx::query(
        "INSERT INTO jobs (job_id, kind, intent, status, phase, attempt, priority, auth_snapshot_json, created_at, updated_at) VALUES (?, 'source', 'run', 'queued', 'queued', 1, 'normal', '{}', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(now_ms())
    .bind(now_ms())
    .execute(&pool)
    .await
    .expect("queued job");
    drop(pool);

    assert_eq!(count_pending_jobs(&path).await.expect("count"), 1);
}

#[tokio::test]
async fn quick_check_reports_clean_database() {
    let pool = SqlitePool::connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite pool");

    assert!(
        quick_check_is_clean(&pool).await.expect("quick_check runs"),
        "a freshly opened SQLite database must report ok"
    );
}

#[test]
fn failed_integrity_probe_does_not_advance_marker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("jobs.db");
    let db_path = db_path.to_string_lossy();

    assert!(should_run_integrity_probe(&db_path));
    assert!(
        !finish_integrity_probe(
            &db_path,
            Err(sqlx::Error::Protocol(
                "injected quick_check failure".to_string()
            ))
        ),
        "an unavailable probe is not evidence of corruption"
    );
    assert!(
        should_run_integrity_probe(&db_path),
        "an unavailable probe must not suppress the next integrity check"
    );
}

#[test]
fn clean_integrity_probe_advances_marker() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("jobs.db");
    let db_path = db_path.to_string_lossy();

    assert!(!finish_integrity_probe(&db_path, Ok(true)));
    assert!(!should_run_integrity_probe(&db_path));
}
