// ── Numeric precision policy ──────────────────────────────────────────────────
// All Rust i64 fields cross the IPC boundary as JS `number` (IEEE 754 double,
// safe up to 2^53 ≈ 9 quadrillion). Every i64 in this codebase represents a
// bounded counter (deliverable_count, observations, sample_count), a day
// index, a duration in seconds, a byte size, a SQLite ROWID, or a schema
// version — none realistically approach 2^53. If a future field carries a
// uint64 hash, a nanosecond timestamp, or other unbounded user input,
// escalate the TS type to `bigint` and document why next to the field.

export type InitiativeStatus = "live" | "paused" | "shipped" | "parked";

export interface UserProfile {
  id: number;
  name: string;
  role: string | null;
  bio: string | null;
  avatar_url: string | null;
  email: string | null;
  updated_at: string;
}

export interface UpdateUserProfileInput {
  name: string;
  role: string | null;
  bio: string | null;
  avatar_url: string | null;
  email: string | null;
}


export interface Initiative {
  id: string;
  title: string;
  framing: string;
  status: InitiativeStatus;
  icon: string;
  icon_color: string;
  created_at: string;
  updated_at: string;
}

export interface CreateInitiativeInput {
  title: string;
  framing: string;
  status: InitiativeStatus;
  icon?: string;
  icon_color?: string;
}

export interface UpdateInitiativeInput {
  title: string;
  framing: string;
  status: InitiativeStatus;
  icon?: string;
  icon_color?: string;
}

export interface Stakeholder {
  id: string;
  name: string;
  display_order: number;
  email: string;
  role: string;
  notes: string;
}

export interface StakeholderRef {
  id: string;
  name: string;
  role: string;
}

export interface CreateStakeholderInput {
  name: string;
  email?: string;
  role?: string;
  notes?: string;
}

export interface UpdateStakeholderInput {
  name: string;
  email: string;
  role: string;
  notes: string;
}

export interface StakeholderDetail {
  stakeholder: Stakeholder;
  deliverable_count: number;
  shipped_count: number;
  in_flight_count: number;
  days_since_last_delivery: number | null;
}

export interface BriefingItem {
  text: string;
  source?: string;
}

export interface BriefingSections {
  tldr: string;
  open_commitments: BriefingItem[];
  waiting_on_them: BriefingItem[];
  recent_wins: BriefingItem[];
  in_flight: BriefingItem[];
  talking_points: string[];
  watch_out?: string;
}

export interface StakeholderBriefing {
  stakeholder: Stakeholder;
  generated_with: string;
  generated_at: string;
  sections: BriefingSections;
}

export type DeliverableType =
  | "deck"
  | "design_doc"
  | "prototype"
  | "analysis"
  | "framework"
  | "pitch"
  | "research"
  | "code"
  | "email"
  | "meeting_prep"
  | "spec"
  | "report"
  | "roadmap"
  | "brief"
  | "plan"
  | "other";

export type DeliverableState = "backlog" | "todo" | "drafting" | "in_review" | "shipped" | "killed";

export type DeliverablePriority = "p1" | "p2" | "p3";

export interface Label {
  id: string;
  name: string;
  color: string;
}

export interface LabelRef {
  id: string;
  name: string;
  color: string;
}

export interface CreateLabelInput {
  name: string;
  color: string;
}

export interface ApplyFlaggedToBacklogInput {
  action_id: string;
  initiative_ids: string[];
}

export type DeliverableStateColor = "blue" | "amber" | "green" | "zinc" | "violet";

export interface InitiativeRef {
  id: string;
  title: string;
  status: InitiativeStatus;
}

export interface Deliverable {
  id: string;
  title: string;
  type: DeliverableType;
  state: DeliverableState;
  claim: string;
  artifact_url: string | null;
  conversation_id: string | null;
  conversation_url: string | null;
  start_date: string | null;
  stakeholder_id: string | null;
  stakeholder_name: string | null;
  stakeholders: StakeholderRef[];
  created_at: string;
  shipped_at: string | null;
  updated_at: string;
  initiatives: InitiativeRef[];
  deadline: string | null;
  is_focused: boolean;
  effort: number | null;
  impact: number | null;
  blocker_reason: string | null;
  state_changed_at: string | null;
  display_order: number;
  priority: DeliverablePriority | null;
  labels: LabelRef[];
}

export type TaskStatus = "todo" | "doing" | "done";

export interface DeliverableTask {
  id: string;
  deliverable_id: string;
  title: string;
  status: TaskStatus;
  due_date: string | null;
  notes: string | null;
  url: string | null;
  display_order: number;
  created_at: string;
  updated_at: string;
}

export interface CreateTaskInput {
  deliverable_id: string;
  title: string;
  due_date: string | null;
  notes?: string | null;
  url?: string | null;
}

export interface UpdateTaskInput {
  title: string;
  status: TaskStatus;
  due_date: string | null;
  notes?: string | null;
  url?: string | null;
}

export interface DeliverableNote {
  id: string;
  deliverable_id: string;
  body: string;
  created_at: string;
}

export interface CreateNoteInput {
  deliverable_id: string;
  body: string;
}

export interface UpdateDeliverableMetadataInput {
  deadline: string | null;
  effort: number | null;
  impact: number | null;
  blocker_reason: string | null;
  priority: DeliverablePriority | null;
}

export interface GeneratedTasks {
  tasks: GeneratedTaskSuggestion[];
  suggested_deliverable_deadline: string | null;
  rationale: string;
}

export interface GeneratedTaskSuggestion {
  title: string;
  due_date: string | null;
  reason: string;
}

export interface ApplyGeneratedTasksInput {
  deliverable_id: string;
  tasks: GeneratedTaskSuggestion[];
  suggested_deliverable_deadline: string | null;
}

export interface ReorderDirectionInput {
  id: string;
  direction: "up" | "down";
}

export type WorkIntakeKind = "task" | "deliverable" | "initiative";
export type WorkIntakeStatus = "pending" | "approved" | "dismissed";

export interface WorkIntakeSuggestion {
  id: string;
  source_kind: string;
  source_id: string | null;
  source_title: string;
  source_route: string | null;
  item_kind: WorkIntakeKind;
  title: string;
  body: string;
  target_deliverable_id: string | null;
  target_initiative_id: string | null;
  due_date: string | null;
  suggested_type: string | null;
  confidence: number | null;
  rationale: string;
  status: WorkIntakeStatus;
  payload: string;
  created_at: string;
  updated_at: string;
  applied_at: string | null;
}

export interface WorkIntakeFilters {
  status?: WorkIntakeStatus;
  source_kind?: string;
  source_id?: string;
  limit?: number;
}

export interface ApproveWorkIntakeInput {
  id: string;
  target_deliverable_id?: string | null;
  target_initiative_id?: string | null;
  /** Override the AI-suggested kind before approving. */
  item_kind_override?: WorkIntakeKind | null;
  /** User-edited title. */
  title_override?: string | null;
  /** User-edited description body. */
  body_override?: string | null;
  /** User-edited due date (ISO date string). */
  due_date_override?: string | null;
}

export interface WorkIntakeApplyResult {
  suggestion: WorkIntakeSuggestion;
  entity_kind: string;
  entity_id: string;
  route: string;
}

export interface DeliverableFilters {
  initiative_id?: string;
  stakeholder_id?: string;
  type?: DeliverableType;
  state?: DeliverableState;
  /** OR-filter across multiple states. Takes precedence over `state` when both set. */
  state_in?: DeliverableState[];
  priority?: DeliverablePriority;
}

export interface CreateDeliverableInput {
  title: string;
  type: DeliverableType;
  state: DeliverableState;
  claim: string;
  artifact_url: string | null;
  conversation_id: string | null;
  stakeholder_id: string | null;
  stakeholder_ids: string[];
  initiative_ids: string[];
}

export type UpdateDeliverableInput = CreateDeliverableInput;

export interface ExportResult {
  export_path: string;
  initiative_count: number;
  stakeholder_count: number;
  deliverable_count: number;
}

