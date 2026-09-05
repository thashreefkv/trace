import { invoke } from "@tauri-apps/api/core";
import { toast, formatIpcError } from "./toast";
import type {
  ApplyFlaggedToBacklogInput,
  ApplyGeneratedTasksInput,
  ApproveWorkIntakeInput,
  ApplyMeetingActionInput,
  BrainContextResult,
  BrainCypherInput,
  BrainCypherResult,
  BrainBrief,
  BrainFeedbackInput,
  BrainGraphFilters,
  BrainLearningEvent,
  BrainLearningEventInput,
  BrainLearningSnapshot,
  BrainRetrieveInput,
  BrainStatus,
  BrainTemplateInput,
  BrainTemplateResult,
  AskSearchResult,
  AgentReviewItem,
  ClaimReviewItem,
  CommunityReport,
  GraphCommunitySummary,
  SavedBrainView,
  SaveBrainViewInput,
  Capture,
  StakeholderBriefing,
  StakeholderDetail,
  UpdateStakeholderInput,
  CaptureFilters,
  CommitConversationIngestInput,
  Conversation,
  ConversationExtractionInput,
  ConversationExtractionResult,
  ConversationIngestResult,
  CreateInitiativeInput,
  CreateReportRunInput,
  CreateCaptureInput,
  CreateDeliverableInput,
  CreateLabelInput,
  CreateMeetingInput,
  CreateNoteInput,
  CreateSectionInput,
  CreateStakeholderInput,
  CreateTaskInput,
  Deliverable,
  DeliverableFilters,
  DeliverableNote,
  DeliverablePriority,
  DeliverableState,
  DeliverableTask,
  ExportResult,
  ReportChannelEvent,
  ReportExport,
  ReportRun,
  ReportScopePreview,
  ReportSectionPlan,
  ReportStep,
  ReasoningIndexSummary,
  ReasoningQueryInput,
  ReasoningResult,
  ReasoningRun,
  GeneratedTasks,
  GeminiKeyStatus,
  GeminiTestResult,
  GeneratedWeekPlan,
  Initiative,
  InitiativeGantt,
  InitiativeNote,
  InitiativeSection,
  Label,
  Meeting,
  MeetingAction,
  MeetingConfig,
  MeetingWithActions,
  MenuBarState,
  McpConfigSnippet,
  McpInstallState,
  CreateMemoryInput,
  ListMemoryEventsFilters,
  ListMemoryFilters,
  MemoryConsolidationResult,
  MemoryEvent,
  MemoryExtractionResult,
  MemoryFeedbackInput,
  MemoryRecord,
  MemoryRetrievalResult,
  MemorySettings,
  RetrieveMemoryInput,
  ProcessMeetingInput,
  SearchResult,
  SetDayInput,
  Stakeholder,
  UpdateDeliverableInput,
  UpdateDeliverableMetadataInput,
  UpdateGanttDatesInput,
  UpdateInitiativeInput,
  UpdateMemoryInput,
  UpdateMemorySettingsInput,
  UpdateSectionInput,
  UpdateTaskInput,
  WeekView,
  WeeklyDigest,
  WorkGraph,
  WorkGraphFilters,
  GmailStatus,
  GmailAutoLinkReport,
  GmailCategoryCount,
  GmailThread,
  GmailAiResult,
  GmailDraftRecord,
  GmailLabelRecord,
  GmailLocalThread,
  GmailRelationshipEdge,
  GmailAnalysisSnapshot,
  GmailSendInput,
  GmailSendResult,
  LocalEmailDraft,
  LocalEmailDraftAttachment,
  SaveLocalDraftInput,
  GmailStakeholderHealth,
  GmailStakeholderSuggestion,
  GmailSyncReport,
  GmailSyncSettings,
  GmailSyncSettingsInput,
  GmailThreadDetail,
  GmailThreadFilter,
  GmailThreadLinkSuggestion,
  GmailTriageResult,
  GmailWeeklyDigest,
  ReorderDirectionInput,
  WorkIntakeApplyResult,
  WorkIntakeFilters,
  WorkIntakeSuggestion,
  GCalStatus,
  UserProfile,
  UpdateUserProfileInput,
  PromoteCaptureToTaskInput,
  ToolCallLogFilter,
  ToolCallLogSnapshot,
  GeminiUsageSummary,
  EvalFixture,
  CreateEvalFixtureInput,
  EvalRun,
  EvalSummary,
  EmbeddingProgress,
  EffectiveClassification,
  UserClassification,
  SetOverrideInput,
  SenderRule,
  CreateSenderRuleInput,
  GmailInboxDashboard,
  CalibrationReport,
  WorkMailAgentEvent,
  WorkMailBrief,
  WorkMailDomain,
  WorkMailQuery,
  WorkMailReviewSummary,
  WorkMailReviewUpdate,
  WorkMailThread,
  WorkMailViewCounts,
  UpsertWorkMailDomainInput,
  AppConfig,
  BudgetStatus,
  DailyTrend,
  BrainLearningSummary,
  BrainInferenceFilter,
  BrainInferenceListResult,
  ReviewInferenceResult,
  SupersessionRecord,
  RLDigest,
  TemplateDetail,
  BrainInferenceStatus,
  ImportEvalFixturesInput,
  ImportEvalFixturesResult,
  CapturePromotionSuggestion,
  AppliedPromotion,
  PromotionAccuracySummary,
} from "./types";

export interface InvokeOptions {
  /** Suppress the default error toast for this call. Use when the caller surfaces the error inline. */
  silent?: boolean;
  /** Retry on failure with exponential backoff. `attempts` includes the initial try. */
  retry?: { attempts: number; backoffMs?: number };
}

async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
  opts?: InvokeOptions,
): Promise<T> {
  if (!isTauriRuntime()) {
    return Promise.reject(
      "Trace commands are available in the macOS app. Open the Tauri window for live data.",
    );
  }

  const attempts = Math.max(1, opts?.retry?.attempts ?? 1);
  const backoffMs = opts?.retry?.backoffMs ?? 250;

  let lastError: unknown;
  for (let i = 0; i < attempts; i++) {
    try {
      return await invoke<T>(command, args);
    } catch (e) {
      lastError = e;
      if (i < attempts - 1) {
        await new Promise((resolve) => setTimeout(resolve, backoffMs * (i + 1)));
      }
    }
  }

  if (!opts?.silent) {
    const id = `ipc:${command}`;
    const count = (errorCounts.get(id) ?? 0) + 1;
    errorCounts.set(id, count);
    const base = formatIpcError(command, lastError);
    const message = count > 1 ? `${base} (×${count})` : base;
    toast.error(message, {
      id,
      duration: 6000,
    });
    if (countResetTimers.has(id)) {
      clearTimeout(countResetTimers.get(id));
    }
    countResetTimers.set(
      id,
      window.setTimeout(() => {
        errorCounts.delete(id);
        countResetTimers.delete(id);
      }, 10_000),
    );
  }
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw lastError;
}

const errorCounts = new Map<string, number>();
const countResetTimers = new Map<string, number>();

function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

