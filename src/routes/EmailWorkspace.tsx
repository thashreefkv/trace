import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useIpcQuery, qk } from "../lib/queries";
import { queryClient } from "../lib/queryClient";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import DOMPurify from "dompurify";
import {
  Archive,
  BriefcaseBusiness,
  Check,
  ChevronDown,
  ChevronRight,
  ChevronsRight,
  ExternalLink,
  FileText,
  MoreHorizontal,
  CircleAlert,
  AtSign,
  CalendarDays,
  Inbox,
  Loader2,
  Mail,
  Paperclip,
  PenLine,
  Plus,
  RefreshCw,
  Search,
  Send,
  Settings2,
  Sparkles,
  Star,
  User,
  UsersRound,
  X,
} from "lucide-react";
import { safeExternalUrl } from "../lib/urlSafety";
import {
  createDeliverable,
  createStakeholder,
  approveWorkIntakeSuggestion,
  dismissWorkIntakeSuggestion,
  generateWorkspaceWorkIntake,
  listWorkIntakeSuggestions,
  gmailAnalyzeThread,
  gmailArchiveThread,
  gmailConnect,
  gmailCreateCaptureFromThread,
  gmailCreateTaskFromThread,
  gmailDisconnect,
  gmailGetLocalThread,
  gmailListDrafts,
  gmailListWorkMailAgentEvents,
  gmailListWorkMailThreads,
  gmailMarkThreadReadInGmail,
  gmailMarkThreadUnreadInGmail,
  gmailMarkWorkMailThreadSeen,
  gmailMarkThreadImportant,
  gmailExcludeWorkMailThread,
  gmailRestoreWorkMailThread,
  gmailPromoteWorkMailThread,
  gmailReopenWorkMailThread,
  gmailRelationshipGraph,
  gmailSetWorkMailReviewState,
  gmailStarThread,
  gmailStatus,
  gmailSyncNow,
  gmailReanalyzeStaleThreads,
  gmailWorkMailBrief,
  gmailWorkMailViewCounts,
  gmailGenerateWorkIntake,
  gmailMoveThreadToSpam,
  gmailUpdateSyncSettings,
  gmailWeeklyDigest,
  gmailLinkThreadToDeliverable,
  gmailLinkThreadToInitiative,
  getUserProfile,
  listDeliverables,
  listInitiatives,
  listStakeholders,
} from "../lib/ipc";
import type {
  Deliverable,
  EmailAddress,
  GmailAiCandidate,
  GmailAiCategory,
  Stakeholder,
  GmailAiResult,
  GmailDraftRecord,
  GmailLocalThread,
  GmailMessageRecord,
  GmailRelationshipEdge,
  GmailSyncSettings,
  GmailThreadDetail,
  GmailTriageResult,
  GmailAttachmentRecord,
  GmailWeeklyDigest,
  Initiative,
  UserProfile,
  WorkIntakeKind,
  WorkIntakeSuggestion,
  WorkMailAgentEvent,
  WorkMailAttentionState,
  WorkMailMessageType,
  WorkMailQuery,
  WorkMailRelevance,
  WorkMailReviewState,
  WorkMailViewId,
} from "../lib/types";
import { openUrl } from "@tauri-apps/plugin-opener";
import { driveGetFileMetadata } from "../lib/files";
import { formatDateTime } from "../lib/format";
import { ThreadClassificationEditor } from "../components/ThreadClassificationEditor";
import { ReplyComposer } from "../components/ReplyComposer/ReplyComposer";
import { AiAnalysisSheet } from "../components/AiAnalysisSheet/AiAnalysisSheet";
import { AttachmentSheet } from "../components/AttachmentSheet/AttachmentSheet";
import { EmptyState } from "../components/EmptyState";
import { Avatar } from "../components/Avatar";
import { avatarColor, initials as avatarInitialsShared } from "../lib/avatar";
import { GmailSyncSettingsControls } from "../components/GmailSyncSettingsControls";
import { WorkMailDomainSettings } from "../components/WorkMailDomainSettings";

const EMAIL_LIST_PAGE_SIZE = 20;

const workMailViews: Array<{ id: WorkMailViewId; label: string }> = [
  { id: "all_work", label: "All Work" },
  { id: "needs_me", label: "Review Queue" },
  { id: "projects", label: "Projects" },
  { id: "deliverables", label: "Deliverables" },
  { id: "stakeholders", label: "Stakeholders" },
  { id: "files", label: "Files" },
  { id: "meetings", label: "Meetings" },
  { id: "unlinked", label: "Unlinked" },
  { id: "excluded", label: "Excluded" },
  { id: "agent_activity", label: "Agent Activity" },
];

