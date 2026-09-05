use std::{path::Path, time::Duration};

use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    Acquire, SqlitePool,
};

pub const DB_FILE_NAME: &str = "data.db";
pub const INIT_SQL: &str = include_str!("../../migrations/0001_init.sql");
pub const DELIVERABLES_SQL: &str = include_str!("../../migrations/0002_deliverables.sql");
pub const CAPTURES_SQL: &str = include_str!("../../migrations/0003_captures.sql");
pub const CONVERSATIONS_MCP_SQL: &str = include_str!("../../migrations/0004_conversations_mcp.sql");
pub const CONVERSATION_INGEST_SQL: &str =
    include_str!("../../migrations/0005_conversation_ingest.sql");
pub const CAPTURE_INITIATIVE_GRAPH_SQL: &str =
    include_str!("../../migrations/0006_capture_initiative_graph.sql");
pub const DELIVERABLE_ENHANCEMENTS_SQL: &str =
    include_str!("../../migrations/0007_deliverable_enhancements.sql");
pub const WEEK_PLAN_SQL: &str = include_str!("../../migrations/0008_week_plan.sql");
pub const MEETINGS_SQL: &str = include_str!("../../migrations/0009_meetings.sql");
pub const STAKEHOLDER_PROFILE_SQL: &str =
    include_str!("../../migrations/0010_stakeholder_profile.sql");
pub const DELIVERABLE_STAKEHOLDERS_SQL: &str =
    include_str!("../../migrations/0011_deliverable_stakeholders.sql");
pub const GANTT_SQL: &str = include_str!("../../migrations/0012_gantt.sql");
pub const MEETING_ACTION_KINDS_SQL: &str =
    include_str!("../../migrations/0013_meeting_action_kinds.sql");
pub const MEETING_ACTION_PAYLOAD_SQL: &str =
    include_str!("../../migrations/0014_meeting_action_payload.sql");
pub const BOARD_ENHANCEMENTS_SQL: &str =
    include_str!("../../migrations/0015_board_enhancements.sql");
pub const GMAIL_WORKSPACE_SQL: &str = include_str!("../../migrations/0016_gmail_workspace.sql");
pub const GMAIL_SYNC_HARDENING_SQL: &str =
    include_str!("../../migrations/0017_gmail_sync_hardening.sql");
pub const MEMORY_SYSTEM_SQL: &str = include_str!("../../migrations/0018_memory_system.sql");
pub const MEMORY_RETRIEVAL_SQL: &str = include_str!("../../migrations/0019_memory_retrieval.sql");
pub const ASK_CHATS_SQL: &str = include_str!("../../migrations/0020_ask_chats.sql");
pub const EMAIL_CATEGORIES_WORK_INTAKE_SQL: &str =
    include_str!("../../migrations/0021_email_categories_work_intake.sql");
pub const STAKEHOLDER_THREAD_EXCLUDES_SQL: &str =
    include_str!("../../migrations/0022_stakeholder_thread_excludes.sql");
pub const GMAIL_AI_TITLE_SQL: &str = include_str!("../../migrations/0023_gmail_ai_title.sql");
pub const FILES_SQL: &str = include_str!("../../migrations/0024_files.sql");
pub const FILES_WATCHER_SQL: &str = include_str!("../../migrations/0025_files_watcher.sql");
pub const FILE_EMBEDDINGS_SQL: &str = include_str!("../../migrations/0026_file_embeddings.sql");
pub const FOLDER_ENTITY_LINKS_SQL: &str =
    include_str!("../../migrations/0027_folder_entity_links.sql");
pub const GMEET_FOLDER_SQL: &str = include_str!("../../migrations/0028_gmeet_folder.sql");
pub const GCAL_SQL: &str = include_str!("../../migrations/0029_gcal.sql");
pub const GCAL_ATTENDEES_SQL: &str = include_str!("../../migrations/0030_gcal_attendees.sql");
pub const GCAL_FULL_SQL: &str = include_str!("../../migrations/0031_gcal_full.sql");
pub const INITIATIVE_ICON_SQL: &str = include_str!("../../migrations/0032_initiative_icon.sql");
pub const USER_PROFILE_SQL: &str = include_str!("../../migrations/0033_user_profile.sql");
pub const MEETING_STAKEHOLDERS_SQL: &str =
    include_str!("../../migrations/0034_meeting_stakeholders.sql");
pub const SECOND_BRAIN_SQL: &str = include_str!("../../migrations/0036_second_brain.sql");
pub const BRAIN_RL_SQL: &str = include_str!("../../migrations/0037_brain_rl.sql");
pub const GMAIL_INTELLIGENCE_SQL: &str =
    include_str!("../../migrations/0038_gmail_intelligence.sql");
pub const TOOL_CALL_LOG_SQL: &str = include_str!("../../migrations/0039_tool_call_log.sql");
pub const GEMINI_USAGE_SQL: &str = include_str!("../../migrations/0040_gemini_usage.sql");
pub const EVALS_SQL: &str = include_str!("../../migrations/0041_evals.sql");
pub const ENTITY_EMBEDDINGS_SQL: &str = include_str!("../../migrations/0042_entity_embeddings.sql");
pub const GMAIL_USER_CLASSIFICATIONS_SQL: &str =
    include_str!("../../migrations/0043_gmail_user_classifications.sql");
pub const GMAIL_CLASSIFICATION_DIMENSIONS_SQL: &str =
    include_str!("../../migrations/0044_gmail_classification_dimensions.sql");
pub const APP_CONFIG_SETTINGS_SQL: &str =
    include_str!("../../migrations/0046_app_config_settings.sql");
pub const RETRIEVAL_RL_SQL: &str = include_str!("../../migrations/0047_retrieval_rl.sql");
pub const GMAIL_OVERRIDE_EXTRAS_SQL: &str =
    include_str!("../../migrations/0045_gmail_override_extras.sql");
pub const GEMINI_EMBEDDING_2_MIGRATION_SQL: &str =
    include_str!("../../migrations/0043_gemini_embedding_2.sql");
pub const LOCAL_EMAIL_DRAFTS_SQL: &str =
    include_str!("../../migrations/0048_local_email_drafts.sql");
pub const GMAIL_ANALYSIS_HISTORY_SQL: &str =
    include_str!("../../migrations/0049_analysis_history.sql");
pub const CAPTURE_PROMOTION_SQL: &str = include_str!("../../migrations/0050_capture_promotion.sql");
pub const PROMPT_INJECTION_LOG_SQL: &str =
    include_str!("../../migrations/0051_prompt_injection_log.sql");
pub const BRAIN_INFERENCE_TEMPLATE_SQL: &str =
    include_str!("../../migrations/0052_brain_inference_template.sql");
pub const ASK_TURN_SCORED_NODES_SQL: &str =
    include_str!("../../migrations/0053_ask_turn_scored_nodes.sql");
pub const WORK_MAIL_SQL: &str = include_str!("../../migrations/0054_work_mail.sql");
pub const WORK_MAIL_REVIEW_STATE_SQL: &str =
    include_str!("../../migrations/0055_work_mail_review_state.sql");
pub const REASONING_REPORTS_SQL: &str = include_str!("../../migrations/0056_reasoning_reports.sql");
pub const REASONING_REPORTS_UPGRADE_SQL: &str =
    include_str!("../../migrations/0057_reasoning_reports_upgrade.sql");
pub const INITIATIVE_FILE_CALENDAR_LINKS_SQL: &str =
    include_str!("../../migrations/0058_initiative_file_calendar_links.sql");
pub const REPORT_STEPS_SQL: &str = include_str!("../../migrations/0059_report_steps.sql");
pub const BRAIN_SAVED_VIEWS_SQL: &str = include_str!("../../migrations/0060_brain_saved_views.sql");
pub const APPLE_NOTES_SYNC_SQL: &str = include_str!("../../migrations/0061_apple_notes_sync.sql");
pub const BRAIN_LAYOUT_CACHE_SQL: &str =
    include_str!("../../migrations/0062_brain_layout_cache.sql");

