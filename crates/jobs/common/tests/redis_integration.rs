use super::super::*;
use redis::AsyncCommands;
use std::error::Error;
use uuid::Uuid;

// ── T-REDIS-1: basic SET / GET / DEL round-trip ─────────────────────────────

/// Set a cancel key, read it back, then delete it — all via the test Redis instance.
/// Validates that the Redis client, connection, and cancel-key format all work end-to-end.
#[tokio::test]
async fn redis_set_get_delete_cancel_key_roundtrip() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = resolve_test_redis_url() else {
        return Ok(());
    };
    let id = Uuid::new_v4();
    let key = format!("axon:crawl:cancel:{id}");

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    // Key must not exist before the test.
    let before: Option<String> = conn.get(&key).await?;
    assert!(before.is_none(), "cancel key must not pre-exist");

    // Write the cancel marker.
    conn.set::<_, _, ()>(&key, "1").await?;

    // Read it back — must be present with the expected value.
    let after: Option<String> = conn.get(&key).await?;
    assert_eq!(
        after.as_deref(),
        Some("1"),
        "cancel key must be readable after SET"
    );

    // Delete and confirm gone.
    conn.del::<_, ()>(&key).await?;
    let gone: Option<String> = conn.get(&key).await?;
    assert!(gone.is_none(), "cancel key must be absent after DEL");

    Ok(())
}

// ── T-REDIS-2: TTL / SETEX semantics ────────────────────────────────────────

/// Keys written with SET EX expire after their TTL and are no longer visible.
/// The crawl worker writes cancel keys with a 60-second TTL via `SET EX 60` —
/// this test verifies the same behaviour with a 1-second TTL so the test stays fast.
#[tokio::test]
async fn redis_cancel_key_with_short_expiry_disappears_after_ttl() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = resolve_test_redis_url() else {
        return Ok(());
    };
    let id = Uuid::new_v4();
    let key = format!("axon:crawl:cancel:{id}");

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    // Write with a 1-second TTL.
    conn.set_ex::<_, _, ()>(&key, "1", 1).await?;

    // Immediately readable.
    let present: Option<String> = conn.get(&key).await?;
    assert_eq!(
        present.as_deref(),
        Some("1"),
        "key must be readable before TTL"
    );

    // Wait for expiry.
    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;

    // Must have expired.
    let expired: Option<String> = conn.get(&key).await?;
    assert!(expired.is_none(), "cancel key must expire after TTL");

    Ok(())
}

// ── T-REDIS-3: embed cancel key pattern ─────────────────────────────────────

/// The embed worker uses `axon:embed:cancel:{id}` — a different namespace than
/// the crawl worker's `axon:crawl:cancel:{id}`. Verify that both namespaces
/// coexist without collision for the same job ID.
#[tokio::test]
async fn crawl_and_embed_cancel_keys_are_isolated_namespaces() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = resolve_test_redis_url() else {
        return Ok(());
    };
    let id = Uuid::new_v4();
    let crawl_key = format!("axon:crawl:cancel:{id}");
    let embed_key = format!("axon:embed:cancel:{id}");

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    // Set crawl cancel only.
    conn.set::<_, _, ()>(&crawl_key, "crawl").await?;

    // Embed cancel must not be set.
    let embed_val: Option<String> = conn.get(&embed_key).await?;
    assert!(
        embed_val.is_none(),
        "embed cancel key must be absent when only crawl key is set"
    );

    // Set embed cancel too.
    conn.set::<_, _, ()>(&embed_key, "embed").await?;

    // Both coexist independently.
    let crawl_val: Option<String> = conn.get(&crawl_key).await?;
    let embed_val2: Option<String> = conn.get(&embed_key).await?;
    assert_eq!(
        crawl_val.as_deref(),
        Some("crawl"),
        "crawl key must retain its value"
    );
    assert_eq!(
        embed_val2.as_deref(),
        Some("embed"),
        "embed key must retain its value"
    );

    // Cleanup.
    let _: () = conn.del(&crawl_key).await?;
    let _: () = conn.del(&embed_key).await?;

    Ok(())
}

// ── T-REDIS-4: extract cancel key pattern ───────────────────────────────────

/// The extract worker uses `axon:extract:cancel:{id}`. Verify the key can be
/// written and read, and that it does not interfere with the crawl key.
#[tokio::test]
async fn extract_cancel_key_format_is_independently_addressable() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = resolve_test_redis_url() else {
        return Ok(());
    };
    let id = Uuid::new_v4();
    let extract_key = format!("axon:extract:cancel:{id}");

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    conn.set::<_, _, ()>(&extract_key, "1").await?;
    let val: Option<String> = conn.get(&extract_key).await?;
    assert_eq!(
        val.as_deref(),
        Some("1"),
        "extract cancel key must be readable"
    );

    let _: () = conn.del(&extract_key).await?;
    Ok(())
}

// ── T-REDIS-5: multi-key isolation ──────────────────────────────────────────

/// Multiple cancel keys for different job IDs must not alias each other.
/// Writes two keys with distinct values, reads both back independently.
#[tokio::test]
async fn multiple_cancel_keys_for_different_job_ids_do_not_alias() -> Result<(), Box<dyn Error>> {
    let Some(redis_url) = resolve_test_redis_url() else {
        return Ok(());
    };
    let id_a = Uuid::new_v4();
    let id_b = Uuid::new_v4();
    let key_a = format!("axon:crawl:cancel:{id_a}");
    let key_b = format!("axon:crawl:cancel:{id_b}");

    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_multiplexed_async_connection().await?;

    conn.set::<_, _, ()>(&key_a, "job-a").await?;
    conn.set::<_, _, ()>(&key_b, "job-b").await?;

    let val_a: Option<String> = conn.get(&key_a).await?;
    let val_b: Option<String> = conn.get(&key_b).await?;

    let _: () = conn.del(&key_a).await?;
    let _: () = conn.del(&key_b).await?;

    assert_eq!(val_a.as_deref(), Some("job-a"));
    assert_eq!(val_b.as_deref(), Some("job-b"));
    assert_ne!(
        key_a, key_b,
        "distinct UUIDs must produce distinct cancel keys"
    );

    Ok(())
}