export function listInitiatives() {
  return invokeCommand<Initiative[]>("list_initiatives");
}

export function getInitiative(id: string) {
  return invokeCommand<Initiative>("get_initiative", { id });
}

export function createInitiative(input: CreateInitiativeInput) {
  return invokeCommand<Initiative>("create_initiative", { input });
}

export function updateInitiative(id: string, input: UpdateInitiativeInput) {
  return invokeCommand<Initiative>("update_initiative", { id, input });
}

export function deleteInitiative(id: string) {
  return invokeCommand<void>("delete_initiative", { id });
}

export function listStakeholders() {
  return invokeCommand<Stakeholder[]>("list_stakeholders");
}

export function listStakeholderDetails() {
  return invokeCommand<StakeholderDetail[]>("list_stakeholder_details");
}

export function getStakeholderDetail(id: string) {
  return invokeCommand<StakeholderDetail>("get_stakeholder_detail", { id });
}

export function createStakeholder(input: CreateStakeholderInput) {
  return invokeCommand<Stakeholder>("create_stakeholder", { input });
}

export function updateStakeholder(id: string, input: UpdateStakeholderInput) {
  return invokeCommand<Stakeholder>("update_stakeholder", { id, input });
}

export function deleteStakeholder(id: string) {
  return invokeCommand<void>("delete_stakeholder", { id });
}

export function generateStakeholderBriefing(stakeholderId: string) {
  return invokeCommand<StakeholderBriefing>("generate_stakeholder_briefing", {
    stakeholderId,
  });
}

export function commentOnBriefingItem(
  stakeholderId: string,
  section: string,
  itemText: string,
  itemSource: string | undefined,
  comment: string,
) {
  return invokeCommand<string>("comment_on_briefing_item", {
    stakeholderId,
    section,
    itemText,
    itemSource,
    comment,
  });
}

export function createCapture(input: CreateCaptureInput) {
  return invokeCommand<Capture>("create_capture", { input });
}

export function listCaptures(filters?: CaptureFilters) {
  return invokeCommand<Capture[]>("list_captures", { filters: filters ?? null });
}

/** Ensure "Trace" and "Trace — Captured" folders exist in Apple Notes.
 *  Returns [inboxName, capturedName]. */
export function appleNotesSetup() {
  return invokeCommand<[string, string]>("apple_notes_setup");
}

/** Run an immediate sync of the Apple Notes "Trace" folder.
 *  Returns the number of new captures created. */
export function appleNotesSyncNow() {
  return invokeCommand<number>("apple_notes_sync_now");
}

export function getCapture(id: string) {
  return invokeCommand<Capture>("get_capture", { id });
}

export function dismissCapture(id: string) {
  return invokeCommand<Capture>("dismiss_capture", { id });
}

export function suggestCapture(id: string) {
  return invokeCommand<Capture>("suggest_capture", { id });
}

export function restoreCaptureToInbox(id: string) {
  return invokeCommand<Capture>("restore_capture_to_inbox", { id });
}

export function promoteCaptureToDeliverable(id: string, input: CreateDeliverableInput) {
  return invokeCommand<Deliverable>("promote_capture_to_deliverable", { captureId: id, input });
}

export function promoteCaptureToInitiative(id: string, input: CreateInitiativeInput) {
  return invokeCommand<Initiative>("promote_capture_to_initiative", { captureId: id, input });
}

export function promoteCaptureToTask(id: string, input: PromoteCaptureToTaskInput) {
  return invokeCommand<DeliverableTask>("promote_capture_to_task", {
    captureId: id,
    deliverableId: input.deliverable_id,
    title: input.title,
    notes: input.notes ?? null,
    dueDate: input.due_date ?? null,
  });
}

export function suggestCapturePromotion(captureId: string) {
  return invokeCommand<CapturePromotionSuggestion>(
    "suggest_capture_promotion",
    { captureId },
    { silent: true },
  );
}

export function getCapturePromotionSuggestion(captureId: string) {
  return invokeCommand<CapturePromotionSuggestion | null>(
    "get_capture_promotion_suggestion",
    { captureId },
    { silent: true },
  );
}

export function getCapturePromotionAccuracy() {
  return invokeCommand<PromotionAccuracySummary>(
    "get_capture_promotion_accuracy",
    undefined,
    { silent: true },
  );
}

export interface ApplyCapturePromotionInput {
  captureId: string;
  suggestionId: string;
  overrideKind?: "task" | "deliverable" | "initiative";
  overrideTargetId?: string | null;
  overrideAlternativeIndex?: number;
}

export function applyCapturePromotionSuggestion(input: ApplyCapturePromotionInput) {
  return invokeCommand<AppliedPromotion>("apply_capture_promotion_suggestion", {
    captureId: input.captureId,
    suggestionId: input.suggestionId,
    overrideKind: input.overrideKind ?? null,
    overrideTargetId: input.overrideTargetId ?? null,
    overrideAlternativeIndex:
      input.overrideAlternativeIndex === undefined ? null : input.overrideAlternativeIndex,
  });
}

export function undoCapturePromotion(suggestionId: string) {
  return invokeCommand<Capture>("undo_capture_promotion", { suggestionId });
}

export function recordCapturePromotionEvent(suggestionId: string, eventType: string) {
  return invokeCommand<void>(
    "record_capture_promotion_event",
    { suggestionId, eventType },
    { silent: true },
  );
}

export function extractConversation(input: ConversationExtractionInput) {
  return invokeCommand<ConversationExtractionResult>("extract_conversation", { input });
}

export function commitConversationIngest(input: CommitConversationIngestInput) {
  return invokeCommand<ConversationIngestResult>("commit_conversation_ingest", { input });
}

export function promoteClaudeCaptureToIngest(
  captureId: string,
  input: CommitConversationIngestInput,
) {
  return invokeCommand<ConversationIngestResult>("promote_claude_capture_to_ingest", {
    captureId,
    input,
  });
}

export function getConversation(id: string) {
  return invokeCommand<Conversation>("get_conversation", { id });
}

export function listConversations() {
  return invokeCommand<Conversation[]>("list_conversations");
}

export function listDeliverables(filters?: DeliverableFilters) {
  return invokeCommand<Deliverable[]>("list_deliverables", { filters: filters ?? null });
}

export function getDeliverable(id: string) {
  return invokeCommand<Deliverable>("get_deliverable", { id });
}

export function createDeliverable(input: CreateDeliverableInput) {
  return invokeCommand<Deliverable>("create_deliverable", { input });
}

export function updateDeliverable(id: string, input: UpdateDeliverableInput) {
  return invokeCommand<Deliverable>("update_deliverable", { id, input });
}

export function deleteDeliverable(id: string) {
  return invokeCommand<void>("delete_deliverable", { id });
}

export function updateDeliverableState(id: string, state: DeliverableState) {
  return invokeCommand<Deliverable>("update_deliverable_state", { id, stateValue: state });
}

export function searchDeliverables(query: string, filters?: DeliverableFilters) {
  return invokeCommand<Deliverable[]>("search_deliverables", {
    query,
    filters: filters ?? null,
  });
}

