//! Brain on-disk projection: the materialized `BrainEntity` / `BrainRelation`
//! graph that gets rewritten into Kuzu on every rebuild, plus the small set of
//! file-IO helpers that read and write the sidecar `brain.kuzu.meta.json`.
//!
//! The heavy `build_projection` / `rebuild_brain` / `read_graph` machinery
//! still lives in `super::legacy` for now (the populators — `add_initiatives`,
//! `add_deliverables`, `add_gmail`, … — are large and tightly coupled to it).
//! This module owns:
//!
//! - The shared data types (`BrainProjection`, `BrainEntity`, `BrainRelation`,
//!   `BrainMeta`) and the schema constants. `pub(super)` lets the legacy
//!   populators construct entities directly while keeping them off the public
//!   crate API.
//! - `get_brain_status` (the public status getter) and the four `*_meta`
//!   helpers it relies on (`meta_path` / `read_meta` / `write_meta` /
//!   `remove_brain_path`). These are the smallest chokepoint for "what does
//!   the brain look like on disk right now?".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::BrainStatus;

pub const BRAIN_FILE_NAME: &str = "brain.kuzu";
pub(super) const BRAIN_META_FILE_NAME: &str = "brain.kuzu.meta.json";
pub(super) const BRAIN_SCHEMA_VERSION: i64 = 1;

#[derive(Debug, Clone)]
pub(super) struct BrainEntity {
    pub(super) id: String,
    pub(super) kind: String,
    pub(super) source_table: String,
    pub(super) source_id: String,
    pub(super) title: String,
    pub(super) summary: String,
    pub(super) status: String,
    pub(super) route: String,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) importance: f64,
    pub(super) payload_json: String,
}

#[derive(Debug, Clone)]
pub(super) struct BrainRelation {
    pub(super) source: String,
    pub(super) target: String,
    pub(super) kind: String,
    pub(super) label: String,
    pub(super) strength: f64,
    pub(super) evidence_json: String,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) payload_json: String,
}

#[derive(Debug, Clone, Default)]
pub(super) struct BrainProjection {
    pub(super) nodes: BTreeMap<String, BrainEntity>,
    pub(super) edges: BTreeMap<String, BrainRelation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct BrainMeta {
    pub(super) schema_version: i64,
    pub(super) generated_at: String,
    pub(super) node_count: usize,
    pub(super) edge_count: usize,
}

impl BrainProjection {
    pub(super) fn push_node(&mut self, entity: BrainEntity) {
        self.nodes.entry(entity.id.clone()).or_insert(entity);
    }

    pub(super) fn push_edge(&mut self, relation: BrainRelation) {
        if !self.nodes.contains_key(&relation.source) || !self.nodes.contains_key(&relation.target)
        {
            return;
        }
        let key = super::legacy::graph_edge_id(&relation.kind, &relation.source, &relation.target);
        self.edges.entry(key).or_insert(relation);
    }
}

pub async fn get_brain_status(path: &Path) -> BrainStatus {
    let exists = path.exists();
    let meta = read_meta(path).ok();
    let (node_count, edge_count, generated_at) = meta
        .map(|meta| (meta.node_count, meta.edge_count, Some(meta.generated_at)))
        .unwrap_or((0, 0, None));

    BrainStatus {
        path: path.display().to_string(),
        exists,
        schema_version: BRAIN_SCHEMA_VERSION,
        storage_version: kuzu::get_storage_version(),
        generated_at,
        node_count,
        edge_count,
        error: None,
    }
}

pub(super) fn meta_path(path: &Path) -> PathBuf {
    path.parent()
        .map(|parent| parent.join(BRAIN_META_FILE_NAME))
        .unwrap_or_else(|| PathBuf::from(BRAIN_META_FILE_NAME))
}

pub(super) fn read_meta(path: &Path) -> Result<BrainMeta, String> {
    let meta_path = meta_path(path);
    let raw = std::fs::read_to_string(&meta_path)
        .map_err(|error| format!("failed to read brain meta: {error}"))?;
    serde_json::from_str(&raw).map_err(|error| format!("failed to parse brain meta: {error}"))
}

pub(super) fn write_meta(path: &Path, meta: &BrainMeta) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(meta)
        .map_err(|error| format!("failed to serialize brain meta: {error}"))?;
    std::fs::write(meta_path(path), raw)
        .map_err(|error| format!("failed to write brain meta: {error}"))
}

pub(super) fn remove_brain_path(path: &Path) -> Result<(), String> {
    if path.exists() {
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("failed to inspect brain path: {error}"))?;
        if metadata.is_dir() {
            std::fs::remove_dir_all(path)
                .map_err(|error| format!("failed to remove old brain directory: {error}"))?;
        } else {
            std::fs::remove_file(path)
                .map_err(|error| format!("failed to remove old brain file: {error}"))?;
        }
    }
    // Kuzu leaves sibling .wal and .lock files next to the database directory.
    // Without removing them, a fresh Database::new() at the same path picks up
    // the stale WAL and believes the schema already exists, causing CREATE NODE
    // TABLE to fail on every subsequent rebuild.
    for suffix in [".wal", ".lock"] {
        let mut s = path.as_os_str().to_os_string();
        s.push(suffix);
        let sibling = PathBuf::from(s);
        if sibling.exists() {
            let _ = std::fs::remove_file(&sibling);
        }
    }
    let meta_path = meta_path(path);
    if meta_path.exists() {
        std::fs::remove_file(meta_path)
            .map_err(|error| format!("failed to remove old brain meta: {error}"))?;
    }
    Ok(())
}
