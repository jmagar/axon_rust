use super::*;

pub(super) async fn cancel_batch_job(cfg: &Config, id: Uuid) -> Result<bool, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query(&format!(
        "UPDATE axon_batch_jobs SET status='{canceled}',updated_at=NOW(),finished_at=NOW() WHERE id=$1 AND status IN ('{pending}','{running}')",
        canceled = JobStatus::Canceled.as_str(),
        pending = JobStatus::Pending.as_str(),
        running = JobStatus::Running.as_str(),
    ))
    .bind(id)
    .execute(&pool)
    .await?
    .rows_affected();

    let redis_client = redis::Client::open(cfg.redis_url.clone())?;
    let mut conn = redis_client.get_multiplexed_async_connection().await?;
    let key = format!("axon:batch:cancel:{id}");
    let _: () = conn.set_ex(key, "1", 86400).await?;
    Ok(rows > 0)
}

pub(super) async fn cleanup_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let mut total = 0u64;
    loop {
        let deleted = sqlx::query(&format!(
            "DELETE FROM axon_batch_jobs WHERE id IN (
                SELECT id FROM axon_batch_jobs
                WHERE status IN ('{failed}','{canceled}')
                LIMIT 1000
            )",
            failed = JobStatus::Failed.as_str(),
            canceled = JobStatus::Canceled.as_str(),
        ))
        .execute(&pool)
        .await?
        .rows_affected();
        total += deleted;
        if deleted == 0 {
            break;
        }
    }
    Ok(total)
}

pub(super) async fn clear_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let rows = sqlx::query("DELETE FROM axon_batch_jobs")
        .execute(&pool)
        .await?
        .rows_affected();
    let _ = purge_queue_safe(cfg, &cfg.batch_queue).await;
    Ok(rows)
}

pub(super) async fn recover_stale_batch_jobs(cfg: &Config) -> Result<u64, Box<dyn Error>> {
    let pool = make_pool(cfg).await?;
    if SCHEMA_INIT.get().is_none() {
        ensure_schema(&pool).await?;
        let _ = SCHEMA_INIT.set(());
    }
    let stats = reclaim_stale_running_jobs(
        &pool,
        TABLE,
        "batch",
        cfg.watchdog_stale_timeout_secs,
        cfg.watchdog_confirm_secs,
        "manual",
    )
    .await?;
    Ok(stats.reclaimed_jobs)
}

pub(super) async fn batch_doctor(cfg: &Config) -> Result<serde_json::Value, Box<dyn Error>> {
    let pg_ok = make_pool(cfg).await.is_ok();
    let amqp_ok = open_amqp_channel(cfg, &cfg.batch_queue).await.is_ok();
    let redis_ok = redis_healthy(&cfg.redis_url).await;
    Ok(serde_json::json!({
        "postgres_ok": pg_ok,
        "amqp_ok": amqp_ok,
        "redis_ok": redis_ok,
        "all_ok": pg_ok && amqp_ok && redis_ok
    }))
}
