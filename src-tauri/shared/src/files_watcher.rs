use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use notify::{EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use sqlx::SqlitePool;
use tokio::sync::Mutex as AsyncMutex;

use crate::{
    brain,
    files::{all_local_paths, mark_file_missing, update_file_path},
};

/// Long-running async task: watches all registered local file paths for
/// renames and deletes. Spawn this with `tauri::async_runtime::spawn` from
/// the Tauri setup closure.
///
/// Uses FSEvents on macOS. On other platforms it falls through to the
/// cross-platform backend but is compiled only on macOS for now.
#[cfg(target_os = "macos")]
pub async fn run_watcher(
    pool: SqlitePool,
    brain_path: PathBuf,
    brain_rebuild_lock: Arc<AsyncMutex<()>>,
) {
    if let Err(e) = watch_loop(pool, brain_path, brain_rebuild_lock).await {
        eprintln!("[files_watcher] error: {e}");
    }
}

#[cfg(not(target_os = "macos"))]
pub async fn run_watcher(
    _pool: SqlitePool,
    _brain_path: PathBuf,
    _brain_rebuild_lock: Arc<AsyncMutex<()>>,
) {
}

#[cfg(target_os = "macos")]
async fn watch_loop(
    pool: SqlitePool,
    brain_path: PathBuf,
    brain_rebuild_lock: Arc<AsyncMutex<()>>,
) -> Result<(), String> {
    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::unbounded_channel::<DebounceEventResult>();

    let mut debouncer = new_debouncer(Duration::from_millis(500), None, move |res| {
        let _ = tx.send(res);
    })
    .map_err(|e| format!("failed to create debouncer: {e}"))?;

    // Build path→id map and watch every unique parent directory.
    let mut path_to_id: HashMap<String, String> = HashMap::new();
    let mut watched_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

    if let Ok(rows) = all_local_paths(&pool).await {
        for (id, path) in rows {
            let pb = PathBuf::from(&path);
            if let Some(parent) = pb.parent() {
                if watched_dirs.insert(parent.to_path_buf()) {
                    let _ = debouncer.watch(parent, RecursiveMode::NonRecursive);
                }
            }
            path_to_id.insert(path, id);
        }
    }

    while let Some(result) = rx.recv().await {
        match result {
            Ok(events) => {
                let mut changed = false;
                for event in events {
                    match event.kind {
                        EventKind::Remove(_) => {
                            for path in &event.paths {
                                let p = path.to_string_lossy().to_string();
                                if path_to_id.contains_key(&p) {
                                    if mark_file_missing(&pool, &p).await.is_ok() {
                                        changed = true;
                                    }
                                }
                            }
                        }
                        EventKind::Modify(notify::event::ModifyKind::Name(
                            notify::event::RenameMode::Both,
                        )) => {
                            if event.paths.len() == 2 {
                                let old = event.paths[0].to_string_lossy().to_string();
                                let new_path = event.paths[1].to_string_lossy().to_string();
                                if path_to_id.contains_key(&old) {
                                    if update_file_path(&pool, &old, &new_path).await.is_ok() {
                                        changed = true;
                                    }
                                    if let Some(id) = path_to_id.remove(&old) {
                                        path_to_id.insert(new_path, id);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if changed {
                    let _guard = brain_rebuild_lock.lock().await;
                    let _ = brain::rebuild_brain(&pool, &brain_path).await;
                }
            }
            Err(errors) => {
                for e in errors {
                    eprintln!("[files_watcher] watch error: {e}");
                }
            }
        }
    }

    Ok(())
}
