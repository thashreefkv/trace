use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use tauri::State;

use crate::{
    db::AppState,
    models::{Deliverable, DeliverableFilters, ExportResult, Initiative, Stakeholder},
};

#[derive(Debug, Serialize)]
struct ExportIndex {
    generated_at: String,
    initiatives: Vec<Initiative>,
    stakeholders: Vec<Stakeholder>,
    deliverables: Vec<Deliverable>,
}

#[tauri::command]
pub async fn export_markdown(
    destination_dir: String,
    state: State<'_, AppState>,
) -> Result<ExportResult, String> {
    let destination_dir = PathBuf::from(destination_dir);
    if !destination_dir.exists() || !destination_dir.is_dir() {
        return Err("export destination must be an existing folder".to_string());
    }

    let initiatives = project_manager_shared::repo::list_initiatives_by_title(&state.pool).await?;
    let stakeholders = project_manager_shared::repo::list_stakeholders(&state.pool).await?;
    let deliverables =
        project_manager_shared::repo::list_deliverables(&state.pool, DeliverableFilters::default())
            .await?;

    let export_root = destination_dir.join(format!(
        "trace-export-{}",
        Utc::now().format("%Y%m%d-%H%M%S")
    ));
    let initiatives_dir = export_root.join("initiatives");
    let deliverables_dir = export_root.join("deliverables");

    fs::create_dir_all(&initiatives_dir)
        .map_err(|error| format!("failed to create export folder: {error}"))?;
    fs::create_dir_all(&deliverables_dir)
        .map_err(|error| format!("failed to create export folder: {error}"))?;

    for initiative in &initiatives {
        let path = initiatives_dir.join(format!(
            "{}-{}.md",
            slugify(&initiative.title),
            initiative.id
        ));
        fs::write(path, initiative_markdown(initiative, &deliverables))
            .map_err(|error| format!("failed to write initiative export: {error}"))?;
    }

    for deliverable in &deliverables {
        let path = deliverables_dir.join(format!(
            "{}-{}.md",
            slugify(&deliverable.title),
            deliverable.id
        ));
        fs::write(path, deliverable_markdown(deliverable))
            .map_err(|error| format!("failed to write deliverable export: {error}"))?;
    }

    fs::write(
        export_root.join("stakeholders.md"),
        stakeholders_markdown(&stakeholders),
    )
    .map_err(|error| format!("failed to write stakeholder export: {error}"))?;

    let index = ExportIndex {
        generated_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        initiatives,
        stakeholders,
        deliverables,
    };
    let index_json = serde_json::to_string_pretty(&index)
        .map_err(|error| format!("failed to serialize export index: {error}"))?;
    fs::write(export_root.join("index.json"), index_json)
        .map_err(|error| format!("failed to write export index: {error}"))?;

    Ok(ExportResult {
        export_path: display_path(&export_root),
        initiative_count: index.initiatives.len(),
        stakeholder_count: index.stakeholders.len(),
        deliverable_count: index.deliverables.len(),
    })
}

fn initiative_markdown(initiative: &Initiative, deliverables: &[Deliverable]) -> String {
    let mut body = format!(
        "# {}\n\nStatus: `{}`\n\nCreated: `{}`\nUpdated: `{}`\n\n## Framing\n\n{}\n\n## Deliverables\n",
        initiative.title,
        initiative.status,
        initiative.created_at,
        initiative.updated_at,
        empty_as_placeholder(&initiative.framing)
    );

    let attached = deliverables
        .iter()
        .filter(|deliverable| {
            deliverable
                .initiatives
                .iter()
                .any(|linked| linked.id == initiative.id)
        })
        .collect::<Vec<_>>();

    if attached.is_empty() {
        body.push_str("\nNo deliverables attached.\n");
    } else {
        for deliverable in attached {
            body.push_str(&format!(
                "\n- `[{}]` {} - {}\n",
                deliverable.deliverable_type, deliverable.title, deliverable.claim
            ));
        }
    }

    body
}

fn deliverable_markdown(deliverable: &Deliverable) -> String {
    let initiatives = deliverable
        .initiatives
        .iter()
        .map(|initiative| initiative.title.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "# {}\n\nType: `{}`\nState: `{}`\nStakeholder: {}\nInitiatives: {}\nArtifact: {}\nConversation: {}\nCreated: `{}`\nUpdated: `{}`\nShipped: {}\n\n## Claim\n\n{}\n",
        deliverable.title,
        deliverable.deliverable_type,
        deliverable.state,
        deliverable_stakeholder_names(deliverable),
        if initiatives.is_empty() {
            "None".to_string()
        } else {
            initiatives
        },
        deliverable.artifact_url.as_deref().unwrap_or("None"),
        deliverable.conversation_url.as_deref().unwrap_or("None"),
        deliverable.created_at,
        deliverable.updated_at,
        deliverable.shipped_at.as_deref().unwrap_or("None"),
        deliverable.claim
    )
}

fn deliverable_stakeholder_names(deliverable: &Deliverable) -> String {
    if deliverable.stakeholders.is_empty() {
        return deliverable
            .stakeholder_name
            .clone()
            .unwrap_or_else(|| "None".to_string());
    }

    deliverable
        .stakeholders
        .iter()
        .map(|stakeholder| stakeholder.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn stakeholders_markdown(stakeholders: &[Stakeholder]) -> String {
    let mut body = "# Stakeholders\n".to_string();

    if stakeholders.is_empty() {
        body.push_str("\nNo stakeholders.\n");
    } else {
        for stakeholder in stakeholders {
            body.push_str(&format!(
                "\n- {} (`{}`), display order {}\n",
                stakeholder.name, stakeholder.id, stakeholder.display_order
            ));
        }
    }

    body
}

fn empty_as_placeholder(value: &str) -> &str {
    if value.trim().is_empty() {
        "No framing."
    } else {
        value
    }
}

fn slugify(value: &str) -> String {
    let slug = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_produces_safe_ascii_names() {
        assert_eq!(
            slugify("Content Quality Pipeline"),
            "content-quality-pipeline"
        );
        assert_eq!(slugify("  !!!  "), "untitled");
    }
}
