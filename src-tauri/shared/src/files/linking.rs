//! File/folder <-> entity linking. Extracted from legacy.rs (13-std5).

use sqlx::{Row, SqlitePool};

use crate::db::sql_error;
use crate::models::*;
use super::legacy::*;
use super::folders::list_files_in_folder;
pub async fn link_file(
    pool: &SqlitePool,
    file_id: &str,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<(), String> {
    let ts = now();
    sqlx::query(
        "INSERT OR IGNORE INTO file_links (file_id, entity_kind, entity_id, linked_at, source) \
         VALUES (?, ?, ?, ?, 'manual')",
    )
    .bind(file_id)
    .bind(entity_kind.as_str())
    .bind(entity_id)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn unlink_file(
    pool: &SqlitePool,
    file_id: &str,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<(), String> {
    sqlx::query("DELETE FROM file_links WHERE file_id = ? AND entity_kind = ? AND entity_id = ?")
        .bind(file_id)
        .bind(entity_kind.as_str())
        .bind(entity_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn link_folder_files_to_entity(
    pool: &SqlitePool,
    folder_id: &str,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<u32, String> {
    let files = list_files_in_folder(pool, Some(folder_id)).await?;
    let mut count = 0u32;
    for file in &files {
        link_file(pool, &file.id, entity_kind, entity_id).await?;
        count += 1;
    }
    Ok(count)
}

pub async fn files_for_entity(
    pool: &SqlitePool,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<Vec<FileRow>, String> {
    let rows = sqlx::query(
        "SELECT f.* FROM files f \
         JOIN file_links l ON l.file_id = f.id \
         WHERE l.entity_kind = ? AND l.entity_id = ? \
         ORDER BY f.name COLLATE NOCASE",
    )
    .bind(entity_kind.as_str())
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let mut files: Vec<FileRow> = rows.into_iter().map(row_to_file).collect();
    hydrate_links(pool, &mut files).await?;
    Ok(files)
}

// ---- Folder-entity links ----

pub async fn link_folder(
    pool: &SqlitePool,
    folder_id: &str,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<(), String> {
    let ts = now();
    sqlx::query(
        "INSERT OR IGNORE INTO folder_entity_links (folder_id, entity_kind, entity_id, linked_at) \
         VALUES (?, ?, ?, ?)",
    )
    .bind(folder_id)
    .bind(entity_kind.as_str())
    .bind(entity_id)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn unlink_folder(
    pool: &SqlitePool,
    folder_id: &str,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<(), String> {
    sqlx::query(
        "DELETE FROM folder_entity_links WHERE folder_id = ? AND entity_kind = ? AND entity_id = ?",
    )
    .bind(folder_id)
    .bind(entity_kind.as_str())
    .bind(entity_id)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn folder_links(pool: &SqlitePool, folder_id: &str) -> Result<Vec<FileLinkRef>, String> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT entity_kind, entity_id, linked_at FROM folder_entity_links WHERE folder_id = ?",
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    Ok(rows
        .into_iter()
        .map(|(entity_kind, entity_id, linked_at)| FileLinkRef {
            entity_kind,
            entity_id,
            linked_at,
        })
        .collect())
}

pub async fn folders_for_entity(
    pool: &SqlitePool,
    entity_kind: FileEntityKind,
    entity_id: &str,
) -> Result<Vec<TraceFolderWithLinks>, String> {
    let rows = sqlx::query(
        "SELECT tf.id, tf.parent_id, tf.name, tf.created_at, tf.updated_at, \
                COUNT(f.id) AS file_count \
         FROM trace_folders tf \
         JOIN folder_entity_links fel ON fel.folder_id = tf.id \
         LEFT JOIN files f ON f.trace_folder_id = tf.id \
         WHERE fel.entity_kind = ? AND fel.entity_id = ? \
         GROUP BY tf.id \
         ORDER BY tf.name COLLATE NOCASE",
    )
    .bind(entity_kind.as_str())
    .bind(entity_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    let mut result: Vec<TraceFolderWithLinks> = rows
        .into_iter()
        .map(|row| TraceFolderWithLinks {
            id: row.get("id"),
            parent_id: row.get("parent_id"),
            name: row.get("name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            file_count: row.get("file_count"),
            links: Vec::new(),
        })
        .collect();

    for folder in &mut result {
        folder.links = folder_links(pool, &folder.id).await?;
    }

    Ok(result)
}

// ---- Watcher helpers ----

/// Called by the filesystem watcher when a local file path no longer exists.
pub async fn mark_file_missing(pool: &SqlitePool, path: &str) -> Result<(), String> {
    let ts = now();
    sqlx::query(
        "UPDATE files SET is_missing = 1, updated_at = ? WHERE local_path = ? AND kind = 'local'",
    )
    .bind(&ts)
    .bind(path)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

/// Called by the filesystem watcher when a renamed/moved file is detected (inode match).
pub async fn update_file_path(
    pool: &SqlitePool,
    old_path: &str,
    new_path: &str,
) -> Result<(), String> {
    let ts = now();
    sqlx::query(
        "UPDATE files SET local_path = ?, is_missing = 0, updated_at = ? WHERE local_path = ? AND kind = 'local'",
    )
    .bind(new_path)
    .bind(&ts)
    .bind(old_path)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

/// Returns all tracked local file paths.
pub async fn all_local_paths(pool: &SqlitePool) -> Result<Vec<(String, String)>, String> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, local_path FROM files WHERE kind = 'local' AND local_path IS NOT NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    Ok(rows)
}
