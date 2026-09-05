import { type FormEvent, type ReactNode, useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useIpcQuery, qk } from "../lib/queries";
import { queryClient } from "../lib/queryClient";
import { motion, AnimatePresence } from "framer-motion";
import {
  DndContext,
  DragEndEvent,
  KeyboardSensor,
  PointerSensor,
  useDroppable,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  CalendarDays,
  ChevronDown,
  ChevronRight,
  Download,
  Flag,
  FolderKanban,
  Gauge,
  Link as LinkIcon,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Target,
  Users,
  X,
  Star,
} from "lucide-react";
import { Link } from "react-router-dom";
import {
  createDeliverable,
  createStakeholder,
  exportMarkdown,
  listDeliverables,
  listInitiatives,
  listStakeholders,
  updateDeliverableStateFriction,
  reorderDeliverableWithinState,
  updateDeliverableMetadata,
} from "../lib/ipc";
import type {
  CreateDeliverableInput,
  Deliverable,
  DeliverableFilters,
  DeliverablePriority,
  DeliverableState,
  DeliverableType,
  Initiative,
  Stakeholder,
  UpdateDeliverableMetadataInput,
} from "../lib/types";
import {
  deliverableStateColors,
  deliverableStateLabels,
  deliverableStateOptions,
  deliverableTypeLabels,
  deliverableTypeOptions,
  initiativeStatusLabels,
  priorityLabels,
} from "../lib/types";
import { DeliverableCard } from "../components/DeliverableCard";
import { WeeklyDigestPanel } from "../components/MinutesPanel";
import { EmptyState } from "../components/EmptyState";

const KanbanIllustration = () => (
  <svg viewBox="0 0 200 140" fill="none" className="mx-auto mb-5 h-28 w-auto text-zinc-200" xmlns="http://www.w3.org/2000/svg">
    <rect x="10" y="18" width="52" height="108" rx="8" stroke="currentColor" strokeWidth="1.5" />
    <text x="36" y="34" textAnchor="middle" fontSize="7" fill="currentColor" fontFamily="system-ui" fontWeight="600">TO DO</text>
    <rect x="20" y="40" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
    <rect x="20" y="64" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
    <rect x="20" y="88" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
    <rect x="74" y="18" width="52" height="108" rx="8" stroke="currentColor" strokeWidth="1.5" />
    <text x="100" y="34" textAnchor="middle" fontSize="7" fill="currentColor" fontFamily="system-ui" fontWeight="600">IN REVIEW</text>
    <rect x="84" y="40" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
    <rect x="84" y="64" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
    <rect x="138" y="18" width="52" height="108" rx="8" stroke="currentColor" strokeWidth="1.5" />
    <text x="164" y="34" textAnchor="middle" fontSize="7" fill="currentColor" fontFamily="system-ui" fontWeight="600">DONE</text>
    <rect x="148" y="40" width="32" height="18" rx="4" stroke="currentColor" strokeWidth="1.5" strokeDasharray="3 3" />
  </svg>
);

const FILTER_STORAGE_KEY = "project-manager:deliverable-board-filters";

const columnAccent: Record<string, string> = {
  blue: "border-blue-400 bg-blue-400",
  amber: "border-amber-400 bg-amber-400",
  green: "border-emerald-400 bg-emerald-400",
  zinc: "border-zinc-300 bg-zinc-300",
  violet: "border-violet-400 bg-violet-400",
};

const columnBg: Record<string, string> = {
  blue: "bg-blue-50/40 dark:bg-blue-950/10",
  amber: "bg-amber-50/40 dark:bg-amber-950/10",
  green: "bg-emerald-50/40 dark:bg-emerald-950/10",
  zinc: "bg-zinc-50/30 dark:bg-zinc-900/10",
  violet: "bg-violet-50/40 dark:bg-violet-950/10",
};

const columnOver: Record<string, string> = {
  blue: "ring-2 ring-blue-400/50 bg-blue-50/60",
  amber: "ring-2 ring-amber-400/50 bg-amber-50/60",
  green: "ring-2 ring-emerald-400/50 bg-emerald-50/60",
  zinc: "ring-2 ring-zinc-400/40 bg-zinc-50/50",
  violet: "ring-2 ring-violet-400/50 bg-violet-50/60",
};

type AiPanel = "digest" | null;

interface FrictionModal {
  deliverableId: string;
  toState: DeliverableState;
  title: string;
}