export type CaptureKind = "thought" | "claude_link" | "artifact_link";
export type CaptureStatus = "inbox" | "promoted" | "dismissed" | "suggested";

export interface Capture {
  id: string;
  kind: CaptureKind;
  body: string;
  status: CaptureStatus;
  promoted_deliverable_id: string | null;
  promoted_deliverable_title: string | null;
  promoted_initiative_id: string | null;
  promoted_initiative_title: string | null;
  promoted_conversation_id: string | null;
  promoted_conversation_title: string | null;
  promoted_task_id: string | null;
  promoted_task_title: string | null;
  created_at: string;
  updated_at: string;
  promoted_at: string | null;
}

export interface PromoteCaptureToTaskInput {
  deliverable_id: string;
  title: string;
  notes?: string | null;
  due_date?: string | null;
}

export interface CreateCaptureInput {
  kind: CaptureKind;
  body: string;
}

export interface CaptureFilters {
  status?: CaptureStatus;
  kind?: CaptureKind;
}

export type PromotionKind = "task" | "deliverable" | "initiative";
export type PromotionStatus =
  | "pending"
  | "accepted"
  | "accepted_alternative"
  | "overridden"
  | "stale"
  | "errored"
  | "undone";

export interface CapturePromotionAlternative {
  kind: PromotionKind;
  target_id: string | null;
  target_kind: "deliverable" | "initiative" | null;
  target_title: string | null;
  confidence: number;
  rationale: string;
}

export interface CapturePromotionSuggestion {
  id: string;
  capture_id: string;
  kind: PromotionKind | "";
  target_id: string | null;
  target_kind: "deliverable" | "initiative" | null;
  target_title: string | null;
  confidence: number;
  rationale: string;
  alternatives: CapturePromotionAlternative[];
  status: PromotionStatus;
  error_reason: string | null;
  applied_entity_kind: string | null;
  applied_entity_id: string | null;
  model: string;
  latency_ms: number;
  created_at: number;
  resolved_at: number | null;
}

export interface AppliedPromotion {
  capture: Capture;
  kind: PromotionKind;
  applied_entity_kind: string;
  applied_entity_id: string;
}

export interface PromotionAccuracySummary {
  sample_count: number;
  accept_rate: number;
  target_match_rate: number;
  errored_count_7d: number;
  hint_threshold: number;
}

export interface MenuBarState {
  active_deliverable_id: string | null;
  active_deliverable_title: string | null;
  tray_title: string;
  shipped_this_week: number;
}

export interface McpInstallState {
  installed: boolean;
  binary_path: string;
  database_path: string;
  log_path: string;
  snippet_path: string;
  last_error: string | null;
}

export interface McpConfigSnippet {
  config_path: string;
  snippet: string;
}

export interface GeminiKeyStatus {
  configured: boolean;
}

export interface ToolCallLogEntry {
  id: string;
  ts: number;
  source: string;
  run_id: string | null;
  call_id: string | null;
  tool: string;
  args_json: string;
  result_summary: string | null;
  result_json: string | null;
  ok: boolean;
  error: string | null;
  latency_ms: number;
}

export interface ToolCallLogFilter {
  source?: string | null;
  tool?: string | null;
  only_errors?: boolean | null;
  run_id?: string | null;
  limit?: number | null;
}

export interface ToolCallLogSnapshot {
  entries: ToolCallLogEntry[];
  total_calls_24h: number;
  error_calls_24h: number;
}

export interface UsageByFeature {
  feature: string;
  calls: number;
  total_tokens: number;
  cached_tokens: number;
  cost_usd: number;
}

export interface UsageByModel {
  model: string;
  calls: number;
  total_tokens: number;
  cached_tokens: number;
  cost_usd: number;
}

export interface GeminiUsageSummary {
  period_hours: number;
  total_calls: number;
  error_calls: number;
  total_tokens: number;
  total_prompt_tokens: number;
  total_completion_tokens: number;
  total_cached_tokens: number;
  total_cost_usd: number;
  by_feature: UsageByFeature[];
  by_model: UsageByModel[];
}

export interface DailyBucket {
  date: string; // YYYY-MM-DD
  calls: number;
  tokens: number;
  cost_usd: number;
}

export interface DailyTrend {
  period_days: number;
  total_cost_usd: number;
  buckets: DailyBucket[];
}

export interface AppConfig {
  budget_daily_usd: number;
  budget_monthly_usd: number;
  budget_alert_threshold_pct: number;
  budget_block_when_exceeded: boolean;
}

export type AlertState = "ok" | "warning" | "exceeded";

export interface BudgetStatus {
  daily_spent_usd: number;
  monthly_spent_usd: number;
  daily_limit_usd: number;
  monthly_limit_usd: number;
  daily_pct: number;
  monthly_pct: number;
  alert_threshold_pct: number;
  alert_state: AlertState;
  block_active: boolean;
}

export interface BudgetEvent {
  kind: "warning" | "exceeded" | "recovery";
  scope: "daily" | "monthly";
  spent_usd: number;
  limit_usd: number;
  threshold_pct: number;
  block_active: boolean;
}

export type EvalFixtureKind =
  | "retrieval"
  | "ask"
  | "classification"
  | "promotion";

export interface EvalFixture {
  id: string;
  kind: string;
  name: string;
  input_json: string;
  expectation_json: string;
  notes: string | null;
  enabled: boolean;
  created_at: number;
  updated_at: number;
}

export interface CreateEvalFixtureInput {
  kind: EvalFixtureKind;
  name: string;
  input_json: string;
  expectation_json: string;
  notes?: string | null;
}

export interface EvalRun {
  id: string;
  fixture_id: string;
  ts: number;
  passed: boolean;
  score: number;
  metric: string;
  details_json: string | null;
  latency_ms: number;
  baseline_score: number | null;
  delta: number | null;
}

export interface EvalKindBreakdown {
  kind: string;
  count: number;
  passed: number;
  avg_score: number;
}

export interface EvalSummary {
  total: number;
  passed: number;
  failed: number;
  avg_score: number;
  by_kind: EvalKindBreakdown[];
}

export interface ImportEvalFixturesInput {
  fixtures: unknown[];
  skip_existing?: boolean;
}

export interface ImportEvalFixturesResult {
  imported: number;
  skipped: number;
  errors: string[];
}

export interface EmbeddingProgress {
  model: string;
  dim: number;
  total: number;
  embedded: number;
  pending: number;
  stale: number;
}

export type ClassificationSource = "override" | "rule" | "llm";

export interface EffectiveClassification {
  category: string;
  priority: string;
  intent: string | null;
  action_required: boolean;
  thread_state: string | null;
  work_relevance: WorkMailRelevance;
  attention_state: WorkMailAttentionState;
  message_type: WorkMailMessageType;
  recency_adjusted_priority: string;
  recency_decay_note: string | null;
  source: ClassificationSource;
  override_note: string | null;
  rule_id: string | null;
}

export interface UserClassification {
  thread_id: string;
  category: string | null;
  priority: string | null;
  intent: string | null;
  action_required: boolean | null;
  thread_state: string | null;
  work_relevance: WorkMailRelevance | null;
  attention_state: WorkMailAttentionState | null;
  message_type: WorkMailMessageType | null;
  note: string | null;
  source: string;
  rule_id: string | null;
  created_at: number;
  updated_at: number;
}

export interface SetOverrideInput {
  thread_id: string;
  category?: string | null;
  priority?: string | null;
  intent?: string | null;
  action_required?: boolean | null;
  thread_state?: string | null;
  work_relevance?: WorkMailRelevance | null;
  attention_state?: WorkMailAttentionState | null;
  message_type?: WorkMailMessageType | null;
  note?: string | null;
}

export interface SenderRule {
  id: string;
  pattern: string;
  pattern_kind: "exact" | "glob" | "domain" | string;
  category: string | null;
  priority: string | null;
  work_relevance: WorkMailRelevance | null;
  attention_state: WorkMailAttentionState | null;
  message_type: WorkMailMessageType | null;
  note: string | null;
  enabled: boolean;
  applied_count: number;
  created_at: number;
  updated_at: number;
}

