use super::{RefreshPageResult, RefreshTargetState};
use sqlx::PgPool;
use std::collections::HashMap;
use std::error::Error;

pub(crate) async fn load_target_states(
    pool: &PgPool,
    urls: &[String],
) -> Result<HashMap<String, RefreshTargetState>, Box<dyn Error>> {
    if urls.is_empty() {
        return Ok(HashMap::new());
    }

    let mut states = HashMap::new();
    let rows = sqlx::query_as::<_, (String, Option<String>, Option<String>, Option<String>)>(
        "SELECT url,etag,last_modified,content_hash FROM axon_refresh_targets WHERE url = ANY($1)",
    )
    .bind(urls)
    .fetch_all(pool)
    .await?;
    for (url, etag, last_modified, content_hash) in rows {
        states.insert(
            url,
            RefreshTargetState {
                etag,
                last_modified,
                content_hash,
            },
        );
    }
    Ok(states)
}

pub(crate) async fn upsert_target_state(
    pool: &PgPool,
    url: &str,
    result: &RefreshPageResult,
    error_text: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    sqlx::query(
        r#"
        INSERT INTO axon_refresh_targets (
            url, etag, last_modified, content_hash, markdown_chars, last_status, last_checked_at, last_changed_at, error_text
        ) VALUES (
            $1, $2, $3, $4, $5, $6, NOW(), CASE WHEN $7 THEN NOW() ELSE NULL END, $8
        )
        ON CONFLICT (url)
        DO UPDATE SET
            etag = COALESCE(EXCLUDED.etag, axon_refresh_targets.etag),
            last_modified = COALESCE(EXCLUDED.last_modified, axon_refresh_targets.last_modified),
            content_hash = COALESCE(EXCLUDED.content_hash, axon_refresh_targets.content_hash),
            markdown_chars = COALESCE(EXCLUDED.markdown_chars, axon_refresh_targets.markdown_chars),
            last_status = EXCLUDED.last_status,
            last_checked_at = NOW(),
            last_changed_at = CASE
                WHEN $7 THEN NOW()
                ELSE axon_refresh_targets.last_changed_at
            END,
            error_text = EXCLUDED.error_text
        "#,
    )
    .bind(url)
    .bind(result.etag.as_deref())
    .bind(result.last_modified.as_deref())
    .bind(result.content_hash.as_deref())
    .bind(result.markdown_chars.map(|v| v as i32))
    .bind(result.status_code as i32)
    .bind(result.changed)
    .bind(error_text)
    .execute(pool)
    .await?;
    Ok(())
}
