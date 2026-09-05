use project_manager_shared::models::{
    BrainTemplateKind, CaptureKind, CaptureStatus, DeliverableState, DeliverableType,
    InitiativeStatus,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_LIMIT: i64 = 10;
pub const MAX_LIMIT: i64 = 50;
pub const DEFAULT_SINCE_DAYS: i64 = 30;

#[derive(Debug, Serialize, JsonSchema)]
pub struct ToolResponse {
    pub message: String,
    pub data: Value,
}

impl ToolResponse {
    pub fn new<T: Serialize>(message: String, data: &T) -> Result<Self, String> {
        let data = serde_json::to_value(data)
            .map_err(|error| format!("failed to serialize tool response: {error}"))?;
        Ok(Self { message, data })
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EmptyToolInput {}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListInitiativesToolInput {
    pub status: Option<ToolInitiativeStatus>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetByIdToolInput {
    pub id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateInitiativeToolInput {
    pub title: String,
    pub framing: String,
    pub status: ToolInitiativeStatus,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateInitiativeToolInput {
    pub initiative_id: String,
    pub title: String,
    pub framing: String,
    pub status: ToolInitiativeStatus,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteInitiativeToolInput {
    pub initiative_id: String,
    pub confirm_title: String,
    pub confirm_orphan_deliverables: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateDeliverableToolInput {
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: ToolDeliverableType,
    pub claim: String,
    pub initiative_titles: Vec<String>,
    pub stakeholder_name: Option<String>,
    pub stakeholder_names: Option<Vec<String>>,
    pub artifact_url: Option<String>,
    pub conversation_url: Option<String>,
    pub state: Option<ToolDeliverableState>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateDeliverableToolInput {
    pub deliverable_id: String,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub deliverable_type: Option<ToolDeliverableType>,
    pub state: Option<ToolDeliverableState>,
    pub claim: Option<String>,
    pub initiative_titles: Option<Vec<String>>,
    pub stakeholder_name: Option<String>,
    pub stakeholder_names: Option<Vec<String>>,
    pub clear_stakeholder: Option<bool>,
    pub artifact_url: Option<String>,
    pub clear_artifact_url: Option<bool>,
    pub conversation_url: Option<String>,
    pub clear_conversation: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListDeliverablesToolInput {
    pub initiative_title: Option<String>,
    pub stakeholder_name: Option<String>,
    #[serde(rename = "type")]
    pub deliverable_type: Option<ToolDeliverableType>,
    pub state: Option<ToolDeliverableState>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchDeliverablesToolInput {
    pub query: String,
    pub initiative_title: Option<String>,
    pub stakeholder_name: Option<String>,
    #[serde(rename = "type")]
    pub deliverable_type: Option<ToolDeliverableType>,
    pub state: Option<ToolDeliverableState>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateDeliverableStateToolInput {
    pub deliverable_id: String,
    pub state: ToolDeliverableState,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteDeliverableToolInput {
    pub deliverable_id: String,
    pub confirm_title: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateStakeholderToolInput {
    pub name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateCaptureToolInput {
    pub kind: ToolCaptureKind,
    pub body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCapturesToolInput {
    pub status: Option<ToolCaptureStatus>,
    pub kind: Option<ToolCaptureKind>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DismissCaptureToolInput {
    pub capture_id: String,
    pub confirm_body: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PromoteCaptureToDeliverableToolInput {
    pub capture_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub deliverable_type: ToolDeliverableType,
    pub state: ToolDeliverableState,
    pub claim: String,
    pub initiative_titles: Vec<String>,
    pub stakeholder_name: Option<String>,
    pub stakeholder_names: Option<Vec<String>>,
    pub artifact_url: Option<String>,
    pub conversation_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PromoteCaptureToInitiativeToolInput {
    pub capture_id: String,
    pub title: String,
    pub framing: String,
    pub status: ToolInitiativeStatus,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateConversationToolInput {
    pub chat_url: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub occurred_at: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetInitiativeStateToolInput {
    pub initiative_title: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListStakeholderBriefingToolInput {
    pub stakeholder_name: String,
    pub since_days: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetWorkContextGraphToolInput {
    pub include_dismissed_captures: Option<bool>,
    pub include_killed_deliverables: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RetrieveBrainContextToolInput {
    pub query: String,
    pub focus_entity_id: Option<String>,
    pub max_hops: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct QueryBrainCypherToolInput {
    pub query: String,
    pub params: Option<Value>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunBrainTemplateToolInput {
    pub template: ToolBrainTemplateKind,
    pub focus_entity_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrainFeedbackToolInput {
    pub question: String,
    pub template: Option<String>,
    pub feedback: String,
    pub corrected: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrainLearningEventToolInput {
    pub template: Option<String>,
    pub item_id: String,
    pub item_kind: Option<String>,
    pub event_type: String,
    pub reward: Option<f64>,
    pub context: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrainLearningSnapshotToolInput {
    pub template: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolBrainTemplateKind {
    FocusToday,
    BlockedWork,
    EmailFollowups,
    StaleWork,
    StakeholderContext,
}

impl From<ToolBrainTemplateKind> for BrainTemplateKind {
    fn from(value: ToolBrainTemplateKind) -> Self {
        match value {
            ToolBrainTemplateKind::FocusToday => Self::FocusToday,
            ToolBrainTemplateKind::BlockedWork => Self::BlockedWork,
            ToolBrainTemplateKind::EmailFollowups => Self::EmailFollowups,
            ToolBrainTemplateKind::StaleWork => Self::StaleWork,
            ToolBrainTemplateKind::StakeholderContext => Self::StakeholderContext,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolInitiativeStatus {
    Live,
    Paused,
    Shipped,
    Parked,
}

impl From<ToolInitiativeStatus> for InitiativeStatus {
    fn from(value: ToolInitiativeStatus) -> Self {
        match value {
            ToolInitiativeStatus::Live => Self::Live,
            ToolInitiativeStatus::Paused => Self::Paused,
            ToolInitiativeStatus::Shipped => Self::Shipped,
            ToolInitiativeStatus::Parked => Self::Parked,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolDeliverableType {
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
    Other,
}

impl From<ToolDeliverableType> for DeliverableType {
    fn from(value: ToolDeliverableType) -> Self {
        match value {
            ToolDeliverableType::Deck => Self::Deck,
            ToolDeliverableType::DesignDoc => Self::DesignDoc,
            ToolDeliverableType::Prototype => Self::Prototype,
            ToolDeliverableType::Analysis => Self::Analysis,
            ToolDeliverableType::Framework => Self::Framework,
            ToolDeliverableType::Pitch => Self::Pitch,
            ToolDeliverableType::Research => Self::Research,
            ToolDeliverableType::Code => Self::Code,
            ToolDeliverableType::Email => Self::Email,
            ToolDeliverableType::MeetingPrep => Self::MeetingPrep,
            ToolDeliverableType::Other => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolDeliverableState {
    Drafting,
    InReview,
    Shipped,
    Killed,
}

impl ToolDeliverableState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Drafting => "drafting",
            Self::InReview => "in_review",
            Self::Shipped => "shipped",
            Self::Killed => "killed",
        }
    }
}

impl From<ToolDeliverableState> for DeliverableState {
    fn from(value: ToolDeliverableState) -> Self {
        match value {
            ToolDeliverableState::Drafting => Self::Drafting,
            ToolDeliverableState::InReview => Self::InReview,
            ToolDeliverableState::Shipped => Self::Shipped,
            ToolDeliverableState::Killed => Self::Killed,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolCaptureKind {
    Thought,
    ClaudeLink,
    ArtifactLink,
}

impl From<ToolCaptureKind> for CaptureKind {
    fn from(value: ToolCaptureKind) -> Self {
        match value {
            ToolCaptureKind::Thought => Self::Thought,
            ToolCaptureKind::ClaudeLink => Self::ClaudeLink,
            ToolCaptureKind::ArtifactLink => Self::ArtifactLink,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ToolCaptureStatus {
    Inbox,
    Promoted,
    Dismissed,
}

impl From<ToolCaptureStatus> for CaptureStatus {
    fn from(value: ToolCaptureStatus) -> Self {
        match value {
            ToolCaptureStatus::Inbox => Self::Inbox,
            ToolCaptureStatus::Promoted => Self::Promoted,
            ToolCaptureStatus::Dismissed => Self::Dismissed,
        }
    }
}