export interface CreateSenderRuleInput {
  pattern: string;
  pattern_kind?: "exact" | "glob" | "domain";
  category?: string | null;
  priority?: string | null;
  work_relevance?: WorkMailRelevance | null;
  attention_state?: WorkMailAttentionState | null;
  message_type?: WorkMailMessageType | null;
  note?: string | null;
}

export type GmailIntent =
  | "asking"
  | "informing"
  | "requesting_decision"
  | "scheduling"
  | "acknowledging"
  | "venting"
  | "other";

export type GmailPredictedAction =
  | "reply"
  | "schedule"
  | "file"
  | "ignore"
  | "other";

export type GmailThreadState =
  | "waiting_on_you"
  | "waiting_on_them"
  | "resolved"
  | "dormant";

export interface GmailInboxDashboard {
  since_ms: number;
  classified_count: number;
  action_required_count: number;
  waiting_on_you_count: number;
  high_priority_count: number;
  failed_classifications: number;
  top_intents: [string, number][];
}

export interface CalibrationByDimension {
  dimension: string;
  original: string;
  corrected: string;
  count: number;
  rate: number;
  rate_lo: number;
  rate_hi: number;
}

export interface IsotonicPoint {
  confidence: number;
  accuracy: number;
  raw_accuracy: number;
  samples: number;
}

export interface IsotonicDimensionCurve {
  dimension: string;
  sample_count: number;
  points: IsotonicPoint[];
}

export interface CalibrationReport {
  total_overrides: number;
  by_dimension: CalibrationByDimension[];
  note: string;
  isotonic: IsotonicDimensionCurve[];
}

export interface GeminiTestResult {
  ok: boolean;
  message: string;
}

export interface Conversation {
  id: string;
  chat_url: string;
  title: string | null;
  summary: string | null;
  occurred_at: string | null;
  ingested_at: string;
}

export interface ConversationExtractionInput {
  chat_url: string | null;
  pasted_text: string | null;
}

export interface ExtractedConversation {
  title: string;
  summary: string;
  occurred_at: string | null;
}

export interface ExtractedDeliverableCandidate {
  title: string;
  type: DeliverableType;
  claim: string;
  artifact_url: string | null;
  stakeholder_name: string | null;
  initiative_titles: string[];
  validation_errors: string[];
}

export interface ConversationExtractionResult {
  conversation: ExtractedConversation;
  candidates: ExtractedDeliverableCandidate[];
  source_chat_url: string | null;
  source_kind: string;
}

export interface CommitExtractedDeliverableInput {
  accepted: boolean;
  title: string;
  type: DeliverableType;
  claim: string;
  artifact_url: string | null;
  stakeholder_id: string | null;
  stakeholder_ids: string[];
  initiative_ids: string[];
}

export interface CommitConversationIngestInput {
  chat_url: string | null;
  conversation: ExtractedConversation;
  deliverables: CommitExtractedDeliverableInput[];
}

export interface ConversationIngestResult {
  conversation: Conversation;
  deliverables: Deliverable[];
}

export interface WorkGraphFilters {
  include_dismissed_captures: boolean;
  include_killed_deliverables: boolean;
}

export interface BrainGraphFilters extends WorkGraphFilters {
  node_kinds?: string[];
  relation_kinds?: string[];
  query?: string | null;
  limit?: number | null;
}

export interface BrainStatus {
  path: string;
  exists: boolean;
  schema_version: number;
  storage_version: number;
  generated_at: string | null;
  node_count: number;
  edge_count: number;
  error: string | null;
}

// ── Phase 2 brain explorer surfaces ─────────────────────────────────────

export interface SavedBrainView {
  id: string;
  name: string;
  description: string;
  filters: Record<string, unknown>;
  layout: Record<string, unknown>;
  viewport: Record<string, unknown>;
  pinned: unknown;
  created_at: number;
  updated_at: number;
}

export interface SaveBrainViewInput {
  id?: string | null;
  name: string;
  description?: string | null;
  filters: Record<string, unknown>;
  layout: Record<string, unknown>;
  viewport: Record<string, unknown>;
  pinned: unknown;
}

export interface GraphCommunityMemberSummary {
  entity_kind: string;
  entity_id: string;
}

export interface GraphCommunitySummary {
  id: string;
  community_kind: string;
  scope_key: string;
  title: string;
  level: number;
  status: "active" | "stale" | "archived" | string;
  summary_markdown: string | null;
  member_count: number;
  members: GraphCommunityMemberSummary[];
  created_at: string;
  updated_at: string;
}

export interface BrainRetrieveInput {
  query: string;
  focus_entity_id?: string | null;
  max_hops?: number | null;
  limit?: number | null;
}

export interface ScoredBrainNode {
  node_id: string;
  entity_id: string;
  kind: string;
  bm25_norm: number;
  cosine: number;
  recency_multiplier: number;
  node_weight_norm: number;
  focus_proximity: number;
  learned_factor: number;
  blended_score: number;
}

export interface BrainContextResult {
  query: string;
  summary: string;
  graph: WorkGraph;
  ranked_nodes: WorkGraphNode[];
  scored_nodes?: ScoredBrainNode[];
}

export interface BrainPolicySummary {
  template: string;
  observations: number;
  updated_at: string;
}

export interface BrainRetrievalBlend {
  bm25: number;
  cosine: number;
  node_weight: number;
  focus_proximity: number;
}

export interface InferenceThresholdSummary {
  template: string;
  threshold: number;
  sample_count: number;
  last_recomputed: string;
}

export interface BrainLearningSummary {
  policies: BrainPolicySummary[];
  blend: BrainRetrievalBlend;
  inference_thresholds: InferenceThresholdSummary[];
}

// ── Section 6.2 — RL feedback surface ────────────────────────────────────

export type BrainInferenceStatus = "pending" | "accepted" | "rejected";

export interface BrainInferenceRecord {
  id: string;
  source_kind: string;
  source_id: string;
  relation_kind: string;
  target_kind: string;
  target_id: string;
  confidence: number;
  rationale: string;
  evidence_json: string;
  status: BrainInferenceStatus;
  generated_by: string;
  created_at: string;
  updated_at: string;
  reviewed_at: string | null;
  template: string | null;
  superseded_by: string | null;
  supersede_reason: string | null;
}

export interface BrainInferenceRow extends BrainInferenceRecord {
  subject_label: string | null;
  target_label: string | null;
  threshold: number | null;
}

export interface BrainInferenceFilter {
  status?: BrainInferenceStatus | null;
  template?: string | null;
  limit?: number | null;
  before_updated_at?: string | null;
}

export interface BrainInferenceListResult {
  items: BrainInferenceRow[];
  total_pending: number;
  total_accepted_7d: number;
  total_rejected_7d: number;
  has_more: boolean;
  next_cursor: string | null;
}

export interface ReviewInferenceResult {
  inference_id: string;
  status: BrainInferenceStatus;
  template: string | null;
  threshold_before: number | null;
  threshold_after: number | null;
  sample_count: number | null;
  superseded_inference_ids: string[];
}

export interface SupersessionRecord {
  loser: BrainInferenceRow;
  winner: BrainInferenceRow;
  supersede_reason: string;
  superseded_at: string;
}

export interface TopTemplateSummary {
  name: string;
  accepted: number;
  rejected: number;
  acceptance_rate: number;
}

export interface RLDigest {
  window_days: number;
  inferences_generated: number;
  inferences_accepted: number;
  inferences_rejected: number;
  acceptance_rate: number;
  supersessions: number;
  top_template: TopTemplateSummary | null;
  ask_useful: number;
  ask_wrong: number;
  ask_feedback_rate: number;
  embeddings_added: number;
  threshold_drift: number;
}

export interface TemplateCoefficient {
  feature: string;
  mean: number;
  abs_max: number;
}

export interface TemplateDetail {
  template: string;
  observations: number;
  threshold: number | null;
  sample_count: number | null;
  feature_names: string[];
  coefficient_summary: TemplateCoefficient[];
  recent_events: BrainLearningEvent[];
}

