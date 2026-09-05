use chrono::{SecondsFormat, Utc};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use ulid::Ulid;

use crate::db::sql_error;
use crate::models::{
    AttachLocalFileInput, CreateTraceFolderInput, FileLinkRef, FileRow, MoveFileInput,
    RenameFileInput, TraceFolder,
};

use super::folders::create_folder;

pub(crate) fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub(crate) const SUPPORTED_EMBEDDING_DRIVE_MIME: &[&str] = &[
    "application/vnd.google-apps.document",
    "application/vnd.google-apps.spreadsheet",
    "application/vnd.google-apps.presentation",
];

// ---- Folders ----


pub fn row_to_file_public(row: sqlx::sqlite::SqliteRow) -> FileRow {
    row_to_file(row)
}

pub(crate) fn row_to_file(row: sqlx::sqlite::SqliteRow) -> FileRow {
    let local_path: Option<String> = row.try_get("local_path").ok();
    let kind: String = row.try_get("kind").unwrap_or_else(|_| "local".into());
    let is_missing: bool = row.try_get::<i64, _>("is_missing").unwrap_or(0) != 0;
    FileRow {
        id: row.try_get("id").unwrap_or_default(),
        kind,
        trace_folder_id: row.try_get("trace_folder_id").ok(),
        name: row.try_get("name").unwrap_or_default(),
        mime_type: row.try_get("mime_type").ok(),
        size_bytes: row.try_get("size_bytes").ok(),
        description: row.try_get("description").ok(),
        local_path,
        is_missing,
        drive_file_id: row.try_get("drive_file_id").ok(),
        drive_account_id: row.try_get("drive_account_id").ok(),
        drive_parent_id: row.try_get("drive_parent_id").ok(),
        drive_mime: row.try_get("drive_mime").ok(),
        drive_web_view_link: row.try_get("drive_web_view_link").ok(),
        drive_trashed: row.try_get::<i64, _>("drive_trashed").unwrap_or(0) != 0,
        created_at: row.try_get("created_at").unwrap_or_default(),
        updated_at: row.try_get("updated_at").unwrap_or_default(),
        links: Vec::new(),
    }
}

pub(crate) async fn hydrate_links(pool: &SqlitePool, files: &mut [FileRow]) -> Result<(), String> {
    if files.is_empty() {
        return Ok(());
    }
    for file in files.iter_mut() {
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT entity_kind, entity_id, linked_at FROM file_links WHERE file_id = ?",
        )
        .bind(&file.id)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;
        file.links = rows
            .into_iter()
            .map(|(entity_kind, entity_id, linked_at)| FileLinkRef {
                entity_kind,
                entity_id,
                linked_at,
            })
            .collect();
    }
    Ok(())
}

pub async fn attach_local_directory(
    pool: &SqlitePool,
    path: &str,
    parent_trace_folder_id: Option<&str>,
) -> Result<TraceFolder, String> {
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path '{path}': {e}"))?;
    let metadata = std::fs::metadata(&canonical)
        .map_err(|e| format!("Cannot stat '{}': {e}", canonical.display()))?;
    if !metadata.is_dir() {
        return Err("Path is not a directory".into());
    }

    let dir_name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Folder".into());

    let folder = create_folder(
        pool,
        CreateTraceFolderInput {
            parent_id: parent_trace_folder_id.map(String::from),
            name: dir_name,
        },
    )
    .await?;

    // Attach all children recursively.
    let Ok(entries) = std::fs::read_dir(&canonical) else {
        return Ok(folder);
    };
    for entry in entries.flatten() {
        let entry_path = entry.path();
        let Ok(entry_meta) = std::fs::metadata(&entry_path) else {
            continue;
        };
        if entry_meta.is_dir() {
            let sub_path = entry_path.to_string_lossy().to_string();
            let _ = Box::pin(attach_local_directory(pool, &sub_path, Some(&folder.id))).await;
        } else if entry_meta.is_file() {
            let _ = attach_local_file(
                pool,
                AttachLocalFileInput {
                    path: entry_path.to_string_lossy().to_string(),
                    trace_folder_id: Some(folder.id.clone()),
                },
            )
            .await;
        }
    }
    Ok(folder)
}