export function listDeliverablesForInitiative(initiativeId: string) {
  return invokeCommand<Deliverable[]>("list_deliverables_for_initiative", { initiativeId });
}

export function getWorkContextGraph(filters?: WorkGraphFilters) {
  return invokeCommand<WorkGraph>("get_work_context_graph", { filters: filters ?? null });
}

export function getBrainStatus() {
  return invokeCommand<BrainStatus>("get_brain_status");
}

export function rebuildBrain() {
  return invokeCommand<BrainStatus>("rebuild_brain");
}

export function listSavedBrainViews() {
  return invokeCommand<SavedBrainView[]>("list_saved_brain_views");
}

export function saveBrainView(input: SaveBrainViewInput) {
  return invokeCommand<SavedBrainView>("save_brain_view", { input });
}

export function deleteBrainView(id: string) {
  return invokeCommand<void>("delete_brain_view", { id });
}

export function listGraphCommunities(level?: number | null) {
  return invokeCommand<GraphCommunitySummary[]>("list_graph_communities", {
    level: level ?? null,
  });
}

export function getBrainGraph(filters?: BrainGraphFilters) {
  return invokeCommand<WorkGraph>("get_brain_graph", { filters: filters ?? null });
}

export interface BrainLayoutPoint {
  entity_kind: string;
  entity_id: string;
  x: number;
  y: number;
  z?: number | null;
}

export interface BrainLayoutResult {
  mode: string;
  graph_version: string;
  computed_at: number;
  points: BrainLayoutPoint[];
}

export interface NodeEmbeddingId {
  entity_kind: string;
  entity_id: string;
}

export interface NodeEmbedding {
  entity_kind: string;
  entity_id: string;
  vector: number[];
}

export function getBrainLayout(mode: string, graphVersion: string) {
  return invokeCommand<BrainLayoutResult | null>("get_brain_layout", {
    mode,
    graphVersion,
  });
}

export function writeBrainLayout(input: {
  mode: string;
  graph_version: string;
  points: BrainLayoutPoint[];
}) {
  return invokeCommand<void>("write_brain_layout", { input });
}

export function invalidateBrainLayouts() {
  return invokeCommand<void>("invalidate_brain_layouts");
}

export function getNodeEmbeddings(ids: NodeEmbeddingId[]) {
  return invokeCommand<NodeEmbedding[]>("get_node_embeddings", { input: { ids } });
}

export function retrieveBrainContext(input: BrainRetrieveInput) {
  return invokeCommand<BrainContextResult>("retrieve_brain_context", { input });
}

export function queryBrainCypher(input: BrainCypherInput) {
  return invokeCommand<BrainCypherResult>("query_brain_cypher", { input });
}

export function runBrainTemplate(input: BrainTemplateInput) {
  return invokeCommand<BrainTemplateResult>("run_brain_template", { input });
}

export function getDailyBrainBrief() {
  return invokeCommand<BrainBrief>("get_daily_brain_brief");
}

export function recordBrainFeedback(input: BrainFeedbackInput) {
  return invokeCommand<void>("record_brain_feedback", { input });
}

export function recordBrainLearningEvent(input: BrainLearningEventInput) {
  return invokeCommand<BrainLearningEvent>("record_brain_learning_event", { input });
}

export function getBrainLearningSnapshot(template?: string | null, limit?: number | null) {
  return invokeCommand<BrainLearningSnapshot>("get_brain_learning_snapshot", {
    template: template ?? null,
    limit: limit ?? null,
  });
}

export function getMemorySettings() {
  return invokeCommand<MemorySettings>("get_memory_settings");
}

export function updateMemorySettings(input: UpdateMemorySettingsInput) {
  return invokeCommand<MemorySettings>("update_memory_settings", { input });
}

export function listMemories(filters?: ListMemoryFilters) {
  return invokeCommand<MemoryRecord[]>("list_memories", { filters: filters ?? null });
}

export function createMemory(input: CreateMemoryInput) {
  return invokeCommand<MemoryRecord>("create_memory", { input });
}

export function updateMemory(id: string, input: UpdateMemoryInput) {
  return invokeCommand<MemoryRecord>("update_memory", { id, input });
}

export function deleteMemory(id: string) {
  return invokeCommand<void>("delete_memory", { id });
}

export function retrieveMemories(input: RetrieveMemoryInput) {
  return invokeCommand<MemoryRetrievalResult>("retrieve_memories", { input });
}

export function consolidateMemories() {
  return invokeCommand<MemoryConsolidationResult>("consolidate_memories");
}

export function extractMemoriesFromConversation(conversationId: string) {
  return invokeCommand<MemoryExtractionResult>("extract_memories_from_conversation", {
    input: { conversation_id: conversationId },
  });
}

export function recordMemoryFeedback(input: MemoryFeedbackInput) {
  return invokeCommand<void>("record_memory_feedback", { input });
}

export function listMemoryEvents(filters?: ListMemoryEventsFilters) {
  return invokeCommand<MemoryEvent[]>("list_memory_events", { filters: filters ?? null });
}

export function embedMemoriesNow() {
  return invokeCommand<number>("embed_memories_now");
}

export function exportMarkdown(destinationDir: string) {
  return invokeCommand<ExportResult>("export_markdown", { destinationDir });
}

export function showCaptureWindow() {
  return invokeCommand<void>("show_capture_window");
}

export function hideCaptureWindow() {
  return invokeCommand<void>("hide_capture_window");
}

export function showMainWindow(route?: string) {
  return invokeCommand<void>("show_main_window", { route: route ?? null });
}

export function showSpotlightWindow() {
  return invokeCommand<void>("show_spotlight_window");
}

export function hideSpotlightWindow() {
  return invokeCommand<void>("hide_spotlight_window");
}

export function toggleSpotlightWindow() {
  return invokeCommand<void>("toggle_spotlight_window_cmd");
}

export function getMenuBarState() {
  return invokeCommand<MenuBarState>("get_menu_bar_state");
}

export function refreshMenuBar() {
  return invokeCommand<void>("refresh_menu_bar");
}

export function getMcpInstallState() {
  return invokeCommand<McpInstallState>("get_mcp_install_state");
}

export function installMcpServer() {
  return invokeCommand<McpInstallState>("install_mcp_server");
}

export function getMcpConfigSnippet() {
  return invokeCommand<McpConfigSnippet>("get_mcp_config_snippet");
}

export function openMcpLog() {
  return invokeCommand<void>("open_mcp_log");
}

export function saveGeminiApiKey(key: string) {
  return invokeCommand<GeminiKeyStatus>("save_gemini_api_key", { key });
}

export function clearGeminiApiKey() {
  return invokeCommand<GeminiKeyStatus>("clear_gemini_api_key");
}

export function testGeminiApiKey() {
  return invokeCommand<GeminiTestResult>("test_gemini_api_key");
}

export function getGeminiKeyStatus() {
  return invokeCommand<GeminiKeyStatus>("get_gemini_key_status");
}

// ---------------------------------------------------------------------------
// Siri / Apple Shortcuts remote API
//
// Backed by `src-tauri/src/commands/siri_api.rs`. The server itself runs
// inside the Tauri process; these commands let Settings show status and
// hot-swap the bearer token.