export interface BrainCypherInput {
  query: string;
  params?: Record<string, unknown> | null;
  limit?: number | null;
}

export interface BrainCypherResult {
  columns: string[];
  rows: Record<string, unknown>[];
  truncated: boolean;
}

export type BrainTemplateKind =
  | "focus_today"
  | "blocked_work"
  | "email_followups"
  | "stale_work"
  | "stakeholder_context";

export interface BrainTemplateInput {
  template: BrainTemplateKind;
  focus_entity_id?: string | null;
  limit?: number | null;
}

export interface BrainTemplateResult {
  template: string;
  summary: string;
  cypher: string;
  rows: Record<string, unknown>[];
  graph: WorkGraph;
}

export interface BrainBrief {
  generated_at: string;
  focus_today: BrainTemplateResult;
  blocked_or_waiting: BrainTemplateResult;
  email_followups: BrainTemplateResult;
  stale_work: BrainTemplateResult;
  inferences_to_review: Record<string, unknown>[];
  markdown: string;
}

export interface BrainFeedbackInput {
  question: string;
  template?: string | null;
  feedback: "useful" | "wrong" | "ignored";
  corrected?: Record<string, unknown> | null;
}

export interface BrainLearningEventInput {
  template?: string | null;
  item_id: string;
  item_kind?: string | null;
  event_type: string;
  reward?: number | null;
  context?: Record<string, unknown> | null;
}

export interface BrainLearningEvent {
  id: string;
  template: string;
  item_id: string;
  item_kind: string;
  event_type: string;
  reward: number;
  context_json: string;
  created_at: string;
}

export interface BrainRlPolicyRecord {
  template: string;
  feature_names_json: string;
  a_matrix_json: string;
  b_vector_json: string;
  observations: number;
  alpha: number;
  updated_at: string;
}

export interface BrainRlItemScore {
  template: string;
  item_id: string;
  item_kind: string;
  score: number;
  exploitation: number;
  exploration: number;
  features_json: string;
  updated_at: string;
}

export interface BrainLearningSnapshot {
  policies: BrainRlPolicyRecord[];
  recent_events: BrainLearningEvent[];
  top_scores: BrainRlItemScore[];
}

export interface WorkGraph {
  generated_at: string;
  ai_context: string;
  nodes: WorkGraphNode[];
  edges: WorkGraphEdge[];
}

export interface WorkGraphNode {
  id: string;
  entity_id: string;
  kind: string;
  label: string;
  subtitle: string | null;
  status: string | null;
  url: string | null;
  updated_at: string | null;
  hidden_by_default: boolean;
  weight: number;
  context: string;
  properties: Record<string, unknown>;
}

export type MemoryKind = "episodic" | "semantic" | "procedural";
export type MemoryStatus = "active" | "superseded" | "archived" | "deleted";
export type MemoryScope = "global" | "project" | "session";
export type MemorySensitivity = "normal" | "pii" | "sensitive";

export interface MemorySettings {
  enabled: boolean;
  auto_extract_enabled: boolean;
  work_related_only: boolean;
  preserve_continuity: boolean;
  require_confirmation: boolean;
  retrieval_limit: number;
  updated_at: string;
}

export interface UpdateMemorySettingsInput {
  enabled: boolean;
  auto_extract_enabled: boolean;
  work_related_only: boolean;
  preserve_continuity: boolean;
  require_confirmation: boolean;
  retrieval_limit: number;
}

export interface MemoryRecord {
  id: string;
  kind: MemoryKind;
  status: MemoryStatus;
  scope: MemoryScope;
  title: string;
  body: string;
  canonical_key: string;
  source: "manual" | "generated" | "consolidated" | "system";
  source_kind: string | null;
  source_id: string | null;
  confidence: number;
  importance: number;
  retrieval_count: number;
  success_count: number;
  contradiction_count: number;
  tags: string[];
  evidence: string[];
  supersedes_id: string | null;
  version: number;
  expires_at: string | null;
  archived_at: string | null;
  deleted_at: string | null;
  last_retrieved_at: string | null;
  sensitivity: MemorySensitivity;
  pinned: boolean;
  created_at: string;
  updated_at: string;
}

export interface ListMemoryFilters {
  kind?: MemoryKind;
  status?: MemoryStatus;
  query?: string;
  include_archived?: boolean;
}

export interface CreateMemoryInput {
  kind: MemoryKind;
  title: string;
  body: string;
  scope: MemoryScope;
  tags: string[];
  confidence?: number;
  importance?: number;
  sensitivity?: MemorySensitivity;
  pinned?: boolean;
  expires_at?: string;
}

export interface UpdateMemoryInput {
  kind: MemoryKind;
  status: MemoryStatus;
  title: string;
  body: string;
  scope: MemoryScope;
  tags: string[];
  confidence?: number;
  importance?: number;
  sensitivity?: MemorySensitivity;
  pinned?: boolean;
  expires_at?: string;
}

export interface RetrieveMemoryInput {
  query: string;
  limit?: number;
  kinds?: MemoryKind[];
  source_kind?: string;
  source_id?: string;
  task_type?: string;
  include_pinned?: boolean;
}

export interface ScoredMemory {
  memory: MemoryRecord;
  score: number;
  semantic_score: number;
  lexical_score: number;
  recency_score: number;
  procedural_pin: boolean;
}

export interface MemoryRetrievalDiagnostics {
  semantic_used: boolean;
  lexical_used: boolean;
  procedural_pin_count: number;
  embedding_model?: string | null;
  embedding_error?: string | null;
}

export interface MemoryRetrievalResult {
  context: string;
  memories: MemoryRecord[];
  scored: ScoredMemory[];
  diagnostics: MemoryRetrievalDiagnostics;
  retrieval_id?: string | null;
}

export interface MemoryConsolidationResult {
  run_id: string;
  created_count: number;
  updated_count: number;
  archived_count: number;
  memories: MemoryRecord[];
}

export interface MemoryExtractionResult {
  created_count: number;
  updated_count: number;
  skipped_count: number;
  memories: MemoryRecord[];
}

export interface MemoryEvent {
  id: string;
  memory_id: string | null;
  action: string;
  detail_json: string;
  created_at: string;
}

export interface ListMemoryEventsFilters {
  memory_id?: string;
  limit?: number;
}

export type MemoryFeedback = "useful" | "not_useful" | "wrong";

export interface MemoryFeedbackInput {
  retrieval_id: string;
  feedback: MemoryFeedback;
}

// ── Server-side Ask chat persistence ─────────────────────────────────────────

export interface AskChatRecord {
  id: string;
  title: string;
  mode: string;
  summary: string;
  archived_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface AskTurnAttachmentRecord {
  id: string;
  turn_id: string;
  mime_type: string;
  filename: string | null;
  data_b64: string;
  size_bytes: number;
  created_at: string;
}

export interface AskTurnRecord {
  id: string;
  chat_id: string;
  parent_id: string | null;
  fork_of: string | null;
  mode: string | null;
  question: string;
  answer: string;
  reasoning: string;
  status: string;
  error: string | null;
  refs: SearchResult[];
  questions: AskUserQuestion[];
  steps: unknown[];
  attachments: AskTurnAttachmentRecord[];
  /** Section 6.2 — per-cited-node retrieval breakdown for "Why this answer?". */
  scored_nodes?: ScoredBrainNode[];
  /** The query the brain saw (may diverge from the user's question). */
  retrieval_query?: string | null;
  created_at: string;
  updated_at: string;
}

export interface AskChatDetail {
  chat: AskChatRecord;
  turns: AskTurnRecord[];
}

export interface UpsertAskChatInput {
  id?: string | null;
  title: string;
  mode: string;
  summary?: string;
}

export interface AskAttachmentSave {
  mime_type: string;
  filename?: string | null;
  data: string;
  size?: number | null;
}

export interface AppendAskTurnInput {
  chat_id: string;
  turn_id: string;
  parent_id: string | null;
  fork_of: string | null;
  mode: string | null;
  question: string;
  answer?: string | null;
  reasoning?: string | null;
  status: string;
  error?: string | null;
  refs?: SearchResult[];
  questions?: AskUserQuestion[];
  steps?: unknown[];
  attachments?: AskAttachmentSave[];
  /** Section 6.2 — persist retrieval breakdown alongside the turn. */
  scored_nodes?: ScoredBrainNode[];
  retrieval_query?: string | null;
}

export interface ListAskChatsFilters {
  include_archived?: boolean;
  limit?: number;
}

export interface AskChatSearchHit {
  chat_id: string;
  chat_title: string;
  turn_id: string;
  question_snippet: string;
  answer_snippet: string;
  created_at: string;
}

export interface WorkGraphEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  label: string;
  properties: Record<string, unknown>;
}

