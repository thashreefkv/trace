use sqlx::{Sqlite, SqlitePool, Transaction};
use ulid::Ulid;

use crate::{
    db::sql_error,
    models::{
        CaptureKind, CaptureStatus, CommitConversationIngestInput, CommitExtractedDeliverableInput,
        Conversation, ConversationExtractionResult, ConversationIngestResult,
        CreateConversationInput, DeliverableState, ExtractedConversation,
    },
};

use super::{
    clean_ids, clean_optional, consolidate_memories, ensure_references_exist, get_capture,
    get_deliverable, get_memory_settings, insert_deliverable_in_tx, normalize_claude_link,
    now_utc, replace_initiative_links, replace_stakeholder_links, valid_initiative_titles,
    valid_stakeholder_names, validate_deliverable_input, CleanDeliverableInput,
};

pub async fn create_or_get_conversation(
    pool: &SqlitePool,
    chat_url: &str,
    title: Option<String>,
    summary: Option<String>,
    occurred_at: Option<String>,
) -> Result<Conversation, String> {
    let chat_url = normalize_claude_link(chat_url)?;

    if let Some(existing) = sqlx::query_as::<_, Conversation>(
        r#"
        SELECT id, chat_url, title, summary, occurred_at, ingested_at
        FROM conversations
        WHERE chat_url = ?
        "#,
    )
    .bind(&chat_url)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    {
        return Ok(existing);
    }

    let id = Ulid::new().to_string();
    let ingested_at = now_utc();
    sqlx::query(
        r#"
        INSERT INTO conversations (id, chat_url, title, summary, occurred_at, ingested_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&chat_url)
    .bind(clean_optional(title))
    .bind(clean_optional(summary))
    .bind(clean_optional(occurred_at))
    .bind(&ingested_at)
    .execute(pool)
    .await
    .map_err(sql_error)?;

    get_conversation(pool, &id).await
}

pub async fn create_conversation(
    pool: &SqlitePool,
    input: CreateConversationInput,
) -> Result<Conversation, String> {
    create_or_get_conversation(
        pool,
        &input.chat_url,
        input.title,
        input.summary,
        input.occurred_at,
    )
    .await
}

pub async fn get_conversation(pool: &SqlitePool, id: &str) -> Result<Conversation, String> {
    sqlx::query_as::<_, Conversation>(
        r#"
        SELECT id, chat_url, title, summary, occurred_at, ingested_at
        FROM conversations
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .ok_or_else(|| "conversation not found".to_string())
}

pub async fn list_conversations(pool: &SqlitePool) -> Result<Vec<Conversation>, String> {
    sqlx::query_as::<_, Conversation>(
        r#"
        SELECT id, chat_url, title, summary, occurred_at, ingested_at
        FROM conversations
        ORDER BY ingested_at DESC, title COLLATE NOCASE ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

pub async fn commit_conversation_ingest(
    pool: &SqlitePool,
    input: CommitConversationIngestInput,
) -> Result<ConversationIngestResult, String> {
    let prepared = prepare_conversation_ingest(input)?;
    for deliverable in &prepared.deliverables {
        ensure_references_exist(
            pool,
            &deliverable.initiative_ids,
            &deliverable.stakeholder_ids,
            None,
        )
        .await?;
    }

    let mut tx = pool.begin().await.map_err(sql_error)?;
    let (conversation_id, deliverable_ids) =
        commit_prepared_conversation_ingest_in_tx(&mut tx, &prepared).await?;
    tx.commit().await.map_err(sql_error)?;

    let result =
        hydrate_conversation_ingest_result(pool, &conversation_id, &deliverable_ids).await?;
    if get_memory_settings(pool)
        .await
        .map(|settings| settings.enabled && settings.auto_extract_enabled)
        .unwrap_or(false)
    {
        let _ = consolidate_memories(pool).await;
    }
    Ok(result)
}

pub async fn promote_claude_capture_to_ingest(
    pool: &SqlitePool,
    capture_id: &str,
    mut input: CommitConversationIngestInput,
) -> Result<ConversationIngestResult, String> {
    let capture = get_capture(pool, capture_id).await?;
    if capture.status != CaptureStatus::Inbox.as_str() {
        return Err("only inbox captures can be promoted".to_string());
    }
    if capture.kind != CaptureKind::ClaudeLink.as_str() {
        return Err("only Claude link captures can use conversation ingest".to_string());
    }

    if clean_optional(input.chat_url.clone()).is_none() {
        input.chat_url = Some(capture.body.clone());
    }

    let prepared = prepare_conversation_ingest(input)?;
    for deliverable in &prepared.deliverables {
        ensure_references_exist(
            pool,
            &deliverable.initiative_ids,
            &deliverable.stakeholder_ids,
            None,
        )
        .await?;
    }

    let now = now_utc();
    let mut tx = pool.begin().await.map_err(sql_error)?;
    let (conversation_id, deliverable_ids) =
        commit_prepared_conversation_ingest_in_tx(&mut tx, &prepared).await?;

    let result = sqlx::query(
        r#"
        UPDATE captures
        SET status = 'promoted',
            promoted_conversation_id = ?,
            promoted_at = ?,
            updated_at = ?
        WHERE id = ? AND status = 'inbox'
        "#,
    )
    .bind(&conversation_id)
    .bind(&now)
    .bind(&now)
    .bind(capture_id)
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    if result.rows_affected() == 0 {
        return Err("capture not found or not in inbox".to_string());
    }

    tx.commit().await.map_err(sql_error)?;
    let result =
        hydrate_conversation_ingest_result(pool, &conversation_id, &deliverable_ids).await?;
    if get_memory_settings(pool)
        .await
        .map(|settings| settings.enabled && settings.auto_extract_enabled)
        .unwrap_or(false)
    {
        let _ = consolidate_memories(pool).await;
    }
    Ok(result)
}

pub async fn annotate_extraction_mappings(
    pool: &SqlitePool,
    mut result: ConversationExtractionResult,
) -> Result<ConversationExtractionResult, String> {
    let valid_initiatives = valid_initiative_titles(pool).await?;
    let valid_stakeholders = valid_stakeholder_names(pool).await?;

    for candidate in &mut result.candidates {
        candidate.validation_errors.clear();
        candidate.initiative_titles = clean_ids(candidate.initiative_titles.clone());
        if candidate.initiative_titles.is_empty() {
            candidate
                .validation_errors
                .push("Select at least one existing initiative.".to_string());
        }

        for title in &candidate.initiative_titles {
            if !valid_initiatives.iter().any(|valid| valid == title) {
                candidate.validation_errors.push(format!(
                    "Initiative not found: {title}. Valid initiatives: {}",
                    valid_initiatives.join(", ")
                ));
            }
        }

        candidate.stakeholder_name = clean_optional(candidate.stakeholder_name.clone());
        if let Some(stakeholder_name) = &candidate.stakeholder_name {
            if !valid_stakeholders
                .iter()
                .any(|valid| valid == stakeholder_name)
            {
                candidate.validation_errors.push(format!(
                    "Stakeholder not found: {stakeholder_name}. Select an existing stakeholder or create it explicitly."
                ));
            }
        }
    }

    Ok(result)
}

struct PreparedConversationIngest {
    chat_url: String,
    title: String,
    summary: String,
    occurred_at: Option<String>,
    deliverables: Vec<CleanDeliverableInput>,
}

fn prepare_conversation_ingest(
    input: CommitConversationIngestInput,
) -> Result<PreparedConversationIngest, String> {
    let (title, summary, occurred_at) = validate_extracted_conversation(input.conversation)?;
    let chat_url = normalize_ingest_chat_url(input.chat_url)?;
    let deliverables = input
        .deliverables
        .into_iter()
        .filter(|deliverable| deliverable.accepted)
        .map(clean_commit_deliverable)
        .collect::<Result<Vec<_>, _>>()?;

    if deliverables.is_empty() {
        return Err("accept at least one deliverable before committing".to_string());
    }

    Ok(PreparedConversationIngest {
        chat_url,
        title,
        summary,
        occurred_at,
        deliverables,
    })
}

fn validate_extracted_conversation(
    conversation: ExtractedConversation,
) -> Result<(String, String, Option<String>), String> {
    let title = conversation.title.trim().to_string();
    if title.is_empty() {
        return Err("conversation title is required".to_string());
    }

    let summary = conversation.summary.trim().to_string();
    if summary.is_empty() {
        return Err("conversation summary is required".to_string());
    }

    Ok((title, summary, clean_optional(conversation.occurred_at)))
}

fn normalize_ingest_chat_url(chat_url: Option<String>) -> Result<String, String> {
    let Some(chat_url) = clean_optional(chat_url) else {
        return Ok(format!("trace://pasted-export/{}", Ulid::new()));
    };

    normalize_claude_link(&chat_url)
}

fn clean_commit_deliverable(
    deliverable: CommitExtractedDeliverableInput,
) -> Result<CleanDeliverableInput, String> {
    validate_deliverable_input(
        deliverable.title,
        deliverable.deliverable_type,
        DeliverableState::Drafting,
        deliverable.claim,
        deliverable.artifact_url,
        None,
        deliverable.stakeholder_id,
        deliverable.stakeholder_ids,
        deliverable.initiative_ids,
    )
}

async fn commit_prepared_conversation_ingest_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    prepared: &PreparedConversationIngest,
) -> Result<(String, Vec<String>), String> {
    let now = now_utc();
    let conversation_id = upsert_conversation_in_tx(tx, prepared, &now).await?;
    let mut deliverable_ids = Vec::with_capacity(prepared.deliverables.len());

    for deliverable in &prepared.deliverables {
        let deliverable_id = Ulid::new().to_string();
        let mut input = deliverable.clone();
        input.conversation_id = Some(conversation_id.clone());
        insert_deliverable_in_tx(tx, &deliverable_id, &input, &now, None).await?;
        replace_initiative_links(tx, &deliverable_id, &input.initiative_ids).await?;
        replace_stakeholder_links(tx, &deliverable_id, &input.stakeholder_ids).await?;
        deliverable_ids.push(deliverable_id);
    }

    Ok((conversation_id, deliverable_ids))
}

async fn upsert_conversation_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    prepared: &PreparedConversationIngest,
    now: &str,
) -> Result<String, String> {
    let existing_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM conversations WHERE chat_url = ?")
            .bind(&prepared.chat_url)
            .fetch_optional(&mut **tx)
            .await
            .map_err(sql_error)?;

    if let Some(id) = existing_id {
        sqlx::query(
            r#"
            UPDATE conversations
            SET title = ?,
                summary = ?,
                occurred_at = COALESCE(?, occurred_at)
            WHERE id = ?
            "#,
        )
        .bind(&prepared.title)
        .bind(&prepared.summary)
        .bind(&prepared.occurred_at)
        .bind(&id)
        .execute(&mut **tx)
        .await
        .map_err(sql_error)?;
        return Ok(id);
    }

    let id = Ulid::new().to_string();
    sqlx::query(
        r#"
        INSERT INTO conversations (id, chat_url, title, summary, occurred_at, ingested_at)
        VALUES (?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&prepared.chat_url)
    .bind(&prepared.title)
    .bind(&prepared.summary)
    .bind(&prepared.occurred_at)
    .bind(now)
    .execute(&mut **tx)
    .await
    .map_err(sql_error)?;

    Ok(id)
}

async fn hydrate_conversation_ingest_result(
    pool: &SqlitePool,
    conversation_id: &str,
    deliverable_ids: &[String],
) -> Result<ConversationIngestResult, String> {
    let conversation = get_conversation(pool, conversation_id).await?;
    let mut deliverables = Vec::with_capacity(deliverable_ids.len());
    for id in deliverable_ids {
        deliverables.push(get_deliverable(pool, id).await?);
    }

    Ok(ConversationIngestResult {
        conversation,
        deliverables,
    })
}