export function EmailWorkspace() {
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  const [emailListPage, setEmailListPage] = useState(0);
  const [expandedBundles, setExpandedBundles] = useState<Set<string>>(
    () => new Set(),
  );
  const [activeSearchQuery, setActiveSearchQuery] = useState("");
  const [drafts, setDrafts] = useState<GmailDraftRecord[]>([]);
  const [workSuggestions, setWorkSuggestions] = useState<WorkIntakeSuggestion[]>([]);
  const [edges, setEdges] = useState<GmailRelationshipEdge[]>([]);
  const [selected, setSelected] = useState<GmailThreadDetail | null>(null);
  const [ai, setAi] = useState<GmailAiResult | null>(null);
  const [triage, setTriage] = useState<GmailTriageResult | null>(null);
  const [query, setQuery] = useState("");
  const [filtersOpen, setFiltersOpen] = useState(false);
  const [senderDomainFilter, setSenderDomainFilter] = useState("");
  const [attentionFilter, setAttentionFilter] = useState<WorkMailAttentionState | "">("");
  const [messageTypeFilter, setMessageTypeFilter] = useState<WorkMailMessageType | "">("");
  const [relevanceFilter, setRelevanceFilter] = useState<WorkMailRelevance | "">("");
  const [reviewFilter, setReviewFilter] = useState<WorkMailReviewState | "">("");
  const [unreadOnly, setUnreadOnly] = useState(false);
  const [traceUnseenOnly, setTraceUnseenOnly] = useState(false);
  const [seenUnreviewedOnly, setSeenUnreviewedOnly] = useState(false);
  const [artifactOnly, setArtifactOnly] = useState(false);
  const [selectedDeliverableId, setSelectedDeliverableId] = useState("");
  const [selectedInitiativeId, setSelectedInitiativeId] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [intakeLoading, setIntakeLoading] = useState(false);
  const [summarizing, setSummarizing] = useState(false);
  const [threadAction, setThreadAction] = useState<"archive" | "spam" | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [replyOpen, setReplyOpen] = useState(false);
  const [replyToOverride, setReplyToOverride] = useState<string[] | null>(null);
  const [analysisOpen, setAnalysisOpen] = useState(false);
  const [openedAttachment, setOpenedAttachment] = useState<string | null>(null);

  function openComposerForEmail(email: string) {
    setReplyToOverride([email]);
    setReplyOpen(true);
  }
  const [activePanel, setActivePanel] = useState<EmailPanel | null>(null);
  const [userProfile, setUserProfile] = useState<UserProfile | null>(null);

  useEffect(() => {
    getUserProfile()
      .then(setUserProfile)
      .catch(() => {});
  }, []);

  const ownerEmail = (userProfile?.email || "").trim().toLowerCase();
  const ownerName = userProfile?.name?.trim() || null;
  const threadParam = params.get("thread");
  const viewParam = (params.get("view") as WorkMailViewId | null) ?? "all_work";
  const activeView: WorkMailViewId = isWorkMailView(viewParam) ? viewParam : "all_work";

  // ── React-query data ─────────────────────────────────────────────────────────
  const activeFilters = useMemo<WorkMailQuery>(() => ({
    view: activeView,
    query: activeSearchQuery || undefined,
    sender_domain: senderDomainFilter.trim() || undefined,
    attention_state: attentionFilter || undefined,
    message_type: messageTypeFilter || undefined,
    work_relevance: relevanceFilter || undefined,
    review_state: reviewFilter || undefined,
    gmail_unread: unreadOnly || undefined,
    trace_unseen: traceUnseenOnly || undefined,
    seen_unreviewed: seenUnreviewedOnly || undefined,
    has_artifact: artifactOnly || undefined,
    limit: 100,
  }), [
    activeView,
    activeSearchQuery,
    senderDomainFilter,
    attentionFilter,
    messageTypeFilter,
    relevanceFilter,
    reviewFilter,
    unreadOnly,
    traceUnseenOnly,
    seenUnreviewedOnly,
    artifactOnly,
  ]);

  const { data: statusData, isLoading: statusLoading } = useIpcQuery(
    qk.gmail.status,
    gmailStatus,
  );
  const connected = statusData?.connected ?? false;
  const settings = statusData?.settings ?? null;

  const { data: threads = [], isLoading: loading } = useIpcQuery(
    qk.gmail.workMailThreads(activeFilters),
    () => gmailListWorkMailThreads(activeFilters),
    { enabled: connected && activeView !== "agent_activity" },
  );

  const { data: deliverables = [] } = useIpcQuery(
    qk.deliverables.list(),
    listDeliverables,
    { enabled: connected },
  );
  const { data: initiatives = [] } = useIpcQuery(
    qk.initiatives.list,
    listInitiatives,
    { enabled: connected },
  );
  const { data: stakeholders = [] } = useIpcQuery(
    qk.stakeholders.list,
    listStakeholders,
    { enabled: connected },
  );
  const { data: workMailCounts = null } = useIpcQuery(
    qk.gmail.workMailCounts,
    gmailWorkMailViewCounts,
    { enabled: connected },
  );
  const { data: workMailBrief = null } = useIpcQuery(
    qk.gmail.workMailBrief,
    gmailWorkMailBrief,
    { enabled: connected },
  );
  const { data: workMailActivity = [] } = useIpcQuery(
    qk.gmail.workMailActivity(80),
    () => gmailListWorkMailAgentEvents(80),
    { enabled: connected && activeView === "agent_activity" },
  );
  const { data: digest = null } = useIpcQuery(
    qk.gmail.weeklyDigest,
    gmailWeeklyDigest,
    { enabled: connected, staleTime: 10 * 60_000 },
  );
  // ────────────────────────────────────────────────────────────────────────────

  const stakeholderByEmail = useMemo(() => {
    const map = new Map<string, Stakeholder>();
    for (const s of stakeholders) {
      if (s.email) map.set(s.email.toLowerCase(), s);
    }
    return map;
  }, [stakeholders]);

  // Load drafts and relationship graph lazily once connected
  useEffect(() => {
    if (!connected) return;
    gmailListDrafts().then(setDrafts).catch(() => {});
    gmailRelationshipGraph(20).then(setEdges).catch(() => {});
  }, [connected]);

  useEffect(() => {
    if (threadParam) {
      void selectThread(threadParam);
    }
  }, [threadParam]);

  async function handleConnect() {
    try {
      setSyncing(true);
      setError(null);
      await gmailConnect();
      void queryClient.invalidateQueries({ queryKey: qk.gmail.status });
      setMessage("Gmail connected. Run a sync to hydrate local email context.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setSyncing(false);
    }
  }

  async function handleReconnectGmail() {
    try {
      setSyncing(true);
      setError(null);
      setMessage("Opening Gmail authorization. Approve the updated permissions to enable archive and spam actions.");
      await gmailDisconnect();
      await gmailConnect();
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      setMessage("Gmail reconnected with updated permissions. Archive and spam actions are enabled.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setSyncing(false);
    }
  }

  async function handleSync() {
    try {
      setSyncing(true);
      setError(null);
      const report = await gmailSyncNow();
      setMessage(
        `Synced ${report.synced_threads} threads, ${report.synced_messages} messages, ${report.synced_drafts} drafts. AI analyzed ${report.ai_analyzed_threads}, refreshed ${report.analysis_refreshed_threads}, auto-linked ${report.auto_linked_threads}; ${report.orphan_threads} unlinked candidates remain. Skipped ${report.skipped_spam_threads} spam/trash threads. Backfilled ${report.backfilled_threads} older threads${report.backfill_complete ? "; historical backfill is complete." : "."}`,
      );
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
    } catch (caught) {
      setError(String(caught));
    } finally {
      setSyncing(false);
    }
  }

  function handleSearch() {
    setEmailListPage(0);
    setActiveSearchQuery(query.trim());
  }

  async function handleScopeAction(
    threadId: string,
    action: "exclude" | "restore" | "promote",
  ) {
    try {
      setError(null);
      if (action === "exclude") await gmailExcludeWorkMailThread(threadId);
      if (action === "restore") await gmailRestoreWorkMailThread(threadId);
      if (action === "promote") await gmailPromoteWorkMailThread(threadId);
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      setMessage(
        action === "exclude"
          ? "Thread moved to recoverable Excluded."
          : action === "restore"
            ? "Thread restored to Work Mail."
            : "Thread promoted into Work Mail.",
      );
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function selectThread(threadId: string) {
    try {
      setError(null);
      setAi(null);
      setTriage(null);
      const loaded = await gmailGetLocalThread(threadId);
      const seen = await gmailMarkWorkMailThreadSeen(threadId);
      const detail = loaded.thread.has_unread
        ? await gmailMarkThreadReadInGmail(threadId)
        : loaded;
      setSelected({
        ...detail,
        thread: {
          ...detail.thread,
          trace_seen_at: seen.trace_seen_at,
          trace_review_state: seen.review_state,
          seen_through_message_id: seen.seen.message_id,
          seen_through_message_at: seen.seen.message_at,
          reviewed_through_message_id: seen.reviewed_through_message_id,
          reviewed_through_message_at: seen.reviewed_through_message_at,
          deferred_until: seen.deferred_until,
          new_since_review: seen.new_since_review,
        },
      });
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      await loadWorkSuggestions(threadId);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function loadWorkSuggestions(threadId: string) {
    try {
      const suggestions = await listWorkIntakeSuggestions({
        status: "pending",
        source_kind: "gmail",
        source_id: threadId,
        limit: 30,
      });
      setWorkSuggestions(suggestions);
    } catch {
      setWorkSuggestions([]);
    }
  }

  function handleViewChange(view: WorkMailViewId) {
    setAi(null);
    setTriage(null);
    setSelected(null);
    setEmailListPage(0);
    setActiveSearchQuery("");
    setQuery("");
    setParams({ view });
  }

  function handleBriefShortcut(
    shortcut: "queue" | "unseen" | "seen_pending" | "waiting" | "deferred",
  ) {
    setAi(null);
    setTriage(null);
    setSelected(null);
    setEmailListPage(0);
    setUnreadOnly(false);
    setTraceUnseenOnly(false);
    setSeenUnreviewedOnly(false);
    setReviewFilter("");
    setAttentionFilter("");
    setMessageTypeFilter("");
    setRelevanceFilter("");
    setSenderDomainFilter("");
    setArtifactOnly(false);
    setActiveSearchQuery("");
    setQuery("");
    if (shortcut === "queue") {
      setParams({ view: "needs_me" });
      return;
    }
    setParams({ view: "all_work" });
    if (shortcut === "unseen") setTraceUnseenOnly(true);
    if (shortcut === "seen_pending") setSeenUnreviewedOnly(true);
    if (shortcut === "waiting") setReviewFilter("waiting");
    if (shortcut === "deferred") setReviewFilter("deferred");
  }

  async function handleSaveSettings(next: Partial<GmailSyncSettings>) {
    try {
      await gmailUpdateSyncSettings({
        sync_interval_hours: next.sync_interval_hours,
        max_threads_per_sync: next.max_threads_per_sync,
        notify_new_mail: next.notify_new_mail,
        include_sent: next.include_sent,
        include_drafts: next.include_drafts,
        backfill_enabled: next.backfill_enabled,
        relevance_filter_enabled: next.relevance_filter_enabled,
        auto_analyze_enabled: next.auto_analyze_enabled,
        auto_analyze_limit: next.auto_analyze_limit,
      });
      void queryClient.invalidateQueries({ queryKey: qk.gmail.status });
      setMessage("Gmail sync settings saved.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleAnalyze(includeReply: boolean) {
    if (!selected) return;
    try {
      setAnalyzing(true);
      setError(null);
      setAi(await gmailAnalyzeThread(selected.thread.thread_id, includeReply));
      await selectThread(selected.thread.thread_id);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setAnalyzing(false);
    }
  }

  async function handleDraftReply() {
    if (!selected) return;
    // Open the composer; AI Draft inside the composer pulls memory + brain
    // context for the actual draft. Refresh AI panel in the background so the
    // "Open AI review" menu item shows fresh analysis.
    setReplyOpen(true);
    try {
      setAnalyzing(true);
      setError(null);
      const result = await gmailAnalyzeThread(selected.thread.thread_id, false);
      setAi(result);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setAnalyzing(false);
    }
  }

  async function handleCapture() {
    if (!selected) return;
    try {
      await gmailCreateCaptureFromThread(selected.thread.thread_id);
      setMessage("Email thread added to capture inbox.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleCreateStakeholderFromAddress(address: EmailAddress, reason?: string) {
    if (!address.email) return;
    try {
      await createStakeholder({
        name: address.name || address.email,
        email: address.email,
        role: "",
        notes: reason ? `Added from Gmail review: ${reason}` : "Added from Gmail participant.",
      });
      setMessage(`Created stakeholder for ${address.name || address.email}.`);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleArchive() {
    if (!selected) return;
    const threadId = selected.thread.thread_id;
    try {
      setThreadAction("archive");
      setError(null);
      await gmailArchiveThread(threadId);
      removeThreadFromView(threadId);
      setMessage("Thread archived.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setThreadAction(null);
    }
  }

  async function handleMoveToSpam() {
    if (!selected) return;
    const threadId = selected.thread.thread_id;
    try {
      setThreadAction("spam");
      setError(null);
      await gmailMoveThreadToSpam(threadId);
      removeThreadFromView(threadId);
      setMessage("Thread moved to spam and removed from Trace email.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setThreadAction(null);
    }
  }

  async function handleMarkImportant() {
    if (!selected) return;
    try {
      setError(null);
      const detail = await gmailMarkThreadImportant(selected.thread.thread_id);
      setSelected(detail);
      setMessage("Thread marked important.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleStar() {
    if (!selected) return;
    try {
      setError(null);
      const detail = await gmailStarThread(selected.thread.thread_id);
      setSelected(detail);
      setMessage("Thread starred as priority.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleReviewState(state: WorkMailReviewState) {
    if (!selected) return;
    try {
      setError(null);
      await gmailSetWorkMailReviewState(selected.thread.thread_id, {
        state,
        deferred_until: null,
      });
      await selectThread(selected.thread.thread_id);
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      setMessage(`Review state set to ${humanizeWorkMailValue(state)}.`);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleReopenReview() {
    if (!selected) return;
    try {
      setError(null);
      await gmailReopenWorkMailThread(selected.thread.thread_id);
      await selectThread(selected.thread.thread_id);
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      setMessage("Thread reopened for review.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleGmailReadWriteback(next: "read" | "unread") {
    if (!selected) return;
    try {
      setError(null);
      const detail =
        next === "read"
          ? await gmailMarkThreadReadInGmail(selected.thread.thread_id)
          : await gmailMarkThreadUnreadInGmail(selected.thread.thread_id);
      setSelected(detail);
      void queryClient.invalidateQueries({ queryKey: qk.gmail.all });
      setMessage(`Marked ${next} in Gmail.`);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleTriageAction(action: string) {
    if (!selected) return;
    if (action === "mark_important") {
      await handleMarkImportant();
    } else if (action === "star") {
      await handleStar();
    } else if (action === "archive") {
      await handleArchive();
    } else if (action === "move_to_spam") {
      await handleMoveToSpam();
    } else if (action === "create_capture") {
      await handleCapture();
    } else if (action === "reply") {
      await handleDraftReply();
    } else if (action === "create_task" || action === "create_deliverable") {
      await handleAnalyze(false);
      setActivePanel("ai");
    }
    // Note: "create_stakeholder" action is intentionally a no-op — the
    // stakeholder picker lives directly on the participant chips now.
  }

  function removeThreadFromView(threadId: string) {
    queryClient.setQueryData<GmailLocalThread[]>(
      qk.gmail.workMailThreads(activeFilters),
      (old) => old?.filter((t) => t.thread_id !== threadId) ?? [],
    );
    setAi(null);
    setSelected(null);
    setParams({ view: activeView });
  }

  async function handleApproveDeliverable(candidate: GmailAiCandidate) {
    if (!selected) return;
    try {
      setError(null);
      const created = await createDeliverable({
        title: candidate.title || selected.thread.subject || "Email deliverable",
        type: "email",
        state: "backlog",
        claim: candidate.body || selected.thread.summary || selected.thread.snippet || "Created from Gmail thread.",
        artifact_url: candidate.artifact_url ?? selected.thread.artifact_urls[0] ?? null,
        conversation_id: null,
        stakeholder_id: null,
        stakeholder_ids: [],
        initiative_ids: selected.thread.linked_initiatives.map((initiative) => initiative.id),
      });
      await gmailLinkThreadToDeliverable(selected.thread.thread_id, created.id);
      void queryClient.invalidateQueries({ queryKey: qk.deliverables.all });
      await selectThread(selected.thread.thread_id);
      setMessage(`Created deliverable "${created.title}" from email review.`);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleApproveTask(candidate: GmailAiCandidate) {
    if (!selected) return;
    const targetDeliverableId =
      selectedDeliverableId || selected.thread.linked_deliverables[0]?.id || "";
    if (!targetDeliverableId) {
      setError("Select or link a deliverable before approving this email task.");
      return;
    }
    try {
      setError(null);
      await gmailCreateTaskFromThread(
        selected.thread.thread_id,
        targetDeliverableId,
        candidate.title || "Follow up on email",
        candidate.due_date,
      );
      setMessage("Task created from approved email suggestion.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleLinkDeliverable() {
    if (!selected || !selectedDeliverableId) return;
    try {
      await gmailLinkThreadToDeliverable(selected.thread.thread_id, selectedDeliverableId);
      await selectThread(selected.thread.thread_id);
      setMessage("Thread linked to deliverable.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleLinkInitiative() {
    if (!selected || !selectedInitiativeId) return;
    try {
      await gmailLinkThreadToInitiative(selected.thread.thread_id, selectedInitiativeId);
      await selectThread(selected.thread.thread_id);
      setMessage("Thread linked to initiative.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleGenerateThreadIntake() {
    if (!selected) return;
    try {
      setIntakeLoading(true);
      setError(null);
      const suggestions = await gmailGenerateWorkIntake(selected.thread.thread_id);
      setWorkSuggestions(suggestions.filter((item) => item.status === "pending"));
      setActivePanel("work");
      setMessage(`Found ${suggestions.length} work intake suggestion${suggestions.length === 1 ? "" : "s"} from this thread.`);
      await selectThread(selected.thread.thread_id);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIntakeLoading(false);
    }
  }

  async function handleBatchAnalyze() {
    const BATCH = 10;
    const MAX_BATCHES = 50;
    let total = 0;
    let iterations = 0;
    try {
      setSummarizing(true);
      setError(null);
      let count: number;
      do {
        const report = await gmailReanalyzeStaleThreads(BATCH);
        count = report.ai_analyzed_threads;
        total += count;
        iterations++;
        if (count > 0) {
          setMessage(`Analyzed ${total} stale threads so far...`);
          void queryClient.invalidateQueries({ queryKey: qk.gmail.workMailThreads(activeFilters) });
        }
      } while (count > 0 && iterations < MAX_BATCHES);
      setMessage(
        total > 0
          ? `Done. Refreshed AI titles and summaries for ${total} stale thread${total === 1 ? "" : "s"}.`
          : "No threads needed analysis.",
      );
    } catch (caught) {
      setError(String(caught));
    } finally {
      setSummarizing(false);
    }
  }

  async function handleGenerateWorkspaceIntake() {
    try {
      setIntakeLoading(true);
      setError(null);
      const suggestions = await generateWorkspaceWorkIntake();
      setWorkSuggestions(
        selected
          ? suggestions.filter(
              (item) => item.source_kind === "gmail" && item.source_id === selected.thread.thread_id,
            )
          : [],
      );
      setActivePanel("work");
      setMessage(`Refreshed ${suggestions.length} pending work intake suggestion${suggestions.length === 1 ? "" : "s"}.`);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIntakeLoading(false);
    }
  }

  async function handleApproveWorkSuggestion(
    suggestion: WorkIntakeSuggestion,
    kindOverride?: WorkIntakeKind,
    edits?: { title?: string; body?: string; dueDate?: string; targetDeliverableId?: string; targetInitiativeId?: string },
  ) {
    try {
      setError(null);
      const effectiveKind = kindOverride ?? suggestion.item_kind;
      const result = await approveWorkIntakeSuggestion({
        id: suggestion.id,
        item_kind_override: kindOverride ?? null,
        title_override: edits?.title?.trim() || null,
        body_override: edits?.body?.trim() || null,
        due_date_override: edits?.dueDate?.trim() || null,
        // Card-level selection takes priority over the global panel dropdown
        target_deliverable_id:
          effectiveKind === "task"
            ? edits?.targetDeliverableId || selectedDeliverableId || selected?.thread.linked_deliverables[0]?.id || suggestion.target_deliverable_id
            : suggestion.target_deliverable_id,
        target_initiative_id:
          effectiveKind === "deliverable"
            ? edits?.targetInitiativeId || selectedInitiativeId || selected?.thread.linked_initiatives[0]?.id || suggestion.target_initiative_id
            : suggestion.target_initiative_id,
      });
      setWorkSuggestions((current) => current.filter((item) => item.id !== suggestion.id));
      setMessage(`Approved ${result.entity_kind}: ${suggestion.title}.`);
      if (selected) await selectThread(selected.thread.thread_id);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleDismissWorkSuggestion(id: string) {
    try {
      await dismissWorkIntakeSuggestion(id);
      setWorkSuggestions((current) => current.filter((item) => item.id !== id));
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleOpenAnalysis() {
    if (!selected) return;
    setAnalysisOpen(true);
    // If we have no in-memory AI result yet, kick off an initial analyze in
    // the background. The sheet will also pick up history snapshots written
    // by the auto-analyze background pass.
    if (!ai) {
      try {
        setAnalyzing(true);
        const result = await gmailAnalyzeThread(selected.thread.thread_id, false);
        setAi(result);
      } catch (caught) {
        setError(String(caught));
      } finally {
        setAnalyzing(false);
      }
    }
  }

  // Send is now owned by ReplyComposer (which handles drafts + attachments +
  // HTML body). The post-send callback here just refreshes the surface.
  async function handleReplySent() {
    setMessage("Reply sent.");
    if (selected) {
      // Refresh the thread so the new sent message appears immediately.
      void selectThread(selected.thread.thread_id);
    }
  }

  if (statusLoading && !statusData) return null;

  if (!connected) {
    return (
      <div className="mx-auto max-w-2xl px-5 py-10">
        <section className="rounded-2xl border border-zinc-100 bg-white p-6 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
          <div className="mb-4 flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-md bg-red-50 text-red-500">
              <Mail size={18} />
            </div>
            <div>
              <h1 className="text-lg font-semibold text-zinc-950">Email workspace</h1>
              <p className="text-sm text-zinc-500">Connect Gmail to sync threads into local SQLite.</p>
            </div>
          </div>
          {error ? <div className="mb-4 notice notice-error">{error}</div> : null}
          <button className="btn btn-primary" disabled={syncing} onClick={() => void handleConnect()} type="button">
            {syncing ? <Loader2 className="animate-spin" size={16} /> : <Mail size={16} />}
            Connect Gmail
          </button>
        </section>
      </div>
    );
  }

  // Thread detail — full-page view
  if (threadParam) {
    return (
      <div className="min-h-full bg-white">
        {selected ? (
          <>
            <ThreadToolbar
              analyzing={analyzing}
              detail={selected}
              onAnalyze={() => void handleOpenAnalysis()}
              onArchive={() => void handleArchive()}
              onAssets={() => setActivePanel("assets")}
              onCapture={() => void handleCapture()}
              onCompose={() => setReplyOpen(true)}
              onMarkImportant={() => void handleMarkImportant()}
              onMoveToSpam={() => void handleMoveToSpam()}
              onOpenWork={() => setActivePanel("work")}
              onStar={() => void handleStar()}
              threadAction={threadAction}
            />
            <div className="mx-auto max-w-4xl px-8 py-7">
              <ThreadTitleBlock
                detail={selected}
                onCreateStakeholder={(address) => void handleCreateStakeholderFromAddress(address)}
                ownerEmail={ownerEmail}
                ownerName={ownerName}
                stakeholderByEmail={stakeholderByEmail}
              />
              <WorkMailReviewControls
                onGmailRead={() => void handleGmailReadWriteback("read")}
                onGmailUnread={() => void handleGmailReadWriteback("unread")}
                onReopen={() => void handleReopenReview()}
                onReviewState={(state) => void handleReviewState(state)}
                thread={selected.thread}
              />
              <AiInsightsCard
                actionRequired={selected.thread.action_required}
                bundleSize={selected.thread.bundle_size}
                dimensionsConfidence={selected.thread.dimensions_confidence}
                intent={selected.thread.intent}
                predictedAction={selected.thread.predicted_action}
                reasons={selected.thread.ai_category_reasons}
                senderEmail={selected.thread.last_from_email || null}
                summary={selected.thread.summary}
                threadId={selected.thread.thread_id}
                threadState={selected.thread.thread_state}
                triage={triage}
              />
              <ThreadMessageStack
                messages={selected.messages}
                onComposeForEmail={openComposerForEmail}
                onOpenAttachment={setOpenedAttachment}
              />
            </div>
          </>
        ) : (
          <p className="p-8 text-sm text-zinc-500">{loading ? "Loading thread..." : "Thread not found."}</p>
        )}
        {error ? <div className="mx-8 mt-3 notice notice-error">{error}</div> : null}
        <EmailDrawer
          activePanel={activePanel}
          ai={ai}
          detail={selected}
          digest={digest}
          drafts={drafts}
          edges={edges}
          initiatives={initiatives}
          deliverables={deliverables}
          selectedDeliverableId={selectedDeliverableId}
          selectedInitiativeId={selectedInitiativeId}
          triage={triage}
          workSuggestions={workSuggestions}
          intakeLoading={intakeLoading}
          onAnalyze={() => void handleAnalyze(false)}
          onApproveDeliverable={(candidate) => void handleApproveDeliverable(candidate)}
          onApproveTask={(candidate) => void handleApproveTask(candidate)}
          onApproveWorkSuggestion={(suggestion, kind, edits) => void handleApproveWorkSuggestion(suggestion, kind, edits)}
          onClose={() => setActivePanel(null)}
          onDismissWorkSuggestion={(id) => void handleDismissWorkSuggestion(id)}
          onLinkDeliverable={() => void handleLinkDeliverable()}
          onLinkInitiative={() => void handleLinkInitiative()}
          onOpenAttachment={setOpenedAttachment}
          onRefreshWorkspaceIntake={() => void handleGenerateWorkspaceIntake()}
          onRunThreadIntake={() => void handleGenerateThreadIntake()}
          onSetDeliverable={setSelectedDeliverableId}
          onSetInitiative={setSelectedInitiativeId}
          onTriageAction={(action) => void handleTriageAction(action)}
        />
        {settings ? (
          <SettingsDialog
            onClose={() => setSettingsOpen(false)}
            onSave={(next) => void handleSaveSettings(next)}
            open={settingsOpen}
            settings={settings}
          />
        ) : null}
        <AiAnalysisSheet
          detail={selected}
          initialResult={ai}
          onClose={() => setAnalysisOpen(false)}
          open={analysisOpen}
          ownerEmail={ownerEmail}
          ownerName={ownerName}
        />
        <AttachmentSheet
          onClose={() => setOpenedAttachment(null)}
          open={openedAttachment !== null}
          url={openedAttachment}
        />
        <ReplyComposer
          defaultSubject={selected ? replySubjectFor(selected) : ""}
          defaultTo={
            replyToOverride ??
            (selected
              ? (() => {
                  const latestInbound = [...selected.messages]
                    .reverse()
                    .find((item) => !item.is_sent);
                  const addr =
                    latestInbound?.from_email || selected.thread.last_from_email;
                  return addr ? [addr] : [];
                })()
              : [])
          }
          onClose={() => {
            setReplyOpen(false);
            setReplyToOverride(null);
          }}
          onSent={() => void handleReplySent()}
          open={replyOpen}
          threadId={selected?.thread.thread_id ?? null}
        />
      </div>
    );
  }

  // Email list — full-page view.
  // Group threads by bundle_id so a multi-thread bundle (subject + 7-day
  // participant overlap) collapses into a single row with an expand toggle.
  // Threads with bundle_size <= 1 stay singleton groups.
  const groupedThreads = groupThreadsByBundle(threads);
  const totalPages = Math.ceil(groupedThreads.length / EMAIL_LIST_PAGE_SIZE);
  const pagedGroups = groupedThreads.slice(
    emailListPage * EMAIL_LIST_PAGE_SIZE,
    (emailListPage + 1) * EMAIL_LIST_PAGE_SIZE,
  );
  const pagedSections = groupWorkMailSections(pagedGroups, activeView);

  const toggleBundle = (bundleId: string) => {
    setExpandedBundles((prev) => {
      const next = new Set(prev);
      if (next.has(bundleId)) next.delete(bundleId);
      else next.add(bundleId);
      return next;
    });
  };

  return (
    <div className="min-h-full bg-zinc-50/40 px-5 py-6">

      {/* Page header */}
      <div className="mb-4 flex flex-wrap items-start justify-between gap-4">
        <div>
          <p className="page-kicker">Workspace</p>
          <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">Work Mail</h1>
          <p className="mt-1 max-w-2xl text-xs text-zinc-500">
            Work scope: linked external threads, your overrides
            {workMailBrief?.scope_domains.length
              ? `, ${workMailBrief.scope_domains.map((domain) => `@${domain}`).join(", ")}`
              : ""}.
          </p>
        </div>
        <div className="flex items-center gap-1.5">
          <EmailActionsMenu
            retrying={summarizing}
            onDrafts={() => setActivePanel("drafts")}
            onDigest={() => setActivePanel("digest")}
            onRetryTitle={() => void handleBatchAnalyze()}
            onRetrySummary={() => void handleBatchAnalyze()}
          />
          <button className="btn h-8 w-8 px-0" onClick={() => setSettingsOpen(true)} title="Work Mail settings" type="button">
            <Settings2 size={14} />
          </button>
          <button className="btn h-8 px-3 text-[12px]" disabled={syncing} onClick={() => void handleSync()} type="button">
            {syncing ? <Loader2 className="animate-spin" size={13} /> : <RefreshCw size={13} />}
            {syncing ? "Syncing…" : "Sync"}
          </button>
        </div>
      </div>

      <section className="mb-3 flex flex-wrap items-center gap-1.5">
        <WorkMailPulseButton
          active={activeView === "needs_me"}
          label="Review queue"
          onClick={() => handleBriefShortcut("queue")}
          tone="attention"
          value={workMailBrief?.needs_you ?? 0}
        />
        <WorkMailPulseButton
          active={activeView === "all_work" && traceUnseenOnly}
          label="Unseen"
          onClick={() => handleBriefShortcut("unseen")}
          tone="quiet"
          value={workMailBrief?.unseen_in_trace ?? 0}
        />
        {(workMailBrief?.seen_unreviewed ?? 0) > 0 ? (
          <WorkMailPulseButton
            active={activeView === "all_work" && seenUnreviewedOnly}
            label="Seen pending"
            onClick={() => handleBriefShortcut("seen_pending")}
            tone="review"
            value={workMailBrief?.seen_unreviewed ?? 0}
          />
        ) : null}
        {(workMailBrief?.waiting ?? 0) > 0 ? (
          <WorkMailPulseButton
            active={activeView === "all_work" && reviewFilter === "waiting"}
            label="Waiting"
            onClick={() => handleBriefShortcut("waiting")}
            tone="handled"
            value={workMailBrief?.waiting ?? 0}
          />
        ) : null}
        {(workMailBrief?.deferred ?? 0) > 0 ? (
          <WorkMailPulseButton
            active={activeView === "all_work" && reviewFilter === "deferred"}
            label="Deferred"
            onClick={() => handleBriefShortcut("deferred")}
            tone="quiet"
            value={workMailBrief?.deferred ?? 0}
          />
        ) : null}
      </section>

      {/* ── Notices ── */}
      {message ? <div className="mb-3 notice notice-success">{message}</div> : null}
      {error ? (
        <div className="mb-3 notice notice-error space-y-2">
          <p>{error}</p>
          {needsGmailReconnect(error) ? (
            <button className="btn bg-white" disabled={syncing} onClick={() => void handleReconnectGmail()} type="button">
              {syncing ? <Loader2 className="animate-spin" size={15} /> : <Mail size={15} />}
              Reconnect Gmail
            </button>
          ) : null}
        </div>
      ) : null}

      {/* Work Mail workspace */}
      <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">

        <div className="border-b border-zinc-100 px-4 py-3">
          <div className="flex flex-wrap items-center gap-2">
            <div className="relative min-w-[260px] flex-1">
              <Search className="pointer-events-none absolute left-3 top-2.5 text-zinc-400" size={13} />
              <input
                className="w-full rounded-xl border border-zinc-200 bg-zinc-50 py-2 pl-8 pr-8 text-[13px] text-zinc-900 placeholder:text-zinc-400 transition-colors focus:border-zinc-300 focus:bg-white focus:outline-none"
                onChange={(event) => setQuery(event.currentTarget.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void handleSearch();
                }}
                placeholder="Search work mail..."
                value={query}
              />
              {query ? (
                <button
                  className="absolute right-2.5 top-2.5 text-zinc-400 hover:text-zinc-600"
                  onClick={() => { setQuery(""); setActiveSearchQuery(""); setEmailListPage(0); }}
                  type="button"
                >
                  <X size={13} />
                </button>
              ) : null}
            </div>
            <button
              className={[
                "btn h-9 px-3 text-[12px]",
                filtersOpen ? "border-zinc-300 bg-zinc-100 text-zinc-950" : "",
              ].join(" ")}
              onClick={() => setFiltersOpen((value) => !value)}
              type="button"
            >
              <Settings2 size={13} />
              Filters
            </button>
          </div>
          {filtersOpen ? (
            <WorkMailFilterTray
              artifactOnly={artifactOnly}
              attention={attentionFilter}
              messageType={messageTypeFilter}
              onArtifactOnly={setArtifactOnly}
              onAttention={setAttentionFilter}
              onMessageType={setMessageTypeFilter}
              onReview={setReviewFilter}
              onRelevance={setRelevanceFilter}
              onSeenUnreviewedOnly={setSeenUnreviewedOnly}
              onSenderDomain={setSenderDomainFilter}
              onTraceUnseenOnly={setTraceUnseenOnly}
              onUnreadOnly={setUnreadOnly}
              review={reviewFilter}
              relevance={relevanceFilter}
              seenUnreviewedOnly={seenUnreviewedOnly}
              senderDomain={senderDomainFilter}
              traceUnseenOnly={traceUnseenOnly}
              unreadOnly={unreadOnly}
            />
          ) : null}
        </div>

        {/* Built-in Work Mail views */}
        <div className="overflow-x-auto border-b border-zinc-100">
          <div className="relative flex min-w-max px-2">
            {workMailViews.map((view) => {
              const count =
                workMailCounts?.counts.find((item) => item.view === view.id)?.count ?? 0;
              const isActive = activeView === view.id;
              return (
                <button
                  className={[
                    "relative flex shrink-0 items-center gap-1.5 px-3 py-3 text-[12px] font-medium whitespace-nowrap transition-colors",
                    isActive ? "text-zinc-950" : "text-zinc-400 hover:text-zinc-700",
                  ].join(" ")}
                  key={view.id}
                  onClick={() => handleViewChange(view.id)}
                  type="button"
                >
                  {view.label}
                  {count > 0 && (
                    <span className={isActive ? "text-zinc-400" : "text-zinc-300"}>{count}</span>
                  )}
                  {isActive && (
                    <motion.div
                      className="absolute bottom-0 left-0 right-0 h-0.5 rounded-full bg-zinc-950"
                      layoutId="email-tab-underline"
                      transition={{ type: "spring", stiffness: 380, damping: 32 }}
                    />
                  )}
                </button>
              );
            })}
          </div>
        </div>

        {/* View body */}
        {activeView === "agent_activity" ? (
          <WorkMailActivityFeed events={workMailActivity} />
        ) : loading ? (
          <div className="divide-y divide-zinc-50">
            {Array.from({ length: 7 }).map((_, i) => (
              <div className="border-l-2 border-l-transparent px-6 py-4" key={i}>
                <div className="mb-2.5 flex items-center gap-2">
                  <div className="h-4 w-14 animate-pulse rounded-xl bg-zinc-100" />
                  <div className="h-4 w-2/5 animate-pulse rounded-xl bg-zinc-100" />
                </div>
                <div className="mb-2 h-3 w-4/5 animate-pulse rounded-xl bg-zinc-100" />
                <div className="h-3 w-1/3 animate-pulse rounded-xl bg-zinc-100" />
              </div>
            ))}
          </div>
        ) : threads.length === 0 ? (
          <EmptyState
            variant="inline"
            icon={Mail}
            title="No synced threads yet"
            description="Hit Sync to pull your latest email."
          />
        ) : (
          <>
            <AnimatePresence mode="wait">
              <motion.div
                key={activeView}
                animate={{ opacity: 1, y: 0 }}
                className="divide-y divide-zinc-50"
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 4 }}
                transition={{ duration: 0.14 }}
              >
                {pagedSections.map((section) => (
                  <section key={section.key}>
                    {section.title ? (
                      <div className="border-y border-zinc-100 bg-zinc-50/60 px-5 py-2 text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
                        {section.title}
                      </div>
                    ) : null}
                    {section.groups.map((group) => (
                      <BundleGroup
                        activeView={activeView}
                        expanded={
                          !!group.bundleId && expandedBundles.has(group.bundleId)
                        }
                        key={group.parent.thread_id}
                        onSelect={(threadId) =>
                          navigate(`/email?view=${activeView}&thread=${threadId}`)
                        }
                        onScopeAction={(threadId, action) =>
                          void handleScopeAction(threadId, action)
                        }
                        onToggleExpand={() => {
                          if (group.bundleId) toggleBundle(group.bundleId);
                        }}
                        parent={group.parent}
                        siblings={group.siblings}
                        stakeholderByEmail={stakeholderByEmail}
                      />
                    ))}
                  </section>
                ))}
              </motion.div>
            </AnimatePresence>

            {totalPages > 1 && (
              <div className="flex flex-wrap items-center justify-center gap-1 border-t border-zinc-100 px-6 py-4">
                {Array.from({ length: totalPages }, (_, i) => (
                  <button
                    className={[
                      "h-7 min-w-[28px] rounded-lg px-2 text-xs font-medium transition-colors",
                      i === emailListPage
                        ? "bg-zinc-950 text-white"
                        : "text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800",
                    ].join(" ")}
                    key={i}
                    onClick={() => setEmailListPage(i)}
                    type="button"
                  >
                    {i + 1}
                  </button>
                ))}
                <span className="ml-2 text-[11px] text-zinc-400">
                  {emailListPage * EMAIL_LIST_PAGE_SIZE + 1}–
                  {Math.min(
                    (emailListPage + 1) * EMAIL_LIST_PAGE_SIZE,
                    groupedThreads.length,
                  )}{" "}
                  of {groupedThreads.length}
                  {threads.length > groupedThreads.length
                    ? ` (${threads.length} threads in ${groupedThreads.length} bundles)`
                    : ""}
                </span>
              </div>
            )}
          </>
        )}
      </section>

      <EmailDrawer
        activePanel={activePanel}
        ai={null}
        detail={null}
        digest={digest}
        drafts={drafts}
        edges={edges}
        initiatives={initiatives}
        deliverables={deliverables}
        selectedDeliverableId={selectedDeliverableId}
        selectedInitiativeId={selectedInitiativeId}
        triage={null}
        workSuggestions={[]}
        intakeLoading={intakeLoading}
        onAnalyze={() => {}}
        onApproveDeliverable={() => {}}
        onApproveTask={() => {}}
        onApproveWorkSuggestion={() => {}}
        onClose={() => setActivePanel(null)}
        onDismissWorkSuggestion={() => {}}
        onLinkDeliverable={() => {}}
        onLinkInitiative={() => {}}
        onOpenAttachment={setOpenedAttachment}
        onRefreshWorkspaceIntake={() => void handleGenerateWorkspaceIntake()}
        onRunThreadIntake={() => {}}
        onSetDeliverable={setSelectedDeliverableId}
        onSetInitiative={setSelectedInitiativeId}
        onTriageAction={() => {}}
      />

      {settings ? (
        <SettingsDialog
          onClose={() => setSettingsOpen(false)}
          onSave={(next) => void handleSaveSettings(next)}
          open={settingsOpen}
          settings={settings}
        />
      ) : null}
    </div>
  );
}

type EmailPanel = "work" | "assets" | "drafts" | "digest" | "graph" | "ai";


interface ThreadGroup {
  parent: GmailLocalThread;
  siblings: GmailLocalThread[];
  bundleId: string | null;
}

/// Collapse threads sharing the same bundle_id into one group. Threads
/// without a bundle_id (or with bundle_size <= 1) become singleton groups.
/// The first thread seen per bundle becomes the parent — list order is
/// preserved upstream so this is the most recent thread in the bundle.
function groupThreadsByBundle(threads: GmailLocalThread[]): ThreadGroup[] {
  const groups: ThreadGroup[] = [];
  const bundleIndex = new Map<string, number>();
  for (const thread of threads) {
    const bundleId = thread.bundle_id;
    const grouped = bundleId != null && thread.bundle_size > 1;
    if (grouped) {
      const existing = bundleIndex.get(bundleId);
      if (existing != null) {
        groups[existing].siblings.push(thread);
        continue;
      }
      bundleIndex.set(bundleId, groups.length);
    }
    groups.push({
      parent: thread,
      siblings: [],
      bundleId: grouped ? bundleId : null,
    });
  }
  return groups;
}

function groupWorkMailSections(groups: ThreadGroup[], view: WorkMailViewId) {
  if (!["projects", "deliverables", "stakeholders"].includes(view)) {
    return [{ key: "feed", title: null as string | null, groups }];
  }
  const sections = new Map<
    string,
    { key: string; title: string | null; groups: ThreadGroup[] }
  >();
  for (const group of groups) {
    const title =
      view === "projects"
        ? group.parent.linked_initiatives[0]?.title
        : view === "deliverables"
          ? group.parent.linked_deliverables[0]?.title
          : group.parent.linked_stakeholders[0]?.name;
    const key = title || `${view}-unresolved`;
    const section = sections.get(key) ?? {
      key,
      title: title || "Unresolved work object",
      groups: [],
    };
    section.groups.push(group);
    sections.set(key, section);
  }
  return [...sections.values()];
}

function BundleGroup({
  activeView,
  expanded,
  onSelect,
  onScopeAction,
  onToggleExpand,
  parent,
  siblings,
  stakeholderByEmail,
}: {
  activeView: WorkMailViewId;
  expanded: boolean;
  onSelect: (threadId: string) => void;
  onScopeAction: (threadId: string, action: "exclude" | "restore" | "promote") => void;
  onToggleExpand: () => void;
  parent: GmailLocalThread;
  siblings: GmailLocalThread[];
  stakeholderByEmail: Map<string, Stakeholder>;
}) {
  const hasSiblings = siblings.length > 0;
  return (
    <div className="relative">
      <ThreadListItem
        bundleExtra={
          hasSiblings
            ? {
                count: siblings.length + 1,
                expanded,
                onToggle: onToggleExpand,
              }
            : undefined
        }
        onSelect={() => onSelect(parent.thread_id)}
        onScopeAction={onScopeAction}
        selected={false}
        stakeholderByEmail={stakeholderByEmail}
        thread={parent}
        view={activeView}
      />
      {hasSiblings && expanded && (
        <div className="bg-zinc-50/40">
          {siblings.map((sibling) => (
            <div
              className="border-l-2 border-zinc-200 pl-3"
              key={sibling.thread_id}
            >
              <ThreadListItem
                onSelect={() => onSelect(sibling.thread_id)}
                onScopeAction={onScopeAction}
                selected={false}
                stakeholderByEmail={stakeholderByEmail}
                thread={sibling}
                view={activeView}
              />
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function ThreadListItem({
  bundleExtra,
  onSelect,
  onScopeAction,
  selected,
  stakeholderByEmail,
  thread,
  view,
}: {
  bundleExtra?: { count: number; expanded: boolean; onToggle: () => void };
  onSelect: () => void;
  onScopeAction: (threadId: string, action: "exclude" | "restore" | "promote") => void;
  selected: boolean;
  stakeholderByEmail: Map<string, Stakeholder>;
  thread: GmailLocalThread;
  view: WorkMailViewId;
}) {
  const navigate = useNavigate();
  const threadStakeholders = thread.participants
    .map((p) => stakeholderByEmail.get(p.email.toLowerCase()))
    .filter((s): s is Stakeholder => s !== undefined);

  const parsed = thread.ai_title ? parseAiTitle(thread.ai_title) : null;
  const aiLabel = parsed?.label ?? null;
  const snip = threadListSnippet(thread);
  const senderName = thread.last_from_name || thread.last_from_email || "?";
  const workRelations = [
    thread.linked_initiatives[0]?.title,
    thread.linked_deliverables[0]?.title,
    threadStakeholders.length === 0 ? thread.linked_stakeholders[0]?.name : null,
  ].filter(Boolean) as string[];
  const meaning = thread.summary?.trim() || snip.text;
  const scopeReason = thread.work_relevance_reasons[0];
  const showScopeReason = view === "excluded" || view === "unlinked";
  const rowSignals: Array<{ key: string; label: string; className: string }> = [];

  if (!["conversation", "other"].includes(thread.message_type)) {
    rowSignals.push({
      key: "type",
      label: humanizeWorkMailValue(thread.message_type),
      className: "bg-zinc-100 text-zinc-600",
    });
  }
  if (thread.new_since_review) {
    rowSignals.push({
      key: "review",
      label: "New",
      className: "bg-rose-50 text-rose-700",
    });
  } else if (thread.has_unread) {
    rowSignals.push({
      key: "review",
      label: "Unread",
      className: "bg-sky-50 text-sky-700",
    });
  } else if (thread.trace_review_state === "unreviewed" && thread.trace_seen_at) {
    rowSignals.push({
      key: "review",
      label: "Seen",
      className: "bg-amber-50 text-amber-700",
    });
  } else if (
    ["waiting", "deferred", "replied", "resolved"].includes(thread.trace_review_state)
  ) {
    rowSignals.push({
      key: "review",
      label: humanizeWorkMailValue(thread.trace_review_state),
      className:
        thread.trace_review_state === "waiting"
          ? "bg-blue-50 text-blue-700"
          : "bg-emerald-50 text-emerald-700",
    });
  }
  if (thread.needs_me_reason && thread.attention_state === "needs_me") {
    rowSignals.push({
      key: "attention",
      label: "Action",
      className: "bg-rose-50 text-rose-700",
    });
  } else if (thread.needs_me_reason && thread.attention_state === "review") {
    rowSignals.push({
      key: "attention",
      label: "Review",
      className: "bg-amber-50 text-amber-700",
    });
  }
  if (
    thread.needs_me_reason &&
    ["high", "urgent"].includes(thread.effective_priority)
  ) {
    rowSignals.push({
      key: "impact",
      label: humanizeWorkMailValue(thread.effective_priority),
      className: priorityBadgeColor(thread.effective_priority),
    });
  }

  const hasBadgeRow =
    threadStakeholders.length > 0 ||
    rowSignals.length > 0 ||
    workRelations.length > 0 ||
    !!aiLabel;

  return (
    <button
      className={[
        "group flex w-full gap-4 border-b border-zinc-100 border-l-2 px-5 py-3.5 text-left transition-colors",
        thread.has_unread ? "border-l-sky-400" : "border-l-transparent",
        selected ? "bg-sky-50 hover:bg-sky-50" : "hover:bg-zinc-50",
      ].join(" ")}
      onClick={onSelect}
      type="button"
    >
      {/* Sender avatar */}
      <div className="shrink-0 pt-0.5">
        <Avatar name={senderName} size="sm" />
      </div>

      {/* Content */}
      <div className="min-w-0 flex-1">

        {/* Row 1 — title + date */}
        <div className="flex items-start justify-between gap-3">
          <h2
            className={[
              "truncate text-sm font-semibold leading-snug",
              selected
                ? "text-sky-900"
                : thread.has_unread
                  ? "text-zinc-950"
                  : "text-zinc-700",
            ].join(" ")}
          >
            {threadListTitle(thread)}
          </h2>
          <div className="flex shrink-0 items-center gap-2">
            {view === "excluded" ? (
              <button
                className="rounded-md border border-emerald-100 bg-emerald-50 px-2 py-1 text-[10px] font-semibold text-emerald-700 hover:bg-emerald-100"
                onClick={(event) => {
                  event.stopPropagation();
                  onScopeAction(thread.thread_id, "restore");
                }}
                title="Restore this thread into Work Mail"
                type="button"
              >
                Restore
              </button>
            ) : (
              <button
                className="rounded-md border border-zinc-100 bg-white px-2 py-1 text-[10px] font-semibold text-zinc-500 opacity-0 transition-opacity hover:bg-zinc-100 group-hover:opacity-100"
                onClick={(event) => {
                  event.stopPropagation();
                  onScopeAction(thread.thread_id, "exclude");
                }}
                title="Exclude this thread from Work Mail"
                type="button"
              >
                Exclude
              </button>
            )}
            {bundleExtra && (
              <button
                aria-expanded={bundleExtra.expanded}
                aria-label={`${bundleExtra.expanded ? "Collapse" : "Expand"} bundle of ${bundleExtra.count} threads`}
                className="inline-flex items-center gap-1 rounded-full bg-violet-50 px-2 py-0.5 text-[10px] font-semibold text-violet-700 transition-colors hover:bg-violet-100"
                onClick={(e) => {
                  e.stopPropagation();
                  bundleExtra.onToggle();
                }}
                title={`${bundleExtra.count} related threads`}
                type="button"
              >
                <span>{bundleExtra.count} threads</span>
                {bundleExtra.expanded ? (
                  <ChevronDown size={10} />
                ) : (
                  <ChevronRight size={10} />
                )}
              </button>
            )}
            <span className="text-[11px] text-zinc-400 tabular-nums">
              {thread.last_message_at ? compactDate(thread.last_message_at) : ""}
            </span>
          </div>
        </div>

        {/* Row 2 — work meaning */}
        <p className="mt-1 truncate text-xs">
          <span className="font-medium text-zinc-500">{senderName}</span>
          {meaning && (
            <span className="text-zinc-400">
              {" - "}
              {snip.isAi && <span className="text-violet-400">✦ </span>}
              {meaning}
            </span>
          )}
        </p>
        {showScopeReason && scopeReason ? (
          <p className="mt-1 truncate text-[11px] text-zinc-400">
            Why here: {scopeReason}
          </p>
        ) : null}

        {/* Row 3 — work relation + attention */}
        {hasBadgeRow && (
          <div className="mt-2.5 flex items-center justify-between gap-2">
            <div className="flex min-w-0 flex-wrap items-center gap-2">
              {threadStakeholders.length > 0 && (
                <ThreadStakeholderChipsEmail
                  onNavigate={(id) => navigate(`/stakeholders/${id}`)}
                  stakeholders={threadStakeholders}
                />
              )}
              {workRelations.slice(0, 1).map((relation) => (
                <span
                  className="max-w-[190px] truncate rounded-md bg-sky-50 px-2 py-0.5 text-[10px] font-medium text-sky-700"
                  key={relation}
                  title={relation}
                >
                  {relation}
                </span>
              ))}
              {rowSignals.map((signal) => (
                <span
                  className={`rounded-md px-2 py-0.5 text-[10px] font-medium ${signal.className}`}
                  key={signal.key}
                >
                  {signal.label}
                </span>
              ))}
            </div>
            {aiLabel && <AiLabelBadge label={aiLabel} size="xs" />}
          </div>
        )}
      </div>
    </button>
  );
}

function WorkMailPulseButton({
  active,
  label,
  onClick,
  tone,
  value,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
  tone: "attention" | "handled" | "review" | "quiet";
  value: number;
}) {
  const toneClass = {
    attention: "border-rose-100 bg-rose-50 text-rose-800",
    handled: "border-emerald-100 bg-emerald-50 text-emerald-800",
    review: "border-amber-100 bg-amber-50 text-amber-800",
    quiet: "border-zinc-200 bg-white text-zinc-700",
  }[tone];
  return (
    <button
      className={[
        "inline-flex h-8 items-center gap-2 rounded-lg border px-2.5 text-[12px] font-medium transition-colors",
        toneClass,
        active ? "ring-1 ring-zinc-400" : "hover:border-zinc-300",
      ].join(" ")}
      onClick={onClick}
      type="button"
    >
      <span>{label}</span>
      <span className="rounded-md bg-white/70 px-1.5 py-0.5 font-semibold tabular-nums">
        {value}
      </span>
    </button>
  );
}

function WorkMailReviewControls({
  onGmailRead,
  onGmailUnread,
  onReopen,
  onReviewState,
  thread,
}: {
  onGmailRead: () => void;
  onGmailUnread: () => void;
  onReopen: () => void;
  onReviewState: (state: WorkMailReviewState) => void;
  thread: GmailLocalThread;
}) {
  const state = thread.trace_review_state;
  const handled = state !== "unreviewed";
  return (
    <section className="mt-5 border-y border-zinc-100 py-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0">
          <p className="text-[10px] font-bold uppercase tracking-wider text-zinc-400">
            Work Mail review
          </p>
          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            <span className={thread.has_unread ? "rounded-md bg-sky-50 px-2 py-1 text-xs font-medium text-sky-700" : "rounded-md bg-zinc-100 px-2 py-1 text-xs font-medium text-zinc-600"}>
              {thread.has_unread ? "Unread in Gmail" : "Read in Gmail"}
            </span>
            <span className={thread.trace_seen_at ? "rounded-md bg-amber-50 px-2 py-1 text-xs font-medium text-amber-700" : "rounded-md bg-zinc-100 px-2 py-1 text-xs font-medium text-zinc-600"}>
              {thread.trace_seen_at ? "Seen in Trace" : "Unseen in Trace"}
            </span>
            <span className={handled ? "rounded-md bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700" : "rounded-md bg-zinc-100 px-2 py-1 text-xs font-medium text-zinc-600"}>
              {humanizeWorkMailValue(state)}
            </span>
            {thread.new_since_review ? (
              <span className="rounded-md bg-rose-50 px-2 py-1 text-xs font-medium text-rose-700">
                New since review
              </span>
            ) : null}
          </div>
          {thread.needs_me_reason ? (
            <p className="mt-2 text-xs text-zinc-500">{thread.needs_me_reason}</p>
          ) : null}
        </div>
        <div className="flex flex-wrap items-center justify-end gap-1.5">
          <button className="btn h-8 px-2.5 text-[12px]" onClick={() => onReviewState("reviewed")} type="button">
            <Check size={13} />
            Mark reviewed
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={() => onReviewState("deferred")} type="button">
            Defer
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={() => onReviewState("waiting")} type="button">
            Waiting on them
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={() => onReviewState("resolved")} type="button">
            Resolve
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={onReopen} type="button">
            Reopen
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={onGmailRead} type="button">
            Mark read in Gmail
          </button>
          <button className="btn h-8 px-2.5 text-[12px]" onClick={onGmailUnread} type="button">
            Mark unread in Gmail
          </button>
        </div>
      </div>
    </section>
  );
}

function WorkMailFilterTray({
  artifactOnly,
  attention,
  messageType,
  onArtifactOnly,
  onAttention,
  onMessageType,
  onReview,
  onRelevance,
  onSeenUnreviewedOnly,
  onSenderDomain,
  onTraceUnseenOnly,
  onUnreadOnly,
  review,
  relevance,
  seenUnreviewedOnly,
  senderDomain,
  traceUnseenOnly,
  unreadOnly,
}: {
  artifactOnly: boolean;
  attention: WorkMailAttentionState | "";
  messageType: WorkMailMessageType | "";
  onArtifactOnly: (value: boolean) => void;
  onAttention: (value: WorkMailAttentionState | "") => void;
  onMessageType: (value: WorkMailMessageType | "") => void;
  onReview: (value: WorkMailReviewState | "") => void;
  onRelevance: (value: WorkMailRelevance | "") => void;
  onSeenUnreviewedOnly: (value: boolean) => void;
  onSenderDomain: (value: string) => void;
  onTraceUnseenOnly: (value: boolean) => void;
  onUnreadOnly: (value: boolean) => void;
  review: WorkMailReviewState | "";
  relevance: WorkMailRelevance | "";
  seenUnreviewedOnly: boolean;
  senderDomain: string;
  traceUnseenOnly: boolean;
  unreadOnly: boolean;
}) {
  return (
    <div className="mt-3 grid gap-2 border-t border-zinc-100 pt-3 md:grid-cols-2 xl:grid-cols-7">
      <label className="space-y-1 xl:col-span-2">
        <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-400">
          Sender domain
        </span>
        <input
          className="w-full rounded-lg border border-zinc-200 bg-white px-2.5 py-2 text-[12px]"
          onChange={(event) => onSenderDomain(event.currentTarget.value)}
          placeholder="company.example"
          value={senderDomain}
        />
      </label>
      <WorkMailSelect
        label="Attention"
        onChange={(value) => onAttention(value as WorkMailAttentionState | "")}
        options={["needs_me", "waiting", "review", "fyi", "scheduled", "resolved"]}
        value={attention}
      />
      <WorkMailSelect
        label="Type"
        onChange={(value) => onMessageType(value as WorkMailMessageType | "")}
        options={[
          "conversation",
          "file_share",
          "meeting",
          "announcement",
          "notification",
          "newsletter",
          "promotion",
          "receipt",
          "system",
          "other",
        ]}
        value={messageType}
      />
      <WorkMailSelect
        label="Scope"
        onChange={(value) => onRelevance(value as WorkMailRelevance | "")}
        options={["work", "linked_external", "promoted", "excluded", "non_work", "unknown"]}
        value={relevance}
      />
      <WorkMailSelect
        label="Review"
        onChange={(value) => onReview(value as WorkMailReviewState | "")}
        options={["unreviewed", "reviewed", "deferred", "waiting", "resolved", "replied"]}
        value={review}
      />
      <div className="flex flex-wrap items-end gap-3 pb-2 text-[12px] text-zinc-600">
        <label className="inline-flex items-center gap-1.5">
          <input
            checked={unreadOnly}
            onChange={(event) => onUnreadOnly(event.currentTarget.checked)}
            type="checkbox"
          />
          Unread
        </label>
        <label className="inline-flex items-center gap-1.5">
          <input
            checked={traceUnseenOnly}
            onChange={(event) => onTraceUnseenOnly(event.currentTarget.checked)}
            type="checkbox"
          />
          Unseen
        </label>
        <label className="inline-flex items-center gap-1.5">
          <input
            checked={seenUnreviewedOnly}
            onChange={(event) => onSeenUnreviewedOnly(event.currentTarget.checked)}
            type="checkbox"
          />
          Seen not reviewed
        </label>
        <label className="inline-flex items-center gap-1.5">
          <input
            checked={artifactOnly}
            onChange={(event) => onArtifactOnly(event.currentTarget.checked)}
            type="checkbox"
          />
          Files
        </label>
      </div>
    </div>
  );
}

function WorkMailSelect({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: string[];
  value: string;
}) {
  return (
    <label className="space-y-1">
      <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-400">
        {label}
      </span>
      <select
        className="w-full rounded-lg border border-zinc-200 bg-white px-2.5 py-2 text-[12px]"
        onChange={(event) => onChange(event.currentTarget.value)}
        value={value}
      >
        <option value="">All</option>
        {options.map((option) => (
          <option key={option} value={option}>
            {humanizeWorkMailValue(option)}
          </option>
        ))}
      </select>
    </label>
  );
}

function WorkMailActivityFeed({ events }: { events: WorkMailAgentEvent[] }) {
  if (events.length === 0) {
    return (
      <EmptyState
        variant="inline"
        icon={Sparkles}
        title="No Work Mail activity yet"
        description="Trace placement changes, corrections, rules, and scope updates will appear here."
      />
    );
  }
  return (
    <div className="divide-y divide-zinc-100">
      {events.map((event) => (
        <article className="flex gap-3 px-5 py-4" key={event.id}>
          <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-violet-50 text-violet-700">
            <Sparkles size={15} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-2">
              <p className="text-sm font-semibold text-zinc-900">{event.summary}</p>
              <span className="rounded-md bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-zinc-500">
                {event.actor}
              </span>
              <span className="rounded-md bg-zinc-50 px-2 py-0.5 text-[10px] font-semibold text-zinc-500">
                {humanizeWorkMailValue(event.event_kind)}
              </span>
            </div>
            <p className="mt-1 text-xs text-zinc-500">
              {formatDateTime(event.created_at)}
              {event.thread_id ? ` - thread ${event.thread_id}` : ""}
            </p>
            {Array.isArray(event.reason) && event.reason.length > 0 ? (
              <p className="mt-1 truncate text-xs text-zinc-500">
                {event.reason.map(String).join(" ")}
              </p>
            ) : null}
          </div>
        </article>
      ))}
    </div>
  );
}

function humanizeWorkMailValue(value: string) {
  return value
    .replace(/_/g, " ")
    .replace(/\b\w/g, (character) => character.toUpperCase());
}


function ThreadStakeholderChipsEmail({
  onNavigate,
  stakeholders,
}: {
  onNavigate: (id: string) => void;
  stakeholders: Stakeholder[];
}) {
  const visible = stakeholders.slice(0, 3);
  const overflow = stakeholders.length - visible.length;
  return (
    <div className="flex items-center">
      {visible.map((s, i) => (
        <div
          className="group/chip relative cursor-pointer transition-transform hover:scale-110 hover:z-10"
          key={s.id}
          onClick={(e) => {
            e.stopPropagation();
            onNavigate(s.id);
          }}
          style={{ marginLeft: i === 0 ? 0 : "-5px", zIndex: visible.length - i }}
          title={s.name}
        >
          <div
            className={`flex h-[20px] w-[20px] items-center justify-center rounded-full border-2 border-white text-[8px] font-bold ${avatarColor(s.name).bg} ${avatarColor(s.name).text}`}
          >
            {avatarInitialsShared(s.name)}
          </div>
          <div className="pointer-events-none absolute bottom-full left-1/2 z-50 mb-1.5 -translate-x-1/2 whitespace-nowrap rounded bg-zinc-900 px-2 py-1 text-[10px] font-medium text-white opacity-0 shadow-sm transition-opacity group-hover/chip:opacity-100">
            {s.name}
          </div>
        </div>
      ))}
      {overflow > 0 && (
        <div
          className="flex h-[20px] w-[20px] items-center justify-center rounded-full border-2 border-white bg-zinc-100 text-[8px] font-bold text-zinc-500"
          style={{ marginLeft: "-5px" }}
        >
          +{overflow}
        </div>
      )}
    </div>
  );
}

function ThreadToolbar({
  analyzing,
  detail,
  onAnalyze,
  onArchive,
  onAssets,
  onCapture,
  onCompose,
  onMarkImportant,
  onMoveToSpam,
  onOpenWork,
  onStar,
  threadAction,
}: {
  analyzing: boolean;
  detail: GmailThreadDetail;
  onAnalyze: () => void;
  onArchive: () => void;
  onAssets: () => void;
  onCapture: () => void;
  onCompose: () => void;
  onMarkImportant: () => void;
  onMoveToSpam: () => void;
  onOpenWork: () => void;
  onStar: () => void;
  threadAction: "archive" | "spam" | null;
}) {
  return (
    <div className="sticky top-0 z-10 flex min-h-12 items-center justify-between gap-3 border-b border-zinc-200 bg-white/95 px-5 py-2 backdrop-blur">
      <div className="flex min-w-0 items-center gap-1 overflow-x-auto">
        <ToolbarButton disabled={threadAction !== null} icon={threadAction === "archive" ? <Loader2 className="animate-spin" size={15} /> : <Archive size={15} />} label="Archive" onClick={onArchive} />
        <ToolbarButton disabled={threadAction !== null} icon={threadAction === "spam" ? <Loader2 className="animate-spin" size={15} /> : <CircleAlert size={15} />} label="Spam" onClick={onMoveToSpam} />
        <ToolbarButton icon={<Star size={15} />} label="Priority" onClick={onStar} />
        <ToolbarButton icon={<Check size={15} />} label="Important" onClick={onMarkImportant} />
        <span className="mx-1 h-6 w-px bg-zinc-200" />
        <ToolbarButton icon={<Inbox size={15} />} label="Capture" onClick={onCapture} />
        <ToolbarButton icon={<BriefcaseBusiness size={15} />} label="Work" onClick={onOpenWork} />
        <ToolbarButton icon={<Paperclip size={15} />} label="Files" onClick={onAssets} />
      </div>
      <div className="flex shrink-0 items-center gap-2">
        <button
          aria-label="Analyse with AI"
          className="flex h-9 items-center gap-1.5 rounded-lg border border-violet-200 bg-white px-3 text-[13px] font-semibold text-violet-700 transition-colors hover:border-violet-300 hover:bg-violet-50 disabled:cursor-not-allowed disabled:opacity-60"
          onClick={onAnalyze}
          type="button"
        >
          {analyzing ? (
            <Loader2 className="animate-spin" size={14} />
          ) : (
            <Sparkles size={14} />
          )}
          Analyse with AI
        </button>
        <button className="btn btn-primary h-9 px-3" onClick={onCompose} type="button">
          <Send size={15} />
          Reply
        </button>
      </div>
      <span className="sr-only">{threadDetailTitle(detail)}</span>
    </div>
  );
}

function ToolbarButton({
  disabled,
  icon,
  label,
  onClick,
}: {
  disabled?: boolean;
  icon: ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button className="btn h-9 w-9 shrink-0 px-0" disabled={disabled} onClick={onClick} title={label} type="button" aria-label={label}>
      {icon}
    </button>
  );
}

function ThreadTitleBlock({
  detail,
  onCreateStakeholder,
  ownerEmail,
  ownerName,
  stakeholderByEmail,
}: {
  detail: GmailThreadDetail;
  onCreateStakeholder: (address: EmailAddress) => void;
  ownerEmail?: string;
  ownerName?: string | null;
  stakeholderByEmail: Map<string, Stakeholder>;
}) {
  const thread = detail.thread;
  const parsed = thread.ai_title ? parseAiTitle(thread.ai_title) : null;
  const msgCount = thread.message_count;
  const isSent = thread.labels.some(
    (l) => (l.name || "").toUpperCase() === "SENT",
  );
  const folderLabel = isSent ? "Sent" : "Received";
  const isImportant = thread.labels.some(
    (l) => (l.name || "").toUpperCase() === "IMPORTANT",
  );
  // System labels (Sent / Inbox / Important / Starred) are surfaced via
  // dedicated affordances elsewhere — keep them out of the pill row.
  const visibleLabels = thread.labels.filter((label) => {
    const upper = (label.name || "").toUpperCase();
    return (
      upper !== "SENT" &&
      upper !== "INBOX" &&
      upper !== "IMPORTANT" &&
      upper !== "STARRED"
    );
  });
  const sentiment = thread.sentiment?.trim().toLowerCase();
  const showSentiment = sentiment && sentiment !== "neutral";
  const urgency = thread.urgency?.trim().toLowerCase() || null;
  return (
    <div>
      <div className="flex items-center gap-2">
        <p className="page-kicker">Relevant email</p>
        {parsed?.label && <AiLabelBadge label={parsed.label} />}
      </div>
      <h2 className="mt-1 break-words text-2xl font-semibold text-zinc-950">{threadDetailTitle(detail)}</h2>
      {thread.ai_title && usableSubject(thread.subject) && parsed?.title !== usableSubject(thread.subject) ? (
        <p className="mt-1 flex items-center gap-1.5 truncate text-sm text-zinc-400">
          {isImportant ? (
            <ChevronsRight
              aria-label="Marked important in Gmail"
              className="shrink-0 text-amber-500"
              size={14}
            />
          ) : null}
          <span className="truncate">{usableSubject(thread.subject)}</span>
        </p>
      ) : isImportant ? (
        <p className="mt-1 inline-flex items-center gap-1 text-xs text-amber-600">
          <ChevronsRight
            aria-label="Marked important in Gmail"
            className="shrink-0 text-amber-500"
            size={13}
          />
          Marked important
        </p>
      ) : null}
      <div className="mt-3 flex flex-wrap items-center gap-x-4 gap-y-2 text-xs text-zinc-500">
        <span className="inline-flex items-center gap-1.5"><UsersRound size={13} />{thread.participants.length} {thread.participants.length === 1 ? "person" : "people"}</span>
        <span className="inline-flex items-center gap-1.5"><Mail size={13} />{msgCount} {msgCount === 1 ? "message" : "messages"}</span>
        <span className="inline-flex items-center gap-1.5"><CalendarDays size={13} />{thread.last_message_at ? formatUnix(thread.last_message_at) : "No date"}</span>
        <span className="text-zinc-400">· {folderLabel}</span>
      </div>
      <div className="mt-4 flex flex-wrap items-center gap-1.5">
        <span className={`rounded-md px-2 py-0.5 text-[11px] font-semibold ${triageTone(thread.ai_category)}`}>
          {categoryLabel(thread.ai_category)}
        </span>
        <PriorityUrgencyStrip
          impact={thread.effective_priority}
          impactTitle={thread.priority_reasons.join(" ")}
          urgency={urgency}
        />
        {visibleLabels.map((label) => {
          const name = cleanLabelName(label.name);
          return name ? <MiniBadge key={label.gmail_label_id}>{name}</MiniBadge> : null;
        })}
        {showSentiment ? (
          <span className={`rounded-md px-2 py-0.5 text-[11px] font-semibold ${sentimentTone(sentiment!)}`}>
            {sentiment}
          </span>
        ) : null}
      </div>
      <AddressChips
        addresses={thread.participants}
        onCreateStakeholder={onCreateStakeholder}
        ownerEmail={ownerEmail}
        ownerName={ownerName}
        stakeholderByEmail={stakeholderByEmail}
      />
    </div>
  );
}

function PriorityUrgencyStrip({
  impact,
  impactTitle,
  urgency,
}: {
  impact: string;
  impactTitle?: string;
  urgency: string | null;
}) {
  const valueTone = (level: string | null) => {
    if (!level) return "text-zinc-400";
    const lower = level.toLowerCase();
    if (lower === "urgent" || lower === "high") return "text-rose-600";
    if (lower === "low") return "text-zinc-500";
    return "text-zinc-700";
  };
  return (
    <span
      className="inline-flex items-center gap-2 rounded-md border border-zinc-100 bg-zinc-50 px-2 py-1"
      title={impactTitle}
    >
      <span className="flex items-center gap-1">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">Impact</span>
        <span className={`text-[11px] font-semibold ${valueTone(impact)}`}>{impact}</span>
      </span>
      <span className="h-3 w-px bg-zinc-200" />
      <span className="flex items-center gap-1">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">Urgency</span>
        <span className={`text-[11px] font-semibold ${valueTone(urgency)}`}>{urgency ?? "—"}</span>
      </span>
    </span>
  );
}

function sentimentTone(sentiment: string): string {
  const lower = sentiment.toLowerCase();
  if (lower === "positive") return "bg-emerald-50 text-emerald-700";
  if (lower === "negative") return "bg-rose-50 text-rose-700";
  if (lower === "mixed") return "bg-amber-50 text-amber-700";
  return "bg-zinc-100 text-zinc-600";
}

function AddressChips({
  addresses,
  onCreateStakeholder,
  ownerEmail,
  ownerName,
  stakeholderByEmail,
}: {
  addresses: EmailAddress[];
  onCreateStakeholder?: (address: EmailAddress) => void;
  ownerEmail?: string;
  ownerName?: string | null;
  stakeholderByEmail?: Map<string, Stakeholder>;
}) {
  if (addresses.length === 0) return null;
  const normalizedOwner = ownerEmail?.trim().toLowerCase() ?? "";
  return (
    <div className="mt-3 flex flex-wrap gap-1.5">
      {addresses.slice(0, 12).map((address) => {
        const addressEmail = address.email.toLowerCase();
        // Owner ("You") takes precedence over stakeholder lookup so the user
        // is never shown as a generic participant or stakeholder candidate.
        if (normalizedOwner && addressEmail === normalizedOwner) {
          const label = ownerName || address.name || "You";
          return (
            <Link
              className="inline-flex max-w-56 items-center gap-1.5 rounded-md border border-violet-100 bg-violet-50 px-2 py-1 text-xs font-medium text-violet-700 transition-colors hover:border-violet-200 hover:bg-violet-100"
              key={`${address.email}-owner`}
              title="This is you — open My Profile"
              to="/profile"
            >
              <User size={11} className="shrink-0" />
              <span className="truncate">{label}</span>
              <span className="shrink-0 rounded-sm bg-violet-200/60 px-1 py-px text-[9px] font-bold uppercase tracking-wider text-violet-700">
                You
              </span>
            </Link>
          );
        }
        const stakeholder = stakeholderByEmail?.get(addressEmail);
        if (stakeholder) {
          return (
            <Link
              className="inline-flex max-w-56 items-center gap-1.5 rounded-md border border-sky-100 bg-sky-50 px-2 py-1 text-xs font-medium text-sky-700 transition-colors hover:border-sky-200 hover:bg-sky-100"
              key={`${address.email}-${address.name}`}
              title={`View ${stakeholder.name}'s stakeholder page`}
              to={`/stakeholders/${stakeholder.id}`}
            >
              <User size={11} className="shrink-0" />
              <span className="truncate">{stakeholder.name}</span>
              <ChevronRight size={10} className="shrink-0 opacity-50" />
            </Link>
          );
        }
        return (
          <button
            className="inline-flex max-w-56 items-center gap-1 rounded-md border border-zinc-200 bg-white px-2 py-1 text-xs text-zinc-600 transition-colors hover:border-sky-200 hover:bg-sky-50 hover:text-sky-700"
            key={`${address.email}-${address.name}`}
            onClick={() => onCreateStakeholder?.(address)}
            title={`Add ${address.email} as stakeholder`}
            type="button"
          >
            <AtSign size={11} className="shrink-0" />
            <span className="truncate">{address.name || address.email}</span>
            {onCreateStakeholder ? <Plus size={10} className="shrink-0 opacity-50" /> : null}
          </button>
        );
      })}
    </div>
  );
}

function parseAiTitle(aiTitle: string): { label: string | null; title: string } {
  const match = aiTitle.match(/^\[([^\]]+)\]\s*(.+)$/);
  if (match) return { label: match[1], title: match[2] };
  return { label: null, title: aiTitle };
}

function threadListTitle(thread: GmailLocalThread): string {
  if (thread.ai_title) return parseAiTitle(thread.ai_title).title;
  return (
    usableSubject(thread.subject) ||
    meaningfulPreview(thread.snippet) ||
    thread.last_from_name ||
    thread.last_from_email ||
    "(no subject)"
  );
}

function threadDetailTitle(detail: GmailThreadDetail): string {
  if (detail.thread.ai_title) return parseAiTitle(detail.thread.ai_title).title;
  return (
    usableSubject(detail.thread.subject) ||
    detail.messages.map((message) => usableSubject(message.subject)).find(Boolean) ||
    meaningfulPreview(detail.thread.snippet) ||
    detail.thread.last_from_name ||
    detail.thread.last_from_email ||
    "(no subject)"
  );
}


function cleanSummary(raw: string): string {
  return raw
    .replace(/^\s*[*\-•]\s*/gm, "")
    .replace(/\n+/g, " · ")
    .trim();
}

function threadListSnippet(thread: GmailLocalThread): { text: string; isAi: boolean } {
  if (thread.summary?.trim()) {
    return { text: cleanSummary(thread.summary), isAi: true };
  }
  return { text: meaningfulPreview(thread.snippet) || "No preview available", isAi: false };
}

function usableSubject(value: string) {
  const subject = value.trim();
  if (!subject || subject.toLowerCase() === "(no subject)") {
    return "";
  }
  return subject;
}

/**
 * Subject to use when composing a reply. Uses the real Gmail subject (NOT
 * the AI-parsed display title), strips any existing Re:/Fwd:/Fw: prefix, and
 * prepends a single clean "Re:". Falls back to the first message's subject
 * if the thread-level subject is empty.
 */
function replySubjectFor(detail: GmailThreadDetail): string {
  const raw =
    usableSubject(detail.thread.subject) ||
    detail.messages.map((m) => usableSubject(m.subject)).find(Boolean) ||
    "";
  if (!raw) return "Re:";
  const stripped = raw.replace(/^\s*(re|fwd?|fw)\s*:\s*/i, "").trim();
  return `Re: ${stripped || raw}`;
}

function meaningfulPreview(value: string) {
  return stripHtmlText(value)
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 180);
}

function ThreadMessageStack({
  messages,
  onComposeForEmail,
  onOpenAttachment,
}: {
  messages: GmailMessageRecord[];
  onComposeForEmail: (email: string) => void;
  onOpenAttachment: (url: string) => void;
}) {
  const latestId = messages[messages.length - 1]?.message_id ?? "";
  const messageKey = messages.map((message) => message.message_id).join(":");
  const [expandedIds, setExpandedIds] = useState<Set<string>>(
    () => new Set(latestId ? [latestId] : []),
  );

  useEffect(() => {
    setExpandedIds(new Set(latestId ? [latestId] : []));
  }, [latestId, messageKey]);

  function toggleMessage(messageId: string) {
    setExpandedIds((current) => {
      const next = new Set(current);
      if (next.has(messageId)) next.delete(messageId);
      else next.add(messageId);
      return next;
    });
  }

  return (
    <section className="mt-6 divide-y divide-zinc-200 border-y border-zinc-200">
      {messages.map((item) => {
        const isLatest = item.message_id === latestId;
        const expanded = isLatest || expandedIds.has(item.message_id);
        return (
          <MessageCard
            canCollapse={!isLatest}
            collapsed={!expanded}
            item={item}
            key={item.message_id}
            onComposeForEmail={onComposeForEmail}
            onOpenAttachment={onOpenAttachment}
            onToggle={() => toggleMessage(item.message_id)}
          />
        );
      })}
    </section>
  );
}

function MessageCard({
  canCollapse,
  collapsed,
  item,
  onComposeForEmail,
  onOpenAttachment,
  onToggle,
}: {
  canCollapse: boolean;
  collapsed: boolean;
  item: GmailMessageRecord;
  onComposeForEmail: (email: string) => void;
  onOpenAttachment: (url: string) => void;
  onToggle: () => void;
}) {
  const artifactUrls = item.artifact_urls.filter(isArtifactUrl);
  const sender = item.from_name || item.from_email || "Unknown sender";
  if (collapsed) {
    return (
      <button
        className="group flex w-full items-center gap-3 bg-white py-3 text-left transition-colors hover:bg-zinc-50"
        onClick={onToggle}
        type="button"
      >
        <Avatar name={sender} size="sm" />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 items-center gap-2">
            <p className="truncate text-sm font-semibold text-zinc-800">{sender}</p>
            {item.is_sent ? <MiniBadge>Sent</MiniBadge> : null}
            {item.is_unread ? <MiniBadge>Unread</MiniBadge> : null}
          </div>
          <p className="truncate text-xs text-zinc-500">
            {collapsedMessagePreview(item)}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2 text-zinc-400">
          <span className="text-xs">
            {item.internal_date_ts ? formatUnix(item.internal_date_ts) : ""}
          </span>
          <ChevronRight
            className="transition-transform group-hover:translate-x-0.5"
            size={14}
          />
        </div>
      </button>
    );
  }
  return (
    <article className="bg-white py-5">
      <div className="mb-3 flex items-start gap-3">
        <Avatar name={sender} size="sm" />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-start justify-between gap-3">
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold text-zinc-950">
                {sender}
              </p>
              <p className="truncate text-xs text-zinc-500">{item.from_email}</p>
            </div>
            <div className="flex flex-wrap items-center justify-end gap-2">
              {item.is_sent ? <MiniBadge>Sent</MiniBadge> : null}
              {item.is_unread ? <MiniBadge>Unread</MiniBadge> : null}
              {artifactUrls.length > 0 ? <MiniBadge>Artifact</MiniBadge> : null}
              {canCollapse ? (
                <button
                  className="rounded-md px-1.5 py-0.5 text-xs font-medium text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-700"
                  onClick={onToggle}
                  type="button"
                >
                  Collapse
                </button>
              ) : null}
              <span className="text-xs text-zinc-400">
                {item.internal_date_ts ? formatUnix(item.internal_date_ts) : ""}
              </span>
            </div>
          </div>
          {usableSubject(item.subject) ? (
            <p className="mt-1 truncate text-sm text-zinc-700">{item.subject}</p>
          ) : null}
        </div>
      </div>
      <div className="mb-4 ml-11 grid gap-2 text-xs text-zinc-500 sm:grid-cols-2">
        <RecipientLine label="To" people={item.to} />
        {item.cc.length > 0 ? <RecipientLine label="Cc" people={item.cc} /> : null}
      </div>
      <div className="ml-11">
        <EmailBody
          item={item}
          onComposeForEmail={onComposeForEmail}
          onOpenAttachment={onOpenAttachment}
        />
      </div>
      {artifactUrls.length > 0 ? (
        <div className="ml-11 mt-4">
          <p className="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-zinc-400">
            Attached
          </p>
          <div className="flex flex-wrap gap-2">
            {artifactUrls.slice(0, 6).map((url) => (
              <ArtifactLink
                key={url}
                onOpen={() => onOpenAttachment(url)}
                url={url}
              />
            ))}
          </div>
        </div>
      ) : null}
    </article>
  );
}

interface ArtifactSourceMeta {
  label: string;
  bg: string;
  text: string;
  iconBg: string;
  iconText: string;
}

function getArtifactMeta(url: string): ArtifactSourceMeta {
  const lower = url.toLowerCase();
  if (lower.includes("docs.google.com/document")) {
    return {
      label: "Google Doc",
      bg: "bg-white hover:bg-sky-50",
      text: "text-zinc-800",
      iconBg: "bg-sky-100",
      iconText: "text-sky-700",
    };
  }
  if (lower.includes("docs.google.com/presentation")) {
    return {
      label: "Google Slides",
      bg: "bg-white hover:bg-amber-50",
      text: "text-zinc-800",
      iconBg: "bg-amber-100",
      iconText: "text-amber-700",
    };
  }
  if (lower.includes("docs.google.com/spreadsheets")) {
    return {
      label: "Google Sheet",
      bg: "bg-white hover:bg-emerald-50",
      text: "text-zinc-800",
      iconBg: "bg-emerald-100",
      iconText: "text-emerald-700",
    };
  }
  if (lower.includes("drive.google.com")) {
    return {
      label: "Google Drive",
      bg: "bg-white hover:bg-zinc-50",
      text: "text-zinc-800",
      iconBg: "bg-zinc-100",
      iconText: "text-zinc-700",
    };
  }
  if (lower.includes("figma.com")) {
    return {
      label: "Figma file",
      bg: "bg-white hover:bg-violet-50",
      text: "text-zinc-800",
      iconBg: "bg-violet-100",
      iconText: "text-violet-700",
    };
  }
  if (lower.includes("notion.so") || lower.includes("notion.site")) {
    return {
      label: "Notion page",
      bg: "bg-white hover:bg-zinc-50",
      text: "text-zinc-800",
      iconBg: "bg-zinc-900",
      iconText: "text-white",
    };
  }
  if (lower.includes("github.com")) {
    return {
      label: "GitHub",
      bg: "bg-white hover:bg-zinc-50",
      text: "text-zinc-800",
      iconBg: "bg-zinc-900",
      iconText: "text-white",
    };
  }
  if (lower.includes("loom.com")) {
    return {
      label: "Loom video",
      bg: "bg-white hover:bg-violet-50",
      text: "text-zinc-800",
      iconBg: "bg-violet-100",
      iconText: "text-violet-700",
    };
  }
  return {
    label: "Linked file",
    bg: "bg-white hover:bg-zinc-50",
    text: "text-zinc-800",
    iconBg: "bg-zinc-100",
    iconText: "text-zinc-600",
  };
}

function artifactTitle(url: string, label: string): string {
  try {
    const parsed = new URL(url);
    const host = parsed.hostname.replace(/^www\./, "");
    const path = parsed.pathname.replace(/\/$/, "");
    // For Google Workspace, prefer document id tail.
    if (host.includes("google.com")) {
      const idMatch = path.match(/\/d\/([^/]+)/);
      if (idMatch) return `${host} · …${idMatch[1].slice(-6)}`;
    }
    if (host.includes("github.com")) {
      const parts = path.split("/").filter(Boolean);
      if (parts.length >= 2) return `${parts[0]}/${parts[1]}`;
      return host;
    }
    if (host.includes("figma.com")) {
      const fileMatch = path.match(/\/(file|design|proto)\/([^/]+)\/([^/?]+)/);
      if (fileMatch) {
        return decodeURIComponent(fileMatch[3]).replace(/[-_]+/g, " ");
      }
    }
    return `${host}${path.length > 1 ? path : ""}`.replace(/\?.*$/, "");
  } catch {
    return label;
  }
}

// ── Drive filename resolution (module-level cache) ──────────────────────────
//
// Drive file IDs surface in lots of places (inline message chips, the Files
// panel, even potentially the AI Insights card later). Fetching the human
// filename via `driveGetFileMetadata` should happen at most once per file ID
// per session — hence the shared cache. React subscribers re-render when
// resolution completes.

interface DriveLinkInfo {
  fileId: string;
  /** Source kind detected from URL: doc/sheet/slides/drive_file. */
  kind: "doc" | "sheet" | "slides" | "drive_file";
}

function parseDriveLink(url: string): DriveLinkInfo | null {
  const m =
    url.match(/docs\.google\.com\/document\/d\/([a-zA-Z0-9_-]+)/) ??
    url.match(/docs\.google\.com\/presentation\/d\/([a-zA-Z0-9_-]+)/) ??
    url.match(/docs\.google\.com\/spreadsheets\/d\/([a-zA-Z0-9_-]+)/) ??
    url.match(/drive\.google\.com\/file\/d\/([a-zA-Z0-9_-]+)/);
  if (!m?.[1]) return null;
  if (url.includes("/document/")) return { fileId: m[1], kind: "doc" };
  if (url.includes("/presentation/")) return { fileId: m[1], kind: "slides" };
  if (url.includes("/spreadsheets/")) return { fileId: m[1], kind: "sheet" };
  return { fileId: m[1], kind: "drive_file" };
}

const driveNameCache = new Map<string, string>();
const driveNameSubscribers = new Set<() => void>();
const driveInflight = new Set<string>();

function notifyDriveNameSubscribers() {
  driveNameSubscribers.forEach((cb) => cb());
}

function useDriveFileName(url: string): string | null {
  const info = useMemo(() => parseDriveLink(url), [url]);
  const [, force] = useState(0);
  const cached = info ? driveNameCache.get(info.fileId) ?? null : null;

  useEffect(() => {
    if (!info) return;
    if (driveNameCache.has(info.fileId)) return;
    if (driveInflight.has(info.fileId)) return;
    driveInflight.add(info.fileId);
    void (async () => {
      try {
        const meta = await driveGetFileMetadata(info.fileId);
        if (meta?.name) {
          driveNameCache.set(info.fileId, meta.name);
        } else {
          // Cache an empty marker so we don't retry the same file repeatedly.
          driveNameCache.set(info.fileId, "");
        }
      } catch {
        // Cache empty marker on failure so the chip falls back gracefully
        // without hammering the API every render.
        driveNameCache.set(info.fileId, "");
      } finally {
        driveInflight.delete(info.fileId);
        notifyDriveNameSubscribers();
      }
    })();
  }, [info]);

  useEffect(() => {
    const cb = () => force((v) => v + 1);
    driveNameSubscribers.add(cb);
    return () => {
      driveNameSubscribers.delete(cb);
    };
  }, []);

  return cached && cached.length > 0 ? cached : null;
}

function ArtifactLink({ onOpen, url }: { onOpen: () => void; url: string }) {
  const meta = getArtifactMeta(url);
  const fallbackTitle = artifactTitle(url, meta.label);
  const driveName = useDriveFileName(url);
  const title = driveName ?? fallbackTitle;
  return (
    <button
      className={`group inline-flex max-w-[320px] items-center gap-2 rounded-lg border border-zinc-100 px-2.5 py-1.5 text-left transition-colors ${meta.bg}`}
      onClick={onOpen}
      title={driveName ? `${driveName} · ${url}` : url}
      type="button"
    >
      <span
        className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-md ${meta.iconBg} ${meta.iconText}`}
      >
        <FileText size={13} />
      </span>
      <span className="min-w-0 flex-1">
        <span className={`block text-[11px] font-semibold uppercase tracking-wider ${meta.iconText}`}>
          {meta.label}
        </span>
        <span className={`block truncate text-[12px] ${meta.text}`}>
          {title}
        </span>
      </span>
      <ExternalLink
        className="shrink-0 text-zinc-300 transition-colors group-hover:text-zinc-600"
        size={12}
      />
    </button>
  );
}


function EmailBody({
  item,
  onComposeForEmail,
  onOpenAttachment,
}: {
  item: GmailMessageRecord;
  onComposeForEmail: (email: string) => void;
  onOpenAttachment: (url: string) => void;
}) {
  const [height, setHeight] = useState(96);
  const [showQuotedHistory, setShowQuotedHistory] = useState(false);

  useEffect(() => {
    setHeight(96);
    setShowQuotedHistory(false);
  }, [item.message_id]);

  // Single click handler used both inside the iframe (HTML body) and on the
  // plain-text fallback. Routes:
  //   mailto:foo  → open Reply composer pre-filled with foo
  //   artifact URL → open AttachmentSheet bottom-sheet
  //   anything else → open in the system browser via tauri-plugin-opener
  function routeLinkClick(rawHref: string | null | undefined) {
    const href = (rawHref || "").trim();
    if (!href || href.startsWith("#")) return;
    if (href.startsWith("mailto:")) {
      try {
        const email = decodeURIComponent(href.slice(7).split("?")[0]).trim();
        if (/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) onComposeForEmail(email);
      } catch {
        // Ignore malformed percent-encoding in untrusted message HTML.
      }
      return;
    }
    const safeHref = safeExternalUrl(href);
    if (safeHref && isArtifactUrl(safeHref)) {
      onOpenAttachment(safeHref);
      return;
    }
    if (safeHref) {
      openUrl(safeHref).catch(() => {
        // Silent — fallback to noop. The user can copy the URL from the message.
      });
    }
  }

  if (item.html_body && item.html_body.trim().length > 0) {
    const trimmedHtml = trimQuotedEmailHtml(item.html_body);
    return (
      <div>
        <iframe
          className="w-full border-0 bg-white"
          onLoad={(event) => {
            const iframe = event.currentTarget;
            const doc = iframe.contentDocument;
            if (!doc) return;
            const documentElement = doc.documentElement;
            if (documentElement) {
              setHeight(
                Math.min(Math.max(documentElement.scrollHeight + 12, 72), 1200),
              );
            }
            // Intercept link clicks inside the (same-origin srcdoc) iframe so
            // they route to our in-app surfaces instead of opening blank pages.
            doc.addEventListener(
              "click",
              (e) => {
                const targetEl = e.target as Element | null;
                const anchor = targetEl?.closest?.("a") as HTMLAnchorElement | null;
                if (!anchor) return;
                const href = anchor.getAttribute("href");
                if (!href) return;
                e.preventDefault();
                e.stopPropagation();
                routeLinkClick(href);
              },
              { capture: true },
            );
          }}
          referrerPolicy="no-referrer"
          sandbox="allow-same-origin"
          srcDoc={sanitizeEmailHtml(
            showQuotedHistory ? item.html_body : trimmedHtml.visibleBody,
          )}
          style={{ height }}
          title={`Email body ${item.message_id}`}
        />
        {trimmedHtml.hasQuotedHistory ? (
          <QuotedHistoryToggle
            expanded={showQuotedHistory}
            onClick={() => {
              setHeight(96);
              setShowQuotedHistory((value) => !value);
            }}
          />
        ) : null}
      </div>
    );
  }
  const trimmedText = trimQuotedEmailText(
    item.plain_body || stripHtmlText(item.html_body) || item.snippet || "",
  );
  return (
    <div>
      <p className="whitespace-pre-wrap break-words text-[14px] leading-7 text-zinc-700">
        {linkify(messageBody(item, showQuotedHistory), routeLinkClick)}
      </p>
      {trimmedText.hasQuotedHistory ? (
        <QuotedHistoryToggle
          expanded={showQuotedHistory}
          onClick={() => setShowQuotedHistory((value) => !value)}
        />
      ) : null}
    </div>
  );
}

function QuotedHistoryToggle({
  expanded,
  onClick,
}: {
  expanded: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className="mt-2 inline-flex items-center gap-1.5 rounded-md bg-zinc-100 px-2 py-1 text-xs font-medium text-zinc-600 transition-colors hover:bg-zinc-200 hover:text-zinc-800"
      onClick={onClick}
      type="button"
    >
      <ChevronRight
        className={`transition-transform ${expanded ? "rotate-90" : ""}`}
        size={12}
      />
      {expanded ? "Hide quoted history" : "Show quoted history"}
    </button>
  );
}

/**
 * Turn a plain-text string into JSX with anchor-styled spans for any URL it
 * contains. Each URL is wrapped in a clickable element that routes through
 * `onRoute` so artifact URLs go to the AttachmentSheet, mailto: to the
 * composer, and everything else to the system browser via openUrl.
 */
function linkify(text: string, onRoute: (href: string) => void): ReactNode[] {
  if (!text) return [];
  const urlRegex = /https?:\/\/[^\s<>"']+[^\s<>"',.;:!?)\]]/g;
  const nodes: ReactNode[] = [];
  let lastIndex = 0;
  let match: RegExpExecArray | null;
  let i = 0;
  while ((match = urlRegex.exec(text)) !== null) {
    const start = match.index;
    if (start > lastIndex) {
      nodes.push(text.slice(lastIndex, start));
    }
    const url = match[0];
    nodes.push(
      <button
        className="text-sky-600 underline underline-offset-2 hover:text-sky-700"
        key={`u${i++}`}
        onClick={() => onRoute(url)}
        type="button"
      >
        {url}
      </button>,
    );
    lastIndex = start + url.length;
  }
  if (lastIndex < text.length) nodes.push(text.slice(lastIndex));
  return nodes;
}

function EmailActionsMenu({
  retrying,
  onDrafts,
  onDigest,
  onRetryTitle,
  onRetrySummary,
}: {
  retrying: boolean;
  onDrafts: () => void;
  onDigest: () => void;
  onRetryTitle: () => void;
  onRetrySummary: () => void;
}) {
  const [open, setOpen] = useState(false);

  function pick(fn: () => void) {
    fn();
    setOpen(false);
  }

  return (
    <div className="relative">
      <button
        className="btn h-9 w-9 px-0"
        onClick={() => setOpen((v) => !v)}
        title="More actions"
        type="button"
      >
        <MoreHorizontal size={15} />
      </button>
      {open ? (
        <>
          <div className="fixed inset-0 z-10" onMouseDown={() => setOpen(false)} />
          <div className="absolute right-0 top-full z-20 mt-1 w-52 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
            <MenuItem onClick={() => pick(onDrafts)}>Drafts</MenuItem>
            <MenuItem onClick={() => pick(onDigest)}>Digest</MenuItem>
            <div className="my-1 border-t border-zinc-100" />
            <MenuItem
              className="text-violet-600 hover:bg-violet-50"
              disabled={retrying}
              onClick={() => pick(onRetryTitle)}
            >
              {retrying ? <Loader2 className="animate-spin" size={13} /> : <Sparkles className="text-violet-400" size={13} />}
              {retrying ? "Running…" : "Retry AI title"}
            </MenuItem>
            <MenuItem
              className="text-violet-600 hover:bg-violet-50"
              disabled={retrying}
              onClick={() => pick(onRetrySummary)}
            >
              {retrying ? <Loader2 className="animate-spin" size={13} /> : <Sparkles className="text-violet-400" size={13} />}
              {retrying ? "Running…" : "Retry AI summary"}
            </MenuItem>
          </div>
        </>
      ) : null}
    </div>
  );
}

function MenuItem({
  children,
  className = "",
  disabled,
  onClick,
}: {
  children: ReactNode;
  className?: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`flex w-full items-center gap-2 px-4 py-2.5 text-left text-[12px] font-medium text-zinc-600 transition-colors hover:bg-zinc-50 disabled:pointer-events-none disabled:opacity-40 ${className}`}
      disabled={disabled}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}

function AiInsightsCard({
  actionRequired,
  bundleSize,
  dimensionsConfidence,
  intent,
  predictedAction,
  reasons,
  senderEmail,
  summary,
  threadId,
  threadState,
  triage,
}: {
  actionRequired: boolean;
  bundleSize: number;
  dimensionsConfidence: Record<string, number> | null;
  intent: string | null;
  predictedAction: string | null;
  reasons: string[] | null;
  senderEmail: string | null;
  summary: string | null;
  threadId: string;
  threadState: string | null;
  triage: GmailTriageResult | null;
}) {
  const [expanded, setExpanded] = useState(false);
  const cleanedSummary = summary ? cleanSummary(summary) : "";
  const summaryLines = cleanedSummary ? cleanedSummary.split(" · ") : [];
  const hasSummary = cleanedSummary.length > 0;

  return (
    <section className="mt-6 rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <header className="flex items-center gap-2 border-b border-zinc-100 px-5 py-3">
        <Sparkles className="text-violet-500" size={14} />
        <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">AI Insights</span>
      </header>
      <div className="p-5">
        {hasSummary ? (
          <p className="text-sm leading-6 text-zinc-700">
            {summaryLines.map((line, i) => (
              <span className="block" key={i}>{line}</span>
            ))}
          </p>
        ) : (
          <p className="text-sm text-zinc-400">No AI summary yet.</p>
        )}

        <button
          className="mt-3 inline-flex items-center gap-1 text-xs font-semibold text-violet-600 hover:text-violet-700"
          onClick={() => setExpanded((v) => !v)}
          type="button"
        >
          {expanded ? "View less" : "View more"}
          <ChevronDown
            className={`transition-transform duration-150 ${expanded ? "rotate-180" : ""}`}
            size={12}
          />
        </button>

        <AnimatePresence initial={false}>
          {expanded ? (
            <motion.div
              animate={{ opacity: 1, height: "auto" }}
              className="overflow-hidden"
              exit={{ opacity: 0, height: 0 }}
              initial={{ opacity: 0, height: 0 }}
              key="details"
              transition={{ duration: 0.16, ease: "easeOut" }}
            >
              <div className="mt-4 space-y-4 border-t border-zinc-100 pt-4">
                {triage ? <TriageStrip result={triage} /> : null}
                <ThreadClassificationEditor
                  actionRequired={actionRequired}
                  bundleSize={bundleSize}
                  dimensionsConfidence={dimensionsConfidence}
                  intent={intent}
                  predictedAction={predictedAction}
                  reasons={reasons}
                  senderEmail={senderEmail}
                  threadId={threadId}
                  threadState={threadState}
                />
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>
      </div>
      {hasSummary ? (
        <footer className="flex items-center gap-2 border-t border-zinc-100 px-5 py-2 text-[11px] text-zinc-400">
          <Sparkles className="text-violet-400" size={10} />
          <span>AI saved</span>
        </footer>
      ) : null}
    </section>
  );
}

function TriageStrip({ result }: { result: GmailTriageResult }) {
  return (
    <div className="flex flex-wrap items-start justify-between gap-3">
      <div>
        <div className="flex flex-wrap items-center gap-2">
          <span className={["rounded-md px-2 py-1 text-xs font-semibold", triageTone(result.category)].join(" ")}>
            {result.category}
          </span>
          <span className="rounded-md bg-zinc-100 px-2 py-1 text-xs font-semibold text-zinc-600">
            {result.priority} priority
          </span>
          {result.confidence != null ? (
            <span className="text-xs text-zinc-400">{Math.round(result.confidence * 100)}% confidence</span>
          ) : null}
        </div>
        <div className="mt-2 space-y-1 text-sm leading-6 text-zinc-600">
          {result.reasons.slice(0, 3).map((reason) => (
            <p key={reason}>- {reason}</p>
          ))}
        </div>
      </div>
    </div>
  );
}

function EmailDrawer({
  activePanel,
  ai,
  detail,
  digest,
  drafts,
  edges,
  initiatives,
  deliverables,
  selectedDeliverableId,
  selectedInitiativeId,
  triage,
  workSuggestions,
  intakeLoading,
  onAnalyze,
  onApproveDeliverable,
  onApproveTask,
  onApproveWorkSuggestion,
  onClose,
  onDismissWorkSuggestion,
  onLinkDeliverable,
  onLinkInitiative,
  onOpenAttachment,
  onRefreshWorkspaceIntake,
  onRunThreadIntake,
  onSetDeliverable,
  onSetInitiative,
  onTriageAction,
}: {
  activePanel: EmailPanel | null;
  ai: GmailAiResult | null;
  detail: GmailThreadDetail | null;
  digest: GmailWeeklyDigest | null;
  drafts: GmailDraftRecord[];
  edges: GmailRelationshipEdge[];
  initiatives: Initiative[];
  deliverables: Deliverable[];
  selectedDeliverableId: string;
  selectedInitiativeId: string;
  triage: GmailTriageResult | null;
  workSuggestions: WorkIntakeSuggestion[];
  intakeLoading: boolean;
  onAnalyze: () => void;
  onApproveDeliverable: (candidate: GmailAiCandidate) => void;
  onApproveTask: (candidate: GmailAiCandidate) => void;
  onApproveWorkSuggestion: (suggestion: WorkIntakeSuggestion, kindOverride?: WorkIntakeKind, edits?: { title?: string; body?: string; dueDate?: string; targetDeliverableId?: string; targetInitiativeId?: string }) => void;
  onClose: () => void;
  onDismissWorkSuggestion: (id: string) => void;
  onLinkDeliverable: () => void;
  onLinkInitiative: () => void;
  onOpenAttachment: (url: string) => void;
  onRefreshWorkspaceIntake: () => void;
  onRunThreadIntake: () => void;
  onSetDeliverable: (id: string) => void;
  onSetInitiative: (id: string) => void;
  onTriageAction: (action: string) => void;
}) {
  if (!activePanel) return null;
  return (
    <div className="fixed inset-0 z-40 flex justify-end bg-black/10" onMouseDown={onClose}>
      <aside
        className="h-full w-[420px] max-w-[calc(100vw-2rem)] overflow-auto border-l border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="sticky top-0 z-10 flex items-center justify-between border-b border-zinc-200 bg-white px-5 py-4">
          <h3 className="text-sm font-semibold text-zinc-950">{panelTitle(activePanel)}</h3>
          <button className="btn h-8 w-8 px-0" onClick={onClose} type="button">
            <X size={15} />
          </button>
        </div>
        <div className="space-y-5 p-5">
          {activePanel === "work" && detail ? (
            <WorkPanel
              deliverables={deliverables}
              detail={detail}
              initiatives={initiatives}
              onLinkDeliverable={onLinkDeliverable}
              onLinkInitiative={onLinkInitiative}
              onSetDeliverable={onSetDeliverable}
              onSetInitiative={onSetInitiative}
              onApproveWorkSuggestion={onApproveWorkSuggestion}
              onDismissWorkSuggestion={onDismissWorkSuggestion}
              onRefreshWorkspaceIntake={onRefreshWorkspaceIntake}
              onRunThreadIntake={onRunThreadIntake}
              selectedDeliverableId={selectedDeliverableId}
              selectedInitiativeId={selectedInitiativeId}
              suggestions={workSuggestions}
              intakeLoading={intakeLoading}
            />
          ) : null}
          {activePanel === "assets" && detail ? (
            <AssetsPanel detail={detail} onOpenAttachment={onOpenAttachment} />
          ) : null}
          {activePanel === "drafts" ? <DraftsPanel drafts={drafts} /> : null}
          {activePanel === "digest" ? <DigestPanel digest={digest} /> : null}
          {activePanel === "graph" ? <GraphPanel edges={edges} /> : null}
          {activePanel === "ai" ? (
            <AiReviewPanel
              ai={ai}
              onAnalyze={onAnalyze}
              onApproveDeliverable={onApproveDeliverable}
              onApproveTask={onApproveTask}
              onTriageAction={onTriageAction}
              triage={triage}
            />
          ) : null}
        </div>
      </aside>
    </div>
  );
}

function WorkPanel({
  deliverables,
  detail,
  initiatives,
  intakeLoading,
  suggestions,
  onApproveWorkSuggestion,
  onDismissWorkSuggestion,
  onLinkDeliverable,
  onLinkInitiative,
  onRefreshWorkspaceIntake,
  onRunThreadIntake,
  onSetDeliverable,
  onSetInitiative,
  selectedDeliverableId,
  selectedInitiativeId,
}: {
  deliverables: Deliverable[];
  detail: GmailThreadDetail;
  initiatives: Initiative[];
  intakeLoading: boolean;
  suggestions: WorkIntakeSuggestion[];
  onApproveWorkSuggestion: (suggestion: WorkIntakeSuggestion, kindOverride?: WorkIntakeKind, edits?: { title?: string; body?: string; dueDate?: string; targetDeliverableId?: string; targetInitiativeId?: string }) => void;
  onDismissWorkSuggestion: (id: string) => void;
  onLinkDeliverable: () => void;
  onLinkInitiative: () => void;
  onRefreshWorkspaceIntake: () => void;
  onRunThreadIntake: () => void;
  onSetDeliverable: (id: string) => void;
  onSetInitiative: (id: string) => void;
  selectedDeliverableId: string;
  selectedInitiativeId: string;
}) {
  return (
    <div className="space-y-5">
      <section className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
        <div className="mb-3 flex items-start justify-between gap-3">
          <div>
            <p className="text-xs font-semibold uppercase tracking-wide text-zinc-400">Work intake</p>
            <p className="mt-1 text-sm leading-5 text-zinc-600">
              Review AI-suggested tasks, deliverables, and initiatives before creating records.
            </p>
          </div>
        </div>
        <div className="mb-3 grid grid-cols-2 gap-2">
          <button
            className="btn h-8 px-2 text-xs"
            disabled={intakeLoading}
            onClick={onRunThreadIntake}
            title="Re-run AI on this thread — replaces current suggestions"
            type="button"
          >
            {intakeLoading ? <Loader2 className="animate-spin" size={13} /> : <RefreshCw size={13} />}
            Re-run thread
          </button>
          <button className="btn h-8 px-2 text-xs" disabled={intakeLoading} onClick={onRefreshWorkspaceIntake} type="button">
            <RefreshCw size={13} />
            Workspace
          </button>
        </div>
        {suggestions.length === 0 ? (
          <p className="text-xs text-zinc-500">No pending suggestions for this thread.</p>
        ) : (
          <div className="space-y-2">
            {suggestions.map((suggestion) => (
              <WorkSuggestionCard
                key={suggestion.id}
                deliverables={deliverables}
                initiatives={initiatives}
                onApprove={(kind, edits) => onApproveWorkSuggestion(suggestion, kind, edits)}
                onDismiss={() => onDismissWorkSuggestion(suggestion.id)}
                suggestion={suggestion}
              />
            ))}
          </div>
        )}
      </section>
      <div className="space-y-2">
        <select className="field-control" onChange={(event) => onSetDeliverable(event.currentTarget.value)} value={selectedDeliverableId}>
          <option value="">Select deliverable</option>
          {deliverables.map((deliverable) => (
            <option key={deliverable.id} value={deliverable.id}>{deliverable.title}</option>
          ))}
        </select>
        <button className="btn w-full" disabled={!selectedDeliverableId} onClick={onLinkDeliverable} type="button">Link deliverable</button>
        <select className="field-control" onChange={(event) => onSetInitiative(event.currentTarget.value)} value={selectedInitiativeId}>
          <option value="">Select initiative</option>
          {initiatives.map((initiative) => (
            <option key={initiative.id} value={initiative.id}>{initiative.title}</option>
          ))}
        </select>
        <button className="btn w-full" disabled={!selectedInitiativeId} onClick={onLinkInitiative} type="button">Link initiative</button>
      </div>
      <LinkedItems title="Linked deliverables" items={detail.thread.linked_deliverables.map((item) => ({ id: item.id, title: item.title, to: `/deliverables/${item.id}` }))} />
      <LinkedItems title="Linked initiatives" items={detail.thread.linked_initiatives.map((item) => ({ id: item.id, title: item.title, to: `/initiatives/${item.id}` }))} />
    </div>
  );
}

function LinkedItems({ items, title }: { items: Array<{ id: string; title: string; to: string }>; title: string }) {
  return (
    <div>
      <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">{title}</p>
      {items.length === 0 ? <p className="text-sm text-zinc-500">None linked.</p> : (
        <div className="space-y-1">
          {items.map((item) => (
            <Link className="flex items-center justify-between rounded-md border border-zinc-200 px-3 py-2 text-sm font-medium text-sky-700 hover:bg-sky-50" key={item.id} to={item.to}>
              <span className="truncate">{item.title}</span>
              <ChevronRight size={14} />
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

const KIND_OPTIONS: WorkIntakeKind[] = ["task", "deliverable", "initiative"];

function WorkSuggestionCard({
  deliverables,
  initiatives,
  onApprove,
  onDismiss,
  suggestion,
}: {
  deliverables: Deliverable[];
  initiatives: Initiative[];
  onApprove: (kind: WorkIntakeKind, edits?: { title?: string; body?: string; dueDate?: string; targetDeliverableId?: string; targetInitiativeId?: string }) => void;
  onDismiss: () => void;
  suggestion: WorkIntakeSuggestion;
}) {
  const [kind, setKind] = useState<WorkIntakeKind>(suggestion.item_kind as WorkIntakeKind);
  const [editing, setEditing] = useState(false);
  const [editTitle, setEditTitle] = useState(suggestion.title);
  const [editBody, setEditBody] = useState(suggestion.body ?? "");
  const [editDueDate, setEditDueDate] = useState(suggestion.due_date ?? "");
  const [targetDeliverableId, setTargetDeliverableId] = useState(suggestion.target_deliverable_id ?? "");
  const [targetInitiativeId, setTargetInitiativeId] = useState(suggestion.target_initiative_id ?? "");
  const [confirmingDismiss, setConfirmingDismiss] = useState(false);
  const aiKind = suggestion.item_kind as WorkIntakeKind;

  const hasEdits = editTitle !== suggestion.title || editBody !== (suggestion.body ?? "") || editDueDate !== (suggestion.due_date ?? "");

  // When kind changes, try to auto-clear irrelevant target
  function handleKindChange(k: WorkIntakeKind) {
    setKind(k);
  }

  const needsDeliverable = kind === "task";
  const needsInitiative = kind === "deliverable";

  // Approve is blocked only if required target is absent
  const missingDeliverable = needsDeliverable && !targetDeliverableId;

  function handleApprove() {
    const edits = {
      ...(hasEdits ? { title: editTitle, body: editBody, dueDate: editDueDate } : {}),
      ...(targetDeliverableId ? { targetDeliverableId } : {}),
      ...(targetInitiativeId ? { targetInitiativeId } : {}),
    };
    onApprove(kind, Object.keys(edits).length > 0 ? edits : undefined);
  }

  return (
    <div className="rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      {/* Kind selector */}
      <div className="flex items-center gap-1 border-b border-zinc-100 px-3 py-2">
        <span className="mr-1 text-[10px] font-semibold uppercase tracking-widest text-zinc-400">Type</span>
        {KIND_OPTIONS.map((k) => (
          <button
            className={[
              "inline-flex h-6 items-center gap-1 rounded-md px-2 text-[11px] font-semibold transition-colors",
              kind === k
                ? "bg-sky-500 text-white"
                : "text-zinc-500 hover:bg-zinc-100 hover:text-zinc-800",
            ].join(" ")}
            key={k}
            onClick={() => handleKindChange(k)}
            type="button"
          >
            {k}
            {k === aiKind ? (
              <span className={["ml-0.5 text-[8px] font-bold", kind === k ? "opacity-60" : "text-violet-400"].join(" ")}>
                suggested
              </span>
            ) : null}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1.5">
          {suggestion.confidence != null ? (
            <span className="text-[11px] text-zinc-400">{Math.round(suggestion.confidence * 100)}%</span>
          ) : null}
          <button
            className={["icon-btn h-7 w-7", editing ? "bg-sky-50 text-sky-500" : ""].join(" ")}
            onClick={() => setEditing((v) => !v)}
            title={editing ? "Close editor" : "Edit title / description / due date"}
            type="button"
          >
            <PenLine size={13} />
          </button>
        </div>
      </div>

      {/* Content / Edit mode */}
      {editing ? (
        <div className="space-y-2 px-3 py-2.5">
          <div>
            <label className="field-label mb-1 block">Title</label>
            <input
              className="field-control"
              onChange={(e) => setEditTitle(e.target.value)}
              type="text"
              value={editTitle}
            />
          </div>
          <div>
            <label className="field-label mb-1 block">Description</label>
            <textarea
              className="field-control min-h-[56px] text-[12px]"
              onChange={(e) => setEditBody(e.target.value)}
              rows={3}
              value={editBody}
            />
          </div>
          <div>
            <label className="field-label mb-1 block">Due date</label>
            <input
              className="field-control"
              onChange={(e) => setEditDueDate(e.target.value)}
              type="date"
              value={editDueDate}
            />
          </div>
        </div>
      ) : (
        <div className="px-3 py-2.5">
          <p className="text-[13px] font-semibold leading-5 text-zinc-900">
            {editTitle}
            {hasEdits ? <span className="ml-1.5 text-[10px] font-normal text-sky-500">edited</span> : null}
          </p>
          {suggestion.body ? (
            <p className="mt-1 line-clamp-2 text-[12px] leading-5 text-zinc-500">{editBody || suggestion.body}</p>
          ) : null}
          <div className="mt-1.5 flex flex-wrap gap-1.5 text-[11px] text-zinc-400">
            {(editDueDate || suggestion.due_date) ? (
              <span>Due {editDueDate || suggestion.due_date}</span>
            ) : null}
            {suggestion.suggested_type ? <span>{suggestion.suggested_type}</span> : null}
            {suggestion.source_title ? (
              <span className="max-w-36 truncate">{suggestion.source_title}</span>
            ) : null}
          </div>
        </div>
      )}

      {/* Inline target pickers */}
      {needsDeliverable ? (
        <div className="border-t border-zinc-100 px-3 py-2">
          <label className="field-label mb-1 block">
            Under deliverable <span className="text-red-400">*</span>
          </label>
          <select
            className={["field-control text-[12px]", missingDeliverable ? "border-amber-300 bg-amber-50" : ""].join(" ")}
            onChange={(e) => setTargetDeliverableId(e.currentTarget.value)}
            value={targetDeliverableId}
          >
            <option value="">— choose deliverable —</option>
            {deliverables.map((d) => (
              <option key={d.id} value={d.id}>{d.title}</option>
            ))}
          </select>
        </div>
      ) : needsInitiative && initiatives.length > 0 ? (
        <div className="border-t border-zinc-100 px-3 py-2">
          <label className="field-label mb-1 block">Under initiative (optional)</label>
          <select
            className="field-control text-[12px]"
            onChange={(e) => setTargetInitiativeId(e.currentTarget.value)}
            value={targetInitiativeId}
          >
            <option value="">— none —</option>
            {initiatives.map((i) => (
              <option key={i.id} value={i.id}>{i.title}</option>
            ))}
          </select>
        </div>
      ) : null}

      {/* Actions */}
      {confirmingDismiss ? (
        <div className="border-t border-zinc-100 px-3 py-2">
          <p className="mb-2 text-[11px] text-amber-700">Dismiss this suggestion? It won't appear again.</p>
          <div className="flex gap-2">
            <button className="btn btn-danger h-6 px-2 text-[11px]" onClick={onDismiss} type="button">
              Yes, dismiss
            </button>
            <button className="btn h-6 px-2 text-[11px]" onClick={() => setConfirmingDismiss(false)} type="button">
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="flex gap-2 border-t border-zinc-100 px-3 py-2">
          <button
            className="btn btn-primary h-7 flex-1 px-2 text-[11px] disabled:opacity-40"
            disabled={missingDeliverable}
            onClick={handleApprove}
            title={missingDeliverable ? "Choose a deliverable first" : undefined}
            type="button"
          >
            <Check size={12} />
            Approve as {kind}
          </button>
          <button
            className="btn h-7 px-2 text-[11px]"
            onClick={() => setConfirmingDismiss(true)}
            type="button"
          >
            Dismiss
          </button>
        </div>
      )}
    </div>
  );
}

function AssetsPanel({
  detail,
  onOpenAttachment,
}: {
  detail: GmailThreadDetail;
  onOpenAttachment: (url: string) => void;
}) {
  // De-dupe links by URL so the same Drive doc shared across multiple
  // messages only appears once.
  const seenUrls = new Set<string>();
  const uniqueLinks = detail.links.filter((l) => {
    if (seenUrls.has(l.url)) return false;
    seenUrls.add(l.url);
    return true;
  });
  const hasContent = detail.attachments.length > 0 || uniqueLinks.length > 0;

  if (!hasContent) {
    return (
      <div className="py-10 text-center">
        <Paperclip className="mx-auto mb-2 text-zinc-200" size={24} />
        <p className="text-sm text-zinc-400">No attachments or linked files.</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {uniqueLinks.length > 0 ? (
        <section>
          <p className="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-zinc-400">
            Linked files
          </p>
          <div className="space-y-1.5">
            {uniqueLinks.map((link) => (
              <ArtifactLink
                key={link.id}
                onOpen={() => onOpenAttachment(link.url)}
                url={link.url}
              />
            ))}
          </div>
        </section>
      ) : null}
      {detail.attachments.length > 0 ? (
        <section>
          <p className="mb-2 text-[10px] font-bold uppercase tracking-[0.2em] text-zinc-400">
            Email attachments
          </p>
          <div className="space-y-1.5">
            {detail.attachments.map((attachment) => (
              <EmailAttachmentRow attachment={attachment} key={attachment.id} />
            ))}
          </div>
        </section>
      ) : null}
    </div>
  );
}

function EmailAttachmentRow({
  attachment,
}: {
  attachment: GmailAttachmentRecord;
}) {
  const filename = attachment.filename || attachment.mime_type;
  const mimeFamily = (attachment.mime_type || "").split("/")[0];
  const tone =
    mimeFamily === "image"
      ? { iconBg: "bg-violet-100", iconText: "text-violet-700" }
      : attachment.mime_type === "application/pdf"
        ? { iconBg: "bg-rose-100", iconText: "text-rose-700" }
        : { iconBg: "bg-zinc-100", iconText: "text-zinc-600" };
  return (
    <div className="inline-flex w-full max-w-full items-center gap-2 rounded-lg border border-zinc-100 bg-white px-2.5 py-1.5">
      <span
        className={`flex h-7 w-7 shrink-0 items-center justify-center rounded-md ${tone.iconBg} ${tone.iconText}`}
      >
        <FileText size={13} />
      </span>
      <span className="min-w-0 flex-1">
        <span
          className={`block text-[11px] font-semibold uppercase tracking-wider ${tone.iconText}`}
        >
          {attachment.mime_type || "File"}
        </span>
        <span className="block truncate text-[12px] text-zinc-800" title={filename}>
          {filename}
        </span>
      </span>
    </div>
  );
}

function DraftsPanel({ drafts }: { drafts: GmailDraftRecord[] }) {
  return drafts.length === 0 ? <p className="text-sm text-zinc-500">No synced drafts.</p> : (
    <div className="space-y-2">
      {drafts.map((draft) => (
        <div className="rounded-md border border-zinc-200 p-3" key={draft.draft_id}>
          <p className="truncate text-sm font-semibold text-zinc-800">{draft.subject || "(no subject)"}</p>
          <p className="mt-1 line-clamp-3 text-xs leading-5 text-zinc-500">{draft.body_preview}</p>
        </div>
      ))}
    </div>
  );
}

function DigestPanel({ digest }: { digest: GmailWeeklyDigest | null }) {
  return (
    <div className="space-y-4">
      <p className="text-sm leading-6 text-zinc-600">{digest?.summary ?? "Run a sync to build the digest."}</p>
      <DigestList title="Waiting for response" threads={digest?.waiting_for_response ?? []} />
      <DigestList title="Overdue follow-ups" threads={digest?.overdue_followups ?? []} />
      <DigestList title="Urgent threads" threads={digest?.urgent_threads ?? []} />
    </div>
  );
}

function DigestList({ threads, title }: { threads: GmailLocalThread[]; title: string }) {
  return (
    <div>
      <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">{title}</p>
      {threads.length === 0 ? <p className="text-xs text-zinc-500">None.</p> : (
        <div className="space-y-2">
          {threads.slice(0, 5).map((thread) => (
            <p className="truncate rounded-md border border-zinc-200 px-3 py-2 text-xs text-zinc-600" key={thread.thread_id}>{threadListTitle(thread)}</p>
          ))}
        </div>
      )}
    </div>
  );
}

function GraphPanel({ edges }: { edges: GmailRelationshipEdge[] }) {
  return edges.length === 0 ? <p className="text-sm text-zinc-500">No relationship edges yet.</p> : (
    <div className="space-y-2">
      {edges.slice(0, 20).map((edge) => (
        <div className="rounded-md border border-zinc-200 p-3 text-xs leading-5 text-zinc-600" key={`${edge.left_email}-${edge.right_email}`}>
          <p className="font-semibold text-zinc-800">{edge.left_name} / {edge.right_name}</p>
          <p>{edge.thread_count} shared thread(s)</p>
        </div>
      ))}
    </div>
  );
}

function AiReviewPanel({
  ai,
  onAnalyze,
  onApproveDeliverable,
  onApproveTask,
  onTriageAction,
  triage,
}: {
  ai: GmailAiResult | null;
  onAnalyze: () => void;
  onApproveDeliverable: (candidate: GmailAiCandidate) => void;
  onApproveTask: (candidate: GmailAiCandidate) => void;
  onTriageAction: (action: string) => void;
  triage: GmailTriageResult | null;
}) {
  return (
    <div className="space-y-5">
      {triage ? (
        <div className="rounded-lg border border-violet-200 bg-violet-50/40 p-4">
          <TriageStrip result={triage} />
          <div className="mt-4 flex flex-wrap gap-2">
            {triage.suggested_actions.map((action) => (
              <button className="btn bg-white" key={action} onClick={() => onTriageAction(action)} type="button">
                {actionLabel(action)}
              </button>
            ))}
          </div>
        </div>
      ) : (
        <button className="btn btn-primary w-full" onClick={onAnalyze} type="button">
          <Sparkles size={15} />
          Analyze thread
        </button>
      )}
      {ai ? (
        <AiPanel
          onApproveDeliverable={onApproveDeliverable}
          onApproveTask={onApproveTask}
          result={ai}
        />
      ) : null}
    </div>
  );
}

function SettingsDialog({
  onClose,
  onSave,
  open,
  settings,
}: {
  onClose: () => void;
  onSave: (next: Partial<GmailSyncSettings>) => void;
  open: boolean;
  settings: GmailSyncSettings;
}) {
  if (!open) return null;
  return (
    <Modal onClose={onClose} title="Work Mail settings">
      <GmailSyncSettingsControls compact onChange={onSave} settings={settings} />
      <WorkMailDomainSettings embedded />
    </Modal>
  );
}

function Modal({ children, onClose, title }: { children: ReactNode; onClose: () => void; title: string }) {
  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black/20 px-4 py-16" onMouseDown={onClose}>
      <section className="w-full max-w-xl rounded-2xl border border-zinc-100 bg-white shadow-2xl" onMouseDown={(event) => event.stopPropagation()}>
        <div className="flex items-center justify-between border-b border-zinc-200 px-5 py-4">
          <h3 className="text-sm font-semibold text-zinc-950">{title}</h3>
          <button className="btn h-8 w-8 px-0" onClick={onClose} type="button">
            <X size={15} />
          </button>
        </div>
        <div className="p-5">{children}</div>
      </section>
    </div>
  );
}

function RecipientLine({ label, people }: { label: string; people: EmailAddress[] }) {
  if (people.length === 0) {
    return null;
  }
  return (
    <div className="min-w-0">
      <span className="font-semibold text-zinc-500">{label}: </span>
      <span className="text-zinc-500">{people.map((person) => person.name || person.email).join(", ")}</span>
    </div>
  );
}

function collapsedMessagePreview(item: GmailMessageRecord) {
  const preview = messageBody(item, false);
  return meaningfulPreview(preview) || meaningfulPreview(item.snippet) || "(empty message)";
}

function messageBody(item: GmailMessageRecord, includeQuotedHistory = false) {
  const raw = item.plain_body || stripHtmlText(item.html_body) || item.snippet || "";
  const text = includeQuotedHistory ? raw : trimQuotedEmailText(raw).visibleBody;
  return text.trim().slice(0, 12000) || "(empty message)";
}

function trimQuotedEmailText(value: string) {
  const lines = value.replace(/\r\n/g, "\n").split("\n");
  const quoteStart = lines.findIndex((line, index) => {
    if (index === 0) return false;
    const trimmed = line.trim();
    return (
      /^On .{0,260} wrote:\s*$/i.test(trimmed) ||
      /^[-_]{2,}\s*(Original Message|Forwarded message)/i.test(trimmed) ||
      /^Begin forwarded message:/i.test(trimmed) ||
      /^>/.test(trimmed)
    );
  });
  if (quoteStart <= 0) {
    return { visibleBody: value, hasQuotedHistory: false };
  }
  const visibleBody = lines.slice(0, quoteStart).join("\n").trim();
  return {
    visibleBody: visibleBody || value,
    hasQuotedHistory: visibleBody.length > 0,
  };
}

function trimQuotedEmailHtml(value: string) {
  const quoteMarkers = [
    /<div[^>]*class\s*=\s*(['"])[^'"]*\bgmail_quote\b[\s\S]*$/i,
    /<div[^>]*class\s*=\s*(['"])[^'"]*\bgmail_attr\b[\s\S]*$/i,
    /<blockquote[^>]*class\s*=\s*(['"])[^'"]*\bgmail_quote\b[\s\S]*$/i,
    /<blockquote[^>]*type\s*=\s*(['"])cite\1[\s\S]*$/i,
    /<div[^>]*id\s*=\s*(['"])(?:divRplyFwdMsg|appendonsend)\1[\s\S]*$/i,
    /<hr[^>]*(?:id\s*=\s*(['"])replySplit\1|class\s*=\s*(['"])[^'"]*(?:gmail_quote|ms-outlook)[^'"]*\2)[\s\S]*$/i,
    /<blockquote\b[\s\S]*$/i,
  ];
  const quoteStart = quoteMarkers.reduce<number>((earliest, marker) => {
    const match = marker.exec(value);
    if (!match || match.index <= 0) return earliest;
    return earliest === -1 ? match.index : Math.min(earliest, match.index);
  }, -1);
  if (quoteStart < 0) {
    return { visibleBody: value, hasQuotedHistory: false };
  }
  const visibleBody = value.slice(0, quoteStart).trim();
  return {
    visibleBody: visibleBody || value,
    hasQuotedHistory: visibleBody.length > 0,
  };
}

function sanitizeEmailHtml(value: string) {
  const cleaned = DOMPurify.sanitize(value, {
    USE_PROFILES: { html: true },
    FORBID_TAGS: [
      "base",
      "button",
      "embed",
      "form",
      "iframe",
      "input",
      "link",
      "meta",
      "object",
      "script",
      "select",
      "style",
      "textarea",
    ],
    FORBID_ATTR: ["formaction", "srcdoc", "srcset"],
  });

  const parsed = new DOMParser().parseFromString(cleaned, "text/html");
  for (const image of parsed.querySelectorAll("img")) {
    const source = image.getAttribute("src")?.trim() ?? "";
    if (!/^data:image\/(?:gif|jpeg|png|webp);base64,/i.test(source)) {
      image.removeAttribute("src");
    }
    image.removeAttribute("srcset");
  }
  for (const anchor of parsed.querySelectorAll("a")) {
    const href = anchor.getAttribute("href")?.trim() ?? "";
    const safeMailto = /^mailto:[^\s@]+@[^\s@]+\.[^\s@]+(?:\?[^\s]*)?$/i.test(href);
    const safeHref = safeMailto ? href : safeExternalUrl(href);
    if (safeHref) {
      anchor.setAttribute("href", safeHref);
      anchor.setAttribute("rel", "noopener noreferrer");
    } else {
      anchor.removeAttribute("href");
    }
    anchor.removeAttribute("target");
  }
  const safeBody = parsed.body.innerHTML;

  return `<!doctype html>
<html>
  <head>
    <meta charset="utf-8" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'" />
    <style>
      html, body { margin: 0; padding: 0; background: #fff; color: #18181b; font: 14px/1.65 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
      body { padding: 16px 18px; overflow-wrap: anywhere; }
      img { max-width: 100%; height: auto; }
      a { color: #0369a1; text-decoration: none; }
      a:hover { text-decoration: underline; }
      table { max-width: 100%; }
    </style>
  </head>
  <body>${safeBody}</body>
</html>`;
}

function stripHtmlText(value: string) {
  return value
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/\s+\n/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

function isArtifactUrl(url: string) {
  const lower = url.toLowerCase();
  return (
    lower.includes("figma.com") ||
    lower.includes("docs.google.com/document") ||
    lower.includes("docs.google.com/presentation") ||
    lower.includes("docs.google.com/spreadsheets") ||
    lower.includes("notion.so") ||
    lower.includes("github.com")
  );
}

function MiniBadge({ children }: { children: ReactNode }) {
  return (
    <span className="rounded-md bg-zinc-100 px-2 py-0.5 text-[11px] font-semibold text-zinc-600">
      {children}
    </span>
  );
}

function AiPanel({
  onApproveDeliverable,
  onApproveTask,
  result,
}: {
  onApproveDeliverable: (candidate: GmailAiCandidate) => void;
  onApproveTask: (candidate: GmailAiCandidate) => void;
  result: GmailAiResult;
}) {
  return (
    <section className="rounded-lg border border-violet-200 bg-violet-50/40 p-5">
      <div className="mb-3 flex items-center gap-2">
        <Sparkles size={15} className="text-violet-600" />
        <h3 className="text-sm font-semibold text-zinc-950">AI review</h3>
      </div>
      <p className="whitespace-pre-wrap text-sm leading-6 text-zinc-700">{result.summary}</p>
      <div className="mt-4 grid gap-3 md:grid-cols-3">
        <SuggestionList actionLabel="Create task" items={result.tasks} onApprove={onApproveTask} title="Tasks" />
        <SuggestionList actionLabel="Create deliverable" items={result.deliverables} onApprove={onApproveDeliverable} title="Deliverables" />
        <SuggestionList actionLabel="Create task" items={result.deadlines} onApprove={onApproveTask} title="Deadlines" />
      </div>
      {result.reply ? (
        <div className="mt-4 rounded-md bg-white p-3 text-sm leading-6 text-zinc-700">
          <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-zinc-400">Draft reply</p>
          {result.reply}
        </div>
      ) : null}
    </section>
  );
}

function SuggestionList({
  actionLabel,
  items,
  onApprove,
  title,
}: {
  actionLabel: string;
  items: GmailAiCandidate[];
  onApprove: (candidate: GmailAiCandidate) => void;
  title: string;
}) {
  return (
    <div>
      <p className="mb-2 text-xs font-semibold uppercase tracking-wide text-zinc-400">{title}</p>
      {items.length === 0 ? <p className="text-xs text-zinc-500">None.</p> : (
        <div className="space-y-2">
          {items.map((item, index) => (
            <div className="rounded-md bg-white p-2" key={`${item.title}-${index}`}>
              <p className="text-xs font-semibold text-zinc-800">{item.title}</p>
              {item.body ? <p className="mt-1 text-xs leading-5 text-zinc-500">{item.body}</p> : null}
              {item.due_date ? <p className="mt-1 text-xs text-amber-700">{item.due_date}</p> : null}
              <button className="btn mt-2 h-7 px-2 text-[11px]" onClick={() => onApprove(item)} type="button">
                {actionLabel}
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function panelTitle(panel: EmailPanel) {
  switch (panel) {
    case "work":
      return "Link to work";
    case "assets":
      return "Attachments & links";
    case "drafts":
      return "Drafts";
    case "digest":
      return "Weekly digest";
    case "graph":
      return "CC/BCC graph";
    case "ai":
      return "AI review";
  }
}

function priorityBadgeColor(priority: string) {
  switch (priority?.toLowerCase()) {
    case "high": return "bg-red-50 text-red-700";
    case "medium": return "bg-amber-50 text-amber-700";
    default: return "bg-zinc-100 text-zinc-500";
  }
}

function cleanLabelName(name: string): string | null {
  if (name.startsWith("CATEGORY_")) return null;
  if (name === "INBOX") return null;
  const map: Record<string, string> = {
    IMPORTANT: "Important",
    STARRED: "Starred",
    SENT: "Sent",
    DRAFT: "Draft",
    SPAM: "Spam",
    TRASH: "Trash",
    UNREAD: "Unread",
  };
  return map[name] ?? name;
}

function triageTone(category: string) {
  switch (category) {
    case "spam":
      return "bg-red-50 text-red-700";
    case "action_required":
      return "bg-amber-50 text-amber-700";
    case "meeting":
      return "bg-sky-50 text-sky-700";
    case "newsletter":
    case "archive":
    case "receipt":
      return "bg-zinc-100 text-zinc-600";
    case "personal":
      return "bg-emerald-50 text-emerald-700";
    case "work":
      return "bg-violet-50 text-violet-700";
    default:
      return "bg-violet-50 text-violet-700";
  }
}

function isWorkMailView(value: string): value is WorkMailViewId {
  return workMailViews.some((view) => view.id === value);
}

function categoryLabel(category: GmailAiCategory | string) {
  const labels: Record<string, string> = {
    personal: "Personal",
    work: "Work",
    action_required: "Action",
    newsletter: "Newsletter",
    receipt: "Receipt",
    meeting: "Meeting",
    archive: "Archive",
    spam: "Spam",
    other: "Other",
  };
  return labels[category] ?? "Other";
}

const AI_LABEL_COLORS: Record<string, string> = {
  // danger / security
  alert:        "bg-red-50 text-red-600 ring-1 ring-red-100",
  warning:      "bg-red-50 text-red-600 ring-1 ring-red-100",
  security:     "bg-red-50 text-red-600 ring-1 ring-red-100",
  // time-sensitive
  otp:          "bg-amber-50 text-amber-700 ring-1 ring-amber-100",
  verification: "bg-amber-50 text-amber-700 ring-1 ring-amber-100",
  code:         "bg-amber-50 text-amber-700 ring-1 ring-amber-100",
  // positive / action
  invite:       "bg-emerald-50 text-emerald-700 ring-1 ring-emerald-100",
  invitation:   "bg-emerald-50 text-emerald-700 ring-1 ring-emerald-100",
  // financial
  invoice:      "bg-indigo-50 text-indigo-600 ring-1 ring-indigo-100",
  payment:      "bg-indigo-50 text-indigo-600 ring-1 ring-indigo-100",
  receipt:      "bg-indigo-50 text-indigo-600 ring-1 ring-indigo-100",
  // follow-up / reminder
  "follow-up":  "bg-orange-50 text-orange-600 ring-1 ring-orange-100",
  followup:     "bg-orange-50 text-orange-600 ring-1 ring-orange-100",
  reminder:     "bg-orange-50 text-orange-600 ring-1 ring-orange-100",
  // informational
  notification: "bg-sky-50 text-sky-600 ring-1 ring-sky-100",
  update:       "bg-sky-50 text-sky-600 ring-1 ring-sky-100",
  // calendar / meeting
  meeting:      "bg-teal-50 text-teal-700 ring-1 ring-teal-100",
  calendar:     "bg-teal-50 text-teal-700 ring-1 ring-teal-100",
  // low-signal
  promo:        "bg-zinc-100 text-zinc-500",
  newsletter:   "bg-zinc-100 text-zinc-500",
  unsubscribe:  "bg-zinc-100 text-zinc-500",
};

function aiLabelColors(label: string): string {
  return AI_LABEL_COLORS[label.toLowerCase()] ?? "bg-violet-50 text-violet-600 ring-1 ring-violet-100";
}

function AiLabelBadge({ label, size = "sm" }: { label: string; size?: "sm" | "xs" }) {
  return (
    <span
      className={[
        "shrink-0 rounded-full font-semibold",
        size === "xs" ? "px-1.5 py-0.5 text-[10px]" : "px-2 py-0.5 text-[11px]",
        aiLabelColors(label),
      ].join(" ")}
    >
      {label}
    </span>
  );
}

function actionLabel(action: string) {
  return action
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function needsGmailReconnect(error: string) {
  return error.toLowerCase().includes("modify permission");
}

function formatUnix(value: number) {
  return formatDateTime(new Date(value * 1000).toISOString());
}

function compactDate(value: number) {
  const date = new Date(value * 1000);
  const now = new Date();
  const diffDays = Math.floor((now.getTime() - date.getTime()) / 86_400_000);
  if (date.getDate() === now.getDate() && diffDays < 1) {
    return date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  }
  if (diffDays < 7) return date.toLocaleDateString(undefined, { weekday: "short" });
  if (date.getFullYear() === now.getFullYear()) {
    return date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
  }
  return date.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "2-digit" });
}