export const initiativeStatusLabels: Record<InitiativeStatus, string> = {
  live: "Live",
  paused: "Paused",
  shipped: "Finished",
  parked: "Parked",
};

export const initiativeStatusOptions: InitiativeStatus[] = [
  "live",
  "paused",
  "shipped",
  "parked",
];

export const deliverableTypeLabels: Record<DeliverableType, string> = {
  deck: "Deck",
  design_doc: "Design doc",
  prototype: "Prototype",
  analysis: "Analysis",
  framework: "Framework",
  pitch: "Pitch",
  research: "Research",
  code: "Code",
  email: "Email",
  meeting_prep: "Meeting prep",
  spec: "Spec",
  report: "Report",
  roadmap: "Roadmap",
  brief: "Brief",
  plan: "Plan",
  other: "Other",
};

export const deliverableTypeOptions: DeliverableType[] = [
  "deck",
  "design_doc",
  "prototype",
  "analysis",
  "framework",
  "pitch",
  "research",
  "code",
  "email",
  "meeting_prep",
  "spec",
  "report",
  "roadmap",
  "brief",
  "plan",
  "other",
];

export const deliverableStateLabels: Record<DeliverableState, string> = {
  backlog: "Backlog",
  todo: "To Do",
  drafting: "In Progress",
  in_review: "In Review",
  shipped: "Done",
  killed: "Dropped",
};

export const deliverableStateColors: Record<DeliverableState, DeliverableStateColor> = {
  backlog: "zinc",
  todo: "violet",
  drafting: "blue",
  in_review: "amber",
  shipped: "green",
  killed: "zinc",
};

export const deliverableStateOptions: DeliverableState[] = [
  "backlog",
  "todo",
  "drafting",
  "in_review",
  "shipped",
  "killed",
];

export const priorityLabels: Record<DeliverablePriority, string> = {
  p1: "P1",
  p2: "P2",
  p3: "P3",
};

export const priorityColors: Record<DeliverablePriority, string> = {
  p1: "bg-red-100 text-red-700",
  p2: "bg-amber-100 text-amber-700",
  p3: "bg-sky-100 text-sky-700",
};

export const labelColors: Record<string, { bg: string; text: string }> = {
  zinc: { bg: "bg-zinc-100", text: "text-zinc-600" },
  red: { bg: "bg-red-100", text: "text-red-600" },
  amber: { bg: "bg-amber-100", text: "text-amber-700" },
  green: { bg: "bg-emerald-100", text: "text-emerald-700" },
  blue: { bg: "bg-blue-100", text: "text-blue-700" },
  violet: { bg: "bg-violet-100", text: "text-violet-700" },
  pink: { bg: "bg-pink-100", text: "text-pink-700" },
};

export const captureKindLabels: Record<CaptureKind, string> = {
  thought: "Thought",
  claude_link: "Claude link",
  artifact_link: "Artifact link",
};

export const captureKindOptions: CaptureKind[] = ["thought", "claude_link", "artifact_link"];

export const captureStatusLabels: Record<CaptureStatus, string> = {
  inbox: "Inbox",
  suggested: "Suggestions",
  promoted: "Promoted",
  dismissed: "Dismissed",
};

export const captureStatusOptions: CaptureStatus[] = ["inbox", "suggested", "promoted", "dismissed"];

// ── Google Calendar ────────────────────────────────────────────────────────────

export interface GCalAttendee {
  email: string;
  name: string | null;
  self: boolean;
  organizer: boolean;
  responseStatus: string | null;
}

export interface GCalConferenceData {
  conference_id: string | null;
  solution_name: string | null;
  video_uri: string | null;
  phone_uri: string | null;
  phone_pin: string | null;
}

export interface GCalOrganizer {
  email: string;
  name: string | null;
  self: boolean;
}

export interface GCalAttachment {
  fileUrl: string;
  title: string | null;
  mimeType: string | null;
  fileId: string | null;
}

export interface GCalReminderOverride {
  method: string;
  minutes: number;
}

export interface GCalReminders {
  useDefault: boolean;
  overrides: GCalReminderOverride[] | null;
}

export interface GCalEvent {
  id: string;
  gcal_event_id: string;
  title: string;
  description: string | null;
  location: string | null;
  attendees: string | null;        // JSON string → GCalAttendee[]
  conference_data: string | null;  // JSON string → GCalConferenceData
  organizer: string | null;        // JSON string → GCalOrganizer
  recurring_event_id: string | null;
  recurrence: string | null;       // JSON string → string[]
  color_id: string | null;
  transparency: string | null;
  event_updated_at: string | null;
  attachments: string | null;      // JSON string → GCalAttachment[]
  reminders: string | null;        // JSON string → GCalReminders
  visibility: string | null;
  start_date: string;
  end_date: string | null;
  start_datetime: string | null;
  end_datetime: string | null;
  is_all_day: boolean;
  html_link: string | null;
}

export interface GCalStatus {
  connected: boolean;
}

// ── Week view ──────────────────────────────────────────────────────────────────

export interface WeekDay {
  day_index: number;
  deliverable: Deliverable | null;
  deadline_deliverables: Deliverable[];
  tasks: WeekTask[];
  gcal_events: GCalEvent[];
}

export interface WeekTask {
  id: string;
  deliverable_id: string;
  deliverable_title: string;
  deliverable_type: DeliverableType;
  deliverable_state: DeliverableState;
  stakeholder_name: string | null;
  title: string;
  status: TaskStatus;
  due_date: string;
}

export interface WeekView {
  week_start: string;
  days: WeekDay[];
  meeting_date: string | null;
  gcal_connected: boolean;
}

export interface SetDayInput {
  week_start: string;
  day_index: number;
  deliverable_id: string | null;
}

export interface MeetingConfig {
  next_meeting_date: string | null;
}

export interface WeekDayAssignment {
  day_index: number;
  deliverable_id: string;
}

export interface GeneratedWeekPlan {
  assignments: WeekDayAssignment[];
}

// ── Meetings ──────────────────────────────────────────────────────────────────

export type MeetingStatus = "draft" | "done" | "error";
// Audio-recording action kinds (require manual apply/dismiss)
export type AudioMeetingActionKind = "deliverable_note" | "initiative_note" | "capture";
// Agent minutes action kinds (already applied)
export type AgentMeetingActionKind =
  | "note_added"
  | "task_created"
  | "state_updated"
  | "deadline_set"
  | "blocker_set"
  | "capture_created"
  | "flagged";

export type MeetingActionKind = AudioMeetingActionKind | AgentMeetingActionKind;

export interface Meeting {
  id: string;
  title: string;
  date: string;
  duration_secs: number | null;
  transcript: string | null;
  summary: string | null;
  key_decisions: string | null; // JSON-encoded string[] from Rust; parse via parseKeyDecisions() from src/lib/meeting.ts
  status: MeetingStatus;
  error_message: string | null;
  stakeholders: StakeholderRef[];
  created_at: string;
  updated_at: string;
}

export interface MeetingAction {
  id: string;
  meeting_id: string;
  kind: MeetingActionKind;
  target_id: string | null;
  target_title: string | null;
  body: string;
  payload: string | null;
  applied: boolean;
  created_at: string;
}

