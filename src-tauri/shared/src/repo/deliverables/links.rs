use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::{
    db::sql_error,
    models::{InitiativeRef, LabelRef, StakeholderRef},
};

pub async fn fetch_labels_for_deliverable(
    pool: &SqlitePool,
    deliverable_id: &str,
) -> Result<Vec<LabelRef>, String> {
    sqlx::query_as::<_, LabelRef>(
        r#"
        SELECT l.id, l.name, l.color
        FROM labels l
        INNER JOIN deliverable_labels dl ON dl.label_id = l.id
        WHERE dl.deliverable_id = ?
        ORDER BY l.name ASC
        "#,
    )
    .bind(deliverable_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn fetch_initiatives_for_deliverable(
    pool: &SqlitePool,
    deliverable_id: &str,
) -> Result<Vec<InitiativeRef>, String> {
    sqlx::query_as::<_, InitiativeRef>(
        r#"
        SELECT i.id, i.title, i.status
        FROM initiatives i
        INNER JOIN deliverable_initiatives di ON di.initiative_id = i.id
        WHERE di.deliverable_id = ?
        ORDER BY i.title ASC
        "#,
    )
    .bind(deliverable_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn fetch_stakeholders_for_deliverable(
    pool: &SqlitePool,
    deliverable_id: &str,
) -> Result<Vec<StakeholderRef>, String> {
    sqlx::query_as::<_, StakeholderRef>(
        r#"
        SELECT s.id, s.name, COALESCE(s.role, '') AS role
        FROM stakeholders s
        INNER JOIN deliverable_stakeholders ds ON ds.stakeholder_id = s.id
        WHERE ds.deliverable_id = ?
        ORDER BY s.display_order ASC, s.name ASC
        "#,
    )
    .bind(deliverable_id)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn replace_initiative_links(
    tx: &mut Transaction<'_, Sqlite>,
    deliverable_id: &str,
    initiative_ids: &[String],
) -> Result<(), String> {
    sqlx::query("DELETE FROM deliverable_initiatives WHERE deliverable_id = ?")
        .bind(deliverable_id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;

    for initiative_id in initiative_ids {
        sqlx::query(
            r#"
            INSERT INTO deliverable_initiatives (deliverable_id, initiative_id)
            VALUES (?, ?)
            "#,
        )
        .bind(deliverable_id)
        .bind(initiative_id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;
    }

    Ok(())
}

pub async fn replace_stakeholder_links(
    tx: &mut Transaction<'_, Sqlite>,
    deliverable_id: &str,
    stakeholder_ids: &[String],
) -> Result<(), String> {
    sqlx::query("DELETE FROM deliverable_stakeholders WHERE deliverable_id = ?")
        .bind(deliverable_id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;

    for stakeholder_id in stakeholder_ids {
        sqlx::query(
            r#"
            INSERT INTO deliverable_stakeholders (deliverable_id, stakeholder_id)
            VALUES (?, ?)
            "#,
        )
        .bind(deliverable_id)
        .bind(stakeholder_id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;
    }

    Ok(())
}