export function DeliverableBoard() {
  const [filters, setFilters] = useState<DeliverableFilters>(() => readStoredFilters());
  const [collapsed, setCollapsed] = useState<Set<DeliverableState>>(() => new Set(["killed"]));
  const [isSaving, setIsSaving] = useState(false);
  const [exportMessage, setExportMessage] = useState<string | null>(null);

  const deliverableFilters = cleanFilters(filters);
  const { data: deliverables = [], isFetching: isLoadingDeliverables } = useIpcQuery(
    qk.deliverables.list(deliverableFilters),
    () => listDeliverables(deliverableFilters),
  );
  const { data: initiatives = [] } = useIpcQuery(qk.initiatives.list, listInitiatives);
  const { data: stakeholders = [] } = useIpcQuery(qk.stakeholders.list, listStakeholders);
  const [aiPanel, setAiPanel] = useState<AiPanel>(null);
  const [boardSearch, setBoardSearch] = useState("");
  const [isCreateDialogOpen, setIsCreateDialogOpen] = useState(false);
  const [frictionModal, setFrictionModal] = useState<FrictionModal | null>(null);
  const [frictionNote, setFrictionNote] = useState("");
  const frictionInputRef = useRef<HTMLInputElement>(null);
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 8 } }),
    useSensor(KeyboardSensor),
  );

  // Persist filter selection (UI preference — react-query owns the actual data cache)
  useEffect(() => {
    window.localStorage.setItem(FILTER_STORAGE_KEY, JSON.stringify(filters));
  }, [filters]);
  // board-data-changed → qk.deliverables.all invalidation handled centrally in eventInvalidations.ts

  const focused = useMemo(
    () => deliverables.find((d) => d.is_focused) ?? null,
    [deliverables],
  );

  const filtered = useMemo(() => {
    if (!boardSearch.trim()) return deliverables;
    const q = boardSearch.toLowerCase();
    return deliverables.filter(
      (d) =>
        d.title.toLowerCase().includes(q) ||
        d.claim.toLowerCase().includes(q) ||
        (d.stakeholder_name ?? "").toLowerCase().includes(q) ||
        d.labels.some((l) => l.name.toLowerCase().includes(q)),
    );
  }, [deliverables, boardSearch]);

  const grouped = useMemo(() => {
    return deliverableStateOptions.reduce<Record<DeliverableState, Deliverable[]>>(
      (acc, state) => {
        acc[state] = filtered.filter((d) => d.state === state);
        return acc;
      },
      { backlog: [], todo: [], drafting: [], in_review: [], shipped: [], killed: [] },
    );
  }, [filtered]);

  const handleCreate = useCallback(async (input: CreateDeliverableInput, metadata: UpdateDeliverableMetadataInput) => {
    try {
      setIsSaving(true);
      const created = await createDeliverable(input);
      if (hasMetadata(metadata)) {
        await updateDeliverableMetadata(created.id, metadata);
      }
      setIsCreateDialogOpen(false);
      setFilters({});
      void queryClient.invalidateQueries({ queryKey: qk.deliverables.all });
    } catch (caught) {
      throw caught;
    } finally {
      setIsSaving(false);
    }
  }, [deliverableFilters]);

  const handleCreateStakeholder = useCallback(async (name: string) => {
    const created = await createStakeholder({ name });
    void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all });
    return created;
  }, []);

  const handleMoveState = useCallback(async (id: string, state: DeliverableState, friction?: string) => {
    // Optimistic update — flip state locally so drag-drop feels instant
    const prevData = queryClient.getQueryData<Deliverable[]>(qk.deliverables.list(deliverableFilters));
    queryClient.setQueryData<Deliverable[]>(qk.deliverables.list(deliverableFilters), (current) =>
      current?.map((d) => (d.id === id ? { ...d, state } : d)) ?? [],
    );
    try {
      await updateDeliverableStateFriction(id, state, friction ?? null);
      void queryClient.invalidateQueries({ queryKey: qk.deliverables.all });
    } catch {
      queryClient.setQueryData(qk.deliverables.list(deliverableFilters), prevData);
    }
  }, [deliverableFilters]);

  const handleDragEnd = useCallback((event: DragEndEvent) => {
    const nextState = event.over?.id as DeliverableState | undefined;
    const deliverableId = String(event.active.id);
    if (!nextState || !deliverableStateOptions.includes(nextState)) return;
    const current = deliverables.find((d) => d.id === deliverableId);
    if (!current || current.state === nextState) return;

    // Show friction modal when moving forward (drafting→in_review, in_review→shipped)
    const forwardMoves: Array<[DeliverableState, DeliverableState]> = [
      ["drafting", "in_review"],
      ["in_review", "shipped"],
    ];
    const isForward = forwardMoves.some(([from, to]) => current.state === from && nextState === to);
    if (isForward) {
      setFrictionModal({ deliverableId, toState: nextState, title: current.title });
      setFrictionNote("");
      setTimeout(() => frictionInputRef.current?.focus(), 100);
    } else {
      void handleMoveState(deliverableId, nextState);
    }
  }, [deliverables, handleMoveState]);

  const handleFrictionSubmit = useCallback(async (skip: boolean) => {
    if (!frictionModal) return;
    await handleMoveState(
      frictionModal.deliverableId,
      frictionModal.toState,
      skip ? undefined : frictionNote || undefined,
    );
    setFrictionModal(null);
    setFrictionNote("");
  }, [frictionModal, frictionNote, handleMoveState]);

  const handleReorderWithinState = useCallback(async (id: string, direction: "up" | "down") => {
    try {
      const updatedColumn = await reorderDeliverableWithinState({ id, direction });
      const ids = new Set(updatedColumn.map((d) => d.id));
      queryClient.setQueryData<Deliverable[]>(qk.deliverables.list(deliverableFilters), (current) => [
        ...(current ?? []).filter((d) => !ids.has(d.id)),
        ...updatedColumn,
      ]);
    } catch {
      void queryClient.invalidateQueries({ queryKey: qk.deliverables.list(deliverableFilters) });
    }
  }, [deliverableFilters]);

  async function handleExport() {
    try {
      setExportMessage(null);
      const selected = await openDialog({ directory: true, multiple: false, title: "Choose export folder" });
      if (typeof selected !== "string") return;
      const result = await exportMarkdown(selected);
      setExportMessage(`Exported ${result.deliverable_count} deliverables to ${result.export_path}`);
    } catch {
      // error surfaced by invokeCommand toast
    }
  }

  function updateFilter<K extends keyof DeliverableFilters>(key: K, value: DeliverableFilters[K] | "") {
    setFilters((current) => {
      const next = { ...current };
      if (!value) delete next[key];
      else next[key] = value as DeliverableFilters[K];
      return next;
    });
  }

  function toggleCollapsed(state: DeliverableState) {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(state)) next.delete(state);
      else next.add(state);
      return next;
    });
  }

  return (
    <div className="mx-auto min-h-full max-w-[1600px] px-5 py-6">
      <section className="min-w-0">
        {/* Header */}
        <div className="mb-5 flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="mb-1 text-xs font-semibold uppercase tracking-wide text-neutral-500">Work board</p>
            <h1 className="text-2xl font-semibold tracking-normal text-neutral-950 dark:text-neutral-50">
              Deliverables
            </h1>
          </div>
          <div className="flex flex-wrap gap-2">
            <button className="btn btn-primary" onClick={() => setIsCreateDialogOpen(true)} type="button">
              <Plus aria-hidden="true" size={14} />
              New deliverable
            </button>
            <button
              className={["btn", aiPanel === "digest" ? "bg-violet-100 text-violet-700" : ""].join(" ")}
              onClick={() => setAiPanel((p) => p === "digest" ? null : "digest")}
              type="button"
            >
              <Sparkles aria-hidden="true" size={14} />
              Digest
            </button>
            <button
              className="btn"
              onClick={() => void queryClient.invalidateQueries({ queryKey: qk.deliverables.list(deliverableFilters) })}
              type="button"
            >
              <RefreshCw aria-hidden="true" size={14} />
              Refresh
            </button>
            <button className="btn" onClick={() => void handleExport()} type="button">
              <Download aria-hidden="true" size={14} />
              Export
            </button>
          </div>
        </div>

        {/* AI panel */}
        <AnimatePresence>
          {aiPanel && (
            <motion.div
              key={aiPanel}
              initial={{ opacity: 0, y: -8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -8 }}
              transition={{ duration: 0.15 }}
              className="mb-4"
            >
              <WeeklyDigestPanel onClose={() => setAiPanel(null)} />
            </motion.div>
          )}
        </AnimatePresence>

        {/* Inline board search */}
        <div className="mb-3 relative">
          <Search size={13} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 pointer-events-none" />
          <input
            className="field-control pl-8 text-[13px]"
            placeholder="Search board…"
            value={boardSearch}
            onChange={(e) => setBoardSearch(e.target.value)}
          />
          {boardSearch && (
            <button
              className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400 hover:text-zinc-600"
              onClick={() => setBoardSearch("")}
              type="button"
            >
              <X size={13} />
            </button>
          )}
        </div>

        {/* Filters */}
        {Object.keys(filters).length > 0 && (
          <div className="mb-2 flex items-center gap-2">
            <span className="text-xs text-zinc-500">Filters active</span>
            <button
              className="text-xs font-medium text-sky-600 hover:text-sky-800"
              onClick={() => setFilters({})}
              type="button"
            >
              Clear all
            </button>
          </div>
        )}

        <div className="mb-4 grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <label className="block space-y-1">
            <span className="field-label px-1">Initiative</span>
            <select
              className="field-control"
              onChange={(e) => updateFilter("initiative_id", e.currentTarget.value)}
              value={filters.initiative_id ?? ""}
            >
              <option value="">All initiatives</option>
              {initiatives.map((i) => (
                <option key={i.id} value={i.id}>{i.title}</option>
              ))}
            </select>
          </label>
          <label className="block space-y-1">
            <span className="field-label px-1">Stakeholder</span>
            <select
              className="field-control"
              onChange={(e) => updateFilter("stakeholder_id", e.currentTarget.value)}
              value={filters.stakeholder_id ?? ""}
            >
              <option value="">All stakeholders</option>
              {stakeholders.map((s) => (
                <option key={s.id} value={s.id}>{s.name}</option>
              ))}
            </select>
          </label>
          <label className="block space-y-1">
            <span className="field-label px-1">Type</span>
            <select
              className="field-control"
              onChange={(e) => updateFilter("type", e.currentTarget.value as DeliverableType | "")}
              value={filters.type ?? ""}
            >
              <option value="">All types</option>
              {deliverableTypeOptions.map((opt) => (
                <option key={opt} value={opt}>{deliverableTypeLabels[opt]}</option>
              ))}
            </select>
          </label>
          <label className="block space-y-1">
            <span className="field-label px-1">Priority</span>
            <select
              className="field-control"
              onChange={(e) => updateFilter("priority", e.currentTarget.value as DeliverablePriority | "")}
              value={filters.priority ?? ""}
            >
              <option value="">All priorities</option>
              {(["p1", "p2", "p3"] as DeliverablePriority[]).map((p) => (
                <option key={p} value={p}>{priorityLabels[p]}</option>
              ))}
            </select>
          </label>
        </div>

        {exportMessage && (
          <div className="mb-4 rounded-md border border-emerald-300 bg-emerald-50 px-4 py-3 text-sm text-emerald-800">
            {exportMessage}
          </div>
        )}

        {focused && (
          <Link
            className="mb-4 flex items-center gap-3 rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm hover:bg-amber-100"
            to={`/deliverables/${focused.id}`}
          >
            <Star className="shrink-0 fill-amber-400 text-amber-400" size={15} />
            <div className="min-w-0">
              <span className="font-semibold text-amber-800">{focused.title}</span>
              {focused.deadline && (
                <span className="ml-2 text-amber-600">· due {focused.deadline}</span>
              )}
            </div>
          </Link>
        )}

        {isLoadingDeliverables && deliverables.length === 0 ? (
          <div className="grid gap-3 lg:grid-cols-3 xl:grid-cols-6">
            {Array.from({ length: 6 }).map((_, i) => (
              <div key={i} className="skeleton h-48 rounded-2xl" />
            ))}
          </div>
        ) : !isLoadingDeliverables && deliverables.length === 0 ? (
          <EmptyState
            variant="hero"
            illustration={<KanbanIllustration />}
            title="Your board is empty"
            description="Capture work from meetings and emails, or create a deliverable to get started."
            cta={{ label: "New deliverable", onClick: () => setIsCreateDialogOpen(true), primary: true }}
          />
        ) : (
          <DndContext onDragEnd={handleDragEnd} sensors={sensors}>
            <div className="grid gap-3 lg:grid-cols-3 xl:grid-cols-6">
              {deliverableStateOptions.map((state) => (
                <BoardColumn
                  collapsed={collapsed.has(state)}
                  deliverables={grouped[state]}
                  key={state}
                  onMoveState={(id, s) => void handleMoveState(id, s)}
                  onToggleCollapsed={() => toggleCollapsed(state)}
                onReorder={(id, direction) => void handleReorderWithinState(id, direction)}
                state={state}
              />
              ))}
            </div>
          </DndContext>
        )}
      </section>

      <AnimatePresence>
        {isCreateDialogOpen && (
          <CreateDeliverableDialog
            initiatives={initiatives}
            isSubmitting={isSaving}
            key="new-deliverable-dialog"
            onClose={() => setIsCreateDialogOpen(false)}
            onCreateStakeholder={handleCreateStakeholder}
            onSubmit={handleCreate}
            stakeholders={stakeholders}
          />
        )}
      </AnimatePresence>

      {/* Friction log modal */}
      <AnimatePresence>
        {frictionModal && (
          <motion.div
            className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={() => void handleFrictionSubmit(true)}
          >
            <motion.div
              className="w-full max-w-sm rounded-2xl bg-white p-5 shadow-2xl"
              initial={{ scale: 0.95, y: 8 }}
              animate={{ scale: 1, y: 0 }}
              exit={{ scale: 0.95, y: 8 }}
              onClick={(e) => e.stopPropagation()}
            >
              <p className="mb-1 text-[11px] font-semibold uppercase tracking-widest text-zinc-400">
                Friction log
              </p>
              <p className="mb-3 text-[13px] font-medium text-zinc-800 line-clamp-1">
                Moving "{frictionModal.title}" →{" "}
                <span className="font-semibold text-zinc-950">
                  {deliverableStateLabels[frictionModal.toState]}
                </span>
              </p>
              <input
                ref={frictionInputRef}
                className="field-control mb-3 text-[13px]"
                placeholder="What slowed this down? (optional)"
                value={frictionNote}
                onChange={(e) => setFrictionNote(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleFrictionSubmit(false);
                  if (e.key === "Escape") void handleFrictionSubmit(true);
                }}
              />
              <div className="flex gap-2 justify-end">
                <button
                  className="btn text-xs"
                  onClick={() => void handleFrictionSubmit(true)}
                  type="button"
                >
                  Skip
                </button>
                <button
                  className="btn bg-zinc-900 text-white hover:bg-zinc-700 text-xs"
                  onClick={() => void handleFrictionSubmit(false)}
                  type="button"
                >
                  Save &amp; move
                </button>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

interface CreateDeliverableDialogProps {
  initiatives: Initiative[];
  stakeholders: Stakeholder[];
  isSubmitting: boolean;
  onClose: () => void;
  onSubmit: (input: CreateDeliverableInput, metadata: UpdateDeliverableMetadataInput) => Promise<void>;
  onCreateStakeholder: (name: string) => Promise<Stakeholder>;
}

function CreateDeliverableDialog({
  initiatives,
  stakeholders,
  isSubmitting,
  onClose,
  onCreateStakeholder,
  onSubmit,
}: CreateDeliverableDialogProps) {
  const [title, setTitle] = useState("");
  const [type, setType] = useState<DeliverableType>("analysis");
  const [state, setState] = useState<DeliverableState>("drafting");
  const [claim, setClaim] = useState("");
  const [artifactUrl, setArtifactUrl] = useState("");
  const [stakeholderIds, setStakeholderIds] = useState<string[]>([]);
  const [initiativeIds, setInitiativeIds] = useState<string[]>([]);
  const [stakeholderSearch, setStakeholderSearch] = useState("");
  const [initiativeSearch, setInitiativeSearch] = useState("");
  const [newStakeholderName, setNewStakeholderName] = useState("");
  const [deadline, setDeadline] = useState("");
  const [priority, setPriority] = useState<DeliverablePriority | "">("");
  const [blockerReason, setBlockerReason] = useState("");
  const [localError, setLocalError] = useState<string | null>(null);
  const [isAddingStakeholder, setIsAddingStakeholder] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  const claimRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const focusTimer = window.setTimeout(() => titleRef.current?.focus(), 80);
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape" && !isSubmitting) {
        onClose();
      }
    }

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.clearTimeout(focusTimer);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [isSubmitting, onClose]);

  const selectedStakeholders = useMemo(
    () => stakeholders.filter((stakeholder) => stakeholderIds.includes(stakeholder.id)),
    [stakeholderIds, stakeholders],
  );

  const selectedInitiatives = useMemo(
    () => initiatives.filter((initiative) => initiativeIds.includes(initiative.id)),
    [initiativeIds, initiatives],
  );

  const visibleStakeholders = useMemo(() => {
    const query = stakeholderSearch.trim().toLowerCase();
    if (!query) return stakeholders;
    return stakeholders.filter(
      (stakeholder) =>
        stakeholder.name.toLowerCase().includes(query) ||
        (stakeholder.role ?? "").toLowerCase().includes(query),
    );
  }, [stakeholderSearch, stakeholders]);

  const visibleInitiatives = useMemo(() => {
    const query = initiativeSearch.trim().toLowerCase();
    if (!query) return initiatives;
    return initiatives.filter((initiative) => initiative.title.toLowerCase().includes(query));
  }, [initiativeSearch, initiatives]);

  function toggleStakeholder(stakeholderId: string) {
    setStakeholderIds((current) =>
      current.includes(stakeholderId)
        ? current.filter((id) => id !== stakeholderId)
        : [...current, stakeholderId],
    );
  }

  function toggleInitiative(initiativeId: string) {
    setInitiativeIds((current) =>
      current.includes(initiativeId)
        ? current.filter((id) => id !== initiativeId)
        : [...current, initiativeId],
    );
  }

  async function handleAddStakeholder() {
    const name = newStakeholderName.trim();
    if (!name) return;

    try {
      setLocalError(null);
      setIsAddingStakeholder(true);
      const created = await onCreateStakeholder(name);
      setStakeholderIds((current) => [...current, created.id]);
      setNewStakeholderName("");
      setStakeholderSearch("");
    } catch (caught) {
      setLocalError(String(caught));
    } finally {
      setIsAddingStakeholder(false);
    }
  }

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const titleValue = title.trim();
    if (!titleValue) {
      setLocalError("Add a title before creating this deliverable.");
      titleRef.current?.focus();
      return;
    }

    const claimValue = claim.trim();
    if (!claimValue) {
      setLocalError("Add a claim before creating this deliverable.");
      claimRef.current?.focus();
      return;
    }

    if (state !== "backlog" && initiativeIds.length === 0) {
      setLocalError("Choose at least one initiative, or set the board state to Backlog.");
      return;
    }

    const metadata: UpdateDeliverableMetadataInput = {
      deadline: deadline || null,
      effort: null,
      impact: null,
      blocker_reason: blockerReason.trim() || null,
      priority: priority || null,
    };

    try {
      setLocalError(null);
      await onSubmit(
        {
          title: titleValue,
          type,
          state,
          claim: claimValue,
          artifact_url: artifactUrl.trim() ? artifactUrl.trim() : null,
          conversation_id: null,
          stakeholder_id: stakeholderIds[0] ?? null,
          stakeholder_ids: stakeholderIds,
          initiative_ids: initiativeIds,
        },
        metadata,
      );
    } catch (caught) {
      setLocalError(String(caught));
    }
  }

  const planSummary = [
    priority ? priorityLabels[priority] : null,
    deadline ? `Due ${deadline}` : null,
  ].filter(Boolean);

  return (
    <motion.div
      animate={{ opacity: 1 }}
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-zinc-950/35 px-4 py-8 backdrop-blur-sm"
      exit={{ opacity: 0 }}
      initial={{ opacity: 0 }}
      role="presentation"
    >
      <motion.section
        aria-labelledby="new-deliverable-title"
        aria-modal="true"
        animate={{ opacity: 1, scale: 1, y: 0 }}
        className="flex max-h-[calc(100vh-4rem)] w-full max-w-5xl flex-col overflow-hidden rounded-2xl border border-zinc-200 bg-white shadow-2xl"
        exit={{ opacity: 0, scale: 0.98, y: 10 }}
        initial={{ opacity: 0, scale: 0.98, y: 10 }}
        role="dialog"
      >
        <form className="flex min-h-0 flex-1 flex-col" onSubmit={handleSubmit}>
          <header className="flex flex-wrap items-start justify-between gap-4 border-b border-zinc-200 bg-zinc-50/80 px-6 py-5">
            <div className="min-w-0">
              <p className="mb-1 text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">
                Work intake
              </p>
              <h2 className="text-xl font-semibold tracking-normal text-zinc-950" id="new-deliverable-title">
                New deliverable
              </h2>
            </div>
            <button
              aria-label="Close new deliverable dialog"
              className="btn h-9 w-9 px-0"
              disabled={isSubmitting}
              onClick={onClose}
              type="button"
            >
              <X aria-hidden="true" size={16} />
            </button>
          </header>

          {localError ? (
            <div className="mx-6 mt-5 notice notice-error">
              {localError}
            </div>
          ) : null}

          <div className="grid min-h-0 flex-1 gap-0 overflow-y-auto lg:grid-cols-[minmax(0,1fr)_320px]">
            <main className="space-y-6 px-6 py-5">
              <section className="space-y-3">
                <div className="flex items-center gap-2 text-sm font-semibold text-zinc-950">
                  <Target aria-hidden="true" size={16} />
                  Scope
                </div>
                <label className="block space-y-1.5">
                  <span className="field-label">Title</span>
                  <input
                    className="field-control h-11 text-[14px]"
                    maxLength={160}
                    onChange={(event) => setTitle(event.currentTarget.value)}
                    placeholder="Knowledge Graph schema v2"
                    ref={titleRef}
                    required
                    value={title}
                  />
                </label>
                <label className="block space-y-1.5">
                  <span className="field-label">Claim</span>
                  <textarea
                    className="field-control min-h-28 text-[14px]"
                    onChange={(event) => setClaim(event.currentTarget.value)}
                    placeholder="What should this deliverable argue, prove, or make possible?"
                    ref={claimRef}
                    value={claim}
                  />
                </label>
                <label className="block space-y-1.5">
                  <span className="field-label">Artifact URL</span>
                  <div className="relative">
                    <LinkIcon
                      aria-hidden="true"
                      className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
                      size={15}
                    />
                    <input
                      className="field-control h-10 pl-9"
                      onChange={(event) => setArtifactUrl(event.currentTarget.value)}
                      placeholder="https://..."
                      value={artifactUrl}
                    />
                  </div>
                </label>
              </section>

              <section className="grid gap-4 md:grid-cols-2">
                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-sm font-semibold text-zinc-950">
                    <FolderKanban aria-hidden="true" size={16} />
                    Classification
                  </div>
                  <label className="block space-y-1.5">
                    <span className="field-label">Type</span>
                    <select
                      className="field-control h-10"
                      onChange={(event) => setType(event.currentTarget.value as DeliverableType)}
                      value={type}
                    >
                      {deliverableTypeOptions.map((option) => (
                        <option key={option} value={option}>
                          {deliverableTypeLabels[option]}
                        </option>
                      ))}
                    </select>
                  </label>
                  <fieldset className="space-y-2">
                    <legend className="field-label">Board state</legend>
                    <div className="grid grid-cols-2 gap-2">
                      {deliverableStateOptions.map((option) => {
                        const color = deliverableStateColors[option];
                        const active = state === option;
                        return (
                          <button
                            aria-pressed={active}
                            className={[
                              "flex h-10 items-center gap-2 rounded-lg border px-3 text-left text-[12px] font-semibold transition-all",
                              active
                                ? "border-zinc-900 bg-zinc-950 text-white shadow-sm"
                                : "border-zinc-200 bg-white text-zinc-600 hover:border-zinc-300 hover:bg-zinc-50",
                            ].join(" ")}
                            key={option}
                            onClick={() => setState(option)}
                            type="button"
                          >
                            <span
                              className={[
                                "h-2 w-2 rounded-full",
                                active ? "bg-white" : columnAccent[color],
                              ].join(" ")}
                            />
                            {deliverableStateLabels[option]}
                          </button>
                        );
                      })}
                    </div>
                  </fieldset>
                </div>

                <div className="space-y-3">
                  <div className="flex items-center gap-2 text-sm font-semibold text-zinc-950">
                    <Gauge aria-hidden="true" size={16} />
                    Planning
                  </div>
                  <div className="grid gap-3 sm:grid-cols-2">
                    <label className="block space-y-1.5">
                      <span className="field-label">Deadline</span>
                      <div className="relative">
                        <CalendarDays
                          aria-hidden="true"
                          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
                          size={15}
                        />
                        <input
                          className="field-control h-10 pl-9"
                          onChange={(event) => setDeadline(event.currentTarget.value)}
                          type="date"
                          value={deadline}
                        />
                      </div>
                    </label>
                    <label className="block space-y-1.5">
                      <span className="field-label">Priority</span>
                      <select
                        className="field-control h-10"
                        onChange={(event) => setPriority(event.currentTarget.value as DeliverablePriority | "")}
                        value={priority}
                      >
                        <option value="">None</option>
                        <option value="p1">P1 - Critical</option>
                        <option value="p2">P2 - Important</option>
                        <option value="p3">P3 - Nice to have</option>
                      </select>
                    </label>
                  </div>
                  <label className="block space-y-1.5">
                    <span className="field-label">Blocker reason</span>
                    <input
                      className="field-control h-10"
                      onChange={(event) => setBlockerReason(event.currentTarget.value)}
                      placeholder="Leave empty if the work is not blocked"
                      value={blockerReason}
                    />
                  </label>
                </div>
              </section>

              <section className="grid gap-4 md:grid-cols-2">
                <RelationshipPicker
                  emptyLabel="No stakeholders match."
                  icon={<Users aria-hidden="true" size={16} />}
                  items={visibleStakeholders.map((stakeholder) => ({
                    id: stakeholder.id,
                    label: stakeholder.name,
                    meta: stakeholder.role ?? "Stakeholder",
                  }))}
                  onSearch={setStakeholderSearch}
                  onToggle={toggleStakeholder}
                  search={stakeholderSearch}
                  searchPlaceholder="Search stakeholders..."
                  selectedIds={stakeholderIds}
                  title="Stakeholders"
                />

                <RelationshipPicker
                  emptyLabel="No initiatives match."
                  icon={<Flag aria-hidden="true" size={16} />}
                  items={visibleInitiatives.map((initiative) => ({
                    id: initiative.id,
                    label: initiative.title,
                    meta: initiativeStatusLabels[initiative.status],
                  }))}
                  onSearch={setInitiativeSearch}
                  onToggle={toggleInitiative}
                  search={initiativeSearch}
                  searchPlaceholder="Search initiatives..."
                  selectedIds={initiativeIds}
                  title="Initiatives"
                />
              </section>

              <div className="flex gap-2 rounded-xl border border-zinc-200 bg-zinc-50 p-2">
                <input
                  className="field-control h-9"
                  onChange={(event) => setNewStakeholderName(event.currentTarget.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") {
                      event.preventDefault();
                      void handleAddStakeholder();
                    }
                  }}
                  placeholder="Add stakeholder by name"
                  value={newStakeholderName}
                />
                <button
                  className="btn h-9 shrink-0"
                  disabled={isSubmitting || isAddingStakeholder || !newStakeholderName.trim()}
                  onClick={() => void handleAddStakeholder()}
                  type="button"
                >
                  <Plus aria-hidden="true" size={15} />
                  Add
                </button>
              </div>
            </main>

            <aside className="border-t border-zinc-200 bg-zinc-50/70 p-5 lg:border-l lg:border-t-0">
              <div className="sticky top-5 space-y-4">
                <div className="rounded-2xl border border-zinc-100 bg-white p-4 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
                  <p className="mb-3 text-[11px] font-bold uppercase tracking-[0.2em] text-zinc-400">
                    Preview
                  </p>
                  <div className="mb-3 flex flex-wrap items-center gap-2">
                    <span className="rounded-full bg-zinc-100 px-2 py-1 text-[11px] font-semibold text-zinc-700">
                      {deliverableTypeLabels[type]}
                    </span>
                    <span className="rounded-full bg-sky-100 px-2 py-1 text-[11px] font-semibold text-sky-700">
                      {deliverableStateLabels[state]}
                    </span>
                  </div>
                  <h3 className="break-words text-base font-semibold text-zinc-950">
                    {title.trim() || "Untitled deliverable"}
                  </h3>
                  <p className="mt-2 line-clamp-4 text-sm text-zinc-500">
                    {claim.trim() || "No claim captured yet."}
                  </p>
                  <div className="mt-4 space-y-3 text-xs text-zinc-500">
                    <PreviewRow
                      label="Stakeholders"
                      value={selectedStakeholders.map((stakeholder) => stakeholder.name).join(", ") || "None"}
                    />
                    <PreviewRow
                      label="Initiatives"
                      value={selectedInitiatives.map((initiative) => initiative.title).join(", ") || "None"}
                    />
                    <PreviewRow
                      label="Plan"
                      value={planSummary.length ? planSummary.join(" | ") : "No planning metadata"}
                    />
                  </div>
                </div>
              </div>
            </aside>
          </div>

          <footer className="flex flex-wrap items-center justify-between gap-3 border-t border-zinc-200 bg-white px-6 py-4">
            <p className="text-xs text-zinc-500">
              Destination: {deliverableStateLabels[state]}
            </p>
            <div className="flex gap-2">
              <button className="btn" disabled={isSubmitting} onClick={onClose} type="button">
                Cancel
              </button>
              <button className="btn btn-primary h-9 px-5" disabled={isSubmitting} type="submit">
                <Plus aria-hidden="true" size={15} />
                {isSubmitting ? "Creating..." : "Create deliverable"}
              </button>
            </div>
          </footer>
        </form>
      </motion.section>
    </motion.div>
  );
}

