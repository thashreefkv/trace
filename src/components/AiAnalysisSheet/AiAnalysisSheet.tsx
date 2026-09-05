import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { motion, AnimatePresence } from "framer-motion";
import {
  Briefcase,
  Calendar,
  Check,
  CheckCircle2,
  ChevronDown,
  History,
  Link2,
  ListChecks,
  Loader2,
  Pencil,
  RefreshCw,
  Search,
  Sparkles,
  Target,
  UsersRound,
  X,
} from "lucide-react";
import {
  approveWorkIntakeSuggestion,
  dismissWorkIntakeSuggestion,
  gmailAnalyzeThread,
  gmailListAnalysisHistory,
  listDeliverables,
  listInitiatives,
  listWorkIntakeSuggestions,
} from "../../lib/ipc";
import type {
  ApproveWorkIntakeInput,
  Deliverable,
  GmailAiResult,
  GmailAnalysisSnapshot,
  GmailThreadDetail,
  Initiative,
  WorkIntakeKind,
  WorkIntakeSuggestion,
} from "../../lib/types";
import { toast } from "../../lib/toast";

interface Props {
  open: boolean;
  detail: GmailThreadDetail | null;
  /** AI result already in memory (e.g. just fetched). Avoids a re-call on open. */
  initialResult?: GmailAiResult | null;
  /** Workspace owner email (from My Profile). Used to mark "You" in People Involved. */
  ownerEmail?: string;
  /** Workspace owner display name. */
  ownerName?: string | null;
  onClose: () => void;
}

