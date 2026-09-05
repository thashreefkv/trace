//! Trace folder CRUD + listing. Extracted from legacy.rs (13-std5).

use sqlx::SqlitePool;
use ulid::Ulid;

use crate::db::sql_error;
use crate::models::*;
use super::legacy::*;
pub async fn create_folder(
    pool: &SqlitePool,
    input: CreateTraceFolderInput,
) -> Result<TraceFolder, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Folder name cannot be empty".into());
    }
    let id = Ulid::new().to_string();
    let ts = now();
    sqlx::query(
        "INSERT INTO trace_folders (id, parent_id, name, created_at, updated_at) \
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.parent_id)
    .bind(name)
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(TraceFolder {
        id,
        parent_id: input.parent_id,
        name: name.to_string(),
        created_at: ts.clone(),
        updated_at: ts,
    })
}

pub async fn rename_folder(pool: &SqlitePool, input: RenameTraceFolderInput) -> Result<(), String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("Folder name cannot be empty".into());
    }
    let ts = now();
    sqlx::query("UPDATE trace_folders SET name = ?, updated_at = ? WHERE id = ?")
        .bind(name)
        .bind(&ts)
        .bind(&input.id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn move_folder(pool: &SqlitePool, input: MoveTraceFolderInput) -> Result<(), String> {
    if let Some(ref target_parent) = input.new_parent_id {
        if target_parent == &input.id {
            return Err("A folder cannot be its own parent".into());
        }
        if would_create_cycle(pool, &input.id, target_parent).await? {
            return Err("Move would create a cycle in the folder tree".into());
        }
    }
    let ts = now();
    sqlx::query("UPDATE trace_folders SET parent_id = ?, updated_at = ? WHERE id = ?")
        .bind(&input.new_parent_id)
        .bind(&ts)
        .bind(&input.id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

async fn would_create_cycle(
    pool: &SqlitePool,
    folder_id: &str,
    candidate_parent: &str,
) -> Result<bool, String> {
    let mut cursor: Option<String> = Some(candidate_parent.to_string());
    while let Some(current) = cursor {
        if current == folder_id {
            return Ok(true);
        }
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT parent_id FROM trace_folders WHERE id = ?")
                .bind(&current)
                .fetch_optional(pool)
                .await
                .map_err(sql_error)?;
        cursor = row.and_then(|(p,)| p);
    }
    Ok(false)
}

pub async fn delete_folder(pool: &SqlitePool, id: &str, cascade: bool) -> Result<(), String> {
    if !cascade {
        let counts: (i64, i64) = sqlx::query_as(
            "SELECT \
              (SELECT COUNT(*) FROM trace_folders WHERE parent_id = ?1) AS child_folders, \
              (SELECT COUNT(*) FROM files WHERE trace_folder_id = ?1) AS child_files",
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;
        if counts.0 + counts.1 > 0 {
            return Err("Folder is not empty".into());
        }
    }
    sqlx::query("DELETE FROM trace_folders WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

pub async fn list_trace_folders(
    pool: &SqlitePool,
    parent_id: Option<&str>,
) -> Result<Vec<TraceFolder>, String> {
    let rows: Vec<TraceFolder> = match parent_id {
        Some(pid) => sqlx::query_as(
            "SELECT id, parent_id, name, created_at, updated_at \
             FROM trace_folders WHERE parent_id = ? ORDER BY name COLLATE NOCASE",
        )
        .bind(pid)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?,
        None => sqlx::query_as(
            "SELECT id, parent_id, name, created_at, updated_at \
             FROM trace_folders WHERE parent_id IS NULL ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(pool)
        .await
        .map_err(sql_error)?,
    };
    Ok(rows)
}

async fn breadcrumbs_for(pool: &SqlitePool, folder_id: &str) -> Result<Vec<TraceFolder>, String> {
    let mut chain: Vec<TraceFolder> = Vec::new();
    let mut cursor = Some(folder_id.to_string());
    while let Some(id) = cursor {
        let row: Option<TraceFolder> = sqlx::query_as(
            "SELECT id, parent_id, name, created_at, updated_at FROM trace_folders WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;
        match row {
            Some(folder) => {
                cursor = folder.parent_id.clone();
                chain.push(folder);
            }
            None => break,
        }
    }
    chain.reverse();
    Ok(chain)
}

pub async fn list_folder_children(
    pool: &SqlitePool,
    folder_id: Option<&str>,
) -> Result<FolderListing, String> {
    let folders = list_trace_folders(pool, folder_id).await?;
    let file_rows = list_files_in_folder(pool, folder_id).await?;
    let breadcrumbs = match folder_id {
        Some(fid) => breadcrumbs_for(pool, fid).await?,
        None => Vec::new(),
    };
    let folder = breadcrumbs.last().cloned();
    Ok(FolderListing {
        folder,
        breadcrumbs,
        folders,
        files: file_rows,
    })
}

// ---- Files ----

pub(crate) async fn list_files_in_folder(
    pool: &SqlitePool,
    folder_id: Option<&str>,
) -> Result<Vec<FileRow>, String> {
    let rows = match folder_id {
        Some(fid) => sqlx::query(
            "SELECT * FROM files WHERE trace_folder_id = ? ORDER BY name COLLATE NOCASE",
        )
        .bind(fid)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?,
        None => sqlx::query(
            "SELECT * FROM files WHERE trace_folder_id IS NULL ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(pool)
        .await
        .map_err(sql_error)?,
    };
    let mut files: Vec<FileRow> = rows.into_iter().map(row_to_file).collect();
    hydrate_links(pool, &mut files).await?;
    Ok(files)
}
