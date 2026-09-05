//! Background linker — connects files and calendar events to initiatives via
//! embedding similarity. Runs periodically; writes to `file_initiatives` and
//! `gcal_event_initiatives`. The scope resolver in `reasoning/scope_resolver.rs`
//! reads from those tables when building the strict allow-list.

use sqlx::SqlitePool;

use crate::db::sql_error;
use crate::entity_embeddings::{cosine, vector_norm};
#[allow(unused_imports)]
use serde_json;

/// Similarity threshold above which an entity is auto-linked to an initiative.
const AUTO_LINK_THRESHOLD: f32 = 0.78;

/// Run one pass of the linker — walk all (initiative, file) and
/// (initiative, event) pairs that have embeddings and insert or update
/// auto-link rows.
pub async fn run_linker(pool: &SqlitePool) -> Result<(usize, usize), String> {
    let file_links = link_files(pool).await?;
    let event_links = link_events(pool).await?;
    Ok((file_links, event_links))
}

/// Long-running task that calls `run_linker` every 30 min.
///
/// Spawn this from the app with `tauri::async_runtime::spawn`; Tauri setup
/// runs before an ambient Tokio runtime exists.
pub async fn run_linker_worker(pool: SqlitePool) {
    // First run after a short warm-up so migrations are settled.
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    loop {
        match run_linker(&pool).await {
            Ok((files, events)) => {
                if files + events > 0 {
                    eprintln!(
                        "[linking] auto-linked {files} file(s) and {events} event(s) to initiatives"
                    );
                }
            }
            Err(error) => eprintln!("[linking] linker pass failed: {error}"),
        }
        tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

async fn link_files(pool: &SqlitePool) -> Result<usize, String> {
    // Guard: table may not exist yet (pre-migration 0058 build).
    if !table_exists(pool, "file_initiatives").await? {
        return Ok(0);
    }

    let initiative_vecs = fetch_initiative_vectors(pool).await?;
    if initiative_vecs.is_empty() {
        return Ok(0);
    }

    // Load file embeddings
    let file_rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT ee.entity_id, ee.vector_json
           FROM entity_embeddings ee
           WHERE ee.entity_kind = 'file'
             AND ee.vector_json IS NOT NULL"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut count = 0usize;
    let now = crate::repo::now_utc();

    for (file_id, file_json) in &file_rows {
        let file_vec = parse_vec(file_json);
        let file_norm = vector_norm(&file_vec);
        for (initiative_id, init_vec, init_norm) in &initiative_vecs {
            let sim = cosine(&file_vec, file_norm, init_vec, *init_norm);
            if sim >= AUTO_LINK_THRESHOLD {
                upsert_file_link(pool, file_id, initiative_id, sim, &now).await?;
                count += 1;
            }
        }
    }
    Ok(count)
}

async fn link_events(pool: &SqlitePool) -> Result<usize, String> {
    if !table_exists(pool, "gcal_event_initiatives").await? {
        return Ok(0);
    }

    let initiative_vecs = fetch_initiative_vectors(pool).await?;
    if initiative_vecs.is_empty() {
        return Ok(0);
    }

    let event_rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT ee.entity_id, ee.vector_json
           FROM entity_embeddings ee
           WHERE ee.entity_kind = 'calendar_event'
             AND ee.vector_json IS NOT NULL"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut count = 0usize;
    let now = crate::repo::now_utc();

    for (event_id, event_json) in &event_rows {
        let event_vec = parse_vec(event_json);
        let event_norm = vector_norm(&event_vec);
        for (initiative_id, init_vec, init_norm) in &initiative_vecs {
            let sim = cosine(&event_vec, event_norm, init_vec, *init_norm);
            if sim >= AUTO_LINK_THRESHOLD {
                upsert_event_link(pool, event_id, initiative_id, sim, &now).await?;
                count += 1;
            }
        }
    }
    Ok(count)
}

async fn fetch_initiative_vectors(
    pool: &SqlitePool,
) -> Result<Vec<(String, Vec<f32>, f32)>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT ee.entity_id, ee.vector_json
           FROM entity_embeddings ee
           WHERE ee.entity_kind = 'initiative'
             AND ee.vector_json IS NOT NULL"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    Ok(rows
        .into_iter()
        .map(|(id, json)| {
            let vec = parse_vec(&json);
            let norm = vector_norm(&vec);
            (id, vec, norm)
        })
        .collect())
}

async fn upsert_file_link(
    pool: &SqlitePool,
    file_id: &str,
    initiative_id: &str,
    confidence: f32,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO file_initiatives (file_id, initiative_id, source, confidence, linked_at)
           VALUES (?, ?, 'auto', ?, ?)
           ON CONFLICT (file_id, initiative_id) DO UPDATE
             SET confidence = excluded.confidence,
                 linked_at  = excluded.linked_at
           WHERE source = 'auto'"#,
    )
    .bind(file_id)
    .bind(initiative_id)
    .bind(confidence)
    .bind(now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

async fn upsert_event_link(
    pool: &SqlitePool,
    event_id: &str,
    initiative_id: &str,
    confidence: f32,
    now: &str,
) -> Result<(), String> {
    sqlx::query(
        r#"INSERT INTO gcal_event_initiatives (event_id, initiative_id, source, confidence, linked_at)
           VALUES (?, ?, 'auto', ?, ?)
           ON CONFLICT (event_id, initiative_id) DO UPDATE
             SET confidence = excluded.confidence,
                 linked_at  = excluded.linked_at
           WHERE source = 'auto'"#,
    )
    .bind(event_id)
    .bind(initiative_id)
    .bind(confidence)
    .bind(now)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

async fn table_exists(pool: &SqlitePool, table: &str) -> Result<bool, String> {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?")
            .bind(table)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;
    Ok(count > 0)
}

fn parse_vec(json: &str) -> Vec<f32> {
    serde_json::from_str::<Vec<f32>>(json).unwrap_or_default()
}
