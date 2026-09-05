use std::collections::BTreeSet;


use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GmailSyncSettings {
    pub sync_enabled: bool,
    pub sync_interval_hours: i64,
    pub notification_poll_minutes: i64,
    pub max_threads_per_sync: i64,
    pub include_sent: bool,
    pub include_drafts: bool,
    pub notify_new_mail: bool,
    pub backfill_enabled: bool,
    pub relevance_filter_enabled: bool,
    pub auto_analyze_enabled: bool,
    pub auto_analyze_limit: i64,
    pub backfill_page_token: Option<String>,
    pub backfill_query: Option<String>,
    pub last_backfill_at: Option<String>,
    pub backfill_completed_at: Option<String>,
    pub account_email: Option<String>,
    pub last_sync_started_at: Option<String>,
    pub last_sync_completed_at: Option<String>,
    pub last_history_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GmailSyncSettingsInput {
    pub sync_enabled: Option<bool>,
    pub sync_interval_hours: Option<i64>,
    pub notification_poll_minutes: Option<i64>,
    pub max_threads_per_sync: Option<i64>,
    pub include_sent: Option<bool>,
    pub include_drafts: Option<bool>,
    pub notify_new_mail: Option<bool>,
    pub backfill_enabled: Option<bool>,
    pub relevance_filter_enabled: Option<bool>,
    pub auto_analyze_enabled: Option<bool>,
    pub auto_analyze_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailSyncReport {
    pub synced_threads: i64,
    pub synced_messages: i64,
    pub backfilled_threads: i64,
    pub backfill_complete: bool,
    pub skipped_spam_threads: i64,
    pub skipped_irrelevant_threads: i64,
    pub purged_threads: i64,
    pub ai_analyzed_threads: i64,
    pub auto_linked_threads: i64,
    pub analysis_refreshed_threads: i64,
    pub analysis_failed_threads: i64,
    pub orphan_threads: i64,
    pub new_messages: i64,
    pub new_threads: i64,
    pub synced_labels: i64,
    pub synced_drafts: i64,
    pub started_at: String,
    pub completed_at: String,
    pub account_email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailLabelRecord {
    pub gmail_label_id: String,
    pub name: String,
    pub label_type: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailAttachmentRecord {
    pub id: String,
    pub message_id: String,
    pub thread_id: String,
    pub attachment_id: Option<String>,
    pub filename: String,
    pub mime_type: String,
    pub size: Option<i64>,
    pub shared_by_email: String,
    pub shared_with: Vec<EmailAddress>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailLinkRecord {
    pub id: String,
    pub thread_id: String,
    pub message_id: Option<String>,
    pub url: String,
    pub kind: String,
    pub title: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkedDeliverableRef {
    pub id: String,
    pub title: String,
    pub state: String,
    pub linked_at: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkedInitiativeRef {
    pub id: String,
    pub title: String,
    pub status: String,
    pub linked_at: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkedStakeholderRef {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
    pub linked_at: String,
    pub source: String,
    pub confidence: Option<f64>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailThreadLinkSuggestion {
    pub id: String,
    pub thread_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub target_title: String,
    pub confidence: f64,
    pub rationale: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailAutoLinkReport {
    pub thread_id: String,
    pub linked_stakeholders: i64,
    pub linked_deliverables: i64,
    pub linked_initiatives: i64,
    pub suggestions_created: i64,
    pub orphan: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailLocalThread {
    pub thread_id: String,
    pub subject: String,
    pub snippet: String,
    pub participants: Vec<EmailAddress>,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
    pub message_count: i64,
    pub has_unread: bool,
    pub gmail_read_state: String,
    pub is_sent_only: bool,
    pub last_from_name: String,
    pub last_from_email: String,
    pub ai_title: Option<String>,
    pub summary: Option<String>,
    pub sentiment: Option<String>,
    pub urgency: Option<String>,
    pub ai_category: String,
    pub ai_priority: String,
    pub ai_category_confidence: Option<f64>,
    pub ai_category_reasons: Vec<String>,
    pub ai_triaged_at: Option<String>,
    pub labels: Vec<GmailLabelRecord>,
    pub linked_deliverables: Vec<LinkedDeliverableRef>,
    pub linked_initiatives: Vec<LinkedInitiativeRef>,
    pub linked_stakeholders: Vec<LinkedStakeholderRef>,
    pub artifact_urls: Vec<String>,
    pub effective_priority: String,
    pub priority_reasons: Vec<String>,
    pub graph_context: Value,
    pub last_analyzed_message_at: Option<i64>,
    pub last_sync_at: String,
    pub intent: Option<String>,
    pub action_required: bool,
    pub predicted_action: Option<String>,
    pub thread_state: Option<String>,
    pub dimensions_confidence: Value,
    pub work_relevance: String,
    pub work_relevance_reasons: Vec<String>,
    pub work_relevance_confidence: Option<f64>,
    pub attention_state: String,
    pub attention_reasons: Vec<String>,
    pub attention_confidence: Option<f64>,
    pub message_type: String,
    pub message_type_reasons: Vec<String>,
    pub message_type_confidence: Option<f64>,
    pub work_mail_updated_at: Option<String>,
    pub trace_seen_at: Option<String>,
    pub trace_review_state: String,
    pub seen_through_message_id: Option<String>,
    pub seen_through_message_at: Option<i64>,
    pub reviewed_through_message_id: Option<String>,
    pub reviewed_through_message_at: Option<i64>,
    pub deferred_until: Option<String>,
    pub new_since_review: bool,
    pub needs_me_reason: Option<String>,
    pub bundle_id: Option<String>,
    pub bundle_size: i64,
    pub last_analysis_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailMessageRecord {
    pub message_id: String,
    pub thread_id: String,
    pub history_id: Option<String>,
    pub subject: String,
    pub snippet: String,
    pub from_name: String,
    pub from_email: String,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub date_ts: Option<i64>,
    pub internal_date_ts: Option<i64>,
    pub plain_body: String,
    pub html_body: String,
    pub label_ids: Vec<String>,
    pub is_sent: bool,
    pub is_draft: bool,
    pub is_unread: bool,
    pub size_estimate: Option<i64>,
    pub artifact_urls: Vec<String>,
    pub synced_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailThreadDetail {
    pub thread: GmailLocalThread,
    pub messages: Vec<GmailMessageRecord>,
    pub attachments: Vec<GmailAttachmentRecord>,
    pub links: Vec<GmailLinkRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailDraftRecord {
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub subject: String,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub body_preview: String,
    pub updated_at: Option<String>,
    pub synced_at: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GmailThreadFilter {
    pub query: Option<String>,
    pub label_id: Option<String>,
    pub category: Option<String>,
    pub stakeholder_id: Option<String>,
    pub deliverable_id: Option<String>,
    pub initiative_id: Option<String>,
    pub orphan_only: Option<bool>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkMailViewId {
    AllWork,
    NeedsMe,
    Projects,
    Deliverables,
    Stakeholders,
    Files,
    Meetings,
    Unlinked,
    Excluded,
    AgentActivity,
}

impl Default for WorkMailViewId {
    fn default() -> Self {
        Self::AllWork
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkMailQuery {
    #[serde(default)]
    pub view: WorkMailViewId,
    pub query: Option<String>,
    pub sender_domain: Option<String>,
    pub work_relevance: Option<String>,
    pub attention_state: Option<String>,
    pub message_type: Option<String>,
    pub stakeholder_id: Option<String>,
    pub deliverable_id: Option<String>,
    pub initiative_id: Option<String>,
    pub has_artifact: Option<bool>,
    pub unread_only: Option<bool>,
    pub gmail_unread: Option<bool>,
    pub trace_unseen: Option<bool>,
    pub seen_unreviewed: Option<bool>,
    pub review_state: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailViewCount {
    pub view: WorkMailViewId,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailViewCounts {
    pub counts: Vec<WorkMailViewCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailBrief {
    pub needs_you: i64,
    pub handled_by_trace: i64,
    pub unread_work_mail: i64,
    pub unseen_in_trace: i64,
    pub seen_unreviewed: i64,
    pub action_review_queue: i64,
    pub waiting: i64,
    pub deferred: i64,
    pub handled: i64,
    pub unlinked: i64,
    pub excluded: i64,
    pub uncertain_links: i64,
    pub scope_domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkMailDomain {
    pub domain: String,
    pub enabled: bool,
    pub source: String,
    pub note: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertWorkMailDomainInput {
    pub domain: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailAgentEvent {
    pub id: String,
    pub thread_id: Option<String>,
    pub event_kind: String,
    pub actor: String,
    pub summary: String,
    pub reason: Value,
    pub payload: Value,
    pub undo_payload: Option<Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkMailReviewState {
    Unreviewed,
    Reviewed,
    Deferred,
    Waiting,
    Resolved,
    Replied,
}

impl Default for WorkMailReviewState {
    fn default() -> Self {
        Self::Unreviewed
    }
}

impl WorkMailReviewState {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkMailReviewState::Unreviewed => "unreviewed",
            WorkMailReviewState::Reviewed => "reviewed",
            WorkMailReviewState::Deferred => "deferred",
            WorkMailReviewState::Waiting => "waiting",
            WorkMailReviewState::Resolved => "resolved",
            WorkMailReviewState::Replied => "replied",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "reviewed" => Self::Reviewed,
            "deferred" => Self::Deferred,
            "waiting" => Self::Waiting,
            "resolved" => Self::Resolved,
            "replied" => Self::Replied,
            _ => Self::Unreviewed,
        }
    }

    pub fn is_handled(self) -> bool {
        !matches!(self, Self::Unreviewed)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailSeenCheckpoint {
    pub message_id: Option<String>,
    pub message_at: Option<i64>,
    pub seen_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkMailReviewSummary {
    pub thread_id: String,
    pub review_state: WorkMailReviewState,
    pub trace_seen_at: Option<String>,
    pub seen: WorkMailSeenCheckpoint,
    pub reviewed_through_message_id: Option<String>,
    pub reviewed_through_message_at: Option<i64>,
    pub handled_at: Option<String>,
    pub deferred_until: Option<String>,
    pub reopened_at: Option<String>,
    pub updated_at: Option<String>,
    pub new_since_review: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkMailReviewUpdate {
    pub state: WorkMailReviewState,
    pub deferred_until: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct WorkMailReviewRow {
    pub thread_id: String,
    pub review_state: String,
    pub trace_seen_at: Option<String>,
    pub seen_through_message_id: Option<String>,
    pub seen_through_message_at: Option<i64>,
    pub reviewed_through_message_id: Option<String>,
    pub reviewed_through_message_at: Option<i64>,
    pub handled_at: Option<String>,
    pub deferred_until: Option<String>,
    pub reopened_at: Option<String>,
    pub updated_at: String,
}

pub fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailStakeholderHealth {
    pub stakeholder_id: String,
    pub email: String,
    pub days_since_last_email: Option<i64>,
    pub sent_count: i64,
    pub received_count: i64,
    pub thread_count: i64,
    pub response_rate: f64,
    pub health_score: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailStakeholderSuggestion {
    pub email: String,
    pub name: String,
    pub thread_count: i64,
    pub sent_count: i64,
    pub received_count: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailRelationshipEdge {
    pub left_email: String,
    pub left_name: String,
    pub right_email: String,
    pub right_name: String,
    pub thread_count: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailAiCandidate {
    pub title: String,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub artifact_url: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailAiResult {
    pub title: String,
    pub summary: String,
    pub sentiment: String,
    pub urgency: String,
    pub category: String,
    pub priority: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub reasons: Vec<String>,
    pub tasks: Vec<GmailAiCandidate>,
    pub deliverables: Vec<GmailAiCandidate>,
    #[serde(default)]
    pub initiatives: Vec<GmailAiCandidate>,
    pub deadlines: Vec<GmailAiCandidate>,
    pub reply: Option<String>,
    // New multi-dimensional fields (Section 3 rebuild).
    #[serde(default)]
    pub intent: Option<String>,
    #[serde(default)]
    pub action_required: Option<bool>,
    #[serde(default)]
    pub predicted_action: Option<String>,
    #[serde(default)]
    pub thread_state: Option<String>,
    #[serde(default)]
    pub dimensions_confidence: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailTriagePerson {
    pub name: String,
    pub email: String,
    pub reason: String,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailTriageResult {
    pub category: String,
    pub priority: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    pub reasons: Vec<String>,
    pub suggested_actions: Vec<String>,
    pub stakeholder_candidates: Vec<GmailTriagePerson>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailWeeklyDigest {
    pub summary: String,
    pub waiting_for_response: Vec<GmailLocalThread>,
    pub overdue_followups: Vec<GmailLocalThread>,
    pub urgent_threads: Vec<GmailLocalThread>,
    pub draft_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailCategoryCount {
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GmailSendInput {
    pub to: Vec<String>,
    #[serde(default)]
    pub cc: Vec<String>,
    #[serde(default)]
    pub bcc: Vec<String>,
    pub subject: String,
    /// Plain-text body. Required (used as the text/plain alternative when an
    /// HTML body is also provided).
    pub body: String,
    /// Optional HTML body. When present, the message becomes a
    /// multipart/alternative so both clients render correctly.
    #[serde(default)]
    pub body_html: Option<String>,
    /// Local draft id whose attachments should be folded into a
    /// multipart/mixed envelope and uploaded with the send.
    #[serde(default)]
    pub draft_id: Option<String>,
    /// Direct file paths to attach (alternative to draft_id). Read from disk
    /// and base64-encoded at send time.
    #[serde(default)]
    pub attachment_paths: Vec<String>,
    #[serde(default)]
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GmailSendResult {
    pub message_id: String,
    pub thread_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GmailThreadRow {
    pub thread_id: String,
    pub subject: String,
    pub snippet: String,
    pub participants: String,
    pub first_message_at: Option<i64>,
    pub last_message_at: Option<i64>,
    pub message_count: i64,
    pub has_unread: bool,
    pub is_sent_only: bool,
    pub last_from_name: String,
    pub last_from_email: String,
    pub ai_title: Option<String>,
    pub summary: Option<String>,
    pub sentiment: Option<String>,
    pub urgency: Option<String>,
    pub ai_category: String,
    pub ai_priority: String,
    pub ai_category_confidence: Option<f64>,
    pub ai_category_reasons: String,
    pub ai_triaged_at: Option<String>,
    pub last_analyzed_message_at: Option<i64>,
    pub graph_context_json: String,
    pub effective_priority: String,
    pub priority_reasons_json: String,
    pub last_sync_at: String,
    pub intent: Option<String>,
    pub action_required: i64,
    pub predicted_action: Option<String>,
    pub thread_state: Option<String>,
    pub dimensions_confidence_json: String,
    pub work_relevance: String,
    pub work_relevance_reasons_json: String,
    pub work_relevance_confidence: Option<f64>,
    pub attention_state: String,
    pub attention_reasons_json: String,
    pub attention_confidence: Option<f64>,
    pub message_type: String,
    pub message_type_reasons_json: String,
    pub message_type_confidence: Option<f64>,
    pub work_mail_updated_at: Option<String>,
    pub bundle_id: Option<String>,
    pub last_analysis_error: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct GmailMessageRow {
    pub message_id: String,
    pub thread_id: String,
    pub history_id: Option<String>,
    pub subject: String,
    pub snippet: String,
    pub from_name: String,
    pub from_email: String,
    pub to_json: String,
    pub cc_json: String,
    pub bcc_json: String,
    pub date_ts: Option<i64>,
    pub internal_date_ts: Option<i64>,
    pub plain_body: String,
    pub html_body: String,
    pub label_ids_json: String,
    pub is_sent: bool,
    pub is_draft: bool,
    pub is_unread: bool,
    pub size_estimate: Option<i64>,
    pub artifact_urls_json: String,
    pub synced_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct GmailAttachmentRow {
    pub id: String,
    pub message_id: String,
    pub thread_id: String,
    pub attachment_id: Option<String>,
    pub filename: String,
    pub mime_type: String,
    pub size: Option<i64>,
    pub shared_by_email: String,
    pub shared_with_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct GmailDraftRow {
    pub draft_id: String,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub subject: String,
    pub to_json: String,
    pub cc_json: String,
    pub bcc_json: String,
    pub body_preview: String,
    pub updated_at: Option<String>,
    pub synced_at: String,
}

#[derive(Debug, Deserialize)]
pub struct GmailProfile {
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiLabelList {
    pub labels: Option<Vec<ApiLabel>>,
}

#[derive(Debug, Deserialize)]
pub struct ApiLabel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub label_type: Option<String>,
    pub color: Option<ApiLabelColor>,
}

#[derive(Debug, Deserialize)]
pub struct ApiLabelColor {
    #[serde(rename = "backgroundColor")]
    pub background_color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiThreadList {
    pub threads: Option<Vec<ApiThreadRef>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiThreadRef {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiThreadDetail {
    pub id: String,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
    pub messages: Option<Vec<ApiMessage>>,
}

#[derive(Debug, Deserialize)]
pub struct ApiDraftList {
    pub drafts: Option<Vec<ApiDraftRef>>,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiDraftRef {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiDraftDetail {
    pub id: String,
    pub message: ApiMessage,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiMessage {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    #[serde(rename = "historyId")]
    pub history_id: Option<String>,
    #[serde(rename = "labelIds")]
    pub label_ids: Option<Vec<String>>,
    pub snippet: Option<String>,
    pub payload: Option<ApiPayload>,
    #[serde(rename = "internalDate")]
    pub internal_date: Option<String>,
    #[serde(rename = "sizeEstimate")]
    pub size_estimate: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPayload {
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub filename: Option<String>,
    pub headers: Option<Vec<Header>>,
    pub body: Option<ApiBody>,
    pub parts: Option<Vec<ApiPayload>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiBody {
    #[serde(rename = "attachmentId")]
    pub attachment_id: Option<String>,
    pub size: Option<i64>,
    pub data: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ParsedMessage {
    pub message_id: String,
    pub thread_id: String,
    pub history_id: Option<String>,
    pub subject: String,
    pub snippet: String,
    pub from: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub bcc: Vec<EmailAddress>,
    pub date_ts: Option<i64>,
    pub internal_date_ts: Option<i64>,
    pub plain_body: String,
    pub html_body: String,
    pub headers_json: String,
    pub label_ids: Vec<String>,
    pub is_sent: bool,
    pub is_draft: bool,
    pub is_unread: bool,
    pub size_estimate: Option<i64>,
    pub artifact_urls: Vec<String>,
    pub attachments: Vec<ParsedAttachment>,
}

#[derive(Debug, Clone)]
pub struct ParsedAttachment {
    pub attachment_id: Option<String>,
    pub filename: String,
    pub mime_type: String,
    pub size: Option<i64>,
}

#[derive(Debug, Default)]
pub struct ThreadSyncState {
    pub synced_threads: i64,
    pub synced_messages: i64,
    pub new_messages: i64,
    pub skipped_spam_threads: i64,
    pub skipped_irrelevant_threads: i64,
    pub new_thread_ids: BTreeSet<String>,
    pub touched_threads: BTreeSet<String>,
    pub max_history_id: Option<String>,
}

#[derive(Debug, Default)]
pub struct RelevanceContext {
    pub known_emails: BTreeSet<String>,
    pub work_domains: BTreeSet<String>,
    pub terms: Vec<String>,
}

#[derive(Debug)]
pub struct ApiThreadPage {
    pub threads: Vec<ApiThreadRef>,
    pub next_page_token: Option<String>,
}