export function AiAnalysisSheet({
  detail,
  initialResult,
  onClose,
  open,
  ownerEmail,
  ownerName,
}: Props) {
  const threadId = detail?.thread.thread_id ?? null;

  const [result, setResult] = useState<GmailAiResult | null>(initialResult ?? null);
  const [history, setHistory] = useState<GmailAnalysisSnapshot[]>([]);
  const [suggestions, setSuggestions] = useState<WorkIntakeSuggestion[]>([]);
  const [analyzing, setAnalyzing] = useState(false);
  const [loading, setLoading] = useState(false);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [actingOnSuggestionId, setActingOnSuggestionId] = useState<string | null>(
    null,
  );
  const [overrides, setOverrides] = useState<Record<string, SuggestionOverride>>({});
  const [editingId, setEditingId] = useState<string | null>(null);
  const [deliverables, setDeliverables] = useState<Deliverable[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);

  const loadedRef = useRef<string | null>(null);
  const prevInitialResultRef = useRef<GmailAiResult | null | undefined>(initialResult);

  // Load (or refresh) history + work suggestions when opening.
  const loadAll = useCallback(
    async (id: string) => {
      setLoading(true);
      try {
        const [snapshots, intake] = await Promise.all([
          gmailListAnalysisHistory(id, 20).catch(() => []),
          listWorkIntakeSuggestions({
            source_kind: "gmail",
            source_id: id,
            status: "pending",
            limit: 50,
          }).catch(() => []),
        ]);
        setHistory(snapshots);
        setSuggestions(intake);
        // If we have history but no in-memory result, use the latest snapshot.
        if (!result && snapshots.length > 0) {
          setResult(snapshots[0].result);
        }
      } finally {
        setLoading(false);
      }
    },
    [result],
  );

  useEffect(() => {
    if (!open || !threadId) return;
    if (loadedRef.current === threadId) return;
    loadedRef.current = threadId;
    void loadAll(threadId);
  }, [open, threadId, loadAll]);

  useEffect(() => {
    if (!open) {
      loadedRef.current = null;
      setEditingId(null);
      setOverrides({});
    }
  }, [open]);

  // Load deliverables + initiatives once for the link pickers.
  useEffect(() => {
    if (!open) return;
    listDeliverables({})
      .then(setDeliverables)
      .catch(() => {});
    listInitiatives()
      .then(setInitiatives)
      .catch(() => {});
  }, [open]);

  // Sync `result` from external prop changes (e.g. parent just ran analyze).
  // When a NEW result arrives, also reload suggestions — the analysis dismisses old
  // pending suggestions and creates fresh ones, so stale UI state would cause
  // "already resolved" errors if the user tries to approve the old suggestions.
  useEffect(() => {
    if (!initialResult) return;
    setResult(initialResult);
    if (open && threadId && initialResult !== prevInitialResultRef.current) {
      void loadAll(threadId);
    }
    prevInitialResultRef.current = initialResult;
  }, [open, threadId, initialResult, loadAll]);

  // ESC to close.
  useEffect(() => {
    if (!open) return;
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  async function handleReanalyze() {
    if (!threadId || analyzing) return;
    setAnalyzing(true);
    try {
      const fresh = await gmailAnalyzeThread(threadId, false);
      setResult(fresh);
      await loadAll(threadId);
      toast.success("Thread re-analyzed");
    } catch (error) {
      toast.error(`Analysis failed: ${error}`);
    } finally {
      setAnalyzing(false);
    }
  }

  async function handleApprove(s: WorkIntakeSuggestion) {
    setActingOnSuggestionId(s.id);
    try {
      const ov = overrides[s.id];
      const input: ApproveWorkIntakeInput = { id: s.id };
      if (ov?.item_kind && ov.item_kind !== s.item_kind) {
        input.item_kind_override = ov.item_kind;
      }
      if (ov?.title && ov.title !== s.title) input.title_override = ov.title;
      if (ov?.body !== undefined && ov.body !== s.body) input.body_override = ov.body;
      if (ov?.due_date !== undefined && ov.due_date !== s.due_date) {
        input.due_date_override = ov.due_date;
      }
      if (ov?.target_deliverable_id !== undefined) {
        input.target_deliverable_id = ov.target_deliverable_id;
      }
      if (ov?.target_initiative_id !== undefined) {
        input.target_initiative_id = ov.target_initiative_id;
      }
      await approveWorkIntakeSuggestion(input);
      setSuggestions((prev) => prev.filter((x) => x.id !== s.id));
      setOverrides((prev) => {
        const next = { ...prev };
        delete next[s.id];
        return next;
      });
      const finalKind = ov?.item_kind || s.item_kind;
      const linkedNote = input.target_deliverable_id
        ? " (linked to existing deliverable)"
        : input.target_initiative_id
          ? " (linked to existing initiative)"
          : "";
      toast.success(`${humanizeKind(finalKind)} created${linkedNote}`);
    } catch (error) {
      if (String(error).includes("already resolved")) {
        // Another path already handled this suggestion — remove it from the UI silently.
        setSuggestions((prev) => prev.filter((x) => x.id !== s.id));
      } else {
        toast.error(`Could not approve: ${error}`);
      }
    } finally {
      setActingOnSuggestionId(null);
    }
  }

  function updateOverride(id: string, patch: Partial<SuggestionOverride>) {
    setOverrides((prev) => ({
      ...prev,
      [id]: { ...(prev[id] ?? {}), ...patch },
    }));
  }

  async function handleDismiss(s: WorkIntakeSuggestion) {
    setActingOnSuggestionId(s.id);
    try {
      await dismissWorkIntakeSuggestion(s.id);
      setSuggestions((prev) => prev.filter((x) => x.id !== s.id));
    } catch (error) {
      toast.error(`Could not dismiss: ${error}`);
    } finally {
      setActingOnSuggestionId(null);
    }
  }

  const stakeholderRows = useMemo(() => {
    if (!detail) return [];
    return detail.thread.linked_stakeholders.map((s) => ({
      kind: "stakeholder" as const,
      id: s.id,
      name: s.name,
      subtitle: s.role || null,
      email: null as string | null,
    }));
  }, [detail]);

  const normalizedOwnerEmail = (ownerEmail || "").trim().toLowerCase();

  const participantRows = useMemo(() => {
    if (!detail) return [];
    const linkedEmails = new Set<string>();
    detail.thread.linked_stakeholders.forEach((s) => {
      if (s.email) linkedEmails.add(s.email.toLowerCase());
    });
    const rows: Array<{
      kind: "participant" | "owner";
      id: string;
      name: string;
      subtitle: string | null;
      email: string;
    }> = [];
    let ownerSurfaced = false;
    for (const p of detail.thread.participants) {
      const lower = (p.email || "").toLowerCase();
      if (!p.email) continue;
      if (linkedEmails.has(lower)) continue;
      if (normalizedOwnerEmail && lower === normalizedOwnerEmail) {
        if (ownerSurfaced) continue;
        ownerSurfaced = true;
        rows.push({
          kind: "owner",
          id: p.email,
          name: ownerName || p.name || "You",
          subtitle: p.email,
          email: p.email,
        });
      } else {
        rows.push({
          kind: "participant",
          id: p.email,
          name: p.name || p.email,
          subtitle: null,
          email: p.email,
        });
      }
    }
    return rows;
  }, [detail, normalizedOwnerEmail, ownerName]);

  const latestSnapshot = history[0];
  const hasContent = result || history.length > 0;

  return (
    <AnimatePresence>
      {open ? (
        <motion.div
          animate={{ opacity: 1 }}
          className="fixed inset-0 z-40 flex items-end justify-center bg-black/30 backdrop-blur-sm"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          onMouseDown={onClose}
          transition={{ duration: 0.18, ease: "easeOut" }}
        >
          <motion.section
            animate={{ y: 0 }}
            className="flex max-h-[85vh] w-full max-w-4xl flex-col rounded-t-2xl border border-zinc-100 bg-white shadow-[0_-12px_40px_rgba(0,0,0,0.18)]"
            exit={{ y: "100%" }}
            initial={{ y: "100%" }}
            onMouseDown={(event) => event.stopPropagation()}
            transition={{ type: "spring", stiffness: 380, damping: 36 }}
          >
            {/* Header */}
            <header className="flex shrink-0 items-center justify-between gap-3 border-b border-zinc-100 px-6 py-3">
              <div className="flex min-w-0 items-center gap-2">
                <Sparkles className="text-violet-500" size={14} />
                <span className="text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">
                  AI Analysis
                </span>
                {latestSnapshot ? (
                  <span className="text-[11px] text-zinc-400">
                    · {relativeAgo(latestSnapshot.analyzed_at)}
                    {latestSnapshot.trigger === "auto_new_mail" ? " · auto" : ""}
                  </span>
                ) : null}
              </div>
              <div className="flex shrink-0 items-center gap-2">
                <button
                  className="flex items-center gap-1.5 rounded-md px-2.5 py-1.5 text-[12px] font-medium text-violet-700 transition-colors hover:bg-violet-50 disabled:cursor-not-allowed disabled:opacity-40"
                  disabled={analyzing}
                  onClick={() => void handleReanalyze()}
                  title="Re-analyze with Gemini"
                  type="button"
                >
                  {analyzing ? (
                    <Loader2 className="animate-spin" size={13} />
                  ) : (
                    <RefreshCw size={13} />
                  )}
                  {analyzing ? "Analysing…" : "Re-analyze"}
                </button>
                <button
                  aria-label="Close"
                  className="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
                  onClick={onClose}
                  type="button"
                >
                  <X size={16} />
                </button>
              </div>
            </header>

            {/* Body */}
            <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
              {loading && !hasContent ? (
                <div className="flex items-center justify-center py-12">
                  <Loader2 className="animate-spin text-zinc-400" size={20} />
                </div>
              ) : !hasContent ? (
                <EmptyState onAnalyze={() => void handleReanalyze()} analyzing={analyzing} />
              ) : (
                <div className="space-y-8">
                  {/* Summary + Reasoning */}
                  {result ? (
                    <Section
                      icon={<Sparkles className="text-violet-500" size={13} />}
                      title="Summary"
                    >
                      <p className="text-sm leading-7 text-zinc-700">
                        {result.summary}
                      </p>
                      {result.reasons.length > 0 && (
                        <div className="mt-3">
                          <p className="mb-1.5 text-[10px] font-bold uppercase tracking-wider text-zinc-400">
                            Why
                          </p>
                          <ul className="space-y-1 pl-4 text-[12px] leading-6 text-zinc-600">
                            {result.reasons.map((r, i) => (
                              <li className="list-disc" key={i}>
                                {r}
                              </li>
                            ))}
                          </ul>
                        </div>
                      )}
                      <ClassificationRow result={result} />
                    </Section>
                  ) : null}

                  {/* Suggested work */}
                  <Section
                    icon={<Briefcase className="text-zinc-500" size={13} />}
                    title="Suggested work"
                    count={suggestions.length}
                  >
                    {suggestions.length === 0 ? (
                      <p className="text-[12px] text-zinc-400">
                        No work items to suggest from this thread.
                      </p>
                    ) : (
                      <ul className="space-y-3">
                        {suggestions.map((s) => (
                          <SuggestionRow
                            acting={actingOnSuggestionId === s.id}
                            deliverables={deliverables}
                            editing={editingId === s.id}
                            initiatives={initiatives}
                            key={s.id}
                            onApprove={() => void handleApprove(s)}
                            onDismiss={() => void handleDismiss(s)}
                            onSetEditing={(v) => setEditingId(v ? s.id : null)}
                            onUpdateOverride={(patch) => updateOverride(s.id, patch)}
                            override={overrides[s.id]}
                            suggestion={s}
                          />
                        ))}
                      </ul>
                    )}
                  </Section>

                  {/* People involved */}
                  <Section
                    icon={<UsersRound className="text-zinc-500" size={13} />}
                    title="People involved"
                    count={stakeholderRows.length + participantRows.length}
                  >
                    {stakeholderRows.length + participantRows.length === 0 ? (
                      <p className="text-[12px] text-zinc-400">
                        No people linked to this thread yet.
                      </p>
                    ) : (
                      <ul className="space-y-1.5">
                        {stakeholderRows.map((p) => (
                          <PersonRow
                            badge="Stakeholder"
                            badgeTone="sky"
                            key={`s-${p.id}`}
                            name={p.name}
                            subtitle={p.subtitle}
                          />
                        ))}
                        {participantRows.map((p) => (
                          <PersonRow
                            badge={p.kind === "owner" ? "You" : "Participant"}
                            badgeTone={p.kind === "owner" ? "violet" : "zinc"}
                            key={`p-${p.id}`}
                            name={p.name}
                            subtitle={p.subtitle}
                          />
                        ))}
                      </ul>
                    )}
                  </Section>

                  {/* Analysis history */}
                  <Section
                    icon={<History className="text-zinc-500" size={13} />}
                    title="Analysis history"
                    count={history.length}
                  >
                    {history.length === 0 ? (
                      <p className="text-[12px] text-zinc-400">
                        No previous analyses recorded.
                      </p>
                    ) : (
                      <>
                        <button
                          className="flex items-center gap-1 text-[11px] font-medium text-zinc-500 hover:text-zinc-900"
                          onClick={() => setHistoryOpen((v) => !v)}
                          type="button"
                        >
                          {historyOpen ? "Hide" : "Show"} {history.length} entr
                          {history.length === 1 ? "y" : "ies"}
                          <ChevronDown
                            className={`transition-transform duration-150 ${historyOpen ? "rotate-180" : ""}`}
                            size={11}
                          />
                        </button>
                        {historyOpen ? (
                          <ul className="mt-2 space-y-2">
                            {history.map((snap, i) => {
                              const prev = history[i + 1];
                              const diffs = prev
                                ? diffSnapshots(snap, prev)
                                : [];
                              return (
                                <li
                                  className="flex items-start gap-3 text-[12px]"
                                  key={snap.id}
                                >
                                  <span className="mt-0.5 text-zinc-400 tabular-nums">
                                    {relativeAgo(snap.analyzed_at)}
                                  </span>
                                  <div className="min-w-0 flex-1">
                                    <span
                                      className={`mr-1.5 inline-block rounded px-1 py-0.5 text-[9px] font-semibold uppercase tracking-wider ${
                                        snap.trigger === "auto_new_mail"
                                          ? "bg-violet-50 text-violet-700"
                                          : "bg-zinc-100 text-zinc-500"
                                      }`}
                                    >
                                      {snap.trigger === "auto_new_mail"
                                        ? "auto"
                                        : "manual"}
                                    </span>
                                    <span className="text-zinc-600">
                                      {snap.summary || "(no summary)"}
                                    </span>
                                    {diffs.length > 0 ? (
                                      <p className="mt-0.5 text-[11px] text-zinc-400">
                                        Δ {diffs.join(" · ")}
                                      </p>
                                    ) : null}
                                  </div>
                                </li>
                              );
                            })}
                          </ul>
                        ) : null}
                      </>
                    )}
                  </Section>
                </div>
              )}
            </div>
          </motion.section>
        </motion.div>
      ) : null}
    </AnimatePresence>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Suggestion row + editor + link pickers
// ──────────────────────────────────────────────────────────────────────────

interface SuggestionOverride {
  item_kind?: WorkIntakeKind;
  title?: string;
  body?: string;
  due_date?: string | null;
  target_deliverable_id?: string | null;
  target_initiative_id?: string | null;
}

const KIND_OPTIONS: WorkIntakeKind[] = ["task", "deliverable", "initiative"];

function SuggestionRow({
  acting,
  deliverables,
  editing,
  initiatives,
  onApprove,
  onDismiss,
  onSetEditing,
  onUpdateOverride,
  override,
  suggestion: s,
}: {
  acting: boolean;
  deliverables: Deliverable[];
  editing: boolean;
  initiatives: Initiative[];
  onApprove: () => void;
  onDismiss: () => void;
  onSetEditing: (v: boolean) => void;
  onUpdateOverride: (patch: Partial<SuggestionOverride>) => void;
  override: SuggestionOverride | undefined;
  suggestion: WorkIntakeSuggestion;
}) {
  const effectiveKind = (override?.item_kind ?? s.item_kind) as WorkIntakeKind;
  const effectiveTitle = override?.title ?? s.title;
  const effectiveBody = override?.body ?? s.body;
  const effectiveDue = override?.due_date !== undefined ? override.due_date : s.due_date;
  const targetDeliverableId =
    override?.target_deliverable_id !== undefined
      ? override.target_deliverable_id
      : s.target_deliverable_id;
  const targetInitiativeId =
    override?.target_initiative_id !== undefined
      ? override.target_initiative_id
      : s.target_initiative_id;

  const linkedDeliverable = targetDeliverableId
    ? deliverables.find((d) => d.id === targetDeliverableId) ?? null
    : null;
  const linkedInitiative = targetInitiativeId
    ? initiatives.find((i) => i.id === targetInitiativeId) ?? null
    : null;
  const isLinked = !!(linkedDeliverable || linkedInitiative);
  const isEdited = !!override && Object.keys(override).length > 0;

  return (
    <li className="rounded-lg transition-colors">
      <div className="flex items-start gap-3">
        <SuggestionIcon kind={effectiveKind} />
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <p className="text-sm font-medium text-zinc-800">{effectiveTitle}</p>
            <StatusLabel
              isEdited={isEdited && !isLinked}
              linkedDeliverable={linkedDeliverable}
              linkedInitiative={linkedInitiative}
            />
          </div>
          {effectiveBody ? (
            <p className="mt-0.5 text-[12px] leading-5 text-zinc-500">
              {effectiveBody}
            </p>
          ) : null}
          <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[11px] text-zinc-400">
            <span className="font-medium uppercase tracking-wider">
              {humanizeKind(effectiveKind)}
            </span>
            {effectiveDue ? (
              <span className="inline-flex items-center gap-1">
                <Calendar size={10} />
                {effectiveDue}
              </span>
            ) : null}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          <LinkPickerPopover
            entityType="deliverable"
            items={deliverables.map((d) => ({
              id: d.id,
              title: d.title,
              subtitle: d.type || d.state || null,
            }))}
            onClear={() => onUpdateOverride({ target_deliverable_id: null })}
            onPick={(id) =>
              onUpdateOverride({
                target_deliverable_id: id,
                target_initiative_id: null,
              })
            }
            selectedId={targetDeliverableId}
          />
          <LinkPickerPopover
            entityType="initiative"
            items={initiatives.map((i) => ({
              id: i.id,
              title: i.title,
              subtitle: i.status || null,
            }))}
            onClear={() => onUpdateOverride({ target_initiative_id: null })}
            onPick={(id) =>
              onUpdateOverride({
                target_initiative_id: id,
                target_deliverable_id: null,
              })
            }
            selectedId={targetInitiativeId}
          />
          <button
            aria-label="Edit suggestion"
            className={`rounded p-1 transition-colors ${
              editing
                ? "bg-zinc-100 text-zinc-900"
                : "text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
            }`}
            onClick={() => onSetEditing(!editing)}
            title="Edit before approving"
            type="button"
          >
            <Pencil size={12} />
          </button>
          <button
            className="flex items-center gap-1 rounded-md px-2 py-1 text-[11px] font-semibold text-emerald-700 hover:bg-emerald-50 disabled:cursor-not-allowed disabled:opacity-40"
            disabled={acting}
            onClick={onApprove}
            type="button"
          >
            {acting ? (
              <Loader2 className="animate-spin" size={11} />
            ) : (
              <CheckCircle2 size={12} />
            )}
            Approve
          </button>
          <button
            aria-label="Dismiss suggestion"
            className="rounded-md p-1 text-zinc-400 hover:bg-zinc-50 hover:text-rose-600 disabled:cursor-not-allowed disabled:opacity-40"
            disabled={acting}
            onClick={onDismiss}
            title="Dismiss"
            type="button"
          >
            <X size={12} />
          </button>
        </div>
      </div>

      {editing ? (
        <SuggestionEditor
          effectiveBody={effectiveBody}
          effectiveDue={effectiveDue}
          effectiveKind={effectiveKind}
          effectiveTitle={effectiveTitle}
          onClose={() => onSetEditing(false)}
          onUpdate={onUpdateOverride}
        />
      ) : null}
    </li>
  );
}

function StatusLabel({
  isEdited,
  linkedDeliverable,
  linkedInitiative,
}: {
  isEdited: boolean;
  linkedDeliverable: Deliverable | null;
  linkedInitiative: Initiative | null;
}) {
  if (linkedDeliverable) {
    return (
      <span
        className="inline-flex items-center gap-1 rounded-md bg-sky-50 px-1.5 py-0.5 text-[10px] font-semibold text-sky-700"
        title={`Will link to existing deliverable: ${linkedDeliverable.title}`}
      >
        <Link2 size={9} />
        Existing · {linkedDeliverable.title}
      </span>
    );
  }
  if (linkedInitiative) {
    return (
      <span
        className="inline-flex items-center gap-1 rounded-md bg-violet-50 px-1.5 py-0.5 text-[10px] font-semibold text-violet-700"
        title={`Will link to existing initiative: ${linkedInitiative.title}`}
      >
        <Link2 size={9} />
        Existing · {linkedInitiative.title}
      </span>
    );
  }
  return (
    <span
      className={`rounded-md px-1.5 py-0.5 text-[10px] font-semibold ${
        isEdited
          ? "bg-amber-50 text-amber-700"
          : "bg-emerald-50 text-emerald-700"
      }`}
    >
      {isEdited ? "Edited" : "New"}
    </span>
  );
}

function SuggestionEditor({
  effectiveBody,
  effectiveDue,
  effectiveKind,
  effectiveTitle,
  onClose,
  onUpdate,
}: {
  effectiveBody: string;
  effectiveDue: string | null;
  effectiveKind: WorkIntakeKind;
  effectiveTitle: string;
  onClose: () => void;
  onUpdate: (patch: Partial<SuggestionOverride>) => void;
}) {
  return (
    <div className="ml-7 mt-2 space-y-2 rounded-lg border border-zinc-100 bg-zinc-50 p-3">
      <div className="grid grid-cols-2 gap-2">
        <label className="space-y-1">
          <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
            Kind
          </span>
          <select
            className="w-full rounded-md border border-zinc-200 bg-white px-2 py-1.5 text-[12px]"
            onChange={(e) =>
              onUpdate({ item_kind: e.currentTarget.value as WorkIntakeKind })
            }
            value={effectiveKind}
          >
            {KIND_OPTIONS.map((k) => (
              <option key={k} value={k}>
                {humanizeKind(k)}
              </option>
            ))}
          </select>
        </label>
        <label className="space-y-1">
          <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
            Due date
          </span>
          <input
            className="w-full rounded-md border border-zinc-200 bg-white px-2 py-1.5 text-[12px]"
            onChange={(e) =>
              onUpdate({ due_date: e.currentTarget.value || null })
            }
            type="date"
            value={effectiveDue ?? ""}
          />
        </label>
      </div>
      <label className="block space-y-1">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
          Title
        </span>
        <input
          className="w-full rounded-md border border-zinc-200 bg-white px-2 py-1.5 text-[12px]"
          onChange={(e) => onUpdate({ title: e.currentTarget.value })}
          type="text"
          value={effectiveTitle}
        />
      </label>
      <label className="block space-y-1">
        <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
          Description
        </span>
        <textarea
          className="w-full rounded-md border border-zinc-200 bg-white px-2 py-1.5 text-[12px]"
          onChange={(e) => onUpdate({ body: e.currentTarget.value })}
          rows={3}
          value={effectiveBody}
        />
      </label>
      <div className="flex justify-end">
        <button
          className="text-[11px] font-medium text-zinc-500 hover:text-zinc-900"
          onClick={onClose}
          type="button"
        >
          Done
        </button>
      </div>
    </div>
  );
}

function LinkPickerPopover({
  entityType,
  items,
  onClear,
  onPick,
  selectedId,
}: {
  entityType: "deliverable" | "initiative";
  items: { id: string; title: string; subtitle: string | null }[];
  onClear: () => void;
  onPick: (id: string) => void;
  selectedId: string | null;
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");

  const Icon = entityType === "deliverable" ? Briefcase : Target;
  const tone = entityType === "deliverable" ? "text-sky-500" : "text-violet-500";
  const title =
    entityType === "deliverable"
      ? "Link to existing deliverable"
      : "Link to existing initiative";

  const filtered = items.filter((it) => {
    if (selectedId && it.id === selectedId) return true;
    if (!query.trim()) return true;
    const q = query.toLowerCase();
    return (
      it.title.toLowerCase().includes(q) ||
      (it.subtitle && it.subtitle.toLowerCase().includes(q))
    );
  });

  return (
    <div className="relative">
      <button
        aria-label={title}
        className={`rounded p-1 transition-colors ${
          selectedId
            ? `bg-zinc-100 ${tone}`
            : "text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
        }`}
        onClick={() => setOpen((v) => !v)}
        title={title}
        type="button"
      >
        <Icon size={12} />
      </button>
      {open ? (
        <>
          <button
            aria-label="Close picker"
            className="fixed inset-0 z-10 cursor-default"
            onClick={() => setOpen(false)}
            tabIndex={-1}
            type="button"
          />
          <div className="absolute right-0 top-full z-20 mt-1 w-72 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.12)]">
            <div className="flex items-center gap-2 border-b border-zinc-100 px-3 py-2">
              <Search className="text-zinc-400" size={13} />
              <input
                autoFocus
                className="min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-zinc-400"
                onChange={(e) => setQuery(e.currentTarget.value)}
                placeholder={`Search ${entityType}s…`}
                type="text"
                value={query}
              />
            </div>
            <div className="max-h-64 overflow-y-auto py-1">
              {selectedId ? (
                <button
                  className="flex w-full items-center gap-1.5 border-b border-zinc-100 px-3 py-1.5 text-left text-[11px] font-semibold text-rose-600 hover:bg-rose-50"
                  onClick={() => {
                    onClear();
                    setOpen(false);
                  }}
                  type="button"
                >
                  <X size={11} /> Clear link
                </button>
              ) : null}
              {filtered.length === 0 ? (
                <p className="px-3 py-2 text-[12px] text-zinc-400">
                  {items.length === 0
                    ? `No ${entityType}s yet.`
                    : "No matches."}
                </p>
              ) : (
                filtered.map((it) => (
                  <button
                    className={`flex w-full items-start gap-2 px-3 py-1.5 text-left text-sm hover:bg-zinc-50 ${
                      it.id === selectedId ? "bg-zinc-50" : ""
                    }`}
                    key={it.id}
                    onClick={() => {
                      onPick(it.id);
                      setOpen(false);
                    }}
                    type="button"
                  >
                    <Icon className={`mt-0.5 shrink-0 ${tone}`} size={12} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-zinc-800">
                        {it.title}
                      </span>
                      {it.subtitle ? (
                        <span className="block truncate text-[11px] text-zinc-400">
                          {it.subtitle}
                        </span>
                      ) : null}
                    </span>
                    {it.id === selectedId ? (
                      <Check className="shrink-0 text-emerald-600" size={12} />
                    ) : null}
                  </button>
                ))
              )}
            </div>
          </div>
        </>
      ) : null}
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Sub-components
// ──────────────────────────────────────────────────────────────────────────

function Section({
  children,
  count,
  icon,
  title,
}: {
  children: React.ReactNode;
  count?: number;
  icon: React.ReactNode;
  title: string;
}) {
  return (
    <section>
      <header className="mb-3 flex items-center gap-2">
        {icon}
        <h3 className="text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-500">
          {title}
        </h3>
        {typeof count === "number" ? (
          <span className="rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
            {count}
          </span>
        ) : null}
      </header>
      {children}
    </section>
  );
}

function ClassificationRow({ result }: { result: GmailAiResult }) {
  const items: Array<[string, string]> = [
    ["Category", result.category],
    ["Priority", result.priority],
    ["Sentiment", result.sentiment],
    ["Urgency", result.urgency],
  ];
  return (
    <div className="mt-4 flex flex-wrap items-baseline gap-x-4 gap-y-1 text-[11px]">
      {items.map(([label, value]) => (
        <span className="inline-flex items-baseline gap-1.5" key={label}>
          <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
            {label}
          </span>
          <span className="font-medium text-zinc-700">{value}</span>
        </span>
      ))}
      {result.confidence != null ? (
        <span className="inline-flex items-baseline gap-1.5">
          <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
            Confidence
          </span>
          <span className="font-medium text-zinc-700">
            {Math.round(result.confidence * 100)}%
          </span>
        </span>
      ) : null}
    </div>
  );
}

function SuggestionIcon({ kind }: { kind: string }) {
  if (kind === "task") return <ListChecks className="mt-0.5 shrink-0 text-amber-500" size={14} />;
  if (kind === "deliverable") return <Briefcase className="mt-0.5 shrink-0 text-sky-500" size={14} />;
  if (kind === "initiative") return <Target className="mt-0.5 shrink-0 text-violet-500" size={14} />;
  if (kind === "deadline") return <Calendar className="mt-0.5 shrink-0 text-rose-500" size={14} />;
  return <Check className="mt-0.5 shrink-0 text-zinc-400" size={14} />;
}

function PersonRow({
  badge,
  badgeTone,
  name,
  subtitle,
}: {
  badge: string;
  badgeTone: "sky" | "zinc" | "violet";
  name: string;
  subtitle: string | null;
}) {
  const toneClass =
    badgeTone === "sky"
      ? "bg-sky-50 text-sky-700"
      : badgeTone === "violet"
        ? "bg-violet-50 text-violet-700"
        : "bg-zinc-100 text-zinc-500";
  return (
    <li className="flex items-center gap-2 text-[12px]">
      <span className="inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-zinc-100 text-[10px] font-semibold uppercase text-zinc-600">
        {name.slice(0, 2)}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-zinc-800">{name}</span>
        {subtitle ? (
          <span className="block truncate text-[11px] text-zinc-400">
            {subtitle}
          </span>
        ) : null}
      </span>
      <span
        className={`shrink-0 rounded-md px-1.5 py-0.5 text-[10px] font-semibold ${toneClass}`}
      >
        {badge}
      </span>
    </li>
  );
}

function EmptyState({ analyzing, onAnalyze }: { analyzing: boolean; onAnalyze: () => void }) {
  return (
    <div className="py-12 text-center">
      <Sparkles className="mx-auto mb-3 text-zinc-200" size={36} />
      <p className="text-sm font-semibold text-zinc-700">No analysis yet</p>
      <p className="mt-1 text-xs text-zinc-400">
        Run an analysis to see summary, classification, and suggested work.
      </p>
      <button
        className="mt-4 inline-flex items-center gap-1.5 rounded-md border border-violet-200 px-3 py-1.5 text-[12px] font-semibold text-violet-700 hover:bg-violet-50 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={analyzing}
        onClick={onAnalyze}
        type="button"
      >
        {analyzing ? (
          <Loader2 className="animate-spin" size={13} />
        ) : (
          <Sparkles size={13} />
        )}
        {analyzing ? "Analysing…" : "Analyse now"}
      </button>
    </div>
  );
}

// ──────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────

function humanizeKind(kind: string): string {
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}

function diffSnapshots(
  current: GmailAnalysisSnapshot,
  previous: GmailAnalysisSnapshot,
): string[] {
  const diffs: string[] = [];
  if (current.category && previous.category && current.category !== previous.category) {
    diffs.push(`category ${previous.category} → ${current.category}`);
  }
  if (current.priority && previous.priority && current.priority !== previous.priority) {
    diffs.push(`priority ${previous.priority} → ${current.priority}`);
  }
  if (current.message_count_at_analysis !== previous.message_count_at_analysis) {
    const delta =
      current.message_count_at_analysis - previous.message_count_at_analysis;
    if (delta > 0) diffs.push(`+${delta} new message${delta === 1 ? "" : "s"}`);
  }
  return diffs;
}

function relativeAgo(iso: string): string {
  const ts = Date.parse(iso);
  if (Number.isNaN(ts)) return iso;
  const seconds = Math.max(1, Math.floor((Date.now() - ts) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}