export interface SiriApiStatus {
  running: boolean;
  port: number;
  tailscale_ipv4: string | null;
  tailscale_url: string | null;
  localhost_url: string;
  token_preview: string;
}

export function getSiriApiStatus() {
  return invokeCommand<SiriApiStatus>("get_siri_api_status");
}

export function getSiriToken() {
  return invokeCommand<string>("get_siri_token");
}

export function regenerateSiriToken() {
  return invokeCommand<string>("regenerate_siri_token");
}

export function listToolCallLog(filter?: ToolCallLogFilter) {
  return invokeCommand<ToolCallLogSnapshot>("list_tool_call_log", {
    filter: filter ?? null,
  });
}

export function getGeminiUsageSummary(hours?: number) {
  return invokeCommand<GeminiUsageSummary>("get_gemini_usage_summary", {
    hours: hours ?? null,
  });
}

export function getGeminiDailyTrend(days: number) {
  return invokeCommand<DailyTrend>("get_gemini_daily_trend", { days });
}

export function getAppConfig() {
  return invokeCommand<AppConfig>("get_app_config");
}

export function setAppConfig(config: AppConfig) {
  return invokeCommand<void>("set_app_config", { config });
}

export function getBudgetStatus() {
  return invokeCommand<BudgetStatus>("get_budget_status");
}

export function getBrainLearningSummary() {
  return invokeCommand<BrainLearningSummary>("get_brain_learning_summary");
}

// ── Section 6.2 — RL feedback surface ────────────────────────────────────

export function listBrainInferences(filter?: BrainInferenceFilter) {
  return invokeCommand<BrainInferenceListResult>("list_brain_inferences", {
    filter: filter ?? null,
  });
}

export function reviewInference(
  inferenceId: string,
  decision: Exclude<BrainInferenceStatus, "pending">,
) {
  return invokeCommand<ReviewInferenceResult>("review_inference", {
    inferenceId,
    decision,
  });
}

export function listInferenceSupersessions(limit?: number) {
  return invokeCommand<SupersessionRecord[]>("list_inference_supersessions", {
    limit: limit ?? null,
  });
}

export function revertInferenceSupersession(loserInferenceId: string) {
  return invokeCommand<void>("revert_inference_supersession", {
    loserInferenceId,
  });
}

export function getRLDigest(days?: number) {
  return invokeCommand<RLDigest>("get_rl_digest", { days: days ?? null });
}

export function getTemplateDetail(template: string) {
  return invokeCommand<TemplateDetail>("get_template_detail", { template });
}

export function resetBrainTemplateLearning(template: string) {
  return invokeCommand<void>("reset_brain_template_learning", { template });
}

export function listEvalFixtures() {
  return invokeCommand<EvalFixture[]>("list_eval_fixtures");
}

export function createEvalFixture(input: CreateEvalFixtureInput) {
  return invokeCommand<EvalFixture>("create_eval_fixture", { input });
}

export function deleteEvalFixture(id: string) {
  return invokeCommand<void>("delete_eval_fixture", { id });
}

export function runEvalFixture(fixtureId: string) {
  return invokeCommand<EvalRun>("run_eval_fixture", { fixtureId });
}

export function runAllEvals() {
  return invokeCommand<EvalRun[]>("run_all_evals");
}

export function listEvalRuns(fixtureId?: string, limit?: number) {
  return invokeCommand<EvalRun[]>("list_eval_runs", {
    fixtureId: fixtureId ?? null,
    limit: limit ?? null,
  });
}

export function setEvalBaseline(fixtureId: string, runId: string) {
  return invokeCommand<void>("set_eval_baseline", { fixtureId, runId });
}

export function getEvalSummary() {
  return invokeCommand<EvalSummary>("get_eval_summary");
}

export function importEvalFixtures(input: ImportEvalFixturesInput) {
  return invokeCommand<ImportEvalFixturesResult>("import_eval_fixtures", { input });
}

export function getEmbeddingProgress() {
  return invokeCommand<EmbeddingProgress>("get_embedding_progress");
}

export function enqueueAllEmbeddings() {
  return invokeCommand<number>("enqueue_all_embeddings");
}

export function embedNow(batchSize?: number) {
  return invokeCommand<number>("embed_now", {
    batchSize: batchSize ?? null,
  });
}

export function listDeliverableTasks(deliverableId: string) {
  return invokeCommand<DeliverableTask[]>("list_deliverable_tasks", { deliverableId });
}

export function createDeliverableTask(input: CreateTaskInput) {
  return invokeCommand<DeliverableTask>("create_deliverable_task", { input });
}

export function updateDeliverableTask(id: string, input: UpdateTaskInput) {
  return invokeCommand<DeliverableTask>("update_deliverable_task", { id, input });
}

export function deleteDeliverableTask(id: string) {
  return invokeCommand<void>("delete_deliverable_task", { id });
}

export function reorderDeliverableTask(input: import("./types").ReorderDirectionInput) {
  return invokeCommand<import("./types").DeliverableTask[]>("reorder_deliverable_task", { input });
}

export function listDeliverableNotes(deliverableId: string) {
  return invokeCommand<DeliverableNote[]>("list_deliverable_notes", { deliverableId });
}

export function createDeliverableNote(input: CreateNoteInput) {
  return invokeCommand<DeliverableNote>("create_deliverable_note", { input });
}

export function deleteDeliverableNote(id: string) {
  return invokeCommand<void>("delete_deliverable_note", { id });
}

export function updateDeliverableMetadata(id: string, input: UpdateDeliverableMetadataInput) {
  return invokeCommand<Deliverable>("update_deliverable_metadata", { id, input });
}

export function setDeliverableFocus(id: string, focused: boolean) {
  return invokeCommand<Deliverable>("set_deliverable_focus", { id, focused });
}

export function generateDeliverableTasks(deliverableId: string) {
  return invokeCommand<GeneratedTasks>("generate_deliverable_tasks", { deliverableId });
}

export function applyGeneratedDeliverableTasks(input: ApplyGeneratedTasksInput) {
  return invokeCommand<DeliverableTask[]>("apply_generated_deliverable_tasks", { input });
}

// ── Week view ──────────────────────────────────────────────────────────────────

export function getWeekView(weekStart: string) {
  return invokeCommand<WeekView>("get_week_view", { weekStart });
}

export function setDayDeliverable(input: SetDayInput) {
  return invokeCommand<WeekView>("set_day_deliverable", { input });
}

export function getMeetingConfig() {
  return invokeCommand<MeetingConfig>("get_meeting_config");
}

export function setMeetingDate(date: string | null) {
  return invokeCommand<MeetingConfig>("set_meeting_date", { date });
}

export function advanceMeetingDate() {
  return invokeCommand<MeetingConfig>("advance_meeting_date");
}

export function generateWeekPlan(weekStart: string) {
  return invokeCommand<GeneratedWeekPlan>("generate_week_plan", { weekStart });
}

// ── Meetings ──────────────────────────────────────────────────────────────────

export function listMeetings() {
  return invokeCommand<Meeting[]>("list_meetings");
}

