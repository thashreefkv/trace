use std::collections::HashMap;

use serde_json::json;
use sqlx::{FromRow, SqlitePool};

use super::legacy::{entity, graph_node_id, relation, source_node_id, sql_error};
use super::projection::BrainProjection;

#[derive(Debug, FromRow)]
struct ApprovedClaimRow {
    id: String,
    statement: String,
    source_kind: Option<String>,
    source_id: Option<String>,
    confidence: f64,
    evidence_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct ApprovedCommunityReportRow {
    id: String,
    community_id: String,
    title: String,
    summary_markdown: String,
    evidence_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct EvidenceSourceRow {
    id: String,
    source_kind: String,
    source_id: String,
}

#[derive(Debug, FromRow)]
struct CommunityMemberRow {
    community_id: String,
    entity_kind: String,
    entity_id: String,
}

/// Add only reviewed reasoning artifacts to reusable Kuzu context. Pending
/// analysis remains query/report provenance and cannot influence graph truth.
pub(super) async fn add_reasoning_artifacts(
    pool: &SqlitePool,
    projection: &mut BrainProjection,
) -> Result<(), String> {
    let evidence_sources = sqlx::query_as::<_, EvidenceSourceRow>(
        "SELECT id, source_kind, source_id FROM reasoning_source_units WHERE access_state = 'included'",
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?
    .into_iter()
    .map(|source| (source.id.clone(), source))
    .collect::<HashMap<_, _>>();
    let claims = sqlx::query_as::<_, ApprovedClaimRow>(
        r#"SELECT id, statement, source_kind, source_id, confidence, evidence_json,
           created_at, updated_at FROM claim_versions WHERE status = 'approved'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for claim in claims {
        let node_id = graph_node_id("reasoning_claim", &claim.id);
        projection.push_node(entity(
            "reasoning_claim",
            "claim_versions",
            &claim.id,
            "Approved claim",
            &claim.statement,
            "approved",
            None,
            &claim.created_at,
            &claim.updated_at,
            claim.confidence.clamp(0.2, 1.0),
            json!({ "evidence": claim.evidence_json, "review_gated": true }),
        ));
        if let (Some(source_kind), Some(source_id)) = (&claim.source_kind, &claim.source_id) {
            if let Some(source) = source_node_id(source_kind, source_id) {
                projection.push_edge(relation(
                    &source,
                    &node_id,
                    "SUPPORTS_CLAIM",
                    "approved evidence",
                    claim.confidence,
                    json!({ "claim_id": claim.id }),
                    &claim.created_at,
                    &claim.updated_at,
                    json!({ "review_gated": true }),
                ));
            }
        }
        let evidence_ids: Vec<String> =
            serde_json::from_str(&claim.evidence_json).unwrap_or_default();
        for evidence_id in evidence_ids {
            let Some(evidence_source) = evidence_sources.get(&evidence_id) else {
                continue;
            };
            let Some(source) =
                source_node_id(&evidence_source.source_kind, &evidence_source.source_id)
            else {
                continue;
            };
            projection.push_edge(relation(
                &source,
                &node_id,
                "EVIDENCES_APPROVED_CLAIM",
                "cited evidence",
                claim.confidence,
                json!({ "source_unit_id": evidence_id }),
                &claim.created_at,
                &claim.updated_at,
                json!({ "review_gated": true }),
            ));
        }
    }
    let reports = sqlx::query_as::<_, ApprovedCommunityReportRow>(
        r#"SELECT id, community_id, title, summary_markdown, evidence_json, created_at, updated_at
           FROM community_reports WHERE status = 'approved'"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;
    for report in reports {
        let community = graph_node_id("reasoning_community", &report.community_id);
        projection.push_node(entity(
            "reasoning_community",
            "graph_communities",
            &report.community_id,
            &report.title,
            "Approved reasoning community",
            "approved",
            None,
            &report.created_at,
            &report.updated_at,
            0.75,
            json!({ "review_gated": true }),
        ));
        let report_node = graph_node_id("community_report", &report.id);
        projection.push_node(entity(
            "community_report",
            "community_reports",
            &report.id,
            &report.title,
            &report.summary_markdown,
            "approved",
            None,
            &report.created_at,
            &report.updated_at,
            0.8,
            json!({ "evidence": report.evidence_json, "review_gated": true }),
        ));
        projection.push_edge(relation(
            &community,
            &report_node,
            "HAS_APPROVED_REPORT",
            "approved synthesis",
            0.8,
            json!({ "report_id": report.id }),
            &report.created_at,
            &report.updated_at,
            json!({ "review_gated": true }),
        ));
        let members = sqlx::query_as::<_, CommunityMemberRow>(
            r#"SELECT community_id, entity_kind, entity_id
               FROM graph_community_members WHERE community_id = ?"#,
        )
        .bind(&report.community_id)
        .fetch_all(pool)
        .await
        .map_err(sql_error)?;
        for member in members {
            if let Some(source) = source_node_id(&member.entity_kind, &member.entity_id) {
                projection.push_edge(relation(
                    &source,
                    &community,
                    "MEMBER_OF_REASONING_COMMUNITY",
                    "community membership",
                    0.7,
                    json!({ "community_id": member.community_id }),
                    &report.created_at,
                    &report.updated_at,
                    json!({ "review_gated": true }),
                ));
            }
        }
    }
    Ok(())
}