pub async fn attach_local_file(
    pool: &SqlitePool,
    input: AttachLocalFileInput,
) -> Result<FileRow, String> {
    let path = Path::new(&input.path);
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("Cannot resolve path '{}': {e}", input.path))?;
    let canonical_str = canonical.to_string_lossy().to_string();

    let metadata = std::fs::metadata(&canonical)
        .map_err(|e| format!("Cannot stat '{}': {e}", canonical_str))?;
    if !metadata.is_file() {
        return Err(
            "Only regular files can be attached. Use 'attach_local_directory' for folders.".into(),
        );
    }
    let size_bytes = metadata.len() as i64;
    let name = canonical
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".into());
    let mime_type = mime_guess::from_path(&canonical)
        .first()
        .map(|m| m.essence_str().to_string());

    if let Some(existing_id) =
        sqlx::query_scalar::<_, String>("SELECT id FROM files WHERE local_path = ?")
            .bind(&canonical_str)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?
    {
        return get_file(pool, &existing_id).await;
    }

    let id = Ulid::new().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO files \
          (id, kind, trace_folder_id, name, mime_type, size_bytes, local_path, is_missing, created_at, updated_at) \
         VALUES (?, 'local', ?, ?, ?, ?, ?, 0, ?, ?)",
    )
    .bind(&id)
    .bind(&input.trace_folder_id)
    .bind(&name)
    .bind(&mime_type)
    .bind(size_bytes)
    .bind(&canonical_str)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    get_file(pool, &id).await
}