export function getMeeting(id: string) {
  return invokeCommand<MeetingWithActions>("get_meeting", { id });
}

export function createMeeting(input: CreateMeetingInput) {
  return invokeCommand<Meeting>("create_meeting", { input });
}

export function updateMeetingTitle(id: string, title: string) {
  return invokeCommand<Meeting>("update_meeting_title", { id, title });
}

export function deleteMeeting(id: string) {
  return invokeCommand<void>("delete_meeting", { id });
}

export function updateMeetingStakeholders(id: string, stakeholderIds: string[]) {
  return invokeCommand<void>("update_meeting_stakeholders", { id, stakeholderIds });
}

export function processMeetingAudio(input: ProcessMeetingInput) {
  return invokeCommand<MeetingWithActions>("process_meeting_audio", { input });
}

export function transcribeVoiceInput(input: { audio_base64: string; mime_type: string }) {
  return invokeCommand<string>("transcribe_voice_input", { input });
}

export function applyMeetingAction(input: ApplyMeetingActionInput) {
  return invokeCommand<MeetingAction>("apply_meeting_action", { input });
}

export function dismissMeetingAction(actionId: string) {
  return invokeCommand<void>("dismiss_meeting_action", { actionId });
}

export function listInitiativeNotes(initiativeId: string) {
  return invokeCommand<InitiativeNote[]>("list_initiative_notes", { initiativeId });
}

export function createInitiativeNote(initiativeId: string, body: string) {
  return invokeCommand<InitiativeNote>("create_initiative_note", { initiativeId, body });
}

export function deleteInitiativeNote(id: string) {
  return invokeCommand<void>("delete_initiative_note", { id });
}

// ── Unified search ─────────────────────────────────────────────────────────────

export function searchAll(query: string) {
  return invokeCommand<SearchResult[]>("search_all", { query });
}

export function askSearch(question: string, context?: string) {
  return invokeCommand<AskSearchResult>("ask_search", { question, context: context ?? null });
}

export type AskAttachmentInput = {
  kind: "image";
  mime_type: string;
  data: string;
  filename?: string | null;
};

export function startAskRun(
  question: string,
  context?: string,
  attachments?: AskAttachmentInput[],
  mode?: string,
  permissionMode?: string,
  autoConfirmedTools?: string[],
  reasoningDepth?: "standard" | "deep",
) {
  return invokeCommand<string>("start_ask_run", {
    question,
    context: context ?? null,
    mode: mode ?? null,
    attachments: attachments && attachments.length > 0 ? attachments : null,
    permissionMode: permissionMode ?? null,
    autoConfirmedTools: autoConfirmedTools && autoConfirmedTools.length > 0 ? autoConfirmedTools : null,
    reasoningDepth: reasoningDepth ?? "standard",
  });
}

export function queryReasoningGraph(input: ReasoningQueryInput) {
  return invokeCommand<ReasoningResult>("query_reasoning_graph", { input });
}

export function refreshReasoningIndex() {
  return invokeCommand<ReasoningIndexSummary>("refresh_reasoning_index");
}

export function listReasoningRuns(limit?: number) {
  return invokeCommand<ReasoningRun[]>("list_reasoning_runs", { limit: limit ?? null });
}

export function listClaimReviewItems(status?: string) {
  return invokeCommand<ClaimReviewItem[]>("list_claim_review_items", { status: status ?? null });
}

export function reviewClaim(claimId: string, decision: "approved" | "rejected") {
  return invokeCommand<ClaimReviewItem>("review_claim", { claimId, decision });
}

export function listCommunityReports(status?: string) {
  return invokeCommand<CommunityReport[]>("list_community_reports", { status: status ?? null });
}

export function reviewCommunityReport(reportId: string, decision: "approved" | "rejected") {
  return invokeCommand<CommunityReport>("review_community_report", { reportId, decision });
}

export function listReasoningReviewItems(status?: string) {
  return invokeCommand<AgentReviewItem[]>("list_reasoning_review_items", { status: status ?? null });
}

export function reviewReasoningItem(reviewItemId: string, decision: "approved" | "rejected") {
  return invokeCommand<AgentReviewItem>("review_reasoning_item", { reviewItemId, decision });
}

export function listReportRuns() {
  return invokeCommand<ReportRun[]>("list_report_runs");
}

export function getReportRun(id: string) {
  return invokeCommand<ReportRun>("get_report_run", { id });
}

export function createReportRun(input: CreateReportRunInput) {
  return invokeCommand<ReportRun>("create_report_run", { input });
}

export function deleteReportRun(reportRunId: string) {
  return invokeCommand<void>("delete_report_run", { reportRunId });
}

export function generateReportOutline(reportRunId: string) {
  return invokeCommand<ReportRun>("generate_report_outline", { reportRunId });
}

export function approveReportOutline(reportRunId: string, outlineMarkdown: string) {
  return invokeCommand<ReportRun>("approve_report_outline", {
    input: { report_run_id: reportRunId, outline_markdown: outlineMarkdown },
  });
}

export function generateReportDraft(reportRunId: string) {
  return invokeCommand<ReportRun>("generate_report_draft", { reportRunId });
}

export function approveReport(reportRunId: string) {
  return invokeCommand<ReportRun>("approve_report", { reportRunId });
}

export function exportReport(
  reportRunId: string,
  format: "markdown" | "docx" | "pdf" | "google_docs",
  destinationDir?: string,
  externalWriteConfirmed = false,
) {
  return invokeCommand<ReportExport>("export_report", {
    input: {
      report_run_id: reportRunId,
      format,
      destination_dir: destinationDir ?? null,
      external_write_confirmed: externalWriteConfirmed,
    },
  });
}

export function listReportExports(reportRunId: string) {
  return invokeCommand<ReportExport[]>("list_report_exports", { reportRunId });
}

export function linkReportDeliverable(reportRunId: string, deliverableId: string) {
  return invokeCommand<ReportRun>("link_report_deliverable", {
    input: { report_run_id: reportRunId, deliverable_id: deliverableId },
  });
}

export function createReportDeliverable(reportRunId: string) {
  return invokeCommand<Deliverable>("create_report_deliverable", { reportRunId });
}

export function runReportPipeline(reportRunId: string) {
  return invokeCommand<void>("run_report_pipeline", { reportRunId });
}

export function getReportScopePreview(reportRunId: string) {
  return invokeCommand<ReportScopePreview>("get_report_scope_preview", { reportRunId });
}

export function replaceScopeExclusions(reportRunId: string, excludedEntryIds: string[]) {
  return invokeCommand<ReportRun>("replace_scope_exclusions", {
    input: { report_run_id: reportRunId, excluded_entry_ids: excludedEntryIds },
  });
}

export function updateReportSections(reportRunId: string, sections: ReportSectionPlan[]) {
  return invokeCommand<ReportRun>("update_report_sections", {
    input: { report_run_id: reportRunId, sections },
  });
}

export function listReportSteps(reportRunId: string) {
  return invokeCommand<ReportStep[]>("list_report_steps", { reportRunId });
}

