use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InitiativeStatus {
    Live,
    Paused,
    Shipped,
    Parked,
}

impl InitiativeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Live => "live",
            Self::Paused => "paused",
            Self::Shipped => "shipped",
            Self::Parked => "parked",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Initiative {
    pub id: String,
    pub title: String,
    pub framing: String,
    pub status: String,
    pub icon: String,
    pub icon_color: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateInitiativeInput {
    pub title: String,
    pub framing: String,
    pub status: InitiativeStatus,
    #[serde(default = "default_initiative_icon")]
    pub icon: String,
    #[serde(default = "default_initiative_icon_color")]
    pub icon_color: String,
}

fn default_initiative_icon() -> String {
    "target".to_string()
}

fn default_initiative_icon_color() -> String {
    "#6366f1".to_string()
}

impl Default for CreateInitiativeInput {
    fn default() -> Self {
        Self {
            title: String::new(),
            framing: String::new(),
            status: InitiativeStatus::Live,
            icon: default_initiative_icon(),
            icon_color: default_initiative_icon_color(),
        }
    }
}

impl Default for UpdateInitiativeInput {
    fn default() -> Self {
        Self {
            title: String::new(),
            framing: String::new(),
            status: InitiativeStatus::Live,
            icon: default_initiative_icon(),
            icon_color: default_initiative_icon_color(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateInitiativeInput {
    pub title: String,
    pub framing: String,
    pub status: InitiativeStatus,
    #[serde(default = "default_initiative_icon")]
    pub icon: String,
    #[serde(default = "default_initiative_icon_color")]
    pub icon_color: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Stakeholder {
    pub id: String,
    pub name: String,
    pub display_order: i64,
    #[sqlx(default)]
    pub email: String,
    #[sqlx(default)]
    pub role: String,
    #[sqlx(default)]
    pub notes: String,
    #[sqlx(default)]
    pub avatar_url: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct UserProfile {
    pub id: i64,
    pub name: String,
    pub role: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateUserProfileInput {
    pub name: String,
    pub role: Option<String>,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateStakeholderInput {
    pub name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateStakeholderInput {
    pub name: String,
    #[serde(default)]
    pub email: String,
    pub role: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StakeholderDetail {
    pub stakeholder: Stakeholder,
    pub deliverable_count: i64,
    pub shipped_count: i64,
    pub in_flight_count: i64,
    pub days_since_last_delivery: Option<i64>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliverableType {
    Deck,
    DesignDoc,
    Prototype,
    Analysis,
    Framework,
    Pitch,
    Research,
    Code,
    Email,
    MeetingPrep,
    Spec,
    Report,
    Roadmap,
    Brief,
    Plan,
    Other,
}

impl DeliverableType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Deck => "deck",
            Self::DesignDoc => "design_doc",
            Self::Prototype => "prototype",
            Self::Analysis => "analysis",
            Self::Framework => "framework",
            Self::Pitch => "pitch",
            Self::Research => "research",
            Self::Code => "code",
            Self::Email => "email",
            Self::MeetingPrep => "meeting_prep",
            Self::Spec => "spec",
            Self::Report => "report",
            Self::Roadmap => "roadmap",
            Self::Brief => "brief",
            Self::Plan => "plan",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeliverableState {
    Backlog,
    Todo,
    Drafting,
    InReview,
    Shipped,
    Killed,
}

impl DeliverableState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Todo => "todo",
            Self::Drafting => "drafting",
            Self::InReview => "in_review",
            Self::Shipped => "shipped",
            Self::Killed => "killed",
        }
    }

    pub fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "backlog" => Ok(Self::Backlog),
            "todo" => Ok(Self::Todo),
            "drafting" => Ok(Self::Drafting),
            "in_review" => Ok(Self::InReview),
            "shipped" => Ok(Self::Shipped),
            "killed" => Ok(Self::Killed),
            other => Err(format!("unknown deliverable state: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LabelRef {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateLabelInput {
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DeliverableStateHistory {
    pub id: String,
    pub deliverable_id: String,
    pub from_state: String,
    pub to_state: String,
    pub friction_note: Option<String>,
    pub moved_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InitiativeRef {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct StakeholderRef {
    pub id: String,
    pub name: String,
    #[sqlx(default)]
    pub role: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Deliverable {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: String,
    pub state: String,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_url: Option<String>,
    pub stakeholder_id: Option<String>,
    pub stakeholder_name: Option<String>,
    pub stakeholders: Vec<StakeholderRef>,
    pub created_at: String,
    pub shipped_at: Option<String>,
    pub updated_at: String,
    pub initiatives: Vec<InitiativeRef>,
    pub deadline: Option<String>,
    pub is_focused: bool,
    pub effort: Option<i64>,
    pub impact: Option<i64>,
    pub blocker_reason: Option<String>,
    pub state_changed_at: Option<String>,
    pub display_order: i64,
    pub priority: Option<String>,
    pub labels: Vec<LabelRef>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DeliverableRow {
    pub id: String,
    pub title: String,
    pub deliverable_type: String,
    pub state: String,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub conversation_id: Option<String>,
    pub conversation_url: Option<String>,
    pub stakeholder_id: Option<String>,
    pub stakeholder_name: Option<String>,
    pub created_at: String,
    pub shipped_at: Option<String>,
    pub updated_at: String,
    pub deadline: Option<String>,
    pub is_focused: bool,
    pub effort: Option<i64>,
    pub impact: Option<i64>,
    pub blocker_reason: Option<String>,
    pub state_changed_at: Option<String>,
    #[sqlx(default)]
    pub display_order: i64,
    pub priority: Option<String>,
}

impl DeliverableRow {
    pub fn with_refs(
        self,
        initiatives: Vec<InitiativeRef>,
        stakeholders: Vec<StakeholderRef>,
        labels: Vec<LabelRef>,
    ) -> Deliverable {
        let stakeholder_id = stakeholders
            .first()
            .map(|stakeholder| stakeholder.id.clone())
            .or(self.stakeholder_id);
        let stakeholder_name = if stakeholders.is_empty() {
            self.stakeholder_name
        } else {
            Some(
                stakeholders
                    .iter()
                    .map(|stakeholder| stakeholder.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        };

        Deliverable {
            id: self.id,
            title: self.title,
            deliverable_type: self.deliverable_type,
            state: self.state,
            claim: self.claim,
            artifact_url: self.artifact_url,
            conversation_id: self.conversation_id,
            conversation_url: self.conversation_url,
            stakeholder_id,
            stakeholder_name,
            stakeholders,
            created_at: self.created_at,
            shipped_at: self.shipped_at,
            updated_at: self.updated_at,
            initiatives,
            deadline: self.deadline,
            is_focused: self.is_focused,
            effort: self.effort,
            impact: self.impact,
            blocker_reason: self.blocker_reason,
            state_changed_at: self.state_changed_at,
            display_order: self.display_order,
            priority: self.priority,
            labels,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Todo,
    Doing,
    Done,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DeliverableTask {
    pub id: String,
    pub deliverable_id: String,
    pub title: String,
    pub status: String,
    pub due_date: Option<String>,
    pub notes: Option<String>,
    pub url: Option<String>,
    pub display_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskInput {
    pub deliverable_id: String,
    pub title: String,
    pub due_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskInput {
    pub title: String,
    pub status: TaskStatus,
    pub due_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DeliverableNote {
    pub id: String,
    pub deliverable_id: String,
    pub body: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateNoteInput {
    pub deliverable_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeliverableMetadataInput {
    pub deadline: Option<String>,
    pub effort: Option<i64>,
    pub impact: Option<i64>,
    pub blocker_reason: Option<String>,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyFlaggedToBacklogInput {
    pub action_id: String,
    pub initiative_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedTasks {
    pub tasks: Vec<GeneratedTaskSuggestion>,
    pub suggested_deliverable_deadline: Option<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTaskSuggestion {
    pub title: String,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyGeneratedTasksInput {
    pub deliverable_id: String,
    pub tasks: Vec<GeneratedTaskSuggestion>,
    pub suggested_deliverable_deadline: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReorderDirectionInput {
    pub id: String,
    pub direction: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WorkIntakeSuggestion {
    pub id: String,
    pub source_kind: String,
    pub source_id: Option<String>,
    pub source_title: String,
    pub source_route: Option<String>,
    pub item_kind: String,
    pub title: String,
    pub body: String,
    pub target_deliverable_id: Option<String>,
    pub target_initiative_id: Option<String>,
    pub due_date: Option<String>,
    pub suggested_type: Option<String>,
    pub confidence: Option<f64>,
    pub rationale: String,
    pub status: String,
    pub payload: String,
    pub created_at: String,
    pub updated_at: String,
    pub applied_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkIntakeFilters {
    pub status: Option<String>,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveWorkIntakeInput {
    pub id: String,
    pub target_deliverable_id: Option<String>,
    pub target_initiative_id: Option<String>,
    /// Lets the caller override the AI-suggested kind before approving.
    #[serde(default)]
    pub item_kind_override: Option<String>,
    /// User-edited title — overrides the AI-generated value.
    #[serde(default)]
    pub title_override: Option<String>,
    /// User-edited description body.
    #[serde(default)]
    pub body_override: Option<String>,
    /// User-edited due date (ISO date string, e.g. "2026-05-18").
    #[serde(default)]
    pub due_date_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkIntakeApplyResult {
    pub suggestion: WorkIntakeSuggestion,
    pub entity_kind: String,
    pub entity_id: String,
    pub route: String,
}

// ── Google Calendar ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GCalEvent {
    pub id: String,
    pub gcal_event_id: String,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub attendees: Option<String>,
    pub conference_data: Option<String>,
    pub organizer: Option<String>,
    pub recurring_event_id: Option<String>,
    pub recurrence: Option<String>,
    pub color_id: Option<String>,
    pub transparency: Option<String>,
    pub event_updated_at: Option<String>,
    pub attachments: Option<String>,
    pub reminders: Option<String>,
    pub visibility: Option<String>,
    pub start_date: String,
    pub end_date: Option<String>,
    pub start_datetime: Option<String>,
    pub end_datetime: Option<String>,
    pub is_all_day: bool,
    pub html_link: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GCalStatus {
    pub connected: bool,
}

// ── Week view ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct WeekDay {
    pub day_index: i64,
    pub deliverable: Option<Deliverable>,
    pub deadline_deliverables: Vec<Deliverable>,
    pub tasks: Vec<WeekTask>,
    pub gcal_events: Vec<GCalEvent>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WeekTask {
    pub day_index: i64,
    pub id: String,
    pub deliverable_id: String,
    pub deliverable_title: String,
    pub deliverable_type: String,
    pub deliverable_state: String,
    pub stakeholder_name: Option<String>,
    pub title: String,
    pub status: String,
    pub due_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct WeekView {
    pub week_start: String,
    pub days: Vec<WeekDay>,
    pub meeting_date: Option<String>,
    pub gcal_connected: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SetDayInput {
    pub week_start: String,
    pub day_index: i64,
    pub deliverable_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MeetingConfig {
    pub next_meeting_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedWeekPlan {
    pub assignments: Vec<WeekDayAssignment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekDayAssignment {
    pub day_index: i64,
    pub deliverable_id: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DeliverableFilters {
    pub initiative_id: Option<String>,
    pub stakeholder_id: Option<String>,
    #[serde(rename = "type")]
    pub deliverable_type: Option<DeliverableType>,
    pub state: Option<DeliverableState>,
    /// Filter to any of these states (OR semantics). Takes precedence over `state` when both set.
    #[serde(default)]
    pub state_in: Option<Vec<DeliverableState>>,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDeliverableInput {
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: DeliverableType,
    pub state: DeliverableState,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub conversation_id: Option<String>,
    pub stakeholder_id: Option<String>,
    #[serde(default)]
    pub stakeholder_ids: Vec<String>,
    pub initiative_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateDeliverableInput {
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: DeliverableType,
    pub state: DeliverableState,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub conversation_id: Option<String>,
    pub stakeholder_id: Option<String>,
    #[serde(default)]
    pub stakeholder_ids: Vec<String>,
    pub initiative_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub export_path: String,
    pub initiative_count: usize,
    pub stakeholder_count: usize,
    pub deliverable_count: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureKind {
    Thought,
    ClaudeLink,
    ArtifactLink,
}

impl CaptureKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thought => "thought",
            Self::ClaudeLink => "claude_link",
            Self::ArtifactLink => "artifact_link",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptureStatus {
    Inbox,
    Promoted,
    Dismissed,
    Suggested,
}

impl CaptureStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inbox => "inbox",
            Self::Promoted => "promoted",
            Self::Dismissed => "dismissed",
            Self::Suggested => "suggested",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Capture {
    pub id: String,
    pub kind: String,
    pub body: String,
    pub status: String,
    pub promoted_deliverable_id: Option<String>,
    pub promoted_deliverable_title: Option<String>,
    pub promoted_initiative_id: Option<String>,
    pub promoted_initiative_title: Option<String>,
    pub promoted_conversation_id: Option<String>,
    pub promoted_conversation_title: Option<String>,
    pub promoted_task_id: Option<String>,
    pub promoted_task_title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub promoted_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateCaptureInput {
    pub kind: CaptureKind,
    pub body: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CaptureFilters {
    pub status: Option<CaptureStatus>,
    pub kind: Option<CaptureKind>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MenuBarState {
    pub active_deliverable_id: Option<String>,
    pub active_deliverable_title: Option<String>,
    pub tray_title: String,
    pub shipped_this_week: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Conversation {
    pub id: String,
    pub chat_url: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub occurred_at: Option<String>,
    pub ingested_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConversationExtractionInput {
    pub chat_url: Option<String>,
    pub pasted_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedConversation {
    pub title: String,
    pub summary: String,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedDeliverableCandidate {
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: DeliverableType,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub stakeholder_name: Option<String>,
    pub initiative_titles: Vec<String>,
    #[serde(default)]
    pub validation_errors: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversationExtractionResult {
    pub conversation: ExtractedConversation,
    pub candidates: Vec<ExtractedDeliverableCandidate>,
    pub source_chat_url: Option<String>,
    pub source_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitConversationIngestInput {
    pub chat_url: Option<String>,
    pub conversation: ExtractedConversation,
    pub deliverables: Vec<CommitExtractedDeliverableInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitExtractedDeliverableInput {
    pub accepted: bool,
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: DeliverableType,
    pub claim: String,
    pub artifact_url: Option<String>,
    pub stakeholder_id: Option<String>,
    #[serde(default)]
    pub stakeholder_ids: Vec<String>,
    pub initiative_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConversationIngestResult {
    pub conversation: Conversation,
    pub deliverables: Vec<Deliverable>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateConversationInput {
    pub chat_url: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpInstallState {
    pub installed: bool,
    pub binary_path: String,
    pub database_path: String,
    pub log_path: String,
    pub snippet_path: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpConfigSnippet {
    pub config_path: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiKeyStatus {
    pub configured: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeminiTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct McpToolResponse<T: Serialize> {
    pub message: String,
    pub data: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitiativeState {
    pub initiative: Initiative,
    pub deliverables: Vec<Deliverable>,
    pub drafting_count: i64,
    pub in_review_count: i64,
    pub shipped_count: i64,
    pub killed_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingItem {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BriefingSections {
    pub tldr: String,
    #[serde(default)]
    pub open_commitments: Vec<BriefingItem>,
    #[serde(default)]
    pub waiting_on_them: Vec<BriefingItem>,
    #[serde(default)]
    pub recent_wins: Vec<BriefingItem>,
    #[serde(default)]
    pub in_flight: Vec<BriefingItem>,
    #[serde(default)]
    pub talking_points: Vec<String>,
    pub watch_out: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StakeholderBriefing {
    pub stakeholder: Stakeholder,
    pub generated_with: String,
    pub generated_at: String,
    pub sections: BriefingSections,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct WorkGraphFilters {
    pub include_dismissed_captures: bool,
    pub include_killed_deliverables: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BrainGraphFilters {
    #[serde(default)]
    pub include_dismissed_captures: bool,
    #[serde(default)]
    pub include_killed_deliverables: bool,
    #[serde(default)]
    pub node_kinds: Vec<String>,
    #[serde(default)]
    pub relation_kinds: Vec<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

impl From<WorkGraphFilters> for BrainGraphFilters {
    fn from(value: WorkGraphFilters) -> Self {
        Self {
            include_dismissed_captures: value.include_dismissed_captures,
            include_killed_deliverables: value.include_killed_deliverables,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainStatus {
    pub path: String,
    pub exists: bool,
    pub schema_version: i64,
    pub storage_version: u64,
    pub generated_at: Option<String>,
    pub node_count: usize,
    pub edge_count: usize,
    pub error: Option<String>,
}

// ── Brain layout cache (Phase 3) ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrainLayoutPoint {
    pub entity_kind: String,
    pub entity_id: String,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainLayoutResult {
    pub mode: String,
    pub graph_version: String,
    pub computed_at: i64,
    pub points: Vec<BrainLayoutPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WriteBrainLayoutInput {
    pub mode: String,
    pub graph_version: String,
    pub points: Vec<BrainLayoutPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeEmbedding {
    pub entity_kind: String,
    pub entity_id: String,
    pub vector: Vec<f32>,
}

// ── Brain explorer Phase 2 — saved views & community summaries ──────────

#[derive(Debug, Clone, Serialize)]
pub struct SavedBrainView {
    pub id: String,
    pub name: String,
    pub description: String,
    pub filters: serde_json::Value,
    pub layout: serde_json::Value,
    pub viewport: serde_json::Value,
    pub pinned: serde_json::Value,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveBrainViewInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub filters: serde_json::Value,
    #[serde(default)]
    pub layout: serde_json::Value,
    #[serde(default)]
    pub viewport: serde_json::Value,
    #[serde(default)]
    pub pinned: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphCommunityMemberSummary {
    pub entity_kind: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphCommunitySummary {
    pub id: String,
    pub community_kind: String,
    pub scope_key: String,
    pub title: String,
    pub level: i64,
    pub status: String,
    pub summary_markdown: Option<String>,
    pub member_count: i64,
    pub members: Vec<GraphCommunityMemberSummary>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainRetrieveInput {
    pub query: String,
    #[serde(default)]
    pub focus_entity_id: Option<String>,
    #[serde(default)]
    pub max_hops: Option<usize>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainContextResult {
    pub query: String,
    pub summary: String,
    pub graph: WorkGraph,
    pub ranked_nodes: Vec<WorkGraphNode>,
    /// Per-node score breakdown for the top results. Same ordering and
    /// length as `ranked_nodes` so a UI can join by index. Empty when the
    /// query produced no candidates.
    #[serde(default)]
    pub scored_nodes: Vec<ScoredBrainNode>,
}

/// Snapshot of every learned policy + the current retrieval blend +
/// per-template inference thresholds. Consumed by the RL feedback surface
/// so the user can see what the bandit has been doing.
#[derive(Debug, Clone, Serialize)]
pub struct BrainLearningSummary {
    pub policies: Vec<BrainPolicySummary>,
    pub blend: BrainRetrievalBlend,
    pub inference_thresholds: Vec<InferenceThresholdSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainPolicySummary {
    pub template: String,
    pub observations: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainRetrievalBlend {
    pub bm25: f64,
    pub cosine: f64,
    pub node_weight: f64,
    pub focus_proximity: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InferenceThresholdSummary {
    pub template: String,
    pub threshold: f64,
    pub sample_count: i64,
    pub last_recomputed: String,
}

/// Per-node retrieval signal breakdown. Used by the "Why this answer?"
/// expander and as training features for the `retrieval_blend` policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredBrainNode {
    pub node_id: String,
    pub entity_id: String,
    pub kind: String,
    pub bm25_norm: f64,
    pub cosine: f64,
    pub recency_multiplier: f64,
    pub node_weight_norm: f64,
    pub focus_proximity: f64,
    /// Per-entity learned importance multiplier from `brain_rl_item_scores`.
    /// Defaults to 1.0 when no row exists for this entity.
    pub learned_factor: f64,
    /// Final score after blend + recency + learned multiplier (pre-MMR).
    pub blended_score: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainCypherInput {
    pub query: String,
    #[serde(default)]
    pub params: Option<serde_json::Value>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainCypherResult {
    pub columns: Vec<String>,
    pub rows: Vec<serde_json::Value>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BrainTemplateKind {
    FocusToday,
    BlockedWork,
    EmailFollowups,
    StaleWork,
    StakeholderContext,
}

impl BrainTemplateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FocusToday => "focus_today",
            Self::BlockedWork => "blocked_work",
            Self::EmailFollowups => "email_followups",
            Self::StaleWork => "stale_work",
            Self::StakeholderContext => "stakeholder_context",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainTemplateInput {
    pub template: BrainTemplateKind,
    #[serde(default)]
    pub focus_entity_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainTemplateResult {
    pub template: String,
    pub summary: String,
    pub cypher: String,
    pub rows: Vec<serde_json::Value>,
    pub graph: WorkGraph,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainBrief {
    pub generated_at: String,
    pub focus_today: BrainTemplateResult,
    pub blocked_or_waiting: BrainTemplateResult,
    pub email_followups: BrainTemplateResult,
    pub stale_work: BrainTemplateResult,
    pub inferences_to_review: Vec<serde_json::Value>,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BrainInferenceRecord {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub relation_kind: String,
    pub target_kind: String,
    pub target_id: String,
    pub confidence: f64,
    pub rationale: String,
    pub evidence_json: String,
    pub status: String,
    pub generated_by: String,
    pub created_at: String,
    pub updated_at: String,
    pub reviewed_at: Option<String>,
    /// RL template that produced this inference (`meeting_action_exact`,
    /// `meeting_action_fuzzy`, `email_thread_mention`, `blocker_email_match`,
    /// `blocker_fuzzy`). NULL for older rows that pre-date the column and
    /// for user-driven `generated_by='feedback'` rows.
    #[serde(default)]
    pub template: Option<String>,
    /// Inference that auto-superseded this one (set by
    /// `auto_supersede_on_accept`). NULL when the row is still active.
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// `'predicate_opposite'` or `'dominance'`. NULL when `superseded_by` is NULL.
    #[serde(default)]
    pub supersede_reason: Option<String>,
}

/// Row returned by `list_brain_inferences` — extends `BrainInferenceRecord`
/// with derived display fields (subject/target labels, current threshold for
/// the row's template).
#[derive(Debug, Clone, Serialize)]
pub struct BrainInferenceRow {
    #[serde(flatten)]
    pub record: BrainInferenceRecord,
    /// Best-effort display label for the subject entity. Falls back to the
    /// raw `source_id` when no lookup succeeds.
    pub subject_label: Option<String>,
    /// Best-effort display label for the target entity.
    pub target_label: Option<String>,
    /// Current `inference_thresholds.threshold` for this row's template,
    /// or NULL if the row has no template / no threshold seeded.
    pub threshold: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainInferenceFilter {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub template: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub before_updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainInferenceListResult {
    pub items: Vec<BrainInferenceRow>,
    pub total_pending: i64,
    pub total_accepted_7d: i64,
    pub total_rejected_7d: i64,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewInferenceResult {
    pub inference_id: String,
    pub status: String,
    pub template: Option<String>,
    pub threshold_before: Option<f64>,
    pub threshold_after: Option<f64>,
    pub sample_count: Option<i64>,
    pub superseded_inference_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SupersessionRecord {
    pub loser: BrainInferenceRow,
    pub winner: BrainInferenceRow,
    pub supersede_reason: String,
    pub superseded_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TopTemplateSummary {
    pub name: String,
    pub accepted: i64,
    pub rejected: i64,
    pub acceptance_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RLDigest {
    pub window_days: i64,
    pub inferences_generated: i64,
    pub inferences_accepted: i64,
    pub inferences_rejected: i64,
    pub acceptance_rate: f64,
    pub supersessions: i64,
    pub top_template: Option<TopTemplateSummary>,
    pub ask_useful: i64,
    pub ask_wrong: i64,
    pub ask_feedback_rate: f64,
    pub embeddings_added: i64,
    pub threshold_drift: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateCoefficient {
    pub feature: String,
    pub mean: f64,
    pub abs_max: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateDetail {
    pub template: String,
    pub observations: i64,
    pub threshold: Option<f64>,
    pub sample_count: Option<i64>,
    pub feature_names: Vec<String>,
    pub coefficient_summary: Vec<TemplateCoefficient>,
    pub recent_events: Vec<BrainLearningEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainFeedbackInput {
    pub question: String,
    #[serde(default)]
    pub template: Option<String>,
    pub feedback: String,
    #[serde(default)]
    pub corrected: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BrainLearningEventInput {
    #[serde(default)]
    pub template: Option<String>,
    pub item_id: String,
    #[serde(default)]
    pub item_kind: Option<String>,
    pub event_type: String,
    #[serde(default)]
    pub reward: Option<f64>,
    #[serde(default)]
    pub context: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BrainLearningEvent {
    pub id: String,
    pub template: String,
    pub item_id: String,
    pub item_kind: String,
    pub event_type: String,
    pub reward: f64,
    pub context_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BrainRlPolicyRecord {
    pub template: String,
    pub feature_names_json: String,
    pub a_matrix_json: String,
    pub b_vector_json: String,
    pub observations: i64,
    pub alpha: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct BrainRlItemScore {
    pub template: String,
    pub item_id: String,
    pub item_kind: String,
    pub score: f64,
    pub exploitation: f64,
    pub exploration: f64,
    pub features_json: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrainLearningSnapshot {
    pub policies: Vec<BrainRlPolicyRecord>,
    pub recent_events: Vec<BrainLearningEvent>,
    pub top_scores: Vec<BrainRlItemScore>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkGraph {
    pub generated_at: String,
    pub ai_context: String,
    pub nodes: Vec<WorkGraphNode>,
    pub edges: Vec<WorkGraphEdge>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkGraphNode {
    pub id: String,
    pub entity_id: String,
    pub kind: String,
    pub label: String,
    pub subtitle: Option<String>,
    pub status: Option<String>,
    pub url: Option<String>,
    pub updated_at: Option<String>,
    pub hidden_by_default: bool,
    pub weight: i64,
    pub context: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkGraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub label: String,
    pub properties: serde_json::Value,
}

// ── First-class memory ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Episodic,
    Semantic,
    Procedural,
}

impl MemoryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Episodic => "episodic",
            Self::Semantic => "semantic",
            Self::Procedural => "procedural",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryStatus {
    Active,
    Superseded,
    Archived,
    Deleted,
}

impl MemoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MemorySettings {
    pub enabled: bool,
    pub auto_extract_enabled: bool,
    pub work_related_only: bool,
    pub preserve_continuity: bool,
    pub require_confirmation: bool,
    pub retrieval_limit: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct MemorySettingsRow {
    pub enabled: i64,
    pub auto_extract_enabled: i64,
    pub work_related_only: i64,
    pub preserve_continuity: i64,
    pub require_confirmation: i64,
    pub retrieval_limit: i64,
    pub updated_at: String,
}

impl MemorySettingsRow {
    pub fn into_settings(self) -> MemorySettings {
        MemorySettings {
            enabled: self.enabled != 0,
            auto_extract_enabled: self.auto_extract_enabled != 0,
            work_related_only: self.work_related_only != 0,
            preserve_continuity: self.preserve_continuity != 0,
            require_confirmation: self.require_confirmation != 0,
            retrieval_limit: self.retrieval_limit,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMemorySettingsInput {
    pub enabled: bool,
    pub auto_extract_enabled: bool,
    pub work_related_only: bool,
    pub preserve_continuity: bool,
    pub require_confirmation: bool,
    pub retrieval_limit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryRecord {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub scope: String,
    pub title: String,
    pub body: String,
    pub canonical_key: String,
    pub source: String,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub confidence: f64,
    pub importance: f64,
    pub retrieval_count: i64,
    pub success_count: i64,
    pub contradiction_count: i64,
    pub tags: Vec<String>,
    pub evidence: Vec<String>,
    pub supersedes_id: Option<String>,
    pub version: i64,
    pub expires_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub last_retrieved_at: Option<String>,
    pub sensitivity: String,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct MemoryRow {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub scope: String,
    pub title: String,
    pub body: String,
    pub canonical_key: String,
    pub source: String,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub confidence: f64,
    pub importance: f64,
    pub retrieval_count: i64,
    pub success_count: i64,
    pub contradiction_count: i64,
    pub tags_json: String,
    pub evidence_json: String,
    pub supersedes_id: Option<String>,
    pub version: i64,
    pub expires_at: Option<String>,
    pub archived_at: Option<String>,
    pub deleted_at: Option<String>,
    pub last_retrieved_at: Option<String>,
    #[sqlx(default)]
    pub sensitivity: String,
    #[sqlx(default)]
    pub pinned: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl MemoryRow {
    pub fn into_record(self) -> MemoryRecord {
        let sensitivity = if self.sensitivity.is_empty() {
            "normal".to_string()
        } else {
            self.sensitivity
        };
        MemoryRecord {
            id: self.id,
            kind: self.kind,
            status: self.status,
            scope: self.scope,
            title: self.title,
            body: self.body,
            canonical_key: self.canonical_key,
            source: self.source,
            source_kind: self.source_kind,
            source_id: self.source_id,
            confidence: self.confidence,
            importance: self.importance,
            retrieval_count: self.retrieval_count,
            success_count: self.success_count,
            contradiction_count: self.contradiction_count,
            tags: serde_json::from_str(&self.tags_json).unwrap_or_default(),
            evidence: serde_json::from_str(&self.evidence_json).unwrap_or_default(),
            supersedes_id: self.supersedes_id,
            version: self.version,
            expires_at: self.expires_at,
            archived_at: self.archived_at,
            deleted_at: self.deleted_at,
            last_retrieved_at: self.last_retrieved_at,
            sensitivity,
            pinned: self.pinned != 0,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListMemoryFilters {
    pub kind: Option<MemoryKind>,
    pub status: Option<MemoryStatus>,
    pub query: Option<String>,
    #[serde(default)]
    pub include_archived: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMemoryInput {
    pub kind: MemoryKind,
    pub title: String,
    pub body: String,
    #[serde(default = "default_memory_scope")]
    pub scope: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub confidence: Option<f64>,
    pub importance: Option<f64>,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMemoryInput {
    pub kind: MemoryKind,
    pub status: MemoryStatus,
    pub title: String,
    pub body: String,
    #[serde(default = "default_memory_scope")]
    pub scope: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub confidence: Option<f64>,
    pub importance: Option<f64>,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RetrieveMemoryInput {
    pub query: String,
    pub limit: Option<i64>,
    #[serde(default)]
    pub kinds: Vec<MemoryKind>,
    #[serde(default)]
    pub source_kind: Option<String>,
    #[serde(default)]
    pub source_id: Option<String>,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub include_pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScoredMemory {
    pub memory: MemoryRecord,
    pub score: f64,
    pub semantic_score: f64,
    pub lexical_score: f64,
    pub recency_score: f64,
    pub procedural_pin: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct MemoryRetrievalDiagnostics {
    pub semantic_used: bool,
    pub lexical_used: bool,
    pub procedural_pin_count: i64,
    pub embedding_model: Option<String>,
    pub embedding_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryRetrievalResult {
    pub context: String,
    pub memories: Vec<MemoryRecord>,
    #[serde(default)]
    pub scored: Vec<ScoredMemory>,
    #[serde(default)]
    pub diagnostics: MemoryRetrievalDiagnostics,
    #[serde(default)]
    pub retrieval_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryConsolidationResult {
    pub run_id: String,
    pub created_count: i64,
    pub updated_count: i64,
    pub archived_count: i64,
    pub memories: Vec<MemoryRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MemoryEvent {
    pub id: String,
    pub memory_id: Option<String>,
    pub action: String,
    pub detail_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListMemoryEventsFilters {
    pub memory_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExtractMemoriesFromConversationInput {
    pub conversation_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedMemoryCandidate {
    pub kind: MemoryKind,
    pub title: String,
    pub body: String,
    #[serde(default = "default_memory_scope")]
    pub scope: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub importance: Option<f64>,
    #[serde(default)]
    pub sensitivity: Option<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExtractedMemoriesPayload {
    #[serde(default)]
    pub memories: Vec<ExtractedMemoryCandidate>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemoryExtractionResult {
    pub created_count: i64,
    pub updated_count: i64,
    pub skipped_count: i64,
    pub memories: Vec<MemoryRecord>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryFeedbackInput {
    pub retrieval_id: String,
    pub feedback: String,
}

fn default_memory_scope() -> String {
    "global".to_string()
}

// ── Meetings ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MeetingRow {
    pub id: String,
    pub title: String,
    pub date: String,
    pub duration_secs: Option<i64>,
    pub transcript: Option<String>,
    pub summary: Option<String>,
    pub key_decisions: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
    pub date: String,
    pub duration_secs: Option<i64>,
    pub transcript: Option<String>,
    pub summary: Option<String>,
    pub key_decisions: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub stakeholders: Vec<StakeholderRef>,
    pub created_at: String,
    pub updated_at: String,
}

impl MeetingRow {
    pub fn with_stakeholders(self, stakeholders: Vec<StakeholderRef>) -> Meeting {
        Meeting {
            id: self.id,
            title: self.title,
            date: self.date,
            duration_secs: self.duration_secs,
            transcript: self.transcript,
            summary: self.summary,
            key_decisions: self.key_decisions,
            status: self.status,
            error_message: self.error_message,
            stakeholders,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct MeetingAction {
    pub id: String,
    pub meeting_id: String,
    pub kind: String,
    pub target_id: Option<String>,
    pub target_title: Option<String>,
    pub body: String,
    pub payload: Option<String>,
    pub applied: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeetingWithActions {
    pub meeting: Meeting,
    pub actions: Vec<MeetingAction>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateMeetingInput {
    pub title: String,
    pub date: String,
    #[serde(default)]
    pub stakeholder_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProcessMeetingInput {
    pub meeting_id: String,
    pub audio_base64: String,
    pub mime_type: String,
    pub duration_secs: i64,
}

// Gemini structured output for meeting processing
#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiActionSuggestion {
    pub kind: String,
    pub suggested_target: String,
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GeminiMeetingOutput {
    pub title: String,
    pub transcript: String,
    pub summary: String,
    pub key_decisions: Vec<String>,
    pub action_suggestions: Vec<GeminiActionSuggestion>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApplyMeetingActionInput {
    pub action_id: String,
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InitiativeNote {
    pub id: String,
    pub initiative_id: String,
    pub body: String,
    pub created_at: String,
}

// ── Unified search ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchResult {
    pub kind: String,
    pub entity_id: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    pub route: String,
    #[serde(default)]
    pub state: Option<String>,
}

/// Emitted to the frontend during the agentic search loop so the UI
/// can show a live feed of tool calls as they execute.
#[derive(Debug, Clone, Serialize)]
pub struct AskProgressEvent {
    /// "tool_call" when a tool is about to execute
    pub kind: String,
    /// Exact tool function name, e.g. "search_deliverables"
    pub tool: String,
    /// Human-readable label, e.g. `Searching deliverables for "auth"`
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskSearchResult {
    pub answer: String,
    pub refs: Vec<SearchResult>,
    pub questions: Vec<AskUserQuestion>,
    /// Section 6.2 "Why this answer?" — per-cited-node retrieval signal
    /// breakdowns from the most recent `retrieve_brain_context` call inside
    /// this turn. Empty when the turn made no brain-context lookups.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scored_nodes: Vec<ScoredBrainNode>,
    /// The query the brain actually saw (post any rewriting/expansion).
    /// Diverges from the user's question often enough that surfacing it
    /// inside the expander is worth one extra string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_query: Option<String>,
}

/// Multimodal input attached to an Ask turn. Images today; richer kinds (PDF, audio)
/// can be added by extending this enum without touching the streaming protocol.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskAttachment {
    Image {
        /// MIME type, e.g. `image/png` or `image/jpeg`.
        mime_type: String,
        /// Base64-encoded payload (no `data:` prefix).
        data: String,
        /// Optional filename for telemetry / display.
        #[serde(default)]
        filename: Option<String>,
    },
}

// ── Server-side Ask chat persistence ─────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AskChatRecord {
    pub id: String,
    pub title: String,
    pub mode: String,
    pub summary: String,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskChatDetail {
    pub chat: AskChatRecord,
    pub turns: Vec<AskTurnRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskTurnRecord {
    pub id: String,
    pub chat_id: String,
    pub parent_id: Option<String>,
    pub fork_of: Option<String>,
    pub mode: Option<String>,
    pub question: String,
    pub answer: String,
    pub reasoning: String,
    pub status: String,
    pub error: Option<String>,
    pub refs: Vec<SearchResult>,
    pub questions: Vec<AskUserQuestion>,
    pub steps: Vec<serde_json::Value>,
    pub attachments: Vec<AskTurnAttachmentRecord>,
    /// Section 6.2 — persisted retrieval breakdown so the "Why this answer?"
    /// expander survives reload. Empty when the turn didn't touch the brain.
    #[serde(default)]
    pub scored_nodes: Vec<ScoredBrainNode>,
    /// Persisted alongside `scored_nodes`.
    #[serde(default)]
    pub retrieval_query: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AskTurnAttachmentRecord {
    pub id: String,
    pub turn_id: String,
    pub mime_type: String,
    pub filename: Option<String>,
    pub data_b64: String,
    pub size_bytes: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertAskChatInput {
    pub id: Option<String>,
    pub title: String,
    pub mode: String,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AppendAskTurnInput {
    pub chat_id: String,
    pub turn_id: String,
    pub parent_id: Option<String>,
    pub fork_of: Option<String>,
    pub mode: Option<String>,
    pub question: String,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub refs: Vec<SearchResult>,
    #[serde(default)]
    pub questions: Vec<AskUserQuestion>,
    #[serde(default)]
    pub steps: Vec<serde_json::Value>,
    #[serde(default)]
    pub attachments: Vec<AskAttachmentSave>,
    /// Section 6.2 — caller-provided retrieval breakdown to persist alongside the turn.
    #[serde(default)]
    pub scored_nodes: Vec<ScoredBrainNode>,
    #[serde(default)]
    pub retrieval_query: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AskAttachmentSave {
    pub mime_type: String,
    #[serde(default)]
    pub filename: Option<String>,
    pub data: String,
    #[serde(default)]
    pub size: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ListAskChatsFilters {
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskChatSearchHit {
    pub chat_id: String,
    pub chat_title: String,
    pub turn_id: String,
    pub question_snippet: String,
    pub answer_snippet: String,
    pub created_at: String,
}

/// Shared confirmation registry — keyed by `format!("{run_id}:{call_id}")`.
/// The Tauri app stores the oneshot sender here when the agent emits
/// `AwaitingConfirmation`; the `confirm_ask_tool_decision` IPC pops the entry
/// and routes the user's approval back to the awaiting dispatch task.
pub type AskConfirmationMap = std::sync::Arc<
    std::sync::Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
>;

/// Run-time controls handed to `ask_search_stream` so the dispatch loop can
/// pause for user confirmation and consult the per-run auto-allow list.
#[derive(Debug, Clone, Default)]
pub struct AskRunControls {
    pub confirmations: Option<AskConfirmationMap>,
    pub auto_confirmed_tools: Vec<String>,
    pub permission_mode: Option<String>,
}

/// Streaming event emitted as the agent runs. Tag `kind` is the discriminator
/// the frontend switches on. Each turn emits `started`, then a mix of
/// `text_delta` / `reasoning_delta` / `tool_call_started` / `tool_call_done`
/// events, terminating in exactly one of `done` / `error` / `cancelled`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskRunEvent {
    Started {
        run_id: String,
    },
    TextDelta {
        run_id: String,
        delta: String,
    },
    ReasoningDelta {
        run_id: String,
        delta: String,
    },
    ToolCallStarted {
        run_id: String,
        call_id: String,
        tool: String,
        label: String,
        rationale: Option<String>,
        args_preview: Option<String>,
    },
    ToolCallDone {
        run_id: String,
        call_id: String,
        tool: String,
        ok: bool,
        summary: String,
    },
    AwaitingConfirmation {
        run_id: String,
        call_id: String,
        tool: String,
        label: String,
        summary: String,
        args_preview: Option<String>,
        risk_reason: String,
    },
    ToolDenied {
        run_id: String,
        call_id: String,
        tool: String,
        reason: String,
    },
    TurnComplete {
        run_id: String,
        iteration: i32,
    },
    Done {
        run_id: String,
        result: AskSearchResult,
    },
    Cancelled {
        run_id: String,
    },
    Error {
        run_id: String,
        message: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AskUserQuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AskUserQuestion {
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub options: Vec<AskUserQuestionOption>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiAskOutput {
    #[serde(default)]
    pub answer: String,
    #[serde(default)]
    pub refs: Vec<GeminiAskRef>,
    #[serde(default)]
    pub questions: Vec<AskUserQuestion>,
}

#[derive(Debug, Deserialize)]
pub struct GeminiAskRef {
    pub kind: String,
    pub entity_id: String,
    pub title: String,
    pub route: String,
}

// ── Gantt ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct InitiativeSection {
    pub id: String,
    pub initiative_id: String,
    pub title: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateSectionInput {
    pub initiative_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSectionInput {
    pub title: String,
    pub position: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GanttDeliverable {
    pub id: String,
    pub title: String,
    pub state: String,
    pub section_id: Option<String>,
    pub start_date: Option<String>,
    pub deadline: Option<String>,
    pub task_count: i64,
    pub done_task_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct InitiativeGantt {
    pub initiative: Initiative,
    pub sections: Vec<InitiativeSection>,
    pub deliverables: Vec<GanttDeliverable>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateGanttDatesInput {
    pub start_date: Option<String>,
    pub deadline: Option<String>,
}

// ── Minutes processing ─────────────────────────────────────────────────────────

/// Input to the agentic minutes processor. Either plain text (from txt/docx)
/// or a base64-encoded PDF that Gemini processes natively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutesInput {
    pub text: Option<String>,
    pub pdf_base64: Option<String>,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutesAction {
    pub kind: String, // deliverable_note | initiative_note | task_created | state_updated | deadline_set | blocker_set | capture_created
    #[serde(default)]
    pub target_kind: Option<String>,
    #[serde(default)]
    pub target_id: Option<String>,
    pub target: Option<String>,
    pub detail: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub deadline: Option<String>,
    #[serde(default)]
    pub blocker_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlaggedDeliverable {
    pub title: String,
    pub claim: String,
    pub suggested_type: String,
    pub why: String,
}

// Server-internal: parsed from Gemini structured output and written into the DB
// via `repo::save_meeting_minutes`. Not returned to the frontend; no TS counterpart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinutesProcessingResult {
    /// Meeting title extracted from the document (or derived from filename)
    #[serde(default)]
    pub meeting_title: Option<String>,
    /// Meeting date extracted from the document in YYYY-MM-DD format
    #[serde(default)]
    pub meeting_date: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub actions: Vec<MinutesAction>,
    #[serde(default)]
    pub flagged: Vec<FlaggedDeliverable>,
}

// ── Weekly digest ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigestItem {
    pub title: String,
    pub reason: String,
    #[serde(default)]
    pub route: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeeklyDigest {
    pub summary: String,
    #[serde(default)]
    pub at_risk: Vec<DigestItem>,
    #[serde(default)]
    pub stale: Vec<DigestItem>,
    #[serde(default)]
    pub conflicts: Vec<DigestItem>,
    #[serde(default)]
    pub wins: Vec<DigestItem>,
    pub focus_recommendation: String,
}

// ---- Files ----

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileEntityKind {
    Initiative,
    Deliverable,
    DeliverableTask,
    Stakeholder,
    Capture,
    Meeting,
    Conversation,
}

impl FileEntityKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FileEntityKind::Initiative => "initiative",
            FileEntityKind::Deliverable => "deliverable",
            FileEntityKind::DeliverableTask => "deliverable_task",
            FileEntityKind::Stakeholder => "stakeholder",
            FileEntityKind::Capture => "capture",
            FileEntityKind::Meeting => "meeting",
            FileEntityKind::Conversation => "conversation",
        }
    }
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct TraceFolder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileLinkRef {
    pub entity_kind: String,
    pub entity_id: String,
    pub linked_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    pub id: String,
    pub kind: String, // 'local' | 'drive'
    pub trace_folder_id: Option<String>,
    pub name: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub description: Option<String>,
    pub local_path: Option<String>,
    pub is_missing: bool,
    pub drive_file_id: Option<String>,
    pub drive_account_id: Option<String>,
    pub drive_parent_id: Option<String>,
    pub drive_mime: Option<String>,
    pub drive_web_view_link: Option<String>,
    pub drive_trashed: bool,
    pub created_at: String,
    pub updated_at: String,
    pub links: Vec<FileLinkRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderListing {
    pub folder: Option<TraceFolder>,
    pub breadcrumbs: Vec<TraceFolder>,
    pub folders: Vec<TraceFolder>,
    pub files: Vec<FileRow>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTraceFolderInput {
    pub parent_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameTraceFolderInput {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveTraceFolderInput {
    pub id: String,
    pub new_parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttachLocalFileInput {
    pub path: String,
    pub trace_folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MoveFileInput {
    pub file_id: String,
    pub new_trace_folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RenameFileInput {
    pub file_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkFileInput {
    pub file_id: String,
    pub entity_kind: FileEntityKind,
    pub entity_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesForEntityInput {
    pub entity_kind: FileEntityKind,
    pub entity_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkFolderFilesInput {
    pub folder_id: String,
    pub entity_kind: FileEntityKind,
    pub entity_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkFolderInput {
    pub folder_id: String,
    pub entity_kind: FileEntityKind,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceFolderWithLinks {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
    pub file_count: i64,
    pub links: Vec<FileLinkRef>,
}

// ── Reasoning GraphRAG and strategic reports ────────────────────────────────

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningDepth {
    #[default]
    Standard,
    Deep,
}

impl ReasoningDepth {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GraphQueryMode {
    #[default]
    Auto,
    Local,
    Global,
    Drift,
}

impl GraphQueryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Local => "local",
            Self::Global => "global",
            Self::Drift => "drift",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct ReasoningScope {
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub quarter: Option<String>,
    #[serde(default)]
    pub initiative_ids: Vec<String>,
    #[serde(default)]
    pub stakeholder_ids: Vec<String>,
    #[serde(default)]
    pub deliverable_ids: Vec<String>,
    #[serde(default)]
    pub source_kinds: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReasoningQueryInput {
    pub query: String,
    /// Shorter semantic query used only for BM25/embedding source retrieval.
    /// When set, `query` is used only as the Gemini generation prompt.
    /// When absent, `query` serves both roles (backward-compatible with Ask).
    #[serde(default)]
    pub retrieval_query: Option<String>,
    #[serde(default)]
    pub depth: ReasoningDepth,
    #[serde(default)]
    pub query_mode: GraphQueryMode,
    #[serde(default)]
    pub scope: ReasoningScope,
    #[serde(default)]
    pub purpose: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReasoningSourceUnit {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub chunk_index: i64,
    pub title: String,
    pub body: String,
    pub citation_label: String,
    pub source_route: Option<String>,
    pub source_updated_at: Option<String>,
    pub content_hash: String,
    pub access_state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningCitation {
    pub source_unit_id: String,
    pub source_kind: String,
    pub source_id: String,
    pub label: String,
    pub title: String,
    pub route: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedAssertion {
    pub statement: String,
    #[serde(default)]
    pub source_unit_ids: Vec<String>,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ActionProposal {
    pub proposal_type: String,
    pub description: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReasoningResult {
    pub run_id: String,
    pub query_mode: String,
    pub answer_markdown: String,
    pub citations: Vec<ReasoningCitation>,
    pub approved_claim_ids: Vec<String>,
    pub generated_assertions: Vec<GeneratedAssertion>,
    pub action_proposals: Vec<ActionProposal>,
    pub contradictions: Vec<String>,
    pub unsupported_assertions: Vec<String>,
    pub model: String,
    pub cache_hit: bool,
    pub latency_ms: i64,
    pub generated_analysis: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReasoningRun {
    pub id: String,
    pub query_text: String,
    pub depth: String,
    pub query_mode: String,
    pub scope_json: String,
    pub result_markdown: String,
    pub citations_json: String,
    pub generated_assertions_json: String,
    pub action_proposals_json: String,
    pub contradictions_json: String,
    pub unsupported_json: String,
    pub model: String,
    pub cache_hit: i64,
    pub latency_ms: i64,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReasoningIndexSummary {
    pub source_units_created: i64,
    pub source_units_updated: i64,
    pub source_units_unchanged: i64,
    pub source_units_removed: i64,
    pub canonical_claims_synced: i64,
    pub communities_synced: i64,
    pub candidate_claims_staged: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ClaimReviewItem {
    pub id: String,
    pub claim_key: String,
    pub statement: String,
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub confidence: f64,
    pub contradiction_state: String,
    pub evidence_json: String,
    pub status: String,
    pub generated_by: String,
    pub reasoning_run_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentReviewItem {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub proposal_type: String,
    pub proposed_change_json: String,
    pub rationale: String,
    pub status: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CommunityReport {
    pub id: String,
    pub community_id: String,
    pub title: String,
    pub summary_markdown: String,
    pub evidence_json: String,
    pub version_hash: String,
    pub status: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReportType {
    Quarterly,
    Initiative,
    DecisionMemo,
}

impl ReportType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quarterly => "quarterly",
            Self::Initiative => "initiative",
            Self::DecisionMemo => "decision_memo",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateReportRunInput {
    pub report_type: ReportType,
    pub title: String,
    #[serde(default)]
    pub scope: ReasoningScope,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReportRun {
    pub id: String,
    pub report_type: String,
    pub title: String,
    pub status: String,
    pub scope_json: String,
    pub outline_markdown: String,
    pub draft_markdown: String,
    pub citations_json: String,
    pub unsupported_json: String,
    pub reasoning_run_id: Option<String>,
    pub deliverable_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub approved_at: Option<String>,
    #[sqlx(default)]
    pub scope_exclusions_json: String,
    #[sqlx(default)]
    pub sections_json: String,
    #[sqlx(default)]
    pub section_drafts_json: String,
    #[sqlx(default)]
    pub critique_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ReportStep {
    pub id: String,
    pub report_run_id: String,
    pub step_name: String,
    pub section_index: Option<i64>,
    pub status: String,
    pub model: Option<String>,
    pub cache_hit: i64,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub latency_ms: Option<i64>,
    pub input_json: String,
    pub output_json: String,
    pub clarification_json: Option<String>,
    pub ticker_label: Option<String>,
    pub error_text: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSectionPlan {
    pub id: String,
    pub heading: String,
    #[serde(default)]
    pub instructions: String,
    /// 'queued' | 'drafting' | 'done' | 'needs_clarification' | 'error'
    #[serde(default = "default_section_status")]
    pub status: String,
    #[serde(default)]
    pub position: i64,
}

fn default_section_status() -> String {
    "queued".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSectionDraft {
    pub markdown: String,
    #[serde(default)]
    pub citation_ids: Vec<String>,
    #[serde(default)]
    pub last_drafted_at: Option<String>,
    #[serde(default)]
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportCritiqueIssue {
    pub id: String,
    pub section_id: String,
    pub kind: String, // contradiction | weak_citation | scope_leak | unsupported
    pub message: String,
    #[serde(default)]
    pub span_text: Option<String>,
    #[serde(default)]
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReportCritique {
    #[serde(default)]
    pub issues: Vec<ReportCritiqueIssue>,
    #[serde(default)]
    pub generated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportScopePreview {
    pub initiative_ids: Vec<String>,
    pub initiative_titles: Vec<String>,
    pub deliverable_ids: Vec<String>,
    pub stakeholder_ids: Vec<String>,
    pub email_thread_ids: Vec<String>,
    pub meeting_ids: Vec<String>,
    pub file_ids: Vec<String>,
    pub calendar_event_ids: Vec<String>,
    /// Display entries grouped by kind for the scope-preview UI.
    pub entries: Vec<ReportScopeEntry>,
    pub total_entries: i64,
    pub excluded_entry_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportScopeEntry {
    pub id: String,
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportClarification {
    pub question: String,
    pub options: Vec<String>,
    #[serde(default = "default_true")]
    pub free_text_allowed: bool,
    #[serde(default)]
    pub multi_select: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnswerClarificationInput {
    pub step_id: String,
    pub selected_labels: Vec<String>,
    #[serde(default)]
    pub free_text: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReplaceScopeExclusionsInput {
    pub report_run_id: String,
    pub excluded_entry_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateReportSectionsInput {
    pub report_run_id: String,
    pub sections: Vec<ReportSectionPlan>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RerunReportStepInput {
    pub report_run_id: String,
    pub step_name: String,
    #[serde(default)]
    pub section_id: Option<String>,
    #[serde(default)]
    pub ignore_cache: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SteerReportInput {
    pub report_run_id: String,
    pub instruction: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApproveOutlineInput {
    pub report_run_id: String,
    pub outline_markdown: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportReportInput {
    pub report_run_id: String,
    pub format: String,
    #[serde(default)]
    pub destination_dir: Option<String>,
    #[serde(default)]
    pub external_write_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReportExport {
    pub id: String,
    pub report_run_id: String,
    pub format: String,
    pub export_path: Option<String>,
    pub artifact_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LinkReportDeliverableInput {
    pub report_run_id: String,
    pub deliverable_id: String,
}