pub async fn get_file(pool: &SqlitePool, id: &str) -> Result<FileRow, String> {
    let row = sqlx::query("SELECT * FROM files WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?
        .ok_or_else(|| "File not found".to_string())?;
    let mut file = row_to_file(row);
    let mut files = vec![file];
    hydrate_links(pool, &mut files).await?;
    file = files.remove(0);
    Ok(file)
}

pub async fn move_file(pool: &SqlitePool, input: MoveFileInput) -> Result<(), String> {
    let ts = now();
    sqlx::query("UPDATE files SET trace_folder_id = ?, updated_at = ? WHERE id = ?")
        .bind(&input.new_trace_folder_id)
        .bind(&ts)
        .bind(&input.file_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn rename_file(pool: &SqlitePool, input: RenameFileInput) -> Result<(), String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("File name cannot be empty".into());
    }
    let ts = now();
    sqlx::query("UPDATE files SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(&ts)
        .bind(&input.file_id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn delete_file(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM files WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

// ---- Links ----


pub async fn search_files(
    pool: &SqlitePool,
    query: &str,
    limit: i64,
) -> Result<Vec<FileRow>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "SELECT f.* FROM files f \
         JOIN file_search fs ON fs.id = f.id \
         WHERE file_search MATCH ? \
         ORDER BY rank LIMIT ?",
    )
    .bind(q)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    let mut files: Vec<FileRow> = rows.into_iter().map(row_to_file).collect();
    hydrate_links(pool, &mut files).await?;
    Ok(files)
}

// ---- Entity folder resolution ----

/// Walk the entity hierarchy and find-or-create the canonical Trace folder for
/// a given entity.  The hierarchy is:
///
///   initiative        →  {initiative.title}/
///   deliverable       →  {initiative.title}/{deliverable.title}/
///   deliverable_task  →  {initiative.title}/{deliverable.title}/{task.title}/
///   stakeholder       →  {stakeholder.name}/
///
/// For any other entity kind this returns an error so callers can fall back to
/// placing files at the root.
pub async fn ensure_entity_folder(
    pool: &SqlitePool,
    entity_kind: &str,
    entity_id: &str,
) -> Result<TraceFolder, String> {
    match entity_kind {
        "initiative" => {
            let title: String = sqlx::query_scalar("SELECT title FROM initiatives WHERE id = ?")
                .bind(entity_id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?
                .ok_or_else(|| format!("Initiative not found: {entity_id}"))?;
            find_or_create_folder(pool, None, &title).await
        }
        "deliverable" => {
            let row = sqlx::query(
                "SELECT d.title AS d_title, i.title AS i_title \
                 FROM deliverables d \
                 LEFT JOIN deliverable_initiatives di ON di.deliverable_id = d.id \
                 LEFT JOIN initiatives i ON i.id = di.initiative_id \
                 WHERE d.id = ? LIMIT 1",
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?
            .ok_or_else(|| format!("Deliverable not found: {entity_id}"))?;

            let d_title: String = row.get("d_title");
            let i_title: Option<String> = row.get("i_title");

            if let Some(it) = i_title {
                let init_folder = find_or_create_folder(pool, None, &it).await?;
                find_or_create_folder(pool, Some(&init_folder.id), &d_title).await
            } else {
                find_or_create_folder(pool, None, &d_title).await
            }
        }
        "deliverable_task" => {
            let row = sqlx::query(
                "SELECT t.title AS t_title, d.title AS d_title, i.title AS i_title \
                 FROM deliverable_tasks t \
                 JOIN deliverables d ON d.id = t.deliverable_id \
                 LEFT JOIN deliverable_initiatives di ON di.deliverable_id = d.id \
                 LEFT JOIN initiatives i ON i.id = di.initiative_id \
                 WHERE t.id = ? LIMIT 1",
            )
            .bind(entity_id)
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?
            .ok_or_else(|| format!("Task not found: {entity_id}"))?;

            let t_title: String = row.get("t_title");
            let d_title: String = row.get("d_title");
            let i_title: Option<String> = row.get("i_title");

            let deliv_folder = if let Some(it) = i_title {
                let init_folder = find_or_create_folder(pool, None, &it).await?;
                find_or_create_folder(pool, Some(&init_folder.id), &d_title).await?
            } else {
                find_or_create_folder(pool, None, &d_title).await?
            };
            find_or_create_folder(pool, Some(&deliv_folder.id), &t_title).await
        }
        "stakeholder" => {
            let name: String = sqlx::query_scalar("SELECT name FROM stakeholders WHERE id = ?")
                .bind(entity_id)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?
                .ok_or_else(|| format!("Stakeholder not found: {entity_id}"))?;
            find_or_create_folder(pool, None, &name).await
        }
        other => Err(format!("No folder hierarchy for entity kind: {other}")),
    }
}

/// Find an existing folder by (parent_id, name) or create it with INSERT OR IGNORE.
/// Safe to call concurrently — the UNIQUE constraint guarantees idempotency.
async fn find_or_create_folder(
    pool: &SqlitePool,
    parent_id: Option<&str>,
    name: &str,
) -> Result<TraceFolder, String> {
    let name = {
        let trimmed = name.trim();
        if trimmed.len() > 120 {
            &trimmed[..120]
        } else {
            trimmed
        }
    };
    if name.is_empty() {
        return Err("Folder name cannot be empty".into());
    }

    let id = Ulid::new().to_string();
    let ts = now();
    sqlx::query(
        "INSERT OR IGNORE INTO trace_folders (id, parent_id, name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(parent_id)
    .bind(name)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let row = if let Some(pid) = parent_id {
        sqlx::query(
            "SELECT id, parent_id, name, created_at, updated_at \
             FROM trace_folders WHERE parent_id = ? AND name = ?",
        )
        .bind(pid)
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?
    } else {
        sqlx::query(
            "SELECT id, parent_id, name, created_at, updated_at \
             FROM trace_folders WHERE parent_id IS NULL AND name = ?",
        )
        .bind(name)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?
    };

    Ok(TraceFolder {
        id: row.get("id"),
        parent_id: row.get("parent_id"),
        name: row.get("name"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

// ---- Ask AI tool helpers ----

// ── Embedding helpers ─────────────────────────────────────────────────────────

