//! Bounded evidence materialization shared by traversal and detail reads.
use super::*;
use crate::sqlite::row::evidence_from_row;
use sqlx::Row as _;
use std::collections::BTreeMap;

const MAX_ROWS: usize = 1_000;
const MAX_STORED_BYTES: usize = 1024 * 1024;
const MAX_WIRE_BYTES: usize = 2 * 1024 * 1024;
const ROW_BYTES: &str = "512 + length(CAST(evidence_id AS BLOB)) + length(CAST(edge_id AS BLOB))
    + length(CAST(evidence_kind AS BLOB)) + length(CAST(source_id AS BLOB))
    + length(CAST(source_item_key AS BLOB)) + COALESCE(length(CAST(document_id AS BLOB)), 0)
    + COALESCE(length(CAST(chunk_id AS BLOB)), 0) + COALESCE(length(CAST(range_json AS BLOB)), 0)
    + COALESCE(length(CAST(quote AS BLOB)), 0) + length(CAST(metadata_json AS BLOB))";

fn limit_error() -> axon_api::source::ApiError {
    axon_api::source::ApiError::new(
        "graph.evidence_limit_exceeded",
        axon_api::source::ErrorStage::Retrieving,
        format!(
            "graph evidence exceeds {MAX_ROWS} records, {MAX_STORED_BYTES} charged storage bytes, or {MAX_WIRE_BYTES} serialized bytes"
        ),
    )
}

pub(in crate::sqlite) async fn attach_evidence(
    pool: &SqlitePool,
    edges: &mut [GraphEdge],
) -> StoreResult<()> {
    let ids = edges
        .iter()
        .map(|edge| edge.edge_id.0.as_str())
        .collect::<Vec<_>>();
    let mut evidence = load_bounded(pool, &ids).await?;
    for edge in edges {
        edge.evidence = evidence.remove(&edge.edge_id.0).unwrap_or_default();
    }
    Ok(())
}

pub(in crate::sqlite) async fn evidence_for_edge(
    pool: &SqlitePool,
    edge: &str,
) -> StoreResult<Vec<GraphEvidence>> {
    Ok(load_bounded(pool, &[edge])
        .await?
        .remove(edge)
        .unwrap_or_default())
}

async fn load_bounded(
    pool: &SqlitePool,
    edges: &[&str],
) -> StoreResult<BTreeMap<String, Vec<GraphEvidence>>> {
    if edges.is_empty() {
        return Ok(BTreeMap::new());
    }
    // A read snapshot prevents evidence growth between sizing and loading from
    // bypassing admission. WAL writers remain independent of this reader.
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| graph_storage_error(e.to_string()))?;
    let mut row_ids = Vec::new();
    let mut bytes = 0usize;
    for batch in edges.chunks(900) {
        let mut builder = sqlx::QueryBuilder::new(format!(
            "SELECT rowid, {ROW_BYTES} AS bytes FROM graph_evidence WHERE edge_id IN ("
        ));
        let mut separated = builder.separated(", ");
        for edge in batch {
            separated.push_bind(*edge);
        }
        separated.push_unseparated(") ORDER BY edge_id, evidence_id LIMIT ");
        builder.push_bind((MAX_ROWS + 1 - row_ids.len()) as i64);
        // Only integer sizes and rowids cross the SQLite boundary during
        // admission, including for a single arbitrarily large quote/metadata.
        let rows = builder
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| graph_storage_error(e.to_string()))?;
        for row in rows {
            let size = usize::try_from(row.get::<i64, _>("bytes")).map_err(|_| limit_error())?;
            bytes = bytes.checked_add(size).ok_or_else(limit_error)?;
            if row_ids.len() == MAX_ROWS || bytes > MAX_STORED_BYTES {
                return Err(limit_error());
            }
            row_ids.push(row.get::<i64, _>("rowid"));
        }
    }
    let mut result = BTreeMap::<String, Vec<GraphEvidence>>::new();
    let mut wire = WireBudget(0);
    for batch in row_ids.chunks(64) {
        let mut builder = sqlx::QueryBuilder::new("SELECT * FROM graph_evidence WHERE rowid IN (");
        let mut separated = builder.separated(", ");
        for id in batch {
            separated.push_bind(*id);
        }
        separated.push_unseparated(") ORDER BY edge_id, evidence_id");
        let rows = builder
            .build()
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| graph_storage_error(e.to_string()))?;
        for row in rows {
            let evidence = evidence_from_row(&row)?;
            serde_json::to_writer(&mut wire, &evidence).map_err(|_| limit_error())?;
            result.entry(row.get("edge_id")).or_default().push(evidence);
        }
    }
    for items in result.values_mut() {
        items.sort_by(|a, b| a.evidence_id.cmp(&b.evidence_id));
    }
    tx.commit()
        .await
        .map_err(|e| graph_storage_error(e.to_string()))?;
    Ok(result)
}

struct WireBudget(usize);
impl std::io::Write for WireBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(bytes.len());
        if self.0 > MAX_WIRE_BYTES {
            return Err(std::io::Error::other("graph evidence wire budget exceeded"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
