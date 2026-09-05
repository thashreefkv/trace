// Brain layout cache. Persists computed node positions (x,y,z) per layout mode
// in SQLite so revisits paint in < 100ms without re-running force / ELK / UMAP.
//
// Versioning: every cache entry is keyed on (mode, graph_version), where
// graph_version is a sha1 over the sorted set of (entity_kind:entity_id). When
// the brain rebuilds (Kuzu repopulated), we wipe the cache outright — far
// simpler than trying to diff and update.

use sqlx::{Row, SqlitePool};

use crate::models::{BrainLayoutPoint, BrainLayoutResult, NodeEmbedding};

pub async fn read_brain_layout(
    pool: &SqlitePool,
    mode: &str,
    graph_version: &str,
) -> Result<Option<BrainLayoutResult>, String> {
    let rows = sqlx::query(
        "SELECT entity_kind, entity_id, x, y, z, computed_at \
         FROM brain_layout_cache \
         WHERE mode = ?1 AND graph_version = ?2",
    )
    .bind(mode)
    .bind(graph_version)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("read_brain_layout: {e}"))?;

    if rows.is_empty() {
        return Ok(None);
    }
    let mut points = Vec::with_capacity(rows.len());
    let mut latest_at: i64 = 0;
    for row in rows {
        let entity_kind: String = row.try_get("entity_kind").unwrap_or_default();
        let entity_id: String = row.try_get("entity_id").unwrap_or_default();
        let x: f64 = row.try_get("x").unwrap_or(0.0);
        let y: f64 = row.try_get("y").unwrap_or(0.0);
        let z: Option<f64> = row.try_get("z").ok();
        let computed_at: i64 = row.try_get("computed_at").unwrap_or(0);
        if computed_at > latest_at {
            latest_at = computed_at;
        }
        points.push(BrainLayoutPoint {
            entity_kind,
            entity_id,
            x,
            y,
            z,
        });
    }
    Ok(Some(BrainLayoutResult {
        mode: mode.to_string(),
        graph_version: graph_version.to_string(),
        computed_at: latest_at,
        points,
    }))
}

pub async fn write_brain_layout(
    pool: &SqlitePool,
    mode: &str,
    graph_version: &str,
    points: &[BrainLayoutPoint],
) -> Result<(), String> {
    let now = chrono::Utc::now().timestamp();
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| format!("write_brain_layout begin: {e}"))?;
    // Replace any older entries for this mode so we never carry stale rows
    // from a previous graph_version.
    sqlx::query("DELETE FROM brain_layout_cache WHERE mode = ?1")
        .bind(mode)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("write_brain_layout clear: {e}"))?;
    for p in points {
        sqlx::query(
            "INSERT OR REPLACE INTO brain_layout_cache \
             (mode, graph_version, entity_kind, entity_id, x, y, z, computed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(mode)
        .bind(graph_version)
        .bind(&p.entity_kind)
        .bind(&p.entity_id)
        .bind(p.x)
        .bind(p.y)
        .bind(p.z)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("write_brain_layout insert: {e}"))?;
    }
    tx.commit()
        .await
        .map_err(|e| format!("write_brain_layout commit: {e}"))
}

pub async fn invalidate_brain_layouts(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query("DELETE FROM brain_layout_cache")
        .execute(pool)
        .await
        .map(|_| ())
        .map_err(|e| format!("invalidate_brain_layouts: {e}"))
}

/// Read embeddings for the given (entity_kind, entity_id) pairs from
/// `entity_embeddings`. Used by the UMAP web worker to compute semantic
/// projections.
pub async fn get_node_embeddings(
    pool: &SqlitePool,
    ids: &[(String, String)],
) -> Result<Vec<NodeEmbedding>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::with_capacity(ids.len());
    for (entity_kind, entity_id) in ids {
        // Look up vector_json for each entity. Schema (from migration 0042):
        //   entity_embeddings(entity_kind, entity_id, vector_json TEXT, ...)
        // We tolerate missing rows by skipping them; the worker handles the
        // gaps by leaving those nodes at their current positions.
        let row = sqlx::query(
            "SELECT vector_json FROM entity_embeddings \
             WHERE entity_kind = ?1 AND entity_id = ?2 LIMIT 1",
        )
        .bind(entity_kind)
        .bind(entity_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("get_node_embeddings: {e}"))?;
        let Some(row) = row else { continue };
        let vector_json: String = row.try_get("vector_json").unwrap_or_default();
        let vector: Vec<f32> = serde_json::from_str(&vector_json).unwrap_or_default();
        if vector.is_empty() {
            continue;
        }
        result.push(NodeEmbedding {
            entity_kind: entity_kind.clone(),
            entity_id: entity_id.clone(),
            vector,
        });
    }
    Ok(result)
}