export interface MeetingWithActions {
  meeting: Meeting;
  actions: MeetingAction[];
}

export interface CreateMeetingInput {
  title: string;
  date: string;
  stakeholder_ids: string[];
}

export interface ProcessMeetingInput {
  meeting_id: string;
  audio_base64: string;
  mime_type: string;
  duration_secs: number;
}

export interface ApplyMeetingActionInput {
  action_id: string;
  target_id: string | null;
}

export interface InitiativeNote {
  id: string;
  initiative_id: string;
  body: string;
  created_at: string;
}

// ── Unified search ─────────────────────────────────────────────────────────────

export type SearchResultKind =
  | "deliverable"
  | "initiative"
  | "stakeholder"
  | "meeting"
  | "capture"
  | "email"
  | "conversation"
  | "memory"
  | "email_thread"
  | "email_message"
  | "calendar_event"
  | "file"
  | "ask_turn"
  | "web";

export interface SearchResult {
  kind: SearchResultKind;
  entity_id: string;
  title: string;
  subtitle: string | null;
  route: string;
  state: string | null;
}

export interface AskSearchResult {
  answer: string;
  refs: SearchResult[];
  questions: AskUserQuestion[];
  /** Section 6.2 — per-node retrieval breakdown. Empty when the turn made
   *  no `retrieve_brain_context` call. */
  scored_nodes?: ScoredBrainNode[];
  /** The query the brain saw. */
  retrieval_query?: string | null;
}

export interface AskUserQuestionOption {
  label: string;
  description: string;
}

export interface AskUserQuestion {
  header: string;
  question: string;
  options: AskUserQuestionOption[];
}

// ── Reasoning GraphRAG and Reports ──────────────────────────────────────────

export type ReasoningDepth = "standard" | "deep";
export type GraphQueryMode = "auto" | "local" | "global" | "drift";
export type ReportType = "quarterly" | "initiative" | "decision_memo";

export interface ReasoningScope {
  date_from?: string | null;
  date_to?: string | null;
  quarter?: string | null;
  initiative_ids: string[];
  stakeholder_ids: string[];
  deliverable_ids: string[];
  source_kinds: string[];
}

export interface ReasoningQueryInput {
  query: string;
  depth: ReasoningDepth;
  query_mode: GraphQueryMode;
  scope: ReasoningScope;
  purpose?: string | null;
}

export interface ReasoningCitation {
  source_unit_id: string;
  source_kind: string;
  source_id: string;
  label: string;
  title: string;
  route?: string | null;
}

export interface GeneratedAssertion {
  statement: string;
  source_unit_ids: string[];
  confidence: number;
}

export interface ActionProposal {
  proposal_type: string;
  description: string;
  payload_json: string;
}

export interface ReasoningResult {
  run_id: string;
  query_mode: string;
  answer_markdown: string;
  citations: ReasoningCitation[];
  approved_claim_ids: string[];
  generated_assertions: GeneratedAssertion[];
  action_proposals: ActionProposal[];
  contradictions: string[];
  unsupported_assertions: string[];
  model: string;
  cache_hit: boolean;
  latency_ms: number;
  generated_analysis: boolean;
}

export interface ReasoningIndexSummary {
  source_units_created: number;
  source_units_updated: number;
  source_units_unchanged: number;
  source_units_removed: number;
  canonical_claims_synced: number;
  communities_synced: number;
  candidate_claims_staged: number;
}

export interface ReasoningRun {
  id: string;
  query_text: string;
  depth: string;
  query_mode: string;
  scope_json: string;
  result_markdown: string;
  citations_json: string;
  generated_assertions_json: string;
  action_proposals_json: string;
  contradictions_json: string;
  unsupported_json: string;
  model: string;
  cache_hit: number;
  latency_ms: number;
  status: string;
  created_at: string;
  updated_at: string;
}

export interface ClaimReviewItem {
  id: string;
  claim_key: string;
  statement: string;
  source_kind: string | null;
  source_id: string | null;
  confidence: number;
  contradiction_state: string;
  evidence_json: string;
  status: string;
  generated_by: string;
  reasoning_run_id: string | null;
  created_at: string;
  updated_at: string;
  reviewed_at: string | null;
}

export interface AgentReviewItem {
  id: string;
  source_kind: string;
  source_id: string;
  proposal_type: string;
  proposed_change_json: string;
  rationale: string;
  status: string;
  created_at: string;
  resolved_at: string | null;
}

export interface CommunityReport {
  id: string;
  community_id: string;
  title: string;
  summary_markdown: string;
  evidence_json: string;
  version_hash: string;
  status: string;
  model: string;
  created_at: string;
  updated_at: string;
  reviewed_at: string | null;
}

export interface CreateReportRunInput {
  report_type: ReportType;
  title: string;
  scope: ReasoningScope;
}

export interface ReportRun {
  id: string;
  report_type: ReportType;
  title: string;
  status: string;
  scope_json: string;
  outline_markdown: string;
  draft_markdown: string;
  citations_json: string;
  unsupported_json: string;
  reasoning_run_id: string | null;
  deliverable_id: string | null;
  created_at: string;
  updated_at: string;
  approved_at: string | null;
  // Pipeline v2 fields (may be absent in older rows — default to empty)
  scope_exclusions_json: string;
  sections_json: string;
  section_drafts_json: string;
  critique_json: string;
}

export interface ReportSectionPlan {
  id: string;
  heading: string;
  instructions: string;
  status: string;
  position: number;
}

export interface ReportSectionDraft {
  markdown: string;
  citation_ids: string[];
  last_drafted_at: string | null;
  cache_hit: boolean;
}

export interface ReportCritiqueIssue {
  id: string;
  section_id: string;
  kind: "contradiction" | "weak_citation" | "scope_leak" | "unsupported";
  message: string;
  span_text: string | null;
  suggestion: string | null;
}

export interface ReportCritique {
  issues: ReportCritiqueIssue[];
  generated_at: string | null;
}

export interface ReportScopeEntry {
  id: string;
  kind: string;
  title: string;
  subtitle: string | null;
}

export interface ReportScopePreview {
  initiative_ids: string[];
  initiative_titles: string[];
  deliverable_ids: string[];
  stakeholder_ids: string[];
  email_thread_ids: string[];
  meeting_ids: string[];
  file_ids: string[];
  calendar_event_ids: string[];
  entries: ReportScopeEntry[];
  total_entries: number;
  excluded_entry_ids: string[];
}

export interface ReportStep {
  id: string;
  report_run_id: string;
  step_name: string;
  section_index: number | null;
  status: "queued" | "running" | "awaiting_clarification" | "done" | "error" | "cancelled";
  model: string | null;
  cache_hit: boolean;
  started_at: string | null;
  finished_at: string | null;
  latency_ms: number | null;
  input_json: string;
  output_json: string;
  clarification_json: string | null;
  ticker_label: string | null;
  error_text: string | null;
  created_at: string;
  updated_at: string;
}

// Clarification prompt payload (matches Rust ReportClarification)
export interface ReportClarificationPrompt {
  question: string;
  options: string[];
  free_text_allowed: boolean;
  multi_select: boolean;
}

// Discriminated union for the `report:event` Tauri channel.
// Field names match the Rust ReportEvent serde-serialised output exactly.
export type ReportChannelEvent =
  | { kind: "RunStarted"; report_run_id: string }
  | { kind: "StepCreated"; step_id: string; report_run_id: string; step_name: string; section_index: number | null; section_id: string | null }
  | { kind: "StepStarted"; step_id: string; report_run_id: string; step_name: string; ticker_label: string | null }
  | { kind: "StepProgress"; step_id: string; ticker_label: string; detail: string | null }
  | { kind: "SectionDelta"; step_id: string; section_id: string; text: string }
  | { kind: "StepNeedsClarification"; step_id: string; report_run_id: string; prompt: ReportClarificationPrompt }
  | { kind: "StepCompleted"; step_id: string; report_run_id: string; step_name: string; output_summary: string | null; latency_ms: number | null; model: string | null; cache_hit: boolean }
  | { kind: "StepFailed"; step_id: string; report_run_id: string; step_name: string; error: string }
  | { kind: "RunFinished"; report_run_id: string; status: string };