export function answerReportClarification(
  stepId: string,
  selectedLabels: string[],
  freeText?: string,
) {
  return invokeCommand<void>("answer_report_clarification", {
    input: { step_id: stepId, selected_labels: selectedLabels, free_text: freeText ?? null },
  });
}

export function rerunReportStep(
  reportRunId: string,
  stepName: string,
  sectionId?: string,
  ignoreCache = false,
) {
  return invokeCommand<void>("rerun_report_step", {
    input: {
      report_run_id: reportRunId,
      step_name: stepName,
      section_id: sectionId ?? null,
      ignore_cache: ignoreCache,
    },
  });
}

export function steerReport(reportRunId: string, instruction: string) {
  return invokeCommand<void>("steer_report_cmd", {
    input: { report_run_id: reportRunId, instruction },
  });
}

export { type ReportChannelEvent };

export function cancelAskRun(runId: string) {
  return invokeCommand<boolean>("cancel_ask_run", { runId });
}

export function confirmAskToolDecision(runId: string, callId: string, approved: boolean) {
  return invokeCommand<boolean>("confirm_ask_tool_decision", {
    runId,
    callId,
    approved,
  });
}

export interface PromptInjectionFilter {
  source?: string;
  action?: string;
  only_with_flags?: boolean;
  run_id?: string;
  limit?: number;
}

export interface PromptInjectionEntry {
  id: string;
  ts: number;
  source: string;
  origin_kind: string | null;
  origin_id: string | null;
  run_id: string | null;
  call_id: string | null;
  tool: string | null;
  action_taken: string;
  reason: string;
  flags_json: string;
  content_excerpt: string;
  original_bytes: number;
  sanitized_bytes: number;
}

export interface PromptInjectionSnapshot {
  entries: PromptInjectionEntry[];
  total_24h: number;
  refusals_24h: number;
  flagged_24h: number;
}

export function listPromptInjectionLog(filter?: PromptInjectionFilter) {
  return invokeCommand<PromptInjectionSnapshot>(
    "list_prompt_injection_log",
    { filter: filter ?? null },
    { silent: true },
  );
}

export function listAskChats(filters?: import("./types").ListAskChatsFilters) {
  return invokeCommand<import("./types").AskChatRecord[]>("list_ask_chats", {
    filters: filters ?? null,
  });
}

export function getAskChat(chatId: string) {
  return invokeCommand<import("./types").AskChatDetail>("get_ask_chat", { chatId });
}

export function upsertAskChat(input: import("./types").UpsertAskChatInput) {
  return invokeCommand<import("./types").AskChatRecord>("upsert_ask_chat", { input });
}

export function deleteAskChat(chatId: string) {
  return invokeCommand<void>("delete_ask_chat", { chatId });
}

export function archiveAskChat(chatId: string) {
  return invokeCommand<void>("archive_ask_chat", { chatId });
}

export function appendAskTurn(input: import("./types").AppendAskTurnInput) {
  return invokeCommand<import("./types").AskTurnRecord>("append_ask_turn", { input });
}

export function searchAskChats(query: string, limit?: number) {
  return invokeCommand<import("./types").AskChatSearchHit[]>("search_ask_chats", {
    query,
    limit: limit ?? null,
  });
}

// ── Gantt ──────────────────────────────────────────────────────────────────────

export function getInitiativeGantt(initiativeId: string) {
  return invokeCommand<InitiativeGantt>("get_initiative_gantt", { initiativeId });
}

export function createInitiativeSection(input: CreateSectionInput) {
  return invokeCommand<InitiativeSection>("create_initiative_section", { input });
}

export function updateInitiativeSection(id: string, input: UpdateSectionInput) {
  return invokeCommand<InitiativeSection>("update_initiative_section", { id, input });
}

export function deleteInitiativeSection(id: string) {
  return invokeCommand<void>("delete_initiative_section", { id });
}

export function updateDeliverableGanttDates(id: string, input: UpdateGanttDatesInput) {
  return invokeCommand<void>("update_deliverable_gantt_dates", { id, input });
}

export function setDeliverableSection(deliverableId: string, sectionId: string | null) {
  return invokeCommand<void>("set_deliverable_section", { deliverableId, sectionId });
}

// ── Agentic minutes ────────────────────────────────────────────────────────────

export function uploadMeetingMinutes(filePath: string, stakeholderIds: string[]) {
  return invokeCommand<MeetingWithActions>("upload_meeting_minutes", { filePath, stakeholderIds });
}

export function generateWeeklyDigest() {
  return invokeCommand<WeeklyDigest>("generate_weekly_digest");
}

// ── Labels ─────────────────────────────────────────────────────────────────────

export function listLabels() {
  return invokeCommand<Label[]>("list_labels");
}

export function createLabel(input: CreateLabelInput) {
  return invokeCommand<Label>("create_label", { input });
}

export function deleteLabel(id: string) {
  return invokeCommand<void>("delete_label", { id });
}

export function assignLabelToDeliverable(deliverableId: string, labelId: string) {
  return invokeCommand<void>("assign_label_to_deliverable", { deliverableId, labelId });
}

export function removeLabelFromDeliverable(deliverableId: string, labelId: string) {
  return invokeCommand<void>("remove_label_from_deliverable", { deliverableId, labelId });
}

// ── Reorder + friction state move ─────────────────────────────────────────────

export function reorderDeliverable(id: string, displayOrder: number) {
  return invokeCommand<Deliverable>("reorder_deliverable", { id, displayOrder });
}

export function reorderDeliverableWithinState(input: ReorderDirectionInput) {
  return invokeCommand<Deliverable[]>("reorder_deliverable_within_state", { input });
}

export function updateDeliverableStateFriction(
  id: string,
  stateValue: DeliverableState,
  frictionNote: string | null,
) {
  return invokeCommand<Deliverable>("update_deliverable_state_friction", {
    id,
    stateValue,
    frictionNote,
  });
}

export function applyFlaggedToBacklog(input: ApplyFlaggedToBacklogInput) {
  return invokeCommand<Deliverable>("apply_flagged_to_backlog", { input });
}

// Suppress unused import warnings for types only used in other files
export type { DeliverablePriority };

// ── Gmail ──────────────────────────────────────────────────────────────────────

export function gmailStatus() {
  return invokeCommand<GmailStatus>("gmail_status");
}

export function gmailConnect() {
  return invokeCommand<GmailStatus>("gmail_connect");
}

export function gmailDisconnect() {
  return invokeCommand<GmailStatus>("gmail_disconnect");
}

export function gmailSearchThreads(query: string, maxResults?: number) {
  return invokeCommand<GmailThread[]>("gmail_search_threads", {
    query,
    maxResults: maxResults ?? null,
  });
}

export function gmailGetSyncSettings() {
  return invokeCommand<GmailSyncSettings>("gmail_get_sync_settings");
}

export function gmailUpdateSyncSettings(input: GmailSyncSettingsInput) {
  return invokeCommand<GmailSyncSettings>("gmail_update_sync_settings", { input });
}

export function gmailSyncNow() {
  return invokeCommand<GmailSyncReport>("gmail_sync_now");
}

