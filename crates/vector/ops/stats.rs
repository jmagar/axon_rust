use crate::crates::core::config::Config;
use crate::crates::core::http::http_client;
use crate::crates::core::ui::{accent, muted, primary, status_text};
use crate::crates::vector::ops::qdrant::qdrant_base;
use sqlx::{postgres::PgPoolOptions, Row};
use std::error::Error;
use std::time::Duration;

async fn pg_pool_for_stats(cfg: &Config) -> Option<sqlx::PgPool> {
    if cfg.pg_url.is_empty() {
        return None;
    }
    tokio::time::timeout(
        Duration::from_secs(3),
        PgPoolOptions::new().max_connections(2).connect(&cfg.pg_url),
    )
    .await
    .ok()
    .and_then(Result::ok)
}

async fn table_exists(pool: &sqlx::PgPool, table: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT to_regclass($1) IS NOT NULL")
        .bind(table)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}

async fn count_table_rows(pool: &sqlx::PgPool, table: &str) -> Result<i64, sqlx::Error> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    sqlx::query_scalar::<_, i64>(&sql).fetch_one(pool).await
}

async fn command_count(pool: &sqlx::PgPool, command: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM axon_command_runs WHERE command = $1")
        .bind(command)
        .fetch_one(pool)
        .await
}

#[derive(Default)]
struct PostgresMetrics {
    crawl_count: Option<i64>,
    batch_count: Option<i64>,
    extract_count: Option<i64>,
    average_pages_per_second: Option<f64>,
    average_crawl_duration_seconds: Option<f64>,
    average_embedding_duration_seconds: Option<f64>,
    average_overall_crawl_duration_seconds: Option<f64>,
    longest_crawl: Option<serde_json::Value>,
    most_chunks: Option<serde_json::Value>,
    total_chunks: Option<i64>,
    total_docs: Option<i64>,
    base_urls_count: Option<i64>,
    scrape_count: Option<i64>,
    query_count: Option<i64>,
    ask_count: Option<i64>,
    retrieve_count: Option<i64>,
    map_count: Option<i64>,
    search_count: Option<i64>,
    embed_count: Option<i64>,
    evaluate_count: Option<i64>,
    suggest_count: Option<i64>,
}

pub async fn run_stats_native(cfg: &Config) -> Result<(), Box<dyn Error>> {
    run_stats_native_impl(cfg).await
}