export interface ReportExport {
  id: string;
  report_run_id: string;
  format: "markdown" | "docx" | "pdf" | "google_docs";
  export_path: string | null;
  artifact_url: string | null;
  created_at: string;
}

// ── Gantt ──────────────────────────────────────────────────────────────────────

export interface InitiativeSection {
  id: string;
  initiative_id: string;
  title: string;
  position: number;
  created_at: string;
  updated_at: string;
}

export interface CreateSectionInput {
  initiative_id: string;
  title: string;
}

export interface UpdateSectionInput {
  title: string;
  position: number;
}

export interface GanttDeliverable {
  id: string;
  title: string;
  state: DeliverableState;
  section_id: string | null;
  start_date: string | null;
  deadline: string | null;
  task_count: number;
  done_task_count: number;
}

export interface InitiativeGantt {
  initiative: Initiative;
  sections: InitiativeSection[];
  deliverables: GanttDeliverable[];
}

export interface UpdateGanttDatesInput {
  start_date: string | null;
  deadline: string | null;
}

export const meetingActionKindLabels: Record<AudioMeetingActionKind, string> = {
  deliverable_note: "Deliverable note",
  initiative_note: "Initiative note",
  capture: "Capture",
};

// ── Agentic minutes ────────────────────────────────────────────────────────────

export interface AskProgressEvent {
  kind: string;
  tool: string;
  label: string;
}

// ── Weekly digest ──────────────────────────────────────────────────────────────

export interface DigestItem {
  title: string;
  reason: string;
  route: string | null;
}

export interface WeeklyDigest {
  summary: string;
  at_risk: DigestItem[];
  stale: DigestItem[];
  conflicts: DigestItem[];
  wins: DigestItem[];
  focus_recommendation: string;
}

// ── Gmail ──────────────────────────────────────────────────────────────────────

export interface GmailStatus {
  connected: boolean;
  settings: GmailSyncSettings | null;
}

export interface GmailThread {
  thread_id: string;
  subject: string;
  snippet: string;
  sender: string;
  sender_email: string;
  date_ts: number;
  message_count: number;
}

export interface GmailSyncSettings {
  sync_enabled: boolean;
  sync_interval_hours: number;
  notification_poll_minutes: number;
  max_threads_per_sync: number;
  include_sent: boolean;
  include_drafts: boolean;
  notify_new_mail: boolean;
  backfill_enabled: boolean;
  relevance_filter_enabled: boolean;
  auto_analyze_enabled: boolean;
  auto_analyze_limit: number;
  backfill_page_token: string | null;
  backfill_query: string | null;
  last_backfill_at: string | null;
  backfill_completed_at: string | null;
  account_email: string | null;
  last_sync_started_at: string | null;
  last_sync_completed_at: string | null;
  last_history_id: string | null;
  last_error: string | null;
}

export interface GmailSyncSettingsInput {
  sync_enabled?: boolean;
  sync_interval_hours?: number;
  notification_poll_minutes?: number;
  max_threads_per_sync?: number;
  include_sent?: boolean;
  include_drafts?: boolean;
  notify_new_mail?: boolean;
  backfill_enabled?: boolean;
  relevance_filter_enabled?: boolean;
  auto_analyze_enabled?: boolean;
  auto_analyze_limit?: number;
}

export interface GmailSyncReport {
  synced_threads: number;
  synced_messages: number;
  backfilled_threads: number;
  backfill_complete: boolean;
  skipped_spam_threads: number;
  skipped_irrelevant_threads: number;
  purged_threads: number;
  ai_analyzed_threads: number;
  auto_linked_threads: number;
  analysis_refreshed_threads: number;
  analysis_failed_threads: number;
  orphan_threads: number;
  new_messages: number;
  new_threads: number;
  synced_labels: number;
  synced_drafts: number;
  started_at: string;
  completed_at: string;
  account_email: string | null;
}

export interface EmailAddress {
  name: string;
  email: string;
}

export interface GmailLabelRecord {
  gmail_label_id: string;
  name: string;
  label_type: string;
  color: string | null;
}

export interface LinkedDeliverableRef {
  id: string;
  title: string;
  state: string;
  linked_at: string;
  source: string;
  confidence: number | null;
  rationale: string;
}

export interface LinkedInitiativeRef {
  id: string;
  title: string;
  status: string;
  linked_at: string;
  source: string;
  confidence: number | null;
  rationale: string;
}

export interface LinkedStakeholderRef {
  id: string;
  name: string;
  email: string;
  role: string;
  linked_at: string;
  source: string;
  confidence: number | null;
  rationale: string;
}

export interface GmailAutoLinkReport {
  thread_id: string;
  linked_stakeholders: number;
  linked_deliverables: number;
  linked_initiatives: number;
  suggestions_created: number;
  orphan: boolean;
}

export interface GmailThreadLinkSuggestion {
  id: string;
  thread_id: string;
  target_kind: "stakeholder" | "deliverable" | "initiative";
  target_id: string;
  target_title: string;
  confidence: number;
  rationale: string;
  status: "pending" | "accepted" | "rejected";
  created_at: string;
  updated_at: string;
}

export interface GmailLocalThread {
  thread_id: string;
  subject: string;
  snippet: string;
  participants: EmailAddress[];
  first_message_at: number | null;
  last_message_at: number | null;
  message_count: number;
  has_unread: boolean;
  gmail_read_state: "unread" | "read";
  is_sent_only: boolean;
  last_from_name: string;
  last_from_email: string;
  ai_title: string | null;
  summary: string | null;
  sentiment: string | null;
  urgency: string | null;
  ai_category: GmailAiCategory;
  ai_priority: GmailAiPriority;
  ai_category_confidence: number | null;
  ai_category_reasons: string[];
  ai_triaged_at: string | null;
  labels: GmailLabelRecord[];
  linked_deliverables: LinkedDeliverableRef[];
  linked_initiatives: LinkedInitiativeRef[];
  linked_stakeholders: LinkedStakeholderRef[];
  artifact_urls: string[];
  effective_priority: GmailAiPriority;
  priority_reasons: string[];
  graph_context: Record<string, unknown>;
  last_analyzed_message_at: number | null;
  last_sync_at: string;
  intent: GmailIntent | null;
  action_required: boolean;
  predicted_action: GmailPredictedAction | null;
  thread_state: GmailThreadState | null;
  dimensions_confidence: Record<string, number>;
  work_relevance: WorkMailRelevance;
  work_relevance_reasons: string[];
  work_relevance_confidence: number | null;
  attention_state: WorkMailAttentionState;
  attention_reasons: string[];
  attention_confidence: number | null;
  message_type: WorkMailMessageType;
  message_type_reasons: string[];
  message_type_confidence: number | null;
  work_mail_updated_at: string | null;
  trace_seen_at: string | null;
  trace_review_state: WorkMailReviewState;
  seen_through_message_id: string | null;
  seen_through_message_at: number | null;
  reviewed_through_message_id: string | null;
  reviewed_through_message_at: number | null;
  deferred_until: string | null;
  new_since_review: boolean;
  needs_me_reason: string | null;
  bundle_id: string | null;
  bundle_size: number;
  last_analysis_error: string | null;
}

export interface GmailMessageRecord {
  message_id: string;
  thread_id: string;
  history_id: string | null;
  subject: string;
  snippet: string;
  from_name: string;
  from_email: string;
  to: EmailAddress[];
  cc: EmailAddress[];
  bcc: EmailAddress[];
  date_ts: number | null;
  internal_date_ts: number | null;
  plain_body: string;
  html_body: string;
  label_ids: string[];
  is_sent: boolean;
  is_draft: boolean;
  is_unread: boolean;
  size_estimate: number | null;
  artifact_urls: string[];
  synced_at: string;
}