export function gmailListLocalThreads(filters?: GmailThreadFilter) {
  return invokeCommand<GmailLocalThread[]>("gmail_list_local_threads", {
    filters: filters ?? null,
  });
}

export function gmailListWorkMailThreads(query?: WorkMailQuery) {
  return invokeCommand<WorkMailThread[]>("gmail_list_work_mail_threads", {
    query: query ?? null,
  });
}

export function gmailWorkMailViewCounts() {
  return invokeCommand<WorkMailViewCounts>("gmail_work_mail_view_counts");
}

export function gmailWorkMailBrief() {
  return invokeCommand<WorkMailBrief>("gmail_work_mail_brief");
}

export function gmailListWorkMailDomains() {
  return invokeCommand<WorkMailDomain[]>("gmail_list_work_mail_domains");
}

export function gmailUpsertWorkMailDomain(input: UpsertWorkMailDomainInput) {
  return invokeCommand<WorkMailDomain>("gmail_upsert_work_mail_domain", { input });
}

export function gmailDeleteWorkMailDomain(domain: string) {
  return invokeCommand<void>("gmail_delete_work_mail_domain", { domain });
}

export function gmailListWorkMailAgentEvents(limit?: number) {
  return invokeCommand<WorkMailAgentEvent[]>("gmail_list_work_mail_agent_events", {
    limit: limit ?? null,
  });
}

export function gmailMarkWorkMailThreadSeen(threadId: string) {
  return invokeCommand<WorkMailReviewSummary>("gmail_mark_work_mail_thread_seen", { threadId });
}

export function gmailSetWorkMailReviewState(threadId: string, input: WorkMailReviewUpdate) {
  return invokeCommand<WorkMailReviewSummary>("gmail_set_work_mail_review_state", {
    threadId,
    input,
  });
}

export function gmailDeferWorkMailThread(threadId: string, deferredUntil?: string | null) {
  return invokeCommand<WorkMailReviewSummary>("gmail_defer_work_mail_thread", {
    threadId,
    deferredUntil: deferredUntil ?? null,
  });
}

export function gmailReopenWorkMailThread(threadId: string) {
  return invokeCommand<WorkMailReviewSummary>("gmail_reopen_work_mail_thread", { threadId });
}

export function gmailPromoteWorkMailThread(threadId: string) {
  return invokeCommand<UserClassification>("gmail_promote_work_mail_thread", { threadId });
}

export function gmailRestoreWorkMailThread(threadId: string) {
  return invokeCommand<UserClassification>("gmail_restore_work_mail_thread", { threadId });
}

export function gmailExcludeWorkMailThread(threadId: string) {
  return invokeCommand<UserClassification>("gmail_exclude_work_mail_thread", { threadId });
}

export function gmailGetLocalThread(threadId: string) {
  return invokeCommand<GmailThreadDetail>("gmail_get_local_thread", { threadId });
}

export function gmailListLabels() {
  return invokeCommand<GmailLabelRecord[]>("gmail_list_labels");
}

export function gmailCategoryCounts() {
  return invokeCommand<GmailCategoryCount[]>("gmail_category_counts");
}

export function gmailListDrafts() {
  return invokeCommand<GmailDraftRecord[]>("gmail_list_drafts");
}

export function gmailLinkThreadToDeliverable(threadId: string, deliverableId: string) {
  return invokeCommand<void>("gmail_link_thread_to_deliverable", { threadId, deliverableId });
}

export function gmailUnlinkThreadFromDeliverable(threadId: string, deliverableId: string) {
  return invokeCommand<void>("gmail_unlink_thread_from_deliverable", { threadId, deliverableId });
}

export function gmailLinkThreadToInitiative(threadId: string, initiativeId: string) {
  return invokeCommand<void>("gmail_link_thread_to_initiative", { threadId, initiativeId });
}

export function gmailSuggestThreadsForDeliverable(deliverableId: string, limit?: number) {
  return invokeCommand<GmailLocalThread[]>("gmail_suggest_threads_for_deliverable", {
    deliverableId,
    limit: limit ?? null,
  });
}

export function gmailStakeholderThreads(stakeholderId: string, limit?: number) {
  return invokeCommand<GmailLocalThread[]>("gmail_stakeholder_threads", {
    stakeholderId,
    limit: limit ?? null,
  });
}

export function gmailExcludeThreadFromStakeholder(threadId: string, stakeholderId: string) {
  return invokeCommand<void>("gmail_exclude_thread_from_stakeholder", { threadId, stakeholderId });
}

export function gmailStakeholderHealth(stakeholderId: string) {
  return invokeCommand<GmailStakeholderHealth>("gmail_stakeholder_health", { stakeholderId });
}

export function gmailStakeholderSuggestions(minThreads?: number) {
  return invokeCommand<GmailStakeholderSuggestion[]>("gmail_stakeholder_suggestions", {
    minThreads: minThreads ?? null,
  });
}

export function gmailRelationshipGraph(limit?: number) {
  return invokeCommand<GmailRelationshipEdge[]>("gmail_relationship_graph", {
    limit: limit ?? null,
  });
}

export function gmailCreateCaptureFromThread(threadId: string) {
  return invokeCommand<Capture>("gmail_create_capture_from_thread", { threadId });
}

export function gmailCreateTaskFromThread(
  threadId: string,
  deliverableId: string,
  title: string,
  dueDate?: string | null,
) {
  return invokeCommand<DeliverableTask>("gmail_create_task_from_thread", {
    input: { thread_id: threadId, deliverable_id: deliverableId, title, due_date: dueDate ?? null },
  });
}

export function gmailAnalyzeThread(threadId: string, includeReply?: boolean) {
  return invokeCommand<GmailAiResult>("gmail_analyze_thread", {
    threadId,
    includeReply: includeReply ?? null,
  });
}

export function gmailGetEffectiveClassification(threadId: string) {
  return invokeCommand<EffectiveClassification>(
    "gmail_get_effective_classification",
    { threadId },
  );
}

export function gmailGetThreadOverride(threadId: string) {
  return invokeCommand<UserClassification | null>("gmail_get_thread_override", {
    threadId,
  });
}

export function gmailSetThreadOverride(input: SetOverrideInput) {
  return invokeCommand<UserClassification>("gmail_set_thread_override", { input });
}

export function gmailClearThreadOverride(threadId: string) {
  return invokeCommand<void>("gmail_clear_thread_override", { threadId });
}

export function gmailListSenderRules() {
  return invokeCommand<SenderRule[]>("gmail_list_sender_rules");
}

export function gmailCreateSenderRule(input: CreateSenderRuleInput) {
  return invokeCommand<SenderRule>("gmail_create_sender_rule", { input });
}

export function gmailDeleteSenderRule(id: string) {
  return invokeCommand<void>("gmail_delete_sender_rule", { id });
}

export function gmailToggleSenderRule(id: string, enabled: boolean) {
  return invokeCommand<void>("gmail_toggle_sender_rule", { id, enabled });
}

export function gmailInboxDashboard(hours?: number) {
  return invokeCommand<GmailInboxDashboard>("gmail_inbox_dashboard", {
    hours: hours ?? null,
  });
}

