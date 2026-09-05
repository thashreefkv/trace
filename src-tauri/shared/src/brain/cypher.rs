//! Read-only Cypher query surface on top of the Kuzu brain.
//!
//! `query_brain_cypher` is the public entry point used by the Ask tool surface
//! and the Brain UI. `validate_read_only_cypher` rejects any keyword that
//! could mutate the graph (CREATE, MERGE, SET, DELETE, …); the blocking impl
//! opens the Kuzu DB in read-only mode and caps result rows at
//! `MAX_CYPHER_ROWS`. The two `*_to_kuzu_value` / `kuzu_value_to_*` helpers
//! convert parameters and result cells between `serde_json::Value` and the
//! Kuzu `Value` enum.

use std::path::Path;

use kuzu::{Connection, Database, LogicalType, SystemConfig, Value};
use serde_json::{json, Map};

use crate::models::{BrainCypherInput, BrainCypherResult};

const MAX_CYPHER_ROWS: usize = 500;

pub async fn query_brain_cypher(
    path: &Path,
    input: BrainCypherInput,
) -> Result<BrainCypherResult, String> {
    validate_read_only_cypher(&input.query)?;
    let limit = input.limit.unwrap_or(100).clamp(1, MAX_CYPHER_ROWS);
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || query_brain_cypher_blocking(&path, input, limit))
        .await
        .map_err(|error| format!("brain query task failed: {error}"))?
}

pub async fn tool_query_brain_cypher(path: &Path, input: BrainCypherInput) -> serde_json::Value {
    match query_brain_cypher(path, input).await {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

fn query_brain_cypher_blocking(
    path: &Path,
    input: BrainCypherInput,
    limit: usize,
) -> Result<BrainCypherResult, String> {
    let db = Database::new(path, SystemConfig::default().read_only(true))
        .map_err(|error| format!("failed to open Kuzu brain: {error}"))?;
    let conn = Connection::new(&db)
        .map_err(|error| format!("failed to connect to Kuzu brain: {error}"))?;

    let mut result = if let Some(params) = input.params {
        let object = params
            .as_object()
            .ok_or_else(|| "brain cypher params must be a JSON object".to_string())?;
        let mut statement = conn
            .prepare(&input.query)
            .map_err(|error| format!("failed to prepare brain cypher: {error}"))?;
        let mut values = Vec::new();
        for (key, value) in object {
            values.push((key.as_str(), json_to_kuzu_value(value)?));
        }
        conn.execute(&mut statement, values)
            .map_err(|error| format!("failed to execute brain cypher: {error}"))?
    } else {
        conn.query(&input.query)
            .map_err(|error| format!("failed to execute brain cypher: {error}"))?
    };

    let columns = result.get_column_names();
    let mut rows = Vec::new();
    for row in &mut result {
        if rows.len() >= limit {
            return Ok(BrainCypherResult {
                columns,
                rows,
                truncated: true,
            });
        }
        let mut object = Map::new();
        for (index, column) in columns.iter().enumerate() {
            object.insert(
                column.clone(),
                kuzu_value_to_json(row.get(index).unwrap_or(&Value::Null(LogicalType::Any))),
            );
        }
        rows.push(serde_json::Value::Object(object));
    }

    Ok(BrainCypherResult {
        columns,
        rows,
        truncated: false,
    })
}

pub(super) fn validate_read_only_cypher(query: &str) -> Result<(), String> {
    let forbidden = [
        "CREATE", "MERGE", "SET", "DELETE", "DROP", "COPY", "LOAD", "ALTER", "ATTACH", "DETACH",
        "REMOVE", "INSTALL",
    ];
    let words = query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(|word| word.to_ascii_uppercase())
        .collect::<std::collections::BTreeSet<_>>();
    for keyword in forbidden {
        if words.contains(keyword) {
            return Err(format!(
                "read-only brain cypher rejected keyword: {keyword}"
            ));
        }
    }

    let first = query
        .split_whitespace()
        .next()
        .unwrap_or("")
        .trim_start_matches('(')
        .to_ascii_uppercase();
    if !matches!(first.as_str(), "MATCH" | "RETURN" | "WITH" | "UNWIND") {
        return Err("brain cypher must start with MATCH, RETURN, WITH, or UNWIND".to_string());
    }
    Ok(())
}

fn json_to_kuzu_value(value: &serde_json::Value) -> Result<Value, String> {
    Ok(match value {
        serde_json::Value::Null => Value::Null(LogicalType::Any),
        serde_json::Value::Bool(value) => Value::Bool(*value),
        serde_json::Value::Number(value) => {
            if let Some(value) = value.as_i64() {
                Value::Int64(value)
            } else if let Some(value) = value.as_f64() {
                Value::Double(value)
            } else {
                return Err("unsupported numeric brain cypher param".to_string());
            }
        }
        serde_json::Value::String(value) => Value::String(value.clone()),
        serde_json::Value::Array(values) => {
            let values = values
                .iter()
                .map(json_to_kuzu_value)
                .collect::<Result<Vec<_>, _>>()?;
            Value::List(LogicalType::Any, values)
        }
        serde_json::Value::Object(_) => Value::String(value.to_string()),
    })
}

fn kuzu_value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Null(_) => serde_json::Value::Null,
        Value::Bool(value) => json!(value),
        Value::Int64(value) => json!(value),
        Value::Int32(value) => json!(value),
        Value::Int16(value) => json!(value),
        Value::Int8(value) => json!(value),
        Value::UInt64(value) => json!(value),
        Value::UInt32(value) => json!(value),
        Value::UInt16(value) => json!(value),
        Value::UInt8(value) => json!(value),
        Value::Double(value) => json!(value),
        Value::Float(value) => json!(value),
        Value::String(value) => json!(value),
        Value::List(_, values) | Value::Array(_, values) => {
            serde_json::Value::Array(values.iter().map(kuzu_value_to_json).collect())
        }
        Value::Struct(fields) => {
            let mut object = Map::new();
            for (key, value) in fields {
                object.insert(key.clone(), kuzu_value_to_json(value));
            }
            serde_json::Value::Object(object)
        }
        other => json!(other.to_string()),
    }
}
