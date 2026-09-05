//! Eval fixture CRUD + import. Extracted from legacy.rs (13-std6).


use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::legacy::*;

pub async fn list_fixtures(pool: &SqlitePool) -> Result<Vec<EvalFixture>, String> {
    sqlx::query_as::<_, EvalFixtureRow>(
        "SELECT id, kind, name, input_json, expectation_json, notes,
                enabled, created_at, updated_at
           FROM eval_fixtures
          ORDER BY kind, name",
    )
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(EvalFixture::from).collect())
    .map_err(|e| format!("list fixtures: {e}"))
}

#[derive(sqlx::FromRow)]
struct EvalFixtureRow {
    id: String,
    kind: String,
    name: String,
    input_json: String,
    expectation_json: String,
    notes: Option<String>,
    enabled: i64,
    created_at: i64,
    updated_at: i64,
}

impl From<EvalFixtureRow> for EvalFixture {
    fn from(row: EvalFixtureRow) -> Self {
        EvalFixture {
            id: row.id,
            kind: row.kind,
            name: row.name,
            input_json: row.input_json,
            expectation_json: row.expectation_json,
            notes: row.notes,
            enabled: row.enabled != 0,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateFixtureInput {
    pub kind: String,
    pub name: String,
    pub input_json: String,
    pub expectation_json: String,
    pub notes: Option<String>,
}

pub async fn create_fixture(
    pool: &SqlitePool,
    input: CreateFixtureInput,
) -> Result<EvalFixture, String> {
    let id = ulid::Ulid::new().to_string();
    let ts = chrono::Utc::now().timestamp_millis();
    sqlx::query(
        "INSERT INTO eval_fixtures
           (id, kind, name, input_json, expectation_json, notes, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(&input.kind)
    .bind(&input.name)
    .bind(&input.input_json)
    .bind(&input.expectation_json)
    .bind(input.notes.as_deref())
    .bind(ts)
    .bind(ts)
    .execute(pool)
    .await
    .map_err(|e| format!("create fixture: {e}"))?;

    Ok(EvalFixture {
        id,
        kind: input.kind,
        name: input.name,
        input_json: input.input_json,
        expectation_json: input.expectation_json,
        notes: input.notes,
        enabled: true,
        created_at: ts,
        updated_at: ts,
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImportFixturesInput {
    /// Each entry: { kind, name, input_json, expectation_json, notes? }.
    /// `input_json` / `expectation_json` may be either a stringified JSON
    /// payload or a nested object — both are normalized to a string before
    /// insert.
    pub fixtures: Vec<serde_json::Value>,
    /// If true, fixtures whose `name` collides with an existing row are
    /// skipped. Default false (insert each as a new row with a fresh ID).
    #[serde(default)]
    pub skip_existing: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportFixturesResult {
    pub imported: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

pub async fn import_fixtures(
    pool: &SqlitePool,
    input: ImportFixturesInput,
) -> Result<ImportFixturesResult, String> {
    let mut imported = 0_i64;
    let mut skipped = 0_i64;
    let mut errors: Vec<String> = Vec::new();

    // Pull existing names once when skip_existing is on so we can
    // dedupe in O(1) per fixture without N round-trips.
    let existing_names: std::collections::HashSet<String> = if input.skip_existing {
        sqlx::query_scalar::<_, String>("SELECT name FROM eval_fixtures")
            .fetch_all(pool)
            .await
            .map_err(|e| format!("read existing names: {e}"))?
            .into_iter()
            .collect()
    } else {
        std::collections::HashSet::new()
    };

    for (index, entry) in input.fixtures.iter().enumerate() {
        let label = entry
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("<unnamed>")
            .to_string();
        let kind = match entry.get("kind").and_then(|v| v.as_str()) {
            Some(k) if FixtureKind::from_str(k).is_some() => k.to_string(),
            _ => {
                errors.push(format!(
                    "fixture #{}: invalid or missing 'kind' (got {:?})",
                    index,
                    entry.get("kind")
                ));
                continue;
            }
        };
        if label == "<unnamed>" {
            errors.push(format!("fixture #{}: missing 'name'", index));
            continue;
        }
        if input.skip_existing && existing_names.contains(&label) {
            skipped += 1;
            continue;
        }

        let input_json = serialize_payload(entry.get("input_json")).ok_or_else(|| {
            format!("fixture #{} ({}): missing 'input_json'", index, label)
        });
        let input_json = match input_json {
            Ok(v) => v,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let expectation_json = serialize_payload(entry.get("expectation_json"))
            .ok_or_else(|| {
                format!(
                    "fixture #{} ({}): missing 'expectation_json'",
                    index, label
                )
            });
        let expectation_json = match expectation_json {
            Ok(v) => v,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let notes = entry
            .get("notes")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        let create = CreateFixtureInput {
            kind,
            name: label.clone(),
            input_json,
            expectation_json,
            notes,
        };
        match create_fixture(pool, create).await {
            Ok(_) => imported += 1,
            Err(error) => errors.push(format!("fixture #{} ({}): {}", index, label, error)),
        }
    }

    Ok(ImportFixturesResult {
        imported,
        skipped,
        errors,
    })
}

/// Normalize a JSON value to a string suitable for `eval_fixtures.input_json`
/// / `expectation_json`. Strings pass through; objects/arrays serialize.
fn serialize_payload(value: Option<&serde_json::Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    serde_json::to_string(value).ok()
}

pub async fn delete_fixture(pool: &SqlitePool, id: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM eval_fixtures WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("delete fixture: {e}"))?;
    Ok(())
}