pub async fn connect_path(path: &Path) -> Result<SqlitePool, String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create database directory: {error}"))?;
    }

    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|error| format!("failed to open database: {error}"))?;

    apply_migrations(&pool).await?;
    Ok(pool)
}

pub async fn connect_memory() -> Result<SqlitePool, String> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .map_err(|error| format!("failed to open in-memory database: {error}"))?;

    sqlx::raw_sql("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .map_err(sql_error)?;
    apply_migrations(&pool).await?;
    Ok(pool)
}

pub async fn apply_migrations(pool: &SqlitePool) -> Result<(), String> {
    // Create the tracking table first — all subsequent steps are gated on it.
    ensure_migration_tracking(pool).await?;

    // Steps are sequential integers internal to db.rs.  Each step runs at most
    // once: if it is already recorded in _applied_migrations it is skipped
    // entirely, so new migrations never need manual idempotency guards.
    //
    // Existing migrations keep their guards as a safety net, but they will
    // only be called once (on the first run after this tracking was added).

    if !step_applied(pool, 1).await {
        for sql in [INIT_SQL, DELIVERABLES_SQL, CAPTURES_SQL] {
            sqlx::raw_sql(sql)
                .execute(pool)
                .await
                .map_err(|e| format!("step 1 (core tables): {e}"))?;
        }
        mark_step(pool, 1).await?;
    }

    if !step_applied(pool, 2).await {
        apply_conversations_migration(pool).await?;
        mark_step(pool, 2).await?;
    }
    if !step_applied(pool, 3).await {
        apply_conversation_ingest_migration(pool).await?;
        mark_step(pool, 3).await?;
    }
    if !step_applied(pool, 4).await {
        apply_capture_initiative_graph_migration(pool).await?;
        mark_step(pool, 4).await?;
    }
    if !step_applied(pool, 5).await {
        apply_deliverable_enhancements_migration(pool).await?;
        mark_step(pool, 5).await?;
    }
    if !step_applied(pool, 6).await {
        sqlx::raw_sql(WEEK_PLAN_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 6 (week plan): {e}"))?;
        mark_step(pool, 6).await?;
    }
    if !step_applied(pool, 7).await {
        sqlx::raw_sql(MEETINGS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 7 (meetings): {e}"))?;
        mark_step(pool, 7).await?;
    }
    if !step_applied(pool, 8).await {
        apply_meeting_action_kinds_migration(pool).await?;
        mark_step(pool, 8).await?;
    }
    if !step_applied(pool, 9).await {
        apply_meeting_action_payload_migration(pool).await?;
        mark_step(pool, 9).await?;
    }
    if !step_applied(pool, 10).await {
        apply_stakeholder_profile_migration(pool).await?;
        mark_step(pool, 10).await?;
    }
    if !step_applied(pool, 11).await {
        sqlx::raw_sql(DELIVERABLE_STAKEHOLDERS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 11 (deliverable stakeholders): {e}"))?;
        mark_step(pool, 11).await?;
    }
    if !step_applied(pool, 12).await {
        apply_gantt_migration(pool).await?;
        mark_step(pool, 12).await?;
    }
    if !step_applied(pool, 13).await {
        apply_board_enhancements_migration(pool).await?;
        mark_step(pool, 13).await?;
    }
    if !step_applied(pool, 14).await {
        apply_gmail_workspace_migration(pool).await?;
        mark_step(pool, 14).await?;
    }
    if !step_applied(pool, 15).await {
        apply_memory_system_migration(pool).await?;
        mark_step(pool, 15).await?;
    }
    if !step_applied(pool, 16).await {
        apply_memory_retrieval_migration(pool).await?;
        mark_step(pool, 16).await?;
    }
    if !step_applied(pool, 17).await {
        apply_ask_chats_migration(pool).await?;
        mark_step(pool, 17).await?;
    }
    if !step_applied(pool, 18).await {
        apply_email_categories_work_intake_migration(pool).await?;
        mark_step(pool, 18).await?;
    }
    if !step_applied(pool, 19).await {
        apply_task_enhancements_migration(pool).await?;
        mark_step(pool, 19).await?;
    }
    if !step_applied(pool, 20).await {
        apply_captures_suggested_migration(pool).await?;
        mark_step(pool, 20).await?;
    }
    if !step_applied(pool, 21).await {
        apply_deliverable_type_expansion_migration(pool).await?;
        mark_step(pool, 21).await?;
    }
    if !step_applied(pool, 22).await {
        apply_files_migration(pool).await?;
        mark_step(pool, 22).await?;
    }
    if !step_applied(pool, 23).await {
        apply_file_embeddings_migration(pool).await?;
        mark_step(pool, 23).await?;
    }
    if !step_applied(pool, 24).await {
        sqlx::raw_sql(FOLDER_ENTITY_LINKS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 24 (folder entity links): {e}"))?;
        mark_step(pool, 24).await?;
    }
    if !step_applied(pool, 25).await {
        apply_gmeet_folder_migration(pool).await?;
        mark_step(pool, 25).await?;
    }
    if !step_applied(pool, 26).await {
        sqlx::raw_sql(GCAL_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 26 (google calendar): {e}"))?;
        mark_step(pool, 26).await?;
    }
    if !step_applied(pool, 27).await {
        sqlx::raw_sql(GCAL_ATTENDEES_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 27 (gcal attendees/location): {e}"))?;
        mark_step(pool, 27).await?;
    }
    if !step_applied(pool, 28).await {
        sqlx::raw_sql(GCAL_FULL_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 28 (gcal full fields): {e}"))?;
        mark_step(pool, 28).await?;
    }
    if !step_applied(pool, 29).await {
        sqlx::raw_sql(INITIATIVE_ICON_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 29 (initiative icon): {e}"))?;
        mark_step(pool, 29).await?;
    }
    if !step_applied(pool, 30).await {
        sqlx::raw_sql(USER_PROFILE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 30 (user profile): {e}"))?;
        mark_step(pool, 30).await?;
    }
    if !step_applied(pool, 31).await {
        apply_stakeholder_stamps_migration(pool).await?;
        mark_step(pool, 31).await?;
    }
    if !step_applied(pool, 32).await {
        sqlx::raw_sql(MEETING_STAKEHOLDERS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 32 (meeting stakeholders): {e}"))?;
        mark_step(pool, 32).await?;
    }
    if !step_applied(pool, 33).await {
        apply_capture_task_promotion_migration(pool).await?;
        mark_step(pool, 33).await?;
    }
    if !step_applied(pool, 34).await {
        sqlx::raw_sql(SECOND_BRAIN_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 34 (second brain): {e}"))?;
        mark_step(pool, 34).await?;
    }
    if !step_applied(pool, 35).await {
        sqlx::raw_sql(BRAIN_RL_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 35 (brain rl): {e}"))?;
        mark_step(pool, 35).await?;
    }
    if !step_applied(pool, 36).await {
        apply_gmail_intelligence_migration(pool).await?;
        mark_step(pool, 36).await?;
    }
    if !step_applied(pool, 37).await {
        sqlx::raw_sql(TOOL_CALL_LOG_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 37 (tool call log): {e}"))?;
        mark_step(pool, 37).await?;
    }
    if !step_applied(pool, 38).await {
        sqlx::raw_sql(GEMINI_USAGE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 38 (gemini usage): {e}"))?;
        mark_step(pool, 38).await?;
    }
    if !step_applied(pool, 39).await {
        sqlx::raw_sql(EVALS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 39 (evals): {e}"))?;
        mark_step(pool, 39).await?;
    }
    if !step_applied(pool, 40).await {
        sqlx::raw_sql(ENTITY_EMBEDDINGS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 40 (entity embeddings): {e}"))?;
        mark_step(pool, 40).await?;
    }
    if !step_applied(pool, 41).await {
        sqlx::raw_sql(GEMINI_EMBEDDING_2_MIGRATION_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 41 (gemini embedding 2 cleanup): {e}"))?;
        mark_step(pool, 41).await?;
    }
    if !step_applied(pool, 42).await {
        sqlx::raw_sql(GMAIL_USER_CLASSIFICATIONS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 42 (gmail user classifications): {e}"))?;
        mark_step(pool, 42).await?;
    }
    // Always run — `ensure_table_column` is idempotent and self-healing if the
    // step_applied tracker was marked before the columns actually landed (which
    // could happen on builds with the earlier syntax bug).
    apply_gmail_classification_dimensions(pool).await?;
    if !step_applied(pool, 43).await {
        mark_step(pool, 43).await?;
    }
    apply_gmail_override_extras(pool).await?;
    if !step_applied(pool, 44).await {
        mark_step(pool, 44).await?;
    }
    if !step_applied(pool, 45).await {
        sqlx::raw_sql(APP_CONFIG_SETTINGS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 45 (app_config_settings): {e}"))?;
        mark_step(pool, 45).await?;
    }
    if !step_applied(pool, 46).await {
        sqlx::raw_sql(RETRIEVAL_RL_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 46 (retrieval_rl): {e}"))?;
        mark_step(pool, 46).await?;
    }
    if !step_applied(pool, 47).await {
        sqlx::raw_sql(LOCAL_EMAIL_DRAFTS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 47 (local_email_drafts): {e}"))?;
        mark_step(pool, 47).await?;
    }
    if !step_applied(pool, 48).await {
        sqlx::raw_sql(GMAIL_ANALYSIS_HISTORY_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 48 (gmail_thread_analysis_history): {e}"))?;
        mark_step(pool, 48).await?;
    }
    if !step_applied(pool, 50).await {
        sqlx::raw_sql(CAPTURE_PROMOTION_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 50 (capture_promotion_suggestions): {e}"))?;
        mark_step(pool, 50).await?;
    }
    if !step_applied(pool, 51).await {
        sqlx::raw_sql(PROMPT_INJECTION_LOG_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 51 (prompt_injection_log): {e}"))?;
        mark_step(pool, 51).await?;
    }
    // Always run — additive ALTER TABLE ADD COLUMN calls are made
    // idempotent via `ensure_table_column`. Same self-healing pattern as
    // the 0044/0045 gmail dimensions migration.
    apply_brain_inference_supersede(pool).await?;

    // Section 6.2 — brain_inferences.template column + backfill.
    apply_brain_inference_template(pool).await?;
    if !step_applied(pool, 52).await {
        sqlx::raw_sql(BRAIN_INFERENCE_TEMPLATE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 52 (brain_inference_template backfill): {e}"))?;
        mark_step(pool, 52).await?;
    }

    // Section 6.2 — ask_turns.scored_nodes_json + retrieval_query columns.
    apply_ask_turn_scored_nodes(pool).await?;
    if !step_applied(pool, 53).await {
        sqlx::raw_sql(ASK_TURN_SCORED_NODES_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 53 (ask_turn_scored_nodes): {e}"))?;
        mark_step(pool, 53).await?;
    }
    if !step_applied(pool, 54).await {
        sqlx::raw_sql(WORK_MAIL_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 54 (work_mail): {e}"))?;
        mark_step(pool, 54).await?;
    }
    if !step_applied(pool, 55).await {
        sqlx::raw_sql(WORK_MAIL_REVIEW_STATE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 55 (work_mail_review_state): {e}"))?;
        mark_step(pool, 55).await?;
    }
    if !step_applied(pool, 56).await {
        sqlx::raw_sql(REASONING_REPORTS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 56 (reasoning_reports): {e}"))?;
        mark_step(pool, 56).await?;
    }
    if !step_applied(pool, 57).await {
        sqlx::raw_sql(REASONING_REPORTS_UPGRADE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 57 (reasoning_reports_upgrade): {e}"))?;
        mark_step(pool, 57).await?;
    }
    // Step 56 was exercised in development before all run telemetry columns
    // existed. Keep this additive repair idempotent for those databases.
    apply_reasoning_reports_upgrade(pool).await?;

    if !step_applied(pool, 58).await {
        sqlx::raw_sql(INITIATIVE_FILE_CALENDAR_LINKS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 58 (initiative_file_calendar_links): {e}"))?;
        mark_step(pool, 58).await?;
    }
    if !step_applied(pool, 59).await {
        sqlx::raw_sql(REPORT_STEPS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 59 (report_steps): {e}"))?;
        mark_step(pool, 59).await?;
    }
    apply_report_runs_step_columns(pool).await?;

    if !step_applied(pool, 60).await {
        sqlx::raw_sql(BRAIN_SAVED_VIEWS_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 60 (brain_saved_views): {e}"))?;
        mark_step(pool, 60).await?;
    }

    if !step_applied(pool, 61).await {
        sqlx::raw_sql(APPLE_NOTES_SYNC_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 61 (apple_notes_sync): {e}"))?;
        mark_step(pool, 61).await?;
    }

    if !step_applied(pool, 62).await {
        sqlx::raw_sql(BRAIN_LAYOUT_CACHE_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("step 62 (brain_layout_cache): {e}"))?;
        mark_step(pool, 62).await?;
    }

    Ok(())
}

async fn apply_report_runs_step_columns(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("scope_exclusions_json", "TEXT NOT NULL DEFAULT '[]'"),
        ("sections_json", "TEXT NOT NULL DEFAULT '[]'"),
        ("section_drafts_json", "TEXT NOT NULL DEFAULT '{}'"),
        ("critique_json", "TEXT NOT NULL DEFAULT '{}'"),
    ] {
        ensure_table_column(
            pool,
            "report_runs",
            column,
            &format!("ALTER TABLE report_runs ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    Ok(())
}

async fn apply_reasoning_reports_upgrade(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("action_proposals_json", "TEXT NOT NULL DEFAULT '[]'"),
        ("cache_hit", "INTEGER NOT NULL DEFAULT 0"),
        ("latency_ms", "INTEGER NOT NULL DEFAULT 0"),
    ] {
        ensure_table_column(
            pool,
            "reasoning_runs",
            column,
            &format!("ALTER TABLE reasoning_runs ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    Ok(())
}

async fn apply_brain_inference_template(pool: &SqlitePool) -> Result<(), String> {
    ensure_table_column(
        pool,
        "brain_inferences",
        "template",
        "ALTER TABLE brain_inferences ADD COLUMN template TEXT",
    )
    .await?;
    Ok(())
}

async fn apply_ask_turn_scored_nodes(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [("scored_nodes_json", "TEXT"), ("retrieval_query", "TEXT")] {
        ensure_table_column(
            pool,
            "ask_turns",
            column,
            &format!("ALTER TABLE ask_turns ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    Ok(())
}

async fn apply_brain_inference_supersede(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [("superseded_by", "TEXT"), ("supersede_reason", "TEXT")] {
        ensure_table_column(
            pool,
            "brain_inferences",
            column,
            &format!("ALTER TABLE brain_inferences ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    sqlx::raw_sql(
        "CREATE INDEX IF NOT EXISTS idx_brain_inferences_superseded_by
            ON brain_inferences (superseded_by) WHERE superseded_by IS NOT NULL",
    )
    .execute(pool)
    .await
    .map_err(|e| format!("brain inferences superseded_by index: {e}"))?;
    Ok(())
}

async fn apply_gmail_override_extras(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("intent", "TEXT"),
        ("action_required", "INTEGER"),
        ("thread_state", "TEXT"),
    ] {
        ensure_table_column(
            pool,
            "gmail_user_classifications",
            column,
            &format!("ALTER TABLE gmail_user_classifications ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    Ok(())
}

async fn apply_gmail_classification_dimensions(pool: &SqlitePool) -> Result<(), String> {
    // Add columns idempotently — SQLite can't IF NOT EXISTS on ALTER TABLE,
    // so check the column list first.
    for (column, definition) in [
        ("intent", "TEXT"),
        ("action_required", "INTEGER NOT NULL DEFAULT 0"),
        ("predicted_action", "TEXT"),
        ("thread_state", "TEXT"),
        ("dimensions_confidence_json", "TEXT NOT NULL DEFAULT '{}'"),
        ("bundle_id", "TEXT"),
    ] {
        ensure_table_column(
            pool,
            "gmail_threads",
            column,
            &format!("ALTER TABLE gmail_threads ADD COLUMN {column} {definition}"),
        )
        .await?;
    }
    for stmt in [
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_intent ON gmail_threads (intent)",
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_thread_state ON gmail_threads (thread_state)",
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_bundle ON gmail_threads (bundle_id)",
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_action_required ON gmail_threads (action_required, last_message_at DESC)",
    ] {
        sqlx::raw_sql(stmt)
            .execute(pool)
            .await
            .map_err(|e| format!("classification dimensions index: {e}"))?;
    }
    Ok(())
}

pub async fn apply_gmail_intelligence_migration(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("last_analyzed_message_at", "INTEGER"),
        ("last_analyzed_message_count", "INTEGER NOT NULL DEFAULT 0"),
        ("graph_context_json", "TEXT NOT NULL DEFAULT '{}'"),
        ("effective_priority", "TEXT NOT NULL DEFAULT 'low'"),
        ("priority_reasons_json", "TEXT NOT NULL DEFAULT '[]'"),
        ("intelligence_updated_at", "TEXT"),
        ("last_analysis_error", "TEXT"),
    ] {
        ensure_table_column(
            pool,
            "gmail_threads",
            column,
            &format!("ALTER TABLE gmail_threads ADD COLUMN {column} {definition}"),
        )
        .await?;
    }

    for (table, column, definition) in [
        ("gmail_thread_deliverables", "confidence", "REAL"),
        (
            "gmail_thread_deliverables",
            "rationale",
            "TEXT NOT NULL DEFAULT ''",
        ),
        ("gmail_thread_initiatives", "confidence", "REAL"),
        (
            "gmail_thread_initiatives",
            "rationale",
            "TEXT NOT NULL DEFAULT ''",
        ),
    ] {
        ensure_table_column(
            pool,
            table,
            column,
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        )
        .await?;
    }

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS gmail_thread_stakeholders (
          thread_id      TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
          stakeholder_id TEXT NOT NULL REFERENCES stakeholders(id) ON DELETE CASCADE,
          linked_at      TEXT NOT NULL,
          source         TEXT NOT NULL DEFAULT 'auto',
          confidence     REAL,
          rationale      TEXT NOT NULL DEFAULT '',
          PRIMARY KEY (thread_id, stakeholder_id)
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_gmail_thread_stakeholders_stakeholder ON gmail_thread_stakeholders (stakeholder_id, linked_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS gmail_thread_link_suggestions (
          id            TEXT PRIMARY KEY,
          thread_id     TEXT NOT NULL REFERENCES gmail_threads(thread_id) ON DELETE CASCADE,
          target_kind   TEXT NOT NULL CHECK (target_kind IN ('stakeholder','deliverable','initiative')),
          target_id     TEXT NOT NULL,
          target_title  TEXT NOT NULL DEFAULT '',
          confidence    REAL NOT NULL DEFAULT 0,
          rationale     TEXT NOT NULL DEFAULT '',
          status        TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','accepted','rejected')),
          created_at    TEXT NOT NULL,
          updated_at    TEXT NOT NULL,
          resolved_at   TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_gmail_thread_link_suggestions_unique_pending ON gmail_thread_link_suggestions (thread_id, target_kind, target_id, status)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_gmail_thread_link_suggestions_thread ON gmail_thread_link_suggestions (thread_id, status, confidence DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_effective_priority ON gmail_threads (effective_priority, last_message_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_capture_task_promotion_migration(pool: &SqlitePool) -> Result<(), String> {
    let has_task_id: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'promoted_task_id'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_task_id == 0 {
        sqlx::query(
            "ALTER TABLE captures ADD COLUMN promoted_task_id TEXT REFERENCES deliverable_tasks(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await
        .map_err(sql_error)?;

        sqlx::query("ALTER TABLE captures ADD COLUMN promoted_task_title TEXT")
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    Ok(())
}

pub async fn apply_gmeet_folder_migration(pool: &SqlitePool) -> Result<(), String> {
    for col in ["gmeet_folder_id", "gmeet_folder_name"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('google_drive_settings') WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool)
        .await
        .unwrap_or(0);
        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE google_drive_settings ADD COLUMN {col} TEXT"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

pub async fn apply_files_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(FILES_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply files schema: {e}"))?;

    // Guard: ALTER TABLE ADD COLUMN fails if the column already exists.
    let has_col: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('files') WHERE name = 'is_missing'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    if has_col == 0 {
        sqlx::raw_sql(FILES_WATCHER_SQL)
            .execute(pool)
            .await
            .map_err(|e| format!("failed to apply files_watcher schema: {e}"))?;
    }
    Ok(())
}

pub async fn apply_file_embeddings_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(FILE_EMBEDDINGS_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply file_embeddings schema: {e}"))?;
    Ok(())
}

pub async fn apply_deliverable_type_expansion_migration(pool: &SqlitePool) -> Result<(), String> {
    let create_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'deliverables'",
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let Some(create_sql) = create_sql else {
        return Ok(());
    };

    if create_sql.contains("'spec'") {
        return Ok(());
    }

    let mut conn = pool.acquire().await.map_err(sql_error)?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .map_err(sql_error)?;
    let mut tx = (&mut *conn).begin().await.map_err(sql_error)?;

    for trigger in ["deliverables_ai", "deliverables_ad", "deliverables_au"] {
        sqlx::query(&format!("DROP TRIGGER IF EXISTS {trigger}"))
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
    }

    sqlx::query("DROP TABLE IF EXISTS deliverables_new")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    sqlx::query(
        r#"CREATE TABLE deliverables_new (
          id               TEXT PRIMARY KEY,
          title            TEXT NOT NULL,
          type             TEXT NOT NULL CHECK (type IN (
            'deck','design_doc','prototype','analysis','framework',
            'pitch','research','code','email','meeting_prep',
            'spec','report','roadmap','brief','plan','other'
          )),
          state            TEXT NOT NULL CHECK (state IN (
            'backlog','todo','drafting','in_review','shipped','killed'
          )),
          claim            TEXT NOT NULL DEFAULT '',
          artifact_url     TEXT,
          conversation_id  TEXT,
          stakeholder_id   TEXT,
          created_at       TEXT NOT NULL,
          shipped_at       TEXT,
          updated_at       TEXT NOT NULL,
          deadline         TEXT,
          is_focused       INTEGER NOT NULL DEFAULT 0,
          effort           INTEGER,
          impact           INTEGER,
          blocker_reason   TEXT,
          start_date       TEXT,
          section_id       TEXT,
          state_changed_at TEXT,
          display_order    INTEGER NOT NULL DEFAULT 0,
          priority         TEXT
        )"#,
    )
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        r#"INSERT INTO deliverables_new
          SELECT id, title, type, state, claim, artifact_url, conversation_id,
                 stakeholder_id, created_at, shipped_at, updated_at, deadline,
                 is_focused, effort, impact, blocker_reason, start_date, section_id,
                 state_changed_at, display_order, priority
          FROM deliverables"#,
    )
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;

    sqlx::query("DROP TABLE deliverables")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    sqlx::query("ALTER TABLE deliverables_new RENAME TO deliverables")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

    for sql in [
        "CREATE INDEX IF NOT EXISTS idx_deliverables_state_updated ON deliverables (state, updated_at DESC)",
        "CREATE INDEX IF NOT EXISTS idx_deliverables_stakeholder ON deliverables (stakeholder_id)",
        "CREATE INDEX IF NOT EXISTS idx_deliverables_conversation ON deliverables (conversation_id)",
    ] {
        sqlx::query(sql).execute(&mut *tx).await.map_err(sql_error)?;
    }

    for sql in [
        r#"CREATE TRIGGER IF NOT EXISTS deliverables_ai
AFTER INSERT ON deliverables
BEGIN
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END"#,
        r#"CREATE TRIGGER IF NOT EXISTS deliverables_ad
AFTER DELETE ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
END"#,
        r#"CREATE TRIGGER IF NOT EXISTS deliverables_au
AFTER UPDATE OF title, claim ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END"#,
    ] {
        sqlx::query(sql)
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
    }

    tx.commit().await.map_err(sql_error)?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await
        .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_captures_suggested_migration(pool: &SqlitePool) -> Result<(), String> {
    let captures_schema: Option<String> =
        sqlx::query_scalar("SELECT sql FROM sqlite_master WHERE type='table' AND name='captures'")
            .fetch_optional(pool)
            .await
            .map_err(sql_error)?
            .flatten();

    let captures_v2_schema: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='captures_v2'",
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?
    .flatten();

    let captures_has_new = captures_schema
        .as_deref()
        .map_or(false, |s| s.contains("suggested"));

    if captures_has_new {
        // Already migrated. Clean up any stale captures_v2 left over from a prior failure.
        if captures_v2_schema.is_some() {
            sqlx::query("DROP TABLE IF EXISTS captures_v2")
                .execute(pool)
                .await
                .map_err(|e| format!("drop stale captures_v2 failed: {e}"))?;
        }
        return Ok(());
    }

    // Use a single dedicated connection so PRAGMA foreign_keys=OFF and the
    // table swap all run on the same session (PRAGMA is per-connection in
    // SQLite — applying it on the pool only affects whichever connection
    // happened to handle that one query).
    let mut conn = pool.acquire().await.map_err(sql_error)?;

    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .map_err(sql_error)?;

    let mut tx = (&mut *conn).begin().await.map_err(sql_error)?;

    sqlx::query("DROP TABLE IF EXISTS captures_v2")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("drop captures_v2 failed: {e}"))?;

    sqlx::query(
        r#"
        CREATE TABLE captures_v2 (
            id                       TEXT PRIMARY KEY,
            kind                     TEXT NOT NULL,
            body                     TEXT NOT NULL,
            status                   TEXT NOT NULL DEFAULT 'inbox'
                                     CHECK (status IN ('inbox','promoted','dismissed','suggested')),
            promoted_deliverable_id  TEXT REFERENCES deliverables(id) ON DELETE SET NULL,
            created_at               TEXT NOT NULL,
            updated_at               TEXT NOT NULL,
            promoted_at              TEXT,
            promoted_initiative_id   TEXT REFERENCES initiatives(id) ON DELETE SET NULL,
            promoted_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("create captures_v2 failed: {e}"))?;

    if captures_schema.is_some() {
        sqlx::query(
            r#"
            INSERT INTO captures_v2
                SELECT id, kind, body, status, promoted_deliverable_id,
                       created_at, updated_at, promoted_at,
                       promoted_initiative_id, promoted_conversation_id
                FROM captures
            "#,
        )
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("copy captures failed: {e}"))?;

        sqlx::query("DROP TABLE captures")
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("drop captures failed: {e}"))?;
    }

    sqlx::query("ALTER TABLE captures_v2 RENAME TO captures")
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("rename captures_v2 failed: {e}"))?;

    for sql in [
        "CREATE INDEX IF NOT EXISTS idx_captures_status_created ON captures (status, created_at DESC)",
        "CREATE INDEX IF NOT EXISTS idx_captures_kind_status ON captures (kind, status)",
        "CREATE INDEX IF NOT EXISTS idx_captures_promoted_deliverable ON captures (promoted_deliverable_id)",
        "CREATE INDEX IF NOT EXISTS idx_captures_promoted_initiative ON captures (promoted_initiative_id)",
        "CREATE INDEX IF NOT EXISTS idx_captures_promoted_conversation ON captures (promoted_conversation_id)",
    ] {
        sqlx::query(sql)
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;
    }

    tx.commit().await.map_err(sql_error)?;

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *conn)
        .await
        .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_task_enhancements_migration(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [("notes", "TEXT"), ("url", "TEXT")] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('deliverable_tasks') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE deliverable_tasks ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

pub async fn apply_ask_chats_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(ASK_CHATS_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply ask_chats schema: {e}"))?;
    Ok(())
}

pub async fn apply_email_categories_work_intake_migration(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("ai_category", "TEXT NOT NULL DEFAULT 'other'"),
        ("ai_priority", "TEXT NOT NULL DEFAULT 'low'"),
        ("ai_category_confidence", "REAL"),
        ("ai_category_reasons", "TEXT NOT NULL DEFAULT '[]'"),
        ("ai_triaged_at", "TEXT"),
    ] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('gmail_threads') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE gmail_threads ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_gmail_threads_ai_category ON gmail_threads (ai_category, last_message_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS work_intake_suggestions (
          id                    TEXT PRIMARY KEY,
          source_kind           TEXT NOT NULL,
          source_id             TEXT,
          source_title          TEXT NOT NULL DEFAULT '',
          source_route          TEXT,
          item_kind             TEXT NOT NULL CHECK (item_kind IN ('task','deliverable','initiative')),
          title                 TEXT NOT NULL,
          body                  TEXT NOT NULL DEFAULT '',
          target_deliverable_id TEXT,
          target_initiative_id  TEXT,
          due_date              TEXT,
          suggested_type        TEXT,
          confidence            REAL,
          rationale             TEXT NOT NULL DEFAULT '',
          status                TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending','approved','dismissed')),
          payload               TEXT NOT NULL DEFAULT '{}',
          created_at            TEXT NOT NULL,
          updated_at            TEXT NOT NULL,
          applied_at            TEXT
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_work_intake_status ON work_intake_suggestions (status, created_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_work_intake_source ON work_intake_suggestions (source_kind, source_id, status)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

pub async fn apply_conversations_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS conversations (
          id          TEXT PRIMARY KEY,
          chat_url    TEXT NOT NULL UNIQUE,
          title       TEXT,
          summary     TEXT,
          occurred_at TEXT,
          ingested_at TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    let has_conversation_id: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pragma_table_info('deliverables')
        WHERE name = 'conversation_id'
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_conversation_id == 0 {
        sqlx::query(
            "ALTER TABLE deliverables ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_conversations_chat_url ON conversations (chat_url)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_deliverables_conversation ON deliverables (conversation_id)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_conversation_ingest_migration(pool: &SqlitePool) -> Result<(), String> {
    let has_promoted_conversation_id: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pragma_table_info('captures')
        WHERE name = 'promoted_conversation_id'
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_promoted_conversation_id == 0 {
        sqlx::query(
            "ALTER TABLE captures ADD COLUMN promoted_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_captures_promoted_conversation ON captures (promoted_conversation_id)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_conversations_ingested_at ON conversations (ingested_at DESC)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_capture_initiative_graph_migration(pool: &SqlitePool) -> Result<(), String> {
    let has_promoted_initiative_id: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM pragma_table_info('captures')
        WHERE name = 'promoted_initiative_id'
        "#,
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_promoted_initiative_id == 0 {
        sqlx::query(
            "ALTER TABLE captures ADD COLUMN promoted_initiative_id TEXT REFERENCES initiatives(id) ON DELETE SET NULL",
        )
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_captures_promoted_initiative ON captures (promoted_initiative_id)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_deliverable_enhancements_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(DELIVERABLE_ENHANCEMENTS_SQL)
        .execute(pool)
        .await
        .map_err(|error| format!("failed to apply deliverable enhancements schema: {error}"))?;

    for (column, definition) in [
        ("deadline", "TEXT"),
        ("is_focused", "INTEGER NOT NULL DEFAULT 0"),
        ("effort", "INTEGER"),
        ("impact", "INTEGER"),
        ("blocker_reason", "TEXT"),
    ] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('deliverables') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE deliverables ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }

    Ok(())
}

pub async fn apply_stakeholder_profile_migration(pool: &SqlitePool) -> Result<(), String> {
    for column in ["role", "notes"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('stakeholders') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE stakeholders ADD COLUMN {column} TEXT NOT NULL DEFAULT ''"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

pub async fn apply_stakeholder_stamps_migration(pool: &SqlitePool) -> Result<(), String> {
    for (column, definition) in [
        ("avatar_url", "TEXT NOT NULL DEFAULT ''"),
        ("created_at", "TEXT NOT NULL DEFAULT '2026-05-12T00:00:00Z'"),
        ("updated_at", "TEXT NOT NULL DEFAULT '2026-05-12T00:00:00Z'"),
    ] {
        let exists = sqlx::query(&format!("SELECT {} FROM stakeholders LIMIT 0", column))
            .execute(pool)
            .await
            .is_ok();

        if !exists {
            sqlx::query(&format!(
                "ALTER TABLE stakeholders ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }
    Ok(())
}

pub async fn apply_gantt_migration(pool: &SqlitePool) -> Result<(), String> {
    // initiative_sections table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS initiative_sections (
          id            TEXT PRIMARY KEY,
          initiative_id TEXT NOT NULL REFERENCES initiatives(id) ON DELETE CASCADE,
          title         TEXT NOT NULL,
          position      INTEGER NOT NULL DEFAULT 0,
          created_at    TEXT NOT NULL,
          updated_at    TEXT NOT NULL
        )
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_initiative_sections_initiative ON initiative_sections (initiative_id, position)",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Add start_date column to deliverables (idempotent)
    for (column, definition) in [("start_date", "TEXT"), ("section_id", "TEXT")] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('deliverables') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE deliverables ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }

    Ok(())
}

pub async fn apply_meeting_action_kinds_migration(pool: &SqlitePool) -> Result<(), String> {
    let create_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'meeting_actions'",
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let Some(create_sql) = create_sql else {
        return Ok(());
    };

    if !create_sql.contains("CHECK (kind IN") || create_sql.contains("'note_added'") {
        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_meeting_actions_meeting ON meeting_actions (meeting_id)",
        )
        .execute(pool)
        .await
        .map_err(sql_error)?;
        return Ok(());
    }

    let mut tx = pool.begin().await.map_err(sql_error)?;
    sqlx::query("DROP TABLE IF EXISTS meeting_actions_new")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    sqlx::query(
        r#"
        CREATE TABLE meeting_actions_new (
          id              TEXT PRIMARY KEY,
          meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
          kind            TEXT NOT NULL,
          target_id       TEXT,
          target_title    TEXT,
          body            TEXT NOT NULL,
          applied         INTEGER NOT NULL DEFAULT 0,
          created_at      TEXT NOT NULL
        )
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;
    sqlx::query(
        r#"
        INSERT INTO meeting_actions_new
          (id, meeting_id, kind, target_id, target_title, body, applied, created_at)
        SELECT
          id, meeting_id, kind, target_id, target_title, body, applied, created_at
        FROM meeting_actions
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;
    sqlx::query("DROP TABLE meeting_actions")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    sqlx::query("ALTER TABLE meeting_actions_new RENAME TO meeting_actions")
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_meeting_actions_meeting ON meeting_actions (meeting_id)",
    )
    .execute(&mut *tx)
    .await
    .map_err(sql_error)?;
    tx.commit().await.map_err(sql_error)?;
    Ok(())
}

pub async fn apply_meeting_action_payload_migration(pool: &SqlitePool) -> Result<(), String> {
    let has_payload: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('meeting_actions') WHERE name = 'payload'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_payload == 0 {
        sqlx::query("ALTER TABLE meeting_actions ADD COLUMN payload TEXT")
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    Ok(())
}

pub async fn apply_board_enhancements_migration(pool: &SqlitePool) -> Result<(), String> {
    // Add new scalar columns (idempotent via pragma_table_info check)
    for (column, definition) in [
        ("state_changed_at", "TEXT"),
        ("display_order", "INTEGER NOT NULL DEFAULT 0"),
        ("priority", "TEXT"),
    ] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('deliverables') WHERE name = ?",
        )
        .bind(column)
        .fetch_one(pool)
        .await
        .map_err(sql_error)?;

        if exists == 0 {
            sqlx::query(&format!(
                "ALTER TABLE deliverables ADD COLUMN {column} {definition}"
            ))
            .execute(pool)
            .await
            .map_err(sql_error)?;
        }
    }

    // Backfill state_changed_at for rows that have it null
    sqlx::query(
        "UPDATE deliverables SET state_changed_at = updated_at WHERE state_changed_at IS NULL",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    // Recreate the deliverables table to update the state CHECK constraint
    // to include 'backlog' and 'todo'.  Only needed when the old constraint is still in place.
    let create_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'deliverables'",
    )
    .fetch_optional(pool)
    .await
    .map_err(sql_error)?;

    let Some(create_sql) = create_sql else {
        return Ok(());
    };

    // If the new states are already present the migration already ran.
    if !create_sql.contains("'backlog'") {
        let mut conn = pool.acquire().await.map_err(sql_error)?;
        // PRAGMA foreign_keys cannot be changed inside a transaction.  Disable it on
        // a dedicated connection before beginning the transaction so that
        // `DROP TABLE deliverables` does not fire ON DELETE CASCADE on child tables
        // (deliverable_tasks, deliverable_notes) and ON DELETE SET NULL on week_plans.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *conn)
            .await
            .map_err(sql_error)?;
        let mut tx = (&mut *conn).begin().await.map_err(sql_error)?;

        for trigger in ["deliverables_ai", "deliverables_ad", "deliverables_au"] {
            sqlx::query(&format!("DROP TRIGGER IF EXISTS {trigger}"))
                .execute(&mut *tx)
                .await
                .map_err(sql_error)?;
        }

        sqlx::query(
            r#"CREATE TABLE deliverables_new (
              id               TEXT PRIMARY KEY,
              title            TEXT NOT NULL,
              type             TEXT NOT NULL CHECK (type IN (
                'deck','design_doc','prototype','analysis','framework',
                'pitch','research','code','email','meeting_prep','other'
              )),
              state            TEXT NOT NULL CHECK (state IN (
                'backlog','todo','drafting','in_review','shipped','killed'
              )),
              claim            TEXT NOT NULL DEFAULT '',
              artifact_url     TEXT,
              conversation_id  TEXT,
              stakeholder_id   TEXT,
              created_at       TEXT NOT NULL,
              shipped_at       TEXT,
              updated_at       TEXT NOT NULL,
              deadline         TEXT,
              is_focused       INTEGER NOT NULL DEFAULT 0,
              effort           INTEGER,
              impact           INTEGER,
              blocker_reason   TEXT,
              start_date       TEXT,
              section_id       TEXT,
              state_changed_at TEXT,
              display_order    INTEGER NOT NULL DEFAULT 0,
              priority         TEXT
            )"#,
        )
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

        sqlx::query(
            r#"INSERT INTO deliverables_new
              SELECT id, title, type, state, claim, artifact_url, conversation_id,
                     stakeholder_id, created_at, shipped_at, updated_at, deadline,
                     is_focused, effort, impact, blocker_reason, start_date, section_id,
                     state_changed_at, display_order, priority
              FROM deliverables"#,
        )
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

        sqlx::query("DROP TABLE deliverables")
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;

        sqlx::query("ALTER TABLE deliverables_new RENAME TO deliverables")
            .execute(&mut *tx)
            .await
            .map_err(sql_error)?;

        for sql in [
            "CREATE INDEX IF NOT EXISTS idx_deliverables_state_updated ON deliverables (state, updated_at DESC)",
            "CREATE INDEX IF NOT EXISTS idx_deliverables_stakeholder ON deliverables (stakeholder_id)",
            "CREATE INDEX IF NOT EXISTS idx_deliverables_conversation ON deliverables (conversation_id)",
        ] {
            sqlx::query(sql).execute(&mut *tx).await.map_err(sql_error)?;
        }

        sqlx::query(
            r#"CREATE TRIGGER IF NOT EXISTS deliverables_ai
AFTER INSERT ON deliverables
BEGIN
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END"#,
        )
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

        sqlx::query(
            r#"CREATE TRIGGER IF NOT EXISTS deliverables_ad
AFTER DELETE ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
END"#,
        )
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

        sqlx::query(
            r#"CREATE TRIGGER IF NOT EXISTS deliverables_au
AFTER UPDATE OF title, claim ON deliverables
BEGIN
  INSERT INTO deliverable_search(deliverable_search, rowid, deliverable_id, title, claim)
  VALUES ('delete', old.rowid, old.id, old.title, old.claim);
  INSERT INTO deliverable_search(rowid, deliverable_id, title, claim)
  VALUES (new.rowid, new.id, new.title, new.claim);
END"#,
        )
        .execute(&mut *tx)
        .await
        .map_err(sql_error)?;

        tx.commit().await.map_err(sql_error)?;
        // Restore FK enforcement on this connection before it returns to the pool.
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&mut *conn)
            .await
            .map_err(sql_error)?;
    }

    // Labels + state-history tables (from 0015 SQL file, safe to run multiple times)
    sqlx::raw_sql(BOARD_ENHANCEMENTS_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply board enhancements schema: {e}"))?;

    Ok(())
}

pub async fn apply_gmail_workspace_migration(pool: &SqlitePool) -> Result<(), String> {
    let has_email: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('stakeholders') WHERE name = 'email'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_email == 0 {
        sqlx::query("ALTER TABLE stakeholders ADD COLUMN email TEXT NOT NULL DEFAULT ''")
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_stakeholders_email_unique
         ON stakeholders (email) WHERE email != ''",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    sqlx::raw_sql(GMAIL_WORKSPACE_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply gmail workspace schema: {e}"))?;

    let has_account_email: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('gmail_sync_settings') WHERE name = 'account_email'",
    )
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;

    if has_account_email == 0 {
        sqlx::query("ALTER TABLE gmail_sync_settings ADD COLUMN account_email TEXT")
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }

    ensure_gmail_sync_column(
        pool,
        "backfill_enabled",
        "ALTER TABLE gmail_sync_settings ADD COLUMN backfill_enabled INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "relevance_filter_enabled",
        "ALTER TABLE gmail_sync_settings ADD COLUMN relevance_filter_enabled INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "auto_analyze_enabled",
        "ALTER TABLE gmail_sync_settings ADD COLUMN auto_analyze_enabled INTEGER NOT NULL DEFAULT 1",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "auto_analyze_limit",
        "ALTER TABLE gmail_sync_settings ADD COLUMN auto_analyze_limit INTEGER NOT NULL DEFAULT 6",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "backfill_page_token",
        "ALTER TABLE gmail_sync_settings ADD COLUMN backfill_page_token TEXT",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "backfill_query",
        "ALTER TABLE gmail_sync_settings ADD COLUMN backfill_query TEXT",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "last_backfill_at",
        "ALTER TABLE gmail_sync_settings ADD COLUMN last_backfill_at TEXT",
    )
    .await?;
    ensure_gmail_sync_column(
        pool,
        "backfill_completed_at",
        "ALTER TABLE gmail_sync_settings ADD COLUMN backfill_completed_at TEXT",
    )
    .await?;

    sqlx::raw_sql(STAKEHOLDER_THREAD_EXCLUDES_SQL)
        .execute(pool)
        .await
        .map_err(|error| format!("failed to apply Gmail stakeholder exclusions schema: {error}"))?;
    ensure_table_column(
        pool,
        "gmail_threads",
        "ai_title",
        "ALTER TABLE gmail_threads ADD COLUMN ai_title TEXT",
    )
    .await?;

    Ok(())
}

async fn ensure_gmail_sync_column(
    pool: &SqlitePool,
    column: &str,
    alter_sql: &str,
) -> Result<(), String> {
    let exists: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM pragma_table_info('gmail_sync_settings') WHERE name = ?",
    )
    .bind(column)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    if exists == 0 {
        sqlx::query(alter_sql)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }
    Ok(())
}

async fn ensure_table_column(
    pool: &SqlitePool,
    table: &str,
    column: &str,
    alter_sql: &str,
) -> Result<(), String> {
    let exists: i64 = sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name = ?"
    ))
    .bind(column)
    .fetch_one(pool)
    .await
    .map_err(sql_error)?;
    if exists == 0 {
        sqlx::query(alter_sql)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }
    Ok(())
}

pub async fn apply_memory_system_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(MEMORY_SYSTEM_SQL)
        .execute(pool)
        .await
        .map_err(|e| format!("failed to apply memory system schema: {e}"))?;

    sqlx::query(
        r#"
        INSERT INTO memory_search(rowid, memory_id, title, body, tags)
        SELECT rowid, id, title, body, tags_json
        FROM memories
        WHERE status != 'deleted'
          AND deleted_at IS NULL
          AND rowid NOT IN (SELECT rowid FROM memory_search)
        "#,
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;

    Ok(())
}

pub async fn apply_memory_retrieval_migration(pool: &SqlitePool) -> Result<(), String> {
    sqlx::raw_sql(
        r#"
        CREATE TABLE IF NOT EXISTS memory_embeddings (
          memory_id   TEXT PRIMARY KEY REFERENCES memories(id) ON DELETE CASCADE,
          model       TEXT NOT NULL,
          dim         INTEGER NOT NULL,
          vector_json TEXT NOT NULL,
          norm        REAL NOT NULL,
          fingerprint TEXT NOT NULL,
          created_at  TEXT NOT NULL,
          updated_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memory_embeddings_model
          ON memory_embeddings (model);
        CREATE INDEX IF NOT EXISTS idx_memory_embeddings_fingerprint
          ON memory_embeddings (fingerprint);
        CREATE TABLE IF NOT EXISTS memory_retrievals (
          id              TEXT PRIMARY KEY,
          query           TEXT NOT NULL,
          memory_ids_json TEXT NOT NULL DEFAULT '[]',
          scores_json     TEXT NOT NULL DEFAULT '{}',
          context_kind    TEXT NOT NULL DEFAULT 'manual',
          source_kind     TEXT,
          source_id       TEXT,
          feedback        TEXT,
          created_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_memory_retrievals_created
          ON memory_retrievals (created_at DESC);
        "#,
    )
    .execute(pool)
    .await
    .map_err(|e| format!("failed to apply memory retrieval schema: {e}"))?;

    ensure_memory_column(
        pool,
        "sensitivity",
        "ALTER TABLE memories ADD COLUMN sensitivity TEXT NOT NULL DEFAULT 'normal'",
    )
    .await?;
    ensure_memory_column(
        pool,
        "pinned",
        "ALTER TABLE memories ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0",
    )
    .await?;

    Ok(())
}

async fn ensure_memory_column(
    pool: &SqlitePool,
    column: &str,
    alter_sql: &str,
) -> Result<(), String> {
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM pragma_table_info('memories') WHERE name = ?")
            .bind(column)
            .fetch_one(pool)
            .await
            .map_err(sql_error)?;
    if exists == 0 {
        sqlx::query(alter_sql)
            .execute(pool)
            .await
            .map_err(sql_error)?;
    }
    Ok(())
}

pub fn sql_error(error: sqlx::Error) -> String {
    format!("database error: {error}")
}

// ── Migration tracking ────────────────────────────────────────────────────────
// db.rs runs apply_migrations() on every startup (Tauri app + MCP server).
// This table records which steps have already been applied so they are never
// re-executed.  tauri_plugin_sql has its own tracking; this one is for the
// shared sqlx pool.

async fn ensure_migration_tracking(pool: &SqlitePool) -> Result<(), String> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS _applied_migrations (
            step       INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await
    .map_err(sql_error)?;
    Ok(())
}

async fn step_applied(pool: &SqlitePool, step: i64) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM _applied_migrations WHERE step = ?")
        .bind(step)
        .fetch_one(pool)
        .await
        .unwrap_or(0)
        > 0
}

async fn mark_step(pool: &SqlitePool, step: i64) -> Result<(), String> {
    sqlx::query("INSERT OR IGNORE INTO _applied_migrations (step) VALUES (?)")
        .bind(step)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn migrations_add_conversations_without_breaking_existing_rows() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");

        sqlx::raw_sql("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .expect("foreign keys");
        sqlx::raw_sql(INIT_SQL)
            .execute(&pool)
            .await
            .expect("init migration");
        sqlx::raw_sql(DELIVERABLES_SQL)
            .execute(&pool)
            .await
            .expect("deliverables migration");

        sqlx::query(
            r#"
            INSERT INTO deliverables
              (id, title, type, state, claim, created_at, updated_at)
            VALUES ('del1', 'Existing', 'analysis', 'drafting', 'Claim', 'now', 'now')
            "#,
        )
        .execute(&pool)
        .await
        .expect("existing deliverable");

        apply_conversations_migration(&pool)
            .await
            .expect("stage 4 migration");
        apply_conversations_migration(&pool)
            .await
            .expect("stage 4 migration is idempotent");

        let existing_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deliverables")
            .fetch_one(&pool)
            .await
            .expect("deliverable count");
        let has_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('deliverables') WHERE name = 'conversation_id'",
        )
        .fetch_one(&pool)
        .await
        .expect("column query");

        assert_eq!(existing_count, 1);
        assert_eq!(has_column, 1);
    }

    #[tokio::test]
    async fn conversation_ingest_migration_preserves_existing_captures() {
        let pool = connect_memory().await.expect("database");
        sqlx::query(
            r#"
            INSERT INTO captures
              (id, kind, body, status, created_at, updated_at)
            VALUES ('cap1', 'claude_link', 'https://claude.ai/chat/abc123', 'inbox', 'now', 'now')
            "#,
        )
        .execute(&pool)
        .await
        .expect("existing capture");

        apply_conversation_ingest_migration(&pool)
            .await
            .expect("stage 5 migration");

        let existing_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM captures")
            .fetch_one(&pool)
            .await
            .expect("capture count");
        let has_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'promoted_conversation_id'",
        )
        .fetch_one(&pool)
        .await
        .expect("column query");

        assert_eq!(existing_count, 1);
        assert_eq!(has_column, 1);
    }

    #[tokio::test]
    async fn capture_initiative_graph_migration_preserves_existing_captures() {
        let pool = connect_memory().await.expect("database");
        sqlx::query(
            r#"
            INSERT INTO captures
              (id, kind, body, status, created_at, updated_at)
            VALUES ('cap1', 'thought', 'A useful framing', 'inbox', 'now', 'now')
            "#,
        )
        .execute(&pool)
        .await
        .expect("existing capture");

        apply_capture_initiative_graph_migration(&pool)
            .await
            .expect("stage 6 migration");

        let existing_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM captures")
            .fetch_one(&pool)
            .await
            .expect("capture count");
        let has_column: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('captures') WHERE name = 'promoted_initiative_id'",
        )
        .fetch_one(&pool)
        .await
        .expect("column query");

        assert_eq!(existing_count, 1);
        assert_eq!(has_column, 1);
    }

    #[tokio::test]
    async fn gmail_workspace_migration_adds_backfill_columns() {
        let pool = connect_memory().await.expect("database");
        apply_gmail_workspace_migration(&pool)
            .await
            .expect("gmail migration is idempotent");

        for column in [
            "backfill_enabled",
            "relevance_filter_enabled",
            "auto_analyze_enabled",
            "auto_analyze_limit",
            "backfill_page_token",
            "backfill_query",
            "last_backfill_at",
            "backfill_completed_at",
        ] {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM pragma_table_info('gmail_sync_settings') WHERE name = ?",
            )
            .bind(column)
            .fetch_one(&pool)
            .await
            .expect("column query");
            assert_eq!(exists, 1, "missing {column}");
        }
    }

    #[tokio::test]
    async fn meeting_action_kind_migration_allows_agent_history_kinds() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");

        sqlx::raw_sql("PRAGMA foreign_keys = ON;")
            .execute(&pool)
            .await
            .expect("foreign keys");
        sqlx::raw_sql(MEETINGS_SQL)
            .execute(&pool)
            .await
            .expect("legacy meetings schema");
        sqlx::query(
            r#"
            INSERT INTO meetings (id, title, date, created_at, updated_at)
            VALUES ('mtg1', 'Planning', '2026-05-07', 'now', 'now')
            "#,
        )
        .execute(&pool)
        .await
        .expect("meeting");
        sqlx::query(
            r#"
            INSERT INTO meeting_actions
              (id, meeting_id, kind, body, applied, created_at)
            VALUES ('act1', 'mtg1', 'capture', 'Existing action', 0, 'now')
            "#,
        )
        .execute(&pool)
        .await
        .expect("legacy action");

        apply_meeting_action_kinds_migration(&pool)
            .await
            .expect("meeting action migration");
        apply_meeting_action_kinds_migration(&pool)
            .await
            .expect("meeting action migration is idempotent");

        let create_sql: String = sqlx::query_scalar(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'meeting_actions'",
        )
        .fetch_one(&pool)
        .await
        .expect("table schema");
        assert!(!create_sql.contains("CHECK (kind IN"));

        for kind in [
            "note_added",
            "task_created",
            "state_updated",
            "deadline_set",
            "blocker_set",
            "capture_created",
            "flagged",
        ] {
            sqlx::query(
                r#"
                INSERT INTO meeting_actions
                  (id, meeting_id, kind, body, applied, created_at)
                VALUES (?, 'mtg1', ?, 'Agent action', 1, 'now')
                "#,
            )
            .bind(format!("act-{kind}"))
            .bind(kind)
            .execute(&pool)
            .await
            .expect("agent action kind");
        }

        let action_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meeting_actions")
            .fetch_one(&pool)
            .await
            .expect("action count");
        assert_eq!(action_count, 8);
    }

    #[tokio::test]
    async fn reasoning_report_migration_creates_review_gated_storage() {
        let pool = connect_memory().await.expect("database");
        for table in [
            "reasoning_source_units",
            "claim_versions",
            "graph_communities",
            "community_reports",
            "reasoning_runs",
            "reasoning_cache",
            "report_runs",
            "report_exports",
        ] {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
            )
            .bind(table)
            .fetch_one(&pool)
            .await
            .expect("table lookup");
            assert_eq!(exists, 1, "missing {table}");
        }
        let templates: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM report_templates")
            .fetch_one(&pool)
            .await
            .expect("template count");
        assert_eq!(templates, 3);
    }

    #[tokio::test]
    async fn reasoning_reports_upgrade_repairs_existing_reasoning_runs() {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory database");
        sqlx::raw_sql(
            r#"
            CREATE TABLE reasoning_runs (
              id TEXT PRIMARY KEY,
              query_text TEXT NOT NULL,
              depth TEXT NOT NULL,
              query_mode TEXT NOT NULL,
              scope_json TEXT NOT NULL,
              result_markdown TEXT NOT NULL,
              citations_json TEXT NOT NULL,
              generated_assertions_json TEXT NOT NULL,
              contradictions_json TEXT NOT NULL,
              unsupported_json TEXT NOT NULL,
              model TEXT NOT NULL,
              status TEXT NOT NULL,
              created_at TEXT NOT NULL,
              updated_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&pool)
        .await
        .expect("early reasoning runs schema");

        sqlx::raw_sql(REASONING_REPORTS_UPGRADE_SQL)
            .execute(&pool)
            .await
            .expect("upgrade schema");
        apply_reasoning_reports_upgrade(&pool)
            .await
            .expect("upgrade is additive");

        for column in ["action_proposals_json", "cache_hit", "latency_ms"] {
            let exists: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM pragma_table_info('reasoning_runs') WHERE name = ?",
            )
            .bind(column)
            .fetch_one(&pool)
            .await
            .expect("column query");
            assert_eq!(exists, 1, "missing {column}");
        }
    }
}