export interface GmailAttachmentRecord {
  id: string;
  message_id: string;
  thread_id: string;
  attachment_id: string | null;
  filename: string;
  mime_type: string;
  size: number | null;
  shared_by_email: string;
  shared_with: EmailAddress[];
  created_at: string;
}

export interface GmailLinkRecord {
  id: string;
  thread_id: string;
  message_id: string | null;
  url: string;
  kind: string;
  title: string | null;
  created_at: string;
}

export interface GmailThreadDetail {
  thread: GmailLocalThread;
  messages: GmailMessageRecord[];
  attachments: GmailAttachmentRecord[];
  links: GmailLinkRecord[];
}

export interface GmailDraftRecord {
  draft_id: string;
  message_id: string;
  thread_id: string | null;
  subject: string;
  to: EmailAddress[];
  cc: EmailAddress[];
  bcc: EmailAddress[];
  body_preview: string;
  updated_at: string | null;
  synced_at: string;
}

export interface GmailThreadFilter {
  query?: string;
  label_id?: string;
  category?: GmailAiCategory;
  stakeholder_id?: string;
  deliverable_id?: string;
  initiative_id?: string;
  orphan_only?: boolean;
  limit?: number;
}

export type WorkMailRelevance =
  | "work"
  | "linked_external"
  | "promoted"
  | "excluded"
  | "non_work"
  | "unknown";

export type WorkMailAttentionState =
  | "needs_me"
  | "waiting"
  | "review"
  | "fyi"
  | "scheduled"
  | "resolved";

export type WorkMailMessageType =
  | "conversation"
  | "file_share"
  | "meeting"
  | "announcement"
  | "notification"
  | "newsletter"
  | "promotion"
  | "receipt"
  | "system"
  | "other";

export type WorkMailViewId =
  | "all_work"
  | "needs_me"
  | "projects"
  | "deliverables"
  | "stakeholders"
  | "files"
  | "meetings"
  | "unlinked"
  | "excluded"
  | "agent_activity";

export type WorkMailReviewState =
  | "unreviewed"
  | "reviewed"
  | "deferred"
  | "waiting"
  | "resolved"
  | "replied";

export interface WorkMailQuery {
  view?: WorkMailViewId;
  query?: string;
  sender_domain?: string;
  work_relevance?: WorkMailRelevance;
  attention_state?: WorkMailAttentionState;
  message_type?: WorkMailMessageType;
  stakeholder_id?: string;
  deliverable_id?: string;
  initiative_id?: string;
  has_artifact?: boolean;
  unread_only?: boolean;
  gmail_unread?: boolean;
  trace_unseen?: boolean;
  seen_unreviewed?: boolean;
  review_state?: WorkMailReviewState;
  limit?: number;
}

export type WorkMailThread = GmailLocalThread;

export interface WorkMailViewCount {
  view: WorkMailViewId;
  count: number;
}

export interface WorkMailViewCounts {
  counts: WorkMailViewCount[];
}

export interface WorkMailBrief {
  needs_you: number;
  handled_by_trace: number;
  unread_work_mail: number;
  unseen_in_trace: number;
  seen_unreviewed: number;
  action_review_queue: number;
  waiting: number;
  deferred: number;
  handled: number;
  unlinked: number;
  excluded: number;
  uncertain_links: number;
  scope_domains: string[];
}

export interface WorkMailDomain {
  domain: string;
  enabled: boolean;
  source: string;
  note: string | null;
  created_at: number;
  updated_at: number;
}

export interface UpsertWorkMailDomainInput {
  domain: string;
  enabled?: boolean;
  note?: string | null;
}

export interface WorkMailAgentEvent {
  id: string;
  thread_id: string | null;
  event_kind: string;
  actor: string;
  summary: string;
  reason: unknown;
  payload: Record<string, unknown>;
  undo_payload: Record<string, unknown> | null;
  created_at: string;
}

export interface WorkMailSeenCheckpoint {
  message_id: string | null;
  message_at: number | null;
  seen_at: string | null;
}

export interface WorkMailReviewSummary {
  thread_id: string;
  review_state: WorkMailReviewState;
  trace_seen_at: string | null;
  seen: WorkMailSeenCheckpoint;
  reviewed_through_message_id: string | null;
  reviewed_through_message_at: number | null;
  handled_at: string | null;
  deferred_until: string | null;
  reopened_at: string | null;
  updated_at: string | null;
  new_since_review: boolean;
}

export interface WorkMailReviewUpdate {
  state: WorkMailReviewState;
  deferred_until?: string | null;
}

export type GmailAiCategory =
  | "personal"
  | "work"
  | "action_required"
  | "newsletter"
  | "receipt"
  | "meeting"
  | "archive"
  | "spam"
  | "other";

export type GmailAiPriority = "low" | "medium" | "high" | "urgent";

export interface GmailCategoryCount {
  category: GmailAiCategory;
  count: number;
}

export interface GmailStakeholderHealth {
  stakeholder_id: string;
  email: string;
  days_since_last_email: number | null;
  sent_count: number;
  received_count: number;
  thread_count: number;
  response_rate: number;
  health_score: number;
}

export interface GmailStakeholderSuggestion {
  email: string;
  name: string;
  thread_count: number;
  sent_count: number;
  received_count: number;
  last_seen_at: string;
}

export interface GmailRelationshipEdge {
  left_email: string;
  left_name: string;
  right_email: string;
  right_name: string;
  thread_count: number;
  last_seen_at: string;
}

export interface GmailAiCandidate {
  title: string;
  body: string;
  kind: string;
  due_date: string | null;
  artifact_url: string | null;
  confidence: number | null;
}

export interface GmailAiResult {
  summary: string;
  sentiment: string;
  urgency: string;
  category: GmailAiCategory;
  priority: GmailAiPriority;
  confidence: number | null;
  reasons: string[];
  tasks: GmailAiCandidate[];
  deliverables: GmailAiCandidate[];
  initiatives: GmailAiCandidate[];
  deadlines: GmailAiCandidate[];
  reply: string | null;
}

export interface GmailTriagePerson {
  name: string;
  email: string;
  reason: string;
  confidence: number | null;
}

export interface GmailTriageResult {
  category: string;
  priority: string;
  confidence: number | null;
  reasons: string[];
  suggested_actions: string[];
  stakeholder_candidates: GmailTriagePerson[];
}

export interface GmailWeeklyDigest {
  summary: string;
  waiting_for_response: GmailLocalThread[];
  overdue_followups: GmailLocalThread[];
  urgent_threads: GmailLocalThread[];
  draft_count: number;
}

export interface GmailSendInput {
  to: string[];
  cc?: string[];
  bcc?: string[];
  subject: string;
  body: string;
  /** Optional HTML body. When present, message becomes multipart/alternative. */
  body_html?: string | null;
  /** Local draft id whose attachments should be folded into the send. */
  draft_id?: string | null;
  /** Direct file paths to attach (in addition to draft attachments). */
  attachment_paths?: string[];
  thread_id?: string | null;
}

export interface LocalEmailDraftAttachment {
  id: string;
  draft_id: string;
  filename: string;
  mime_type: string;
  file_size: number;
  file_path: string;
  created_at: string;
}

export interface LocalEmailDraft {
  id: string;
  thread_id: string | null;
  to: string[];
  cc: string[];
  bcc: string[];
  subject: string;
  body_html: string;
  body_text: string;
  gmail_draft_id: string | null;
  created_at: string;
  updated_at: string;
  attachments: LocalEmailDraftAttachment[];
}

export interface GmailAnalysisSnapshot {
  id: string;
  thread_id: string;
  analyzed_at: string;
  /** "manual" | "auto_new_mail" | "auto_initial" */
  trigger: string;
  result: GmailAiResult;
  category: string | null;
  priority: string | null;
  summary: string | null;
  message_count_at_analysis: number;
}

export interface SaveLocalDraftInput {
  id?: string | null;
  thread_id?: string | null;
  to?: string[];
  cc?: string[];
  bcc?: string[];
  subject?: string;
  body_html?: string;
  body_text?: string;
}

export interface GmailSendResult {
  message_id: string;
  thread_id: string | null;
}
