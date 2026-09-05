import { FormEvent, memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useIpcQuery, qk } from "../lib/queries";
import { queryClient } from "../lib/queryClient";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import { listen } from "@tauri-apps/api/event";
import {
  ArrowUpRight,
  Bot,
  ChevronDown,
  ChevronUp,
  Inbox,
  Layers,
  Lightbulb,
  Link as LinkIcon,
  RefreshCw,
  Rocket,
  Sparkles,
  SquareCheck,
  Undo2,
  Wand2,
  X,
} from "lucide-react";
import {
  applyCapturePromotionSuggestion,
  createInitiative,
  createStakeholder,
  dismissCapture,
  getCapturePromotionAccuracy,
  getCapturePromotionSuggestion,
  listCaptures,
  listDeliverables,
  listInitiatives,
  listStakeholders,
  promoteCaptureToDeliverable,
  promoteCaptureToInitiative,
  promoteCaptureToTask,
  restoreCaptureToInbox,
  suggestCapture,
  suggestCapturePromotion,
  undoCapturePromotion,
} from "../lib/ipc";
import type {
  AppliedPromotion,
  Capture,
  CapturePromotionAlternative,
  CapturePromotionSuggestion,
  CaptureStatus,
  CreateDeliverableInput,
  CreateInitiativeInput,
  Deliverable,
  Initiative,
  PromoteCaptureToTaskInput,
  PromotionAccuracySummary,
  PromotionKind,
  Stakeholder,
} from "../lib/types";
import { captureStatusLabels } from "../lib/types";
import { formatDateTime } from "../lib/format";
import { DeliverableForm } from "../components/DeliverableForm";
import { InitiativeForm } from "../components/InitiativeForm";
import { TokenPicker } from "../components/TokenPicker";
import { toast } from "../lib/toast";
import { EmptyState } from "../components/EmptyState";

const KIND_ICONS: Record<string, React.ReactNode> = {
  thought: <Lightbulb size={13} />,
  claude_link: <Bot size={13} />,
  artifact_link: <LinkIcon size={13} />,
};

const KIND_LABELS: Record<string, string> = {
  thought: "Thought",
  claude_link: "Claude link",
  artifact_link: "Link",
};

const STATUS_TABS: { id: CaptureStatus; label: string }[] = [
  { id: "inbox", label: "Inbox" },
  { id: "suggested", label: "Saved" },
  { id: "promoted", label: "Promoted" },
  { id: "dismissed", label: "Dismissed" },
];

type PromoteMode = "task" | "deliverable" | "initiative";