export function gmailCalibrationReport() {
  return invokeCommand<CalibrationReport>("gmail_calibration_report");
}

export function gmailRetryFailedAnalyses(limit?: number) {
  return invokeCommand<number>("gmail_retry_failed_analyses", {
    limit: limit ?? null,
  });
}

export function gmailBatchAnalyze(limit?: number) {
  return invokeCommand<number>("gmail_batch_analyze", { limit: limit ?? null });
}

export function gmailReanalyzeStaleThreads(limit?: number) {
  return invokeCommand<GmailSyncReport>("gmail_reanalyze_stale_threads", {
    limit: limit ?? null,
  });
}

export function gmailAutoLinkThread(threadId: string) {
  return invokeCommand<GmailAutoLinkReport>("gmail_auto_link_thread", { threadId });
}

export function gmailListOrphanThreads(limit?: number) {
  return invokeCommand<GmailLocalThread[]>("gmail_list_orphan_threads", {
    limit: limit ?? null,
  });
}

export function gmailListThreadLinkSuggestions(threadId?: string, limit?: number) {
  return invokeCommand<GmailThreadLinkSuggestion[]>("gmail_list_thread_link_suggestions", {
    threadId: threadId ?? null,
    limit: limit ?? null,
  });
}

export function gmailAcceptThreadLink(suggestionId: string) {
  return invokeCommand<void>("gmail_accept_thread_link", { suggestionId });
}

export function gmailRejectThreadLink(suggestionId: string) {
  return invokeCommand<void>("gmail_reject_thread_link", { suggestionId });
}

export function gmailGenerateWorkIntake(threadId: string) {
  return invokeCommand<WorkIntakeSuggestion[]>("gmail_generate_work_intake", { threadId });
}

export function gmailTriageThread(threadId: string) {
  return invokeCommand<GmailTriageResult>("gmail_triage_thread", { threadId });
}

export function gmailWeeklyDigest() {
  return invokeCommand<GmailWeeklyDigest>("gmail_weekly_digest");
}

export function gmailSendEmail(input: GmailSendInput) {
  return invokeCommand<GmailSendResult>("gmail_send_email", { input });
}

// ---------- Local-first drafts + agentic AI draft ----------

export function gmailGetLocalDraft(threadId: string) {
  return invokeCommand<LocalEmailDraft | null>("gmail_get_local_draft", { threadId });
}

export function gmailSaveLocalDraft(input: SaveLocalDraftInput) {
  return invokeCommand<LocalEmailDraft>("gmail_save_local_draft", { input });
}

export function gmailDeleteLocalDraft(draftId: string) {
  return invokeCommand<void>("gmail_delete_local_draft", { draftId });
}

export function gmailAddDraftAttachment(draftId: string, sourcePath: string) {
  return invokeCommand<LocalEmailDraftAttachment>("gmail_add_draft_attachment", {
    draftId,
    sourcePath,
  });
}

export function gmailRemoveDraftAttachment(attachmentId: string) {
  return invokeCommand<void>("gmail_remove_draft_attachment", { attachmentId });
}

/** Agentic AI draft using memory + brain + thread context. Returns plain text. */
export function gmailDraftReplyWithBrain(threadId: string) {
  return invokeCommand<string>("gmail_draft_reply_with_brain", { threadId });
}

/** Snapshot history of every Gemini analysis run for a thread (newest first). */
export function gmailListAnalysisHistory(threadId: string, limit?: number) {
  return invokeCommand<GmailAnalysisSnapshot[]>("gmail_list_analysis_history", {
    threadId,
    limit: limit ?? null,
  });
}

export function gmailArchiveThread(threadId: string) {
  return invokeCommand<void>("gmail_archive_thread", { threadId });
}

export function gmailMoveThreadToSpam(threadId: string) {
  return invokeCommand<void>("gmail_move_thread_to_spam", { threadId });
}

export function gmailMarkThreadImportant(threadId: string) {
  return invokeCommand<GmailThreadDetail>("gmail_mark_thread_important", { threadId });
}

export function gmailStarThread(threadId: string) {
  return invokeCommand<GmailThreadDetail>("gmail_star_thread", { threadId });
}

export function gmailMarkThreadReadInGmail(threadId: string) {
  return invokeCommand<GmailThreadDetail>("gmail_mark_thread_read_in_gmail", { threadId });
}

export function gmailMarkThreadUnreadInGmail(threadId: string) {
  return invokeCommand<GmailThreadDetail>("gmail_mark_thread_unread_in_gmail", { threadId });
}

// ── Work intake ───────────────────────────────────────────────────────────────

export function listWorkIntakeSuggestions(filters?: WorkIntakeFilters) {
  return invokeCommand<WorkIntakeSuggestion[]>("list_work_intake_suggestions", {
    filters: filters ?? null,
  });
}

export function generateWorkspaceWorkIntake() {
  return invokeCommand<WorkIntakeSuggestion[]>("generate_workspace_work_intake");
}

export function approveWorkIntakeSuggestion(input: ApproveWorkIntakeInput) {
  return invokeCommand<WorkIntakeApplyResult>("approve_work_intake_suggestion", { input });
}

export function dismissWorkIntakeSuggestion(id: string) {
  return invokeCommand<WorkIntakeSuggestion>("dismiss_work_intake_suggestion", { id });
}

// ── Google Calendar ────────────────────────────────────────────────────────────

export function gcalStatus() {
  return invokeCommand<GCalStatus>("gcal_status");
}

export function gcalConnect() {
  return invokeCommand<GCalStatus>("gcal_connect");
}

export function gcalDisconnect() {
  return invokeCommand<GCalStatus>("gcal_disconnect");
}

export function gcalSync() {
  return invokeCommand<void>("gcal_sync");
}

export function gcalStakeholderEvents(email: string) {
  return invokeCommand<import("./types").GCalEvent[]>("gcal_stakeholder_events", { email });
}

export interface CreateGcalMeetingInput {
  title: string;
  date: string;
  start_time: string;
  end_time: string;
  time_zone?: string;
  description?: string;
  location?: string;
  attendees: string[];
  add_meet: boolean;
  zoom_url?: string;
}

export interface CreateGcalMeetingResult {
  success: boolean;
  gcal_event_id: string;
  html_link?: string;
  meet_link?: string;
  title: string;
  date: string;
}

export function createGcalMeeting(input: CreateGcalMeetingInput) {
  return invokeCommand<CreateGcalMeetingResult>("create_gcal_meeting", { input });
}

export function deleteGcalMeeting(gcalEventId: string) {
  return invokeCommand<{ success: boolean; gcal_event_id: string }>("delete_gcal_meeting", {
    gcalEventId,
  });
}

export function openExternalUrl(url: string) {
  return invokeCommand<void>("open_external_url", { url });
}
// ── User Profile ──────────────────────────────────────────────────────────────
export function getUserProfile() {
  return invokeCommand<UserProfile>("get_user_profile");
}

export function updateUserProfile(input: UpdateUserProfileInput) {
  return invokeCommand<UserProfile>("update_user_profile", { input });
}