interface RelationshipPickerProps {
  title: string;
  icon: ReactNode;
  items: Array<{ id: string; label: string; meta: string }>;
  selectedIds: string[];
  search: string;
  searchPlaceholder: string;
  emptyLabel: string;
  onSearch: (value: string) => void;
  onToggle: (id: string) => void;
}

function RelationshipPicker({
  title,
  icon,
  items,
  selectedIds,
  search,
  searchPlaceholder,
  emptyLabel,
  onSearch,
  onToggle,
}: RelationshipPickerProps) {
  return (
    <fieldset className="space-y-2">
      <legend className="flex items-center gap-2 text-sm font-semibold text-zinc-950">
        {icon}
        {title}
        <span className="ml-auto rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-bold text-zinc-500">
          {selectedIds.length}
        </span>
      </legend>
      <div className="relative">
        <Search
          aria-hidden="true"
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400"
          size={14}
        />
        <input
          className="field-control h-9 pl-8"
          onChange={(event) => onSearch(event.currentTarget.value)}
          placeholder={searchPlaceholder}
          value={search}
        />
      </div>
      <div className="max-h-48 space-y-2 overflow-y-auto rounded-xl border border-zinc-200 bg-zinc-50/70 p-2">
        {items.length === 0 ? (
          <p className="px-3 py-5 text-center text-xs text-zinc-400">{emptyLabel}</p>
        ) : (
          items.map((item) => {
            const checked = selectedIds.includes(item.id);
            return (
              <label
                className={[
                  "flex cursor-pointer items-start gap-2 rounded-lg border px-3 py-2 text-sm transition-colors",
                  checked
                    ? "border-sky-200 bg-sky-50 text-sky-950"
                    : "border-zinc-200 bg-white text-zinc-800 hover:border-zinc-300",
                ].join(" ")}
                key={item.id}
              >
                <input
                  checked={checked}
                  className="mt-1 h-4 w-4 accent-zinc-950"
                  onChange={() => onToggle(item.id)}
                  type="checkbox"
                />
                <span className="min-w-0">
                  <span className="block truncate font-medium">{item.label}</span>
                  <span className="block truncate text-xs text-zinc-500">{item.meta}</span>
                </span>
              </label>
            );
          })
        )}
      </div>
    </fieldset>
  );
}

function PreviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <span className="block text-[10px] font-bold uppercase tracking-widest text-zinc-400">{label}</span>
      <span className="mt-0.5 block break-words text-zinc-700">{value}</span>
    </div>
  );
}

interface BoardColumnProps {
  state: DeliverableState;
  deliverables: Deliverable[];
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onMoveState: (id: string, state: DeliverableState) => void;
  onReorder: (id: string, direction: "up" | "down") => void;
}

function BoardColumn({ deliverables, onMoveState, onReorder, state, collapsed, onToggleCollapsed }: BoardColumnProps) {
  const { isOver, setNodeRef } = useDroppable({ id: state, data: { state } });
  const color = deliverableStateColors[state];

  return (
    <section
      className={[
        "flex flex-col min-h-[14rem] rounded-2xl transition-all duration-200 border border-transparent",
        columnBg[color],
        isOver ? columnOver[color] : "",
      ].join(" ")}
      ref={setNodeRef}
    >
      {/* Column header */}
      <div className="sticky top-[60px] z-10 rounded-t-2xl border-b border-zinc-200/50 bg-inherit px-3 py-2.5 backdrop-blur-sm">
        <button
          className="flex w-full items-center justify-between text-left outline-none"
          onClick={onToggleCollapsed}
          type="button"
        >
          <div className="flex items-center gap-2">
            <span className={["h-2 w-2 rounded-full", columnAccent[color]].join(" ")} />
            <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-700 dark:text-zinc-300">
              {deliverableStateLabels[state]}
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            <span className="min-w-[18px] rounded-full bg-white/70 px-1.5 py-0.5 text-center text-[10px] font-bold text-zinc-500 shadow-sm dark:bg-zinc-800/70">
              {deliverables.length}
            </span>
            {collapsed
              ? <ChevronRight size={12} className="text-zinc-400" />
              : <ChevronDown size={12} className="text-zinc-400" />
            }
          </div>
        </button>
      </div>

      {collapsed ? (
        <p className="px-3 py-3 text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
          {deliverables.length} hidden
        </p>
      ) : deliverables.length === 0 ? (
        <div className="flex flex-col items-center justify-center px-3 py-8 opacity-40">
          <p className="text-xs text-zinc-400">Empty</p>
        </div>
      ) : (
        <motion.div
          className="flex flex-col gap-2.5 p-2.5"
          initial="hidden"
          animate="visible"
          variants={{
            hidden: { opacity: 0 },
            visible: { opacity: 1, transition: { staggerChildren: 0.04 } },
          }}
        >
          <AnimatePresence>
            {deliverables.map((deliverable, idx) => (
              <DeliverableCard
                deliverable={deliverable}
                key={deliverable.id}
                onMoveState={onMoveState}
                onMoveUp={idx > 0 ? () => onReorder(deliverable.id, "up") : undefined}
                onMoveDown={idx < deliverables.length - 1 ? () => onReorder(deliverable.id, "down") : undefined}
              />
            ))}
          </AnimatePresence>
        </motion.div>
      )}
    </section>
  );
}

function readStoredFilters(): DeliverableFilters {
  try {
    const value = window.localStorage.getItem(FILTER_STORAGE_KEY);
    if (!value) return {};
    return JSON.parse(value) as DeliverableFilters;
  } catch {
    return {};
  }
}

function cleanFilters(filters: DeliverableFilters): DeliverableFilters {
  return Object.fromEntries(
    Object.entries(filters).filter(([, value]) => Boolean(value)),
  ) as DeliverableFilters;
}

function hasMetadata(metadata: UpdateDeliverableMetadataInput) {
  return Boolean(metadata.deadline || metadata.blocker_reason || metadata.priority);
}