export function CaptureInbox() {
  const navigate = useNavigate();
  const [searchParams, setSearchParams] = useSearchParams();
  const [status, setStatus] = useState<CaptureStatus>("inbox");
  const [selectedId, setSelectedId] = useState<string | null>(searchParams.get("selected"));
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accuracy, setAccuracy] = useState<PromotionAccuracySummary | null>(null);
  const [pendingOverride, setPendingOverride] = useState<
    { kind: PromotionKind; targetId: string | null } | null
  >(null);

  // ── React-query data ─────────────────────────────────────────────────────────
  const { data: captures = [], isLoading } = useIpcQuery(
    qk.captures.list({ status }),
    () => listCaptures({ status }),
  );
  const { data: initiatives = [] } = useIpcQuery(qk.initiatives.list, listInitiatives);
  const { data: stakeholders = [] } = useIpcQuery(qk.stakeholders.list, listStakeholders);
  const { data: deliverables = [] } = useIpcQuery(qk.deliverables.list(), listDeliverables);
  const { data: suggestion = null, isLoading: suggestionLoading } = useIpcQuery(
    qk.captures.suggestion(selectedId ?? ""),
    () => getCapturePromotionSuggestion(selectedId!),
    { enabled: !!selectedId },
  );
  // ──────────────────────────────────────────────────────────────────────────

  // Fixed-height list rows + virtualization. Cards have a strict height so the
  // virtualizer's estimate is always correct; no measurement drift, no gaps.
  const CAPTURE_ROW_HEIGHT = 96;
  const listScrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: captures.length,
    getScrollElement: () => listScrollRef.current,
    estimateSize: () => CAPTURE_ROW_HEIGHT,
    overscan: 8,
  });

  useEffect(() => {
    getCapturePromotionAccuracy()
      .then(setAccuracy)
      .catch(() => setAccuracy(null));
  }, []);

  useEffect(() => {
    const nextSelected = searchParams.get("selected");
    if (nextSelected) setSelectedId(nextSelected);
  }, [searchParams]);

  useEffect(() => {
    if (selectedId && captures.some((c) => c.id === selectedId)) return;
    setSelectedId(captures[0]?.id ?? null);
  }, [captures, selectedId]);

  const selectedCapture = useMemo(
    () => captures.find((c) => c.id === selectedId) ?? null,
    [captures, selectedId],
  );

  // Invalidate suggestion when it finishes computing in the background
  useEffect(() => {
    let cancelled = false;
    const unlistenPromise = listen<{ capture_id: string }>(
      "capture:promotion_ready",
      (event) => {
        if (cancelled) return;
        if (event.payload?.capture_id) {
          void queryClient.invalidateQueries({
            queryKey: qk.captures.suggestion(event.payload.capture_id),
          });
        }
      },
    );
    return () => {
      cancelled = true;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  function selectCapture(id: string) {
    setSelectedId(id);
    setSearchParams({ selected: id });
  }

  function changeStatus(nextStatus: CaptureStatus) {
    setStatus(nextStatus);
    setSelectedId(null);
    setSearchParams({});
  }

  const handleDismiss = useCallback(async (id: string) => {
    try {
      setError(null);
      await dismissCapture(id);
      void queryClient.invalidateQueries({ queryKey: qk.captures.all });
    } catch (caught) {
      setError(String(caught));
    }
  }, []);

  const handleSuggest = useCallback(async (id: string) => {
    try {
      setError(null);
      await suggestCapture(id);
      void queryClient.invalidateQueries({ queryKey: qk.captures.all });
    } catch (caught) {
      setError(String(caught));
    }
  }, []);

  const handleRestoreToInbox = useCallback(async (id: string) => {
    try {
      setError(null);
      await restoreCaptureToInbox(id);
      void queryClient.invalidateQueries({ queryKey: qk.captures.all });
    } catch (caught) {
      setError(String(caught));
    }
  }, []);

  async function recordOverrideIfPending(
    kind: PromotionKind,
    targetId: string | null,
    appliedEntityKind: string,
    appliedEntityId: string,
  ) {
    if (!suggestion || suggestion.status !== "pending" || !selectedCapture) return;
    try {
      await applyCapturePromotionSuggestion({
        captureId: selectedCapture.id,
        suggestionId: suggestion.id,
        overrideKind: kind,
        overrideTargetId: targetId,
      });
    } catch {
      // The capture is already promoted; failing to record the RL event
      // shouldn't block the user.
    }
    // Best-effort: ensure the suggestion row reflects what the user actually did.
    void appliedEntityKind;
    void appliedEntityId;
  }

  async function handlePromoteToDeliverable(input: CreateDeliverableInput) {
    if (!selectedCapture) return;
    try {
      setError(null);
      setIsSaving(true);
      const deliverable = await promoteCaptureToDeliverable(selectedCapture.id, input);
      await recordOverrideIfPending(
        "deliverable",
        input.initiative_ids[0] ?? null,
        "deliverable",
        deliverable.id,
      );
      await refreshAccuracyOnly();
      navigate(`/deliverables/${deliverable.id}`);
    } catch (caught) {
      setError(String(caught));
      setIsSaving(false);
    }
  }

  async function handlePromoteToInitiative(input: CreateInitiativeInput) {
    if (!selectedCapture) return;
    try {
      setError(null);
      setIsSaving(true);
      const initiative = await promoteCaptureToInitiative(selectedCapture.id, input);
      await recordOverrideIfPending("initiative", null, "initiative", initiative.id);
      await refreshAccuracyOnly();
      navigate(`/initiatives/${initiative.id}`);
    } catch (caught) {
      setError(String(caught));
      setIsSaving(false);
    }
  }

  async function handlePromoteToTask(input: PromoteCaptureToTaskInput) {
    if (!selectedCapture) return;
    try {
      setError(null);
      setIsSaving(true);
      const task = await promoteCaptureToTask(selectedCapture.id, input);
      await recordOverrideIfPending("task", input.deliverable_id, "task", task.id);
      await refreshAccuracyOnly();
      navigate(`/deliverables/${input.deliverable_id}`);
    } catch (caught) {
      setError(String(caught));
      setIsSaving(false);
    }
  }

  function refreshAccuracyOnly() {
    return getCapturePromotionAccuracy()
      .then(setAccuracy)
      .catch(() => {});
  }

  async function handleCreateStakeholder(name: string) {
    const created = await createStakeholder({ name });
    void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all });
    return created;
  }

  async function handleCreateInitiative(title: string) {
    const created = await createInitiative({ title, framing: "", status: "live" });
    void queryClient.invalidateQueries({ queryKey: qk.initiatives.all });
    return created;
  }

  async function refreshAfterPromotion() {
    void queryClient.invalidateQueries({ queryKey: qk.captures.all });
    getCapturePromotionAccuracy()
      .then(setAccuracy)
      .catch(() => {});
  }

  function navigateToApplied(applied: AppliedPromotion) {
    if (applied.applied_entity_kind === "deliverable") {
      navigate(`/deliverables/${applied.applied_entity_id}`);
    } else if (applied.applied_entity_kind === "initiative") {
      navigate(`/initiatives/${applied.applied_entity_id}`);
    } else if (applied.applied_entity_kind === "task" && applied.capture.promoted_deliverable_id) {
      navigate(`/deliverables/${applied.capture.promoted_deliverable_id}`);
    }
  }

  function offerUndoToast(applied: AppliedPromotion, suggestionId: string) {
    toast.success(`Promoted as ${applied.kind}`, {
      duration: 60_000,
      action: {
        label: "Undo",
        onClick: () => {
          void (async () => {
            try {
              await undoCapturePromotion(suggestionId);
              await refreshAfterPromotion();
              if (selectedId) void queryClient.invalidateQueries({ queryKey: qk.captures.suggestion(selectedId) });
              toast.info("Promotion undone");
            } catch (caught) {
              setError(String(caught));
            }
          })();
        },
      },
    });
  }

  async function handleApplyPrimary() {
    if (!selectedCapture || !suggestion) return;
    try {
      setIsSaving(true);
      const applied = await applyCapturePromotionSuggestion({
        captureId: selectedCapture.id,
        suggestionId: suggestion.id,
      });
      offerUndoToast(applied, suggestion.id);
      await refreshAfterPromotion();
      navigateToApplied(applied);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleApplyAlternative(index: number) {
    if (!selectedCapture || !suggestion) return;
    try {
      setIsSaving(true);
      const applied = await applyCapturePromotionSuggestion({
        captureId: selectedCapture.id,
        suggestionId: suggestion.id,
        overrideAlternativeIndex: index,
      });
      offerUndoToast(applied, suggestion.id);
      await refreshAfterPromotion();
      navigateToApplied(applied);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  function handleOverride() {
    if (!suggestion) return;
    setPendingOverride({
      kind: (suggestion.kind || "task") as PromotionKind,
      targetId: suggestion.target_id,
    });
  }

  function clearOverride() {
    setPendingOverride(null);
  }

  async function handleRetrySuggest() {
    if (!selectedCapture) return;
    try {
      const next = await suggestCapturePromotion(selectedCapture.id);
      queryClient.setQueryData(qk.captures.suggestion(selectedCapture.id), next);
    } catch (caught) {
      setError(String(caught));
    }
  }


  return (
    <div className="mx-auto grid min-h-full max-w-7xl gap-5 px-5 py-6 lg:grid-cols-[320px_minmax(0,1fr)]">
      {/* ── Left: list ── */}
      <aside className="min-w-0">
        <div className="mb-5 flex items-end justify-between gap-3">
          <div>
            <p className="page-kicker">Inbox</p>
            <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">Captures</h1>
          </div>
          <button
            className="btn h-8 w-8 px-0"
            onClick={() => void queryClient.invalidateQueries({ queryKey: qk.captures.all })}
            title="Refresh"
            type="button"
          >
            <RefreshCw size={13} />
          </button>
        </div>

        {/* Status tab bar */}
        <div className="relative mb-4 border-b border-zinc-100">
          <nav className="flex">
            {STATUS_TABS.map((tab) => (
              <button
                className="relative shrink-0 px-4 py-2.5 text-sm transition-colors"
                key={tab.id}
                onClick={() => changeStatus(tab.id)}
                type="button"
              >
                <span
                  className={
                    status === tab.id
                      ? "font-semibold text-zinc-950"
                      : "text-zinc-400 hover:text-zinc-700"
                  }
                >
                  {tab.label}
                </span>
                {status === tab.id && (
                  <motion.div
                    className="absolute bottom-0 left-0 right-0 h-0.5 bg-zinc-900"
                    layoutId="capture-status-indicator"
                    transition={{ type: "spring", stiffness: 500, damping: 40 }}
                  />
                )}
              </button>
            ))}
          </nav>
        </div>

        {error ? (
          <div className="mb-3 rounded-xl border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-600">
            {error}
          </div>
        ) : null}

        <AnimatePresence mode="wait">
          {isLoading ? (
            <motion.div
              animate={{ opacity: 1 }}
              className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
              exit={{ opacity: 0 }}
              initial={{ opacity: 0 }}
              key="loading"
            >
              {[...Array(4)].map((_, i) => (
                <div key={i} className="border-b border-zinc-50 px-4 py-3 last:border-0">
                  <div className="mb-2 h-3 w-20 animate-pulse rounded bg-zinc-100" />
                  <div className="h-3 w-full animate-pulse rounded bg-zinc-100" />
                  <div className="mt-1.5 h-3 w-3/4 animate-pulse rounded bg-zinc-100" />
                </div>
              ))}
            </motion.div>
          ) : captures.length === 0 ? (
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0 }}
              initial={{ opacity: 0, y: 8 }}
              key="empty"
              transition={{ duration: 0.16 }}
            >
              <EmptyState
                variant="inline"
                icon={Inbox}
                title={captureStatusLabels[status] === "Inbox" ? "Inbox at zero" : "Nothing here"}
                description={captureStatusLabels[status] === "Inbox" ? "New captures from the tray widget land here." : "Nothing matches this filter."}
              />
            </motion.div>
          ) : (
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
              exit={{ opacity: 0 }}
              initial={{ opacity: 0, y: 8 }}
              key="list"
              transition={{ duration: 0.16, ease: "easeOut" }}
            >
              <div
                ref={listScrollRef}
                className="overflow-y-auto"
                style={{ height: "calc(100vh - 260px)" }}
              >
                <div
                  style={{
                    height: `${virtualizer.getTotalSize()}px`,
                    position: "relative",
                  }}
                >
                  {virtualizer.getVirtualItems().map((vItem) => {
                    const capture = captures[vItem.index];
                    return (
                      <div
                        key={capture.id}
                        data-capture-row
                        data-index={vItem.index}
                        className="border-b border-zinc-100"
                        style={{
                          position: "absolute",
                          top: 0,
                          left: 0,
                          width: "100%",
                          height: `${CAPTURE_ROW_HEIGHT}px`,
                          transform: `translateY(${vItem.start}px)`,
                        }}
                      >
                        <CaptureCard
                          capture={capture}
                          isSelected={capture.id === selectedId}
                          onClick={() => selectCapture(capture.id)}
                        />
                      </div>
                    );
                  })}
                </div>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </aside>

      {/* ── Right: detail ── */}
      <main className="min-w-0">
        <AnimatePresence mode="wait">
          {selectedCapture ? (
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              initial={{ opacity: 0, y: 8 }}
              key={selectedCapture.id}
              transition={{ duration: 0.16, ease: "easeOut" }}
            >
              <CaptureDetail
                capture={selectedCapture}
                deliverables={deliverables}
                initiatives={initiatives}
                isSaving={isSaving}
                onCreateInitiative={handleCreateInitiative}
                onCreateStakeholder={handleCreateStakeholder}
                onDismiss={handleDismiss}
                onPromoteToDeliverable={handlePromoteToDeliverable}
                onPromoteToInitiative={handlePromoteToInitiative}
                onPromoteToTask={handlePromoteToTask}
                onRestoreToInbox={handleRestoreToInbox}
                onSuggest={handleSuggest}
                stakeholders={stakeholders}
                suggestion={suggestion}
                suggestionLoading={suggestionLoading}
                accuracy={accuracy}
                pendingOverride={pendingOverride}
                onApplySuggestion={handleApplyPrimary}
                onApplyAlternative={handleApplyAlternative}
                onOverrideSuggestion={handleOverride}
                onClearOverride={clearOverride}
                onRetrySuggest={handleRetrySuggest}
              />
            </motion.div>
          ) : !isLoading ? (
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              className="flex min-h-[28rem] items-center justify-center rounded-2xl border border-dashed border-zinc-200 bg-white"
              exit={{ opacity: 0 }}
              initial={{ opacity: 0, y: 8 }}
              key="empty-detail"
              transition={{ duration: 0.16, ease: "easeOut" }}
            >
              <div className="text-center">
                <Inbox className="mx-auto mb-3 text-zinc-200" size={36} />
                <p className="text-sm font-semibold text-zinc-700">Select a capture</p>
                <p className="mt-1 max-w-xs text-xs leading-5 text-zinc-400">
                  Choose something from the list to review and promote.
                </p>
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>
      </main>
    </div>
  );
}

// ── Capture list card ───────────────────────────────────────────────────────

interface CaptureCardProps {
  capture: Capture;
  isSelected: boolean;
  onClick: () => void;
}

const CaptureCard = memo(function CaptureCard({ capture, isSelected, onClick }: CaptureCardProps) {
  return (
    <button
      className={[
        "group relative h-full w-full overflow-hidden py-2.5 pl-4 pr-4 text-left transition-colors duration-150",
        isSelected ? "bg-zinc-50" : "hover:bg-zinc-50/60",
      ].join(" ")}
      onClick={onClick}
      type="button"
    >
      {isSelected && (
        <span className="absolute inset-y-0 left-0 w-0.5 rounded-r-full bg-zinc-900" />
      )}
      <div className="mb-1 flex items-center gap-1.5">
        <span
          className={[
            "flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-semibold",
            capture.kind === "thought"
              ? "bg-violet-50 text-violet-600"
              : capture.kind === "claude_link"
                ? "bg-sky-50 text-sky-600"
                : "bg-zinc-100 text-zinc-500",
          ].join(" ")}
        >
          {KIND_ICONS[capture.kind]}
          {KIND_LABELS[capture.kind]}
        </span>
        <span className="ml-auto text-[10px] text-zinc-400">
          {formatDateTime(capture.created_at)}
        </span>
      </div>
      {(() => {
        const { labels, cleanBody } = parseCaptureTags(capture.body);
        return (
          <>
            {labels.length > 0 && (
              <div className="mb-1 flex flex-wrap gap-1">
                {labels.slice(0, 3).map((l) => (
                  <span
                    className="rounded bg-zinc-100 px-1.5 py-px text-[9px] font-semibold uppercase tracking-wide text-zinc-400"
                    key={l}
                  >
                    {l}
                  </span>
                ))}
              </div>
            )}
            <p
              className={[
                "text-sm leading-5 text-zinc-700",
                labels.length > 0 ? "line-clamp-1" : "line-clamp-2",
              ].join(" ")}
            >
              {cleanBody}
            </p>
          </>
        );
      })()}
    </button>
  );
});

// ── Capture detail ──────────────────────────────────────────────────────────

interface CaptureDetailProps {
  capture: Capture;
  initiatives: Initiative[];
  stakeholders: Stakeholder[];
  deliverables: Deliverable[];
  isSaving: boolean;
  onPromoteToDeliverable: (input: CreateDeliverableInput) => Promise<void>;
  onPromoteToInitiative: (input: CreateInitiativeInput) => Promise<void>;
  onPromoteToTask: (input: PromoteCaptureToTaskInput) => Promise<void>;
  onDismiss: (id: string) => Promise<void>;
  onSuggest: (id: string) => Promise<void>;
  onRestoreToInbox: (id: string) => Promise<void>;
  onCreateStakeholder: (name: string) => Promise<Stakeholder>;
  onCreateInitiative: (title: string) => Promise<Initiative>;
  suggestion: CapturePromotionSuggestion | null;
  suggestionLoading: boolean;
  accuracy: PromotionAccuracySummary | null;
  pendingOverride: { kind: PromotionKind; targetId: string | null } | null;
  onApplySuggestion: () => Promise<void>;
  onApplyAlternative: (index: number) => Promise<void>;
  onOverrideSuggestion: () => void;
  onClearOverride: () => void;
  onRetrySuggest: () => Promise<void>;
}

function CaptureDetail({
  capture,
  initiatives,
  stakeholders,
  deliverables,
  isSaving,
  onPromoteToDeliverable,
  onPromoteToInitiative,
  onPromoteToTask,
  onDismiss,
  onSuggest,
  onRestoreToInbox,
  onCreateStakeholder,
  onCreateInitiative,
  suggestion,
  suggestionLoading,
  accuracy,
  pendingOverride,
  onApplySuggestion,
  onApplyAlternative,
  onOverrideSuggestion,
  onClearOverride,
  onRetrySuggest,
}: CaptureDetailProps) {
  const isActionable = capture.status === "inbox" || capture.status === "suggested";
  const canPromote = isActionable && capture.kind !== "claude_link";
  const canPromoteToInitiative = isActionable && capture.kind === "thought";
  const canIngest = capture.kind === "claude_link" && capture.status === "inbox";

  const defaultMode: PromoteMode = canPromoteToInitiative ? "initiative" : "task";
  const [promoteMode, setPromoteMode] = useState<PromoteMode>(defaultMode);

  useEffect(() => {
    if (pendingOverride) {
      const next = pendingOverride.kind === "initiative" && canPromoteToInitiative
        ? "initiative"
        : pendingOverride.kind === "deliverable"
          ? "deliverable"
          : "task";
      setPromoteMode(next);
    }
  }, [pendingOverride, canPromoteToInitiative]);

  const showSuggestionPanel =
    isActionable && (suggestion !== null || suggestionLoading);

  const promoteTabs: { id: PromoteMode; label: string; icon: React.ReactNode }[] = [
    { id: "task", label: "Task", icon: <SquareCheck size={12} /> },
    { id: "deliverable", label: "Deliverable", icon: <Layers size={12} /> },
    ...(canPromoteToInitiative
      ? [{ id: "initiative" as PromoteMode, label: "Initiative", icon: <Rocket size={12} /> }]
      : []),
  ];

  const { labels: captureTags, cleanBody } = useMemo(
    () => parseCaptureTags(capture.body),
    [capture.body],
  );

  const deliverableFormInitial = useMemo<CreateDeliverableInput>(() => {
    if (capture.kind === "artifact_link") {
      return {
        title: titleFromCapture(capture.body),
        type: "other",
        state: "drafting",
        claim: "",
        artifact_url: capture.body,
        conversation_id: null,
        stakeholder_id: null,
        stakeholder_ids: [],
        initiative_ids: [],
      };
    }
    return {
      title: titleFromCapture(capture.body),
      type: "analysis",
      state: "drafting",
      claim: cleanBody,
      artifact_url: null,
      conversation_id: null,
      stakeholder_id: null,
      stakeholder_ids: [],
      initiative_ids: [],
    };
  }, [capture, cleanBody]);

  const initiativeFormInitial = useMemo<CreateInitiativeInput>(
    () => ({
      title: titleFromCapture(capture.body),
      framing: cleanBody,
      status: "live",
    }),
    [capture, cleanBody],
  );

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      {/* Header */}
      <div className="border-b border-zinc-100 px-5 py-4">
        <div className="mb-3 flex items-center gap-2">
          <span
            className={[
              "flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-semibold",
              capture.kind === "thought"
                ? "bg-violet-50 text-violet-600"
                : capture.kind === "claude_link"
                  ? "bg-sky-50 text-sky-600"
                  : "bg-zinc-100 text-zinc-500",
            ].join(" ")}
          >
            {KIND_ICONS[capture.kind]}
            {KIND_LABELS[capture.kind]}
          </span>
          <span
            className={[
              "rounded-md px-1.5 py-0.5 text-[10px] font-semibold",
              capture.status === "inbox"
                ? "bg-amber-50 text-amber-700"
                : capture.status === "suggested"
                  ? "bg-sky-50 text-sky-700"
                  : capture.status === "promoted"
                    ? "bg-emerald-50 text-emerald-700"
                    : "bg-zinc-100 text-zinc-400",
            ].join(" ")}
          >
            {captureStatusLabels[capture.status]}
          </span>
          <span className="ml-auto text-xs text-zinc-400">{formatDateTime(capture.created_at)}</span>
        </div>

        {captureTags.length > 0 && (
          <div className="mb-2 flex flex-wrap gap-1">
            {captureTags.map((tag) => (
              <span
                className="rounded-md bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold text-zinc-500"
                key={tag}
              >
                {tag}
              </span>
            ))}
          </div>
        )}
        <p className="whitespace-pre-wrap text-sm leading-7 text-zinc-800">{cleanBody}</p>
      </div>

      {/* Action row */}
      <div className="flex items-center gap-2 border-b border-zinc-100 px-5 py-3">
        {capture.status === "inbox" ? (
          <button
            className="text-xs text-zinc-400 transition-colors hover:text-zinc-700"
            disabled={isSaving}
            onClick={() => void onSuggest(capture.id)}
            type="button"
          >
            Save for later
          </button>
        ) : null}
        {capture.status === "suggested" ? (
          <button
            className="text-xs text-zinc-400 transition-colors hover:text-zinc-700"
            disabled={isSaving}
            onClick={() => void onRestoreToInbox(capture.id)}
            type="button"
          >
            Back to inbox
          </button>
        ) : null}
        {isActionable ? (
          <button
            className="flex items-center gap-1 text-xs text-zinc-400 transition-colors hover:text-red-500"
            disabled={isSaving}
            onClick={() => void onDismiss(capture.id)}
            type="button"
          >
            <X size={11} />
            Dismiss
          </button>
        ) : null}

        {/* Promoted link */}
        {capture.promoted_deliverable_id ? (
          <Link
            className="ml-auto flex items-center gap-1 text-xs font-medium text-sky-600 hover:text-sky-700"
            to={`/deliverables/${capture.promoted_deliverable_id}`}
          >
            <ArrowUpRight size={12} />
            {capture.promoted_deliverable_title ?? "View deliverable"}
          </Link>
        ) : capture.promoted_initiative_id ? (
          <Link
            className="ml-auto flex items-center gap-1 text-xs font-medium text-sky-600 hover:text-sky-700"
            to={`/initiatives/${capture.promoted_initiative_id}`}
          >
            <ArrowUpRight size={12} />
            {capture.promoted_initiative_title ?? "View initiative"}
          </Link>
        ) : capture.promoted_task_id ? (
          <span className="ml-auto flex items-center gap-1 text-xs font-medium text-emerald-600">
            <SquareCheck size={12} />
            {capture.promoted_task_title ?? "Task created"}
          </span>
        ) : null}
      </div>

      {/* AI suggestion */}
      {showSuggestionPanel ? (
        <div className="border-b border-zinc-100 px-5 py-4">
          <PromotionSuggestionPanel
            suggestion={suggestion}
            loading={suggestionLoading}
            accuracy={accuracy}
            disabled={isSaving}
            isOverriding={pendingOverride !== null}
            onApply={onApplySuggestion}
            onApplyAlternative={onApplyAlternative}
            onOverride={onOverrideSuggestion}
            onClearOverride={onClearOverride}
            onRetry={onRetrySuggest}
          />
        </div>
      ) : null}

      {/* Claude link ingest */}
      {canIngest ? (
        <div className="px-5 py-4">
          <p className="mb-3 text-xs text-zinc-500">
            This is a Claude conversation link. Ingest it to extract structured work items.
          </p>
          <Link
            className="btn btn-primary"
            to={`/conversations/ingest?captureId=${encodeURIComponent(capture.id)}`}
          >
            <ArrowUpRight size={14} />
            Ingest conversation
          </Link>
        </div>
      ) : null}

      {/* Promote section */}
      {canPromote ? (
        <div className="px-5 py-4">
          <p className="page-kicker mb-3">Promote as</p>

          {/* Mode tab bar */}
          <div className="relative mb-4 border-b border-zinc-100">
            <nav className="flex gap-0">
              {promoteTabs.map((tab) => (
                <button
                  className="relative flex items-center gap-1.5 px-4 py-2 text-sm transition-colors"
                  key={tab.id}
                  onClick={() => setPromoteMode(tab.id)}
                  type="button"
                >
                  <span
                    className={
                      promoteMode === tab.id
                        ? "font-semibold text-zinc-950"
                        : "text-zinc-400 hover:text-zinc-700"
                    }
                  >
                    {tab.icon}
                  </span>
                  <span
                    className={
                      promoteMode === tab.id
                        ? "font-semibold text-zinc-950"
                        : "text-zinc-400 hover:text-zinc-700"
                    }
                  >
                    {tab.label}
                  </span>
                  {promoteMode === tab.id && (
                    <motion.div
                      className="absolute bottom-0 left-0 right-0 h-0.5 bg-zinc-900"
                      layoutId="promote-mode-indicator"
                      transition={{ type: "spring", stiffness: 500, damping: 40 }}
                    />
                  )}
                </button>
              ))}
            </nav>
          </div>

          {/* Mode form */}
          <AnimatePresence mode="wait">
            <motion.div
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              initial={{ opacity: 0, y: 8 }}
              key={promoteMode}
              transition={{ duration: 0.14, ease: "easeOut" }}
            >
              {promoteMode === "task" ? (
                <TaskPromoteForm
                  body={cleanBody}
                  deliverables={deliverables}
                  isSaving={isSaving}
                  onSubmit={onPromoteToTask}
                />
              ) : promoteMode === "deliverable" ? (
                <DeliverableForm
                  initialValue={deliverableFormInitial}
                  initiatives={initiatives}
                  isSubmitting={isSaving}
                  onCreateInitiative={onCreateInitiative}
                  onCreateStakeholder={onCreateStakeholder}
                  onSubmit={onPromoteToDeliverable}
                  stakeholders={stakeholders}
                  submitLabel="Promote as Deliverable"
                />
              ) : (
                <InitiativeForm
                  initialValue={initiativeFormInitial}
                  isSubmitting={isSaving}
                  onSubmit={onPromoteToInitiative}
                  submitLabel="Promote as Initiative"
                />
              )}
            </motion.div>
          </AnimatePresence>
        </div>
      ) : null}
    </section>
  );
}

// ── Task promote form ───────────────────────────────────────────────────────

interface TaskPromoteFormProps {
  body: string;
  deliverables: Deliverable[];
  isSaving: boolean;
  onSubmit: (input: PromoteCaptureToTaskInput) => Promise<void>;
}

function TaskPromoteForm({ body, deliverables, isSaving, onSubmit }: TaskPromoteFormProps) {
  const [title, setTitle] = useState(titleFromCapture(body));
  const [deliverableId, setDeliverableId] = useState(deliverables[0]?.id ?? "");
  const [notes, setNotes] = useState("");

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    if (!deliverableId || !title.trim()) return;
    await onSubmit({ deliverable_id: deliverableId, title: title.trim(), notes: notes || null });
  }

  if (deliverables.length === 0) {
    return (
      <div className="rounded-xl border border-zinc-100 bg-zinc-50 px-4 py-6 text-center">
        <SquareCheck className="mx-auto mb-2 text-zinc-200" size={20} />
        <p className="text-sm text-zinc-400">No deliverables yet.</p>
        <p className="mt-0.5 text-xs text-zinc-300">Create a deliverable first, then attach tasks to it.</p>
      </div>
    );
  }

  return (
    <form className="space-y-3" onSubmit={handleSubmit}>
      <div>
        <label className="field-label mb-1 block">Task title</label>
        <input
          className="field-control h-9 text-sm"
          onChange={(e) => setTitle(e.currentTarget.value)}
          placeholder="Task title…"
          required
          value={title}
        />
      </div>

      <div className="space-y-1.5">
        <span className="field-label">Parent deliverable</span>
        <TokenPicker
          getId={(d) => d.id}
          getLabel={(d) => d.title}
          items={deliverables}
          onToggle={(id) => setDeliverableId(id)}
          placeholder="Search deliverables…"
          selectedIds={deliverableId ? [deliverableId] : []}
          singleSelect
        />
      </div>

      <div>
        <label className="field-label mb-1 block">Notes (optional)</label>
        <textarea
          className="field-control resize-none text-sm"
          onChange={(e) => setNotes(e.currentTarget.value)}
          placeholder="Any additional context…"
          rows={2}
          value={notes}
        />
      </div>

      <div className="flex justify-end pt-1">
        <button
          className="btn btn-primary"
          disabled={isSaving || !title.trim() || !deliverableId}
          type="submit"
        >
          <SquareCheck size={13} />
          Promote as Task
        </button>
      </div>
    </form>
  );
}

// ── Helpers ─────────────────────────────────────────────────────────────────

function parseCaptureTags(body: string): { labels: string[]; cleanBody: string } {
  const labels: string[] = [];
  let remaining = body.trim();
  const tagRe = /^\[([^\]]+)\]\s*/;
  let m: RegExpExecArray | null;
  while ((m = tagRe.exec(remaining)) !== null) {
    labels.push(m[1]);
    remaining = remaining.slice(m[0].length);
  }
  return { labels, cleanBody: remaining.trim() };
}

function titleFromCapture(body: string) {
  const { cleanBody } = parseCaptureTags(body);
  const words = cleanBody.split(/\s+/).slice(0, 8).join(" ");
  if (!words) return "Untitled";
  return words.length <= 80 ? words : `${words.slice(0, 77)}…`;
}

// ── AI promotion suggestion panel ───────────────────────────────────────────

interface PromotionSuggestionPanelProps {
  suggestion: CapturePromotionSuggestion | null;
  loading: boolean;
  accuracy: PromotionAccuracySummary | null;
  disabled: boolean;
  isOverriding: boolean;
  onApply: () => Promise<void>;
  onApplyAlternative: (index: number) => Promise<void>;
  onOverride: () => void;
  onClearOverride: () => void;
  onRetry: () => Promise<void>;
}

const PROMOTION_KIND_LABEL: Record<PromotionKind, string> = {
  task: "Task",
  deliverable: "Deliverable",
  initiative: "Initiative",
};

function PromotionSuggestionPanel({
  suggestion,
  loading,
  accuracy,
  disabled,
  isOverriding,
  onApply,
  onApplyAlternative,
  onOverride,
  onClearOverride,
  onRetry,
}: PromotionSuggestionPanelProps) {
  const [showAlternatives, setShowAlternatives] = useState(false);

  if (loading && !suggestion) {
    return (
      <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
        <div className="mb-2 h-3 w-32 animate-pulse rounded bg-zinc-200" />
        <div className="h-3 w-3/4 animate-pulse rounded bg-zinc-100" />
      </div>
    );
  }

  if (!suggestion) return null;

  if (suggestion.status === "errored") {
    return (
      <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4 text-xs text-zinc-500">
        <div className="mb-1 flex items-center gap-1.5 font-semibold text-zinc-700">
          <Wand2 size={12} className="text-zinc-400" />
          Couldn't generate a suggestion
        </div>
        <p className="mb-3 leading-relaxed text-zinc-500">
          {suggestion.error_reason ?? "Unknown error"}
        </p>
        <button
          className="btn h-7 px-2 text-xs"
          disabled={disabled}
          onClick={() => void onRetry()}
          type="button"
        >
          <RefreshCw size={11} />
          Retry
        </button>
      </div>
    );
  }

  if (
    suggestion.status === "accepted" ||
    suggestion.status === "accepted_alternative" ||
    suggestion.status === "overridden" ||
    suggestion.status === "undone"
  ) {
    // After resolution the panel collapses to a small line so the user knows
    // a suggestion ran but the next promote action isn't blocked behind it.
    return null;
  }

  const isHighConfidence = suggestion.confidence >= 0.55;
  const kindLabel = PROMOTION_KIND_LABEL[suggestion.kind as PromotionKind] ?? "Task";
  const confidencePct = Math.round(suggestion.confidence * 100);

  const containerClass = isHighConfidence
    ? "rounded-xl border border-zinc-100 bg-white p-4 shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
    : "rounded-xl border border-dashed border-zinc-200 bg-zinc-50 p-4";
  const labelClass = isHighConfidence ? "text-zinc-700" : "text-zinc-400";

  return (
    <div className={containerClass}>
      <div className="mb-1.5 flex items-center gap-2">
        <Wand2
          size={13}
          className={isHighConfidence ? "text-violet-500" : "text-zinc-400"}
        />
        <span className={`text-xs font-semibold ${labelClass}`}>
          {isHighConfidence ? "Suggested" : "Low-confidence suggestion"}
        </span>
        <span className={`text-xs font-medium ${labelClass}`}>
          {kindLabel}
          {suggestion.target_title ? (
            <>
              {" "}
              on <span className="text-zinc-800">{suggestion.target_title}</span>
            </>
          ) : null}
        </span>
        <span className="ml-auto text-[10px] font-semibold text-zinc-400">
          {confidencePct}%
        </span>
      </div>
      {suggestion.rationale ? (
        <p className="mb-3 line-clamp-2 text-xs leading-5 text-zinc-500">
          {suggestion.rationale}
        </p>
      ) : null}
      <div className="flex flex-wrap items-center gap-2">
        <button
          className="btn btn-primary h-7 px-3 text-xs"
          disabled={disabled}
          onClick={() => void onApply()}
          type="button"
        >
          <Sparkles size={11} />
          Apply suggestion
        </button>
        {isOverriding ? (
          <button
            className="btn h-7 px-3 text-xs"
            disabled={disabled}
            onClick={onClearOverride}
            type="button"
          >
            <Undo2 size={11} />
            Use suggestion
          </button>
        ) : (
          <button
            className="text-xs text-zinc-500 transition-colors hover:text-zinc-900"
            disabled={disabled}
            onClick={onOverride}
            type="button"
          >
            Override
          </button>
        )}
        {suggestion.alternatives.length > 0 ? (
          <button
            className="ml-auto flex items-center gap-1 text-xs text-zinc-500 transition-colors hover:text-zinc-900"
            disabled={disabled}
            onClick={() => setShowAlternatives((v) => !v)}
            type="button"
          >
            {showAlternatives ? <ChevronUp size={11} /> : <ChevronDown size={11} />}
            See {suggestion.alternatives.length} alternative
            {suggestion.alternatives.length === 1 ? "" : "s"}
          </button>
        ) : null}
      </div>

      <AnimatePresence initial={false}>
        {showAlternatives && suggestion.alternatives.length > 0 ? (
          <motion.ul
            animate={{ opacity: 1, height: "auto" }}
            className="mt-3 space-y-2 overflow-hidden"
            exit={{ opacity: 0, height: 0 }}
            initial={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.16, ease: "easeOut" }}
          >
            {suggestion.alternatives.map((alt, idx) => (
              <li key={`${alt.kind}-${alt.target_id ?? "none"}-${idx}`}>
                <AlternativeRow
                  alt={alt}
                  disabled={disabled}
                  onApply={() => void onApplyAlternative(idx)}
                />
              </li>
            ))}
          </motion.ul>
        ) : null}
      </AnimatePresence>

      {accuracy && accuracy.sample_count >= 10 ? (
        <p className="mt-3 text-[10px] text-zinc-400">
          Suggestions accurate {Math.round(accuracy.accept_rate * 100)}% of last{" "}
          {accuracy.sample_count}
        </p>
      ) : null}
    </div>
  );
}

function AlternativeRow({
  alt,
  disabled,
  onApply,
}: {
  alt: CapturePromotionAlternative;
  disabled: boolean;
  onApply: () => void;
}) {
  const kindLabel = PROMOTION_KIND_LABEL[alt.kind as PromotionKind] ?? alt.kind;
  return (
    <div className="flex items-center gap-2 rounded-lg border border-zinc-100 bg-zinc-50/60 px-3 py-2">
      <div className="min-w-0 flex-1">
        <p className="text-xs font-medium text-zinc-700">
          {kindLabel}
          {alt.target_title ? (
            <>
              {" "}
              on <span className="text-zinc-900">{alt.target_title}</span>
            </>
          ) : null}
          <span className="ml-2 text-[10px] font-normal text-zinc-400">
            {Math.round(alt.confidence * 100)}%
          </span>
        </p>
        {alt.rationale ? (
          <p className="line-clamp-1 text-[11px] text-zinc-500">{alt.rationale}</p>
        ) : null}
      </div>
      <button
        className="btn h-6 px-2 text-[11px]"
        disabled={disabled}
        onClick={onApply}
        type="button"
      >
        Apply
      </button>
    </div>
  );
}