async fn run_stats_native_impl(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let client = http_client()?;
    let (info, count, docs_count) = fetch_qdrant_snapshots(cfg, client).await?;

    let points_count = count["result"]["count"].as_u64().unwrap_or(0);
    let docs_embedded = docs_count["result"]["count"].as_u64().unwrap_or(0);
    let avg_chunks_per_doc = if docs_embedded > 0 {
        points_count as f64 / docs_embedded as f64
    } else {
        0.0
    };
    let indexed_vectors = info["result"]["indexed_vectors_count"]
        .as_u64()
        .or_else(|| info["result"]["vectors_count"].as_u64());
    let segments_count = info["result"]["segments_count"].as_u64();
    let payload_schema = info["result"]["payload_schema"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let payload_fields: Vec<String> = payload_schema.keys().cloned().collect();
    let payload_fields_count = payload_fields.len();
    let pg = collect_postgres_metrics(cfg).await;

    let stats = serde_json::json!({
        "collection": cfg.collection,
        "status": info["result"]["status"],
        "indexed_vectors_count": indexed_vectors,
        "points_count": points_count,
        "dimension": info["result"]["config"]["params"]["vectors"]["size"],
        "distance": info["result"]["config"]["params"]["vectors"]["distance"],
        "segments_count": segments_count,
        "docs_embedded_estimate": docs_embedded,
        "avg_chunks_per_doc": avg_chunks_per_doc,
        "payload_fields_count": payload_fields_count,
        "payload_fields": payload_fields,
        "avg_pages_crawled_per_second": pg.average_pages_per_second,
        "avg_crawl_duration_seconds": pg.average_crawl_duration_seconds,
        "avg_embedding_duration_seconds": pg.average_embedding_duration_seconds,
        "avg_overall_crawl_duration_seconds": pg.average_overall_crawl_duration_seconds,
        "longest_crawl": pg.longest_crawl,
        "most_chunks": pg.most_chunks,
        "total_chunks": pg.total_chunks,
        "total_docs": pg.total_docs,
        "base_urls_count": pg.base_urls_count,
        "counts": {
            "crawls": pg.crawl_count,
            "embeds": pg.embed_count,
            "scrapes": pg.scrape_count,
            "extracts": pg.extract_count,
            "batches": pg.batch_count,
            "queries": pg.query_count,
            "asks": pg.ask_count,
            "retrieves": pg.retrieve_count,
            "evaluates": pg.evaluate_count,
            "suggests": pg.suggest_count,
            "maps": pg.map_count,
            "searches": pg.search_count
        }
    });
    if cfg.json_output {
        println!("{}", serde_json::to_string_pretty(&stats)?);
    } else {
        print_stats_human(&stats);
    }
    Ok(())
}

fn fmt_count(v: &serde_json::Value) -> String {
    accent(
        &v.as_i64()
            .map(|n| n.to_string())
            .unwrap_or_else(|| "n/a".to_string()),
    )
}

fn print_stats_human(stats: &serde_json::Value) {
    print_vector_stats(stats);
    println!();
    print_pipeline_stats(stats);
    println!();
    print_command_counts(stats);
}

fn print_vector_stats(stats: &serde_json::Value) {
    println!("{}", primary("Vector Stats"));
    println!(
        "  {} {}",
        muted("Collection:"),
        accent(stats["collection"].as_str().unwrap_or("unknown"))
    );
    println!(
        "  {} {}",
        muted("Status:"),
        status_text(stats["status"].as_str().unwrap_or("unknown"))
    );
    println!(
        "  {} {}",
        muted("Indexed Vectors:"),
        fmt_count(&stats["indexed_vectors_count"])
    );
    println!(
        "  {} {}",
        muted("Points:"),
        fmt_count(&stats["points_count"])
    );
    println!(
        "  {} {}",
        muted("Docs (est):"),
        fmt_count(&stats["docs_embedded_estimate"])
    );
    println!(
        "  {} {}",
        muted("Avg Chunks/Doc:"),
        accent(&format!(
            "{:.2}",
            stats["avg_chunks_per_doc"].as_f64().unwrap_or(0.0)
        ))
    );
    println!(
        "  {} {}",
        muted("Dimension:"),
        fmt_count(&stats["dimension"])
    );
    println!(
        "  {} {}",
        muted("Distance:"),
        stats["distance"].as_str().unwrap_or("unknown")
    );
    println!(
        "  {} {}",
        muted("Segments:"),
        fmt_count(&stats["segments_count"])
    );
    println!(
        "  {} {}",
        muted("Payload Fields:"),
        fmt_count(&stats["payload_fields_count"])
    );
    if let Some(rendered) = render_payload_fields(stats) {
        println!("  {} {}", muted("Field Names:"), rendered);
    }
}

fn render_payload_fields(stats: &serde_json::Value) -> Option<String> {
    let rendered = stats["payload_fields"]
        .as_array()
        .map(|fields| {
            fields
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered)
    }
}

fn avg_stat_text(stats: &serde_json::Value, key: &str, suffix: &str) -> String {
    stats[key]
        .as_f64()
        .map(|v| format!("{v:.2}{suffix}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn print_pipeline_stats(stats: &serde_json::Value) {
    println!("{}", primary("Pipeline Stats"));
    let avg_pages = avg_stat_text(stats, "avg_pages_crawled_per_second", "");
    let avg_crawl = avg_stat_text(stats, "avg_crawl_duration_seconds", "s");
    let avg_embed = avg_stat_text(stats, "avg_embedding_duration_seconds", "s");
    let avg_overall = avg_stat_text(stats, "avg_overall_crawl_duration_seconds", "s");
    println!("  {} {}", muted("Avg Pages/sec:"), accent(&avg_pages));
    println!("  {} {}", muted("Avg Crawl Duration:"), accent(&avg_crawl));
    println!(
        "  {} {}",
        muted("Avg Embedding Duration:"),
        accent(&avg_embed)
    );
    println!("  {} {}", muted("Avg Overall Crawl:"), accent(&avg_overall));
    println!(
        "  {} {}",
        muted("Total Chunks:"),
        fmt_count(&stats["total_chunks"])
    );
    println!(
        "  {} {}",
        muted("Total Docs:"),
        fmt_count(&stats["total_docs"])
    );
    println!(
        "  {} {}",
        muted("Base URLs:"),
        fmt_count(&stats["base_urls_count"])
    );
    if let Some(longest) = stats["longest_crawl"].as_object() {
        println!(
            "  {} {} ({:.2}s)",
            muted("Longest Crawl:"),
            accent(longest.get("id").and_then(|v| v.as_str()).unwrap_or("n/a")),
            longest
                .get("seconds")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0)
        );
    }
    if let Some(most) = stats["most_chunks"].as_object() {
        println!(
            "  {} {} ({})",
            muted("Most Chunks:"),
            accent(
                most.get("embed_job_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("n/a")
            ),
            accent(
                &most
                    .get("chunks")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0)
                    .to_string()
            )
        );
    }
}

fn print_command_counts(stats: &serde_json::Value) {
    println!("{}", primary("Command Counts"));
    println!(
        "  {} {}",
        muted("Crawls:"),
        fmt_count(&stats["counts"]["crawls"])
    );
    println!(
        "  {} {}",
        muted("Embeds:"),
        fmt_count(&stats["counts"]["embeds"])
    );
    println!(
        "  {} {}",
        muted("Scrapes:"),
        fmt_count(&stats["counts"]["scrapes"])
    );
    println!(
        "  {} {}",
        muted("Extracts:"),
        fmt_count(&stats["counts"]["extracts"])
    );
    println!(
        "  {} {}",
        muted("Batches:"),
        fmt_count(&stats["counts"]["batches"])
    );
    println!(
        "  {} {}",
        muted("Queries:"),
        fmt_count(&stats["counts"]["queries"])
    );
    println!(
        "  {} {}",
        muted("Asks:"),
        fmt_count(&stats["counts"]["asks"])
    );
    println!(
        "  {} {}",
        muted("Retrieves:"),
        fmt_count(&stats["counts"]["retrieves"])
    );
    println!(
        "  {} {}",
        muted("Evaluates:"),
        fmt_count(&stats["counts"]["evaluates"])
    );
    println!(
        "  {} {}",
        muted("Suggests:"),
        fmt_count(&stats["counts"]["suggests"])
    );
    println!(
        "  {} {}",
        muted("Maps:"),
        fmt_count(&stats["counts"]["maps"])
    );
    println!(
        "  {} {}",
        muted("Searches:"),
        fmt_count(&stats["counts"]["searches"])
    );
}

async fn fetch_qdrant_snapshots(
    cfg: &Config,
    client: &reqwest::Client,
) -> Result<(serde_json::Value, serde_json::Value, serde_json::Value), Box<dyn Error>> {
    let info = client
        .get(format!(
            "{}/collections/{}",
            qdrant_base(cfg),
            cfg.collection
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let count = client
        .post(format!(
            "{}/collections/{}/points/count",
            qdrant_base(cfg),
            cfg.collection
        ))
        .json(&serde_json::json!({"exact": true}))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let docs_count = client
        .post(format!(
            "{}/collections/{}/points/count",
            qdrant_base(cfg),
            cfg.collection
        ))
        .json(&serde_json::json!({
            "exact": true,
            "filter": {"must": [{"key": "chunk_index", "match": { "value": 0 }}]}
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    Ok((info, count, docs_count))
}

async fn collect_postgres_metrics(cfg: &Config) -> PostgresMetrics {
    let mut metrics = PostgresMetrics::default();
    let Some(pool) = pg_pool_for_stats(cfg).await else {
        return metrics;
    };
    if table_exists(&pool, "axon_crawl_jobs")
        .await
        .unwrap_or(false)
    {
        collect_crawl_metrics(&pool, &mut metrics).await;
    }
    if table_exists(&pool, "axon_batch_jobs")
        .await
        .unwrap_or(false)
    {
        collect_batch_metrics(&pool, &mut metrics).await;
    }
    if table_exists(&pool, "axon_extract_jobs")
        .await
        .unwrap_or(false)
    {
        collect_extract_metrics(&pool, &mut metrics).await;
    }
    if table_exists(&pool, "axon_embed_jobs")
        .await
        .unwrap_or(false)
    {
        collect_embed_metrics(&pool, &mut metrics).await;
    }
    if table_exists(&pool, "axon_command_runs")
        .await
        .unwrap_or(false)
    {
        collect_command_metrics(&pool, &mut metrics).await;
    }
    metrics
}

async fn collect_crawl_metrics(pool: &sqlx::PgPool, metrics: &mut PostgresMetrics) {
    metrics.crawl_count = count_table_rows(pool, "axon_crawl_jobs").await.ok();
    metrics.base_urls_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(DISTINCT url) FROM axon_crawl_jobs")
            .fetch_one(pool)
            .await
            .ok();
    metrics.average_pages_per_second = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT AVG(
            COALESCE((result_json->>'pages_discovered')::double precision, 0.0)
            / GREATEST(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision, 0.001::double precision)
        )
        FROM axon_crawl_jobs
        WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    metrics.average_crawl_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT AVG(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision) FROM axon_crawl_jobs WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    if let Ok(Some(row)) = sqlx::query(
        r#"
        SELECT id::text AS id, url, EXTRACT(EPOCH FROM (finished_at - started_at))::double precision AS seconds
        FROM axon_crawl_jobs
        WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL
        ORDER BY (finished_at - started_at) DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    {
        let id: String = row.get("id");
        let url: String = row.get("url");
        let seconds: f64 = row.get("seconds");
        metrics.longest_crawl = Some(serde_json::json!({"id": id, "url": url, "seconds": seconds}));
    }
    metrics.average_overall_crawl_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
        r#"
        SELECT AVG(
            EXTRACT(EPOCH FROM (
                COALESCE(e.finished_at, c.finished_at) - c.started_at
            ))::double precision
        )
        FROM axon_crawl_jobs c
        LEFT JOIN LATERAL (
            SELECT finished_at
            FROM axon_embed_jobs e
            WHERE e.status='completed'
              AND e.input_text LIKE ('%' || c.id::text || '/markdown')
            ORDER BY finished_at DESC
            LIMIT 1
        ) e ON TRUE
        WHERE c.status='completed' AND c.started_at IS NOT NULL AND c.finished_at IS NOT NULL
        "#,
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
}

async fn collect_batch_metrics(pool: &sqlx::PgPool, metrics: &mut PostgresMetrics) {
    metrics.batch_count = count_table_rows(pool, "axon_batch_jobs").await.ok();
}

async fn collect_extract_metrics(pool: &sqlx::PgPool, metrics: &mut PostgresMetrics) {
    metrics.extract_count = count_table_rows(pool, "axon_extract_jobs").await.ok();
}

async fn collect_embed_metrics(pool: &sqlx::PgPool, metrics: &mut PostgresMetrics) {
    metrics.average_embedding_duration_seconds = sqlx::query_scalar::<_, Option<f64>>(
        "SELECT AVG(EXTRACT(EPOCH FROM (finished_at - started_at))::double precision) FROM axon_embed_jobs WHERE status='completed' AND started_at IS NOT NULL AND finished_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    metrics.total_chunks = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT SUM(COALESCE((result_json->>'chunks_embedded')::bigint, 0))::bigint FROM axon_embed_jobs WHERE status='completed'",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    metrics.total_docs = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT SUM(COALESCE((result_json->>'docs_embedded')::bigint, 0))::bigint FROM axon_embed_jobs WHERE status='completed'",
    )
    .fetch_one(pool)
    .await
    .ok()
    .flatten();
    metrics.embed_count = count_table_rows(pool, "axon_embed_jobs").await.ok();
    if let Ok(Some(row)) = sqlx::query(
        r#"
        SELECT id::text AS id,
               COALESCE((result_json->>'chunks_embedded')::bigint, 0) AS chunks
        FROM axon_embed_jobs
        WHERE status='completed'
        ORDER BY COALESCE((result_json->>'chunks_embedded')::bigint, 0) DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await
    {
        let id: String = row.get("id");
        let chunks: i64 = row.get("chunks");
        metrics.most_chunks = Some(serde_json::json!({"embed_job_id": id, "chunks": chunks}));
    }
}

async fn collect_command_metrics(pool: &sqlx::PgPool, metrics: &mut PostgresMetrics) {
    metrics.scrape_count = command_count(pool, "scrape").await.ok();
    metrics.query_count = command_count(pool, "query").await.ok();
    metrics.ask_count = command_count(pool, "ask").await.ok();
    metrics.retrieve_count = command_count(pool, "retrieve").await.ok();
    metrics.map_count = command_count(pool, "map").await.ok();
    metrics.search_count = command_count(pool, "search").await.ok();
    metrics.evaluate_count = command_count(pool, "evaluate").await.ok();
    metrics.suggest_count = command_count(pool, "suggest").await.ok();
}
