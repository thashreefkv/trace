import { FormEvent, ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useIpcQuery, qk } from "../lib/queries";
import { queryClient } from "../lib/queryClient";
import { Link, useNavigate, useParams } from "react-router-dom";
import * as Dialog from "@radix-ui/react-dialog";
import { motion, AnimatePresence } from "framer-motion";
import { MOTION, fadeY } from "../lib/motion";
import {
  AlertTriangle,
  CalendarDays,
  Check,
  Clipboard,
  ExternalLink,
  FileText,
  Inbox,
  Mail,
  MessageCircle,
  Package,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  Unlink,
  UsersRound,
  Video,
  X,
} from "lucide-react";
import {
  commentOnBriefingItem,
  createStakeholder,
  deleteStakeholder,
  generateStakeholderBriefing,
  gcalStakeholderEvents,
  gmailExcludeThreadFromStakeholder,
  gmailStakeholderHealth,
  gmailStakeholderThreads,
  listDeliverables,
  listStakeholderDetails,
  openExternalUrl,
  updateStakeholder,
} from "../lib/ipc";
import { EntityFilesPanel } from "../components/files/EntityFilesPanel";
import type {
  BriefingItem,
  Deliverable,
  DeliverableState,
  Stakeholder,
  StakeholderBriefing,
  GmailLocalThread,
  UpdateStakeholderInput,
} from "../lib/types";
import {
  deliverableStateLabels,
  deliverableTypeLabels,
} from "../lib/types";
import { formatDateTime, formatRelativeDate } from "../lib/format";
import { EmptyState } from "../components/EmptyState";
import { Avatar } from "../components/Avatar";
import { avatarColor, initials } from "../lib/avatar";
import { prepareSortable, compareDeliverablesForLens as compareLens } from "../lib/sortDeliverables";
import { parseGCalConferenceData } from "../lib/gcal";

type LensFilter = "active" | "shipped" | "all";
type Tab = "overview" | "email" | "calendar" | "deliverables" | "files";

const filterLabels: Record<LensFilter, string> = {
  active: "Active",
  shipped: "Done",
  all: "All",
};

const THREADS_PER_PAGE = 10;


// Map StakeholderLens filter to state_in values for SQL pushdown
function filterToStateIn(filter: LensFilter): import("../lib/types").DeliverableState[] | undefined {
  if (filter === "active") return ["drafting", "in_review"];
  if (filter === "shipped") return ["shipped"];
  return undefined; // "all" — fetch everything, client-side exclude "killed"
}

export function StakeholderLens() {
  const { stakeholderId } = useParams();
  const navigate = useNavigate();
  const [filter, setFilter] = useState<LensFilter>("active");
  const [activeTab, setActiveTab] = useState<Tab>("overview");
  const [isSaving, setIsSaving] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [briefings, setBriefings] = useState<Map<string, StakeholderBriefing>>(new Map());
  const briefing = useMemo(
    () => (stakeholderId ? (briefings.get(stakeholderId) ?? null) : null),
    [briefings, stakeholderId],
  );
  const [briefingDialogOpen, setBriefingDialogOpen] = useState(false);
  const [stakeholderSearch, setStakeholderSearch] = useState("");
  const autoTriggeredRef = useRef<Set<string>>(new Set());
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [editingStakeholder, setEditingStakeholder] = useState<Stakeholder | null>(null);
  const [pendingDelete, setPendingDelete] = useState<Stakeholder | null>(null);
  const [pendingDelinkThread, setPendingDelinkThread] = useState<GmailLocalThread | null>(null);
  const [isDelinking, setIsDelinking] = useState(false);
  const [emailPage, setEmailPage] = useState(1);
  const [error, setError] = useState<string | null>(null);

  // ── React-query data ───────────────────────────────────────────────────────
  const { data: details = [], isLoading } = useIpcQuery(
    qk.stakeholders.details,
    listStakeholderDetails,
  );

  const stateIn = filterToStateIn(filter);
  const deliverableQueryKey = qk.deliverables.forStakeholder(stakeholderId ?? "", filter);
  const { data: rawDeliverables = [], isLoading: isLoadingDeliverables } = useIpcQuery(
    deliverableQueryKey,
    () => listDeliverables({ stakeholder_id: stakeholderId, state_in: stateIn }),
    { enabled: !!stakeholderId },
  );

  const selectedDetail = useMemo(
    () => details.find((d) => d.stakeholder.id === stakeholderId) ?? null,
    [details, stakeholderId],
  );
  const selectedEmail = selectedDetail?.stakeholder.email ?? "";

  const { data: emailThreads = [] } = useIpcQuery(
    qk.gmail.stakeholderThreads(stakeholderId ?? "", 100),
    () => gmailStakeholderThreads(stakeholderId!, 100),
    { enabled: !!stakeholderId },
  );
  const { data: emailHealth = null } = useIpcQuery(
    qk.gmail.stakeholderHealth(stakeholderId ?? ""),
    () => gmailStakeholderHealth(stakeholderId!),
    { enabled: !!stakeholderId },
  );
  const { data: calendarEvents = [] } = useIpcQuery(
    qk.gcal.stakeholderEvents(selectedEmail),
    () => gcalStakeholderEvents(selectedEmail),
    { enabled: !!selectedEmail },
  );
  // ── Derived data ───────────────────────────────────────────────────────────

  const visibleDeliverables = useMemo(() => {
    // state_in pushes "active"/"shipped" filters to SQL; "all" still needs client-side "killed" exclusion
    const base = filter === "all" ? rawDeliverables.filter((d) => d.state !== "killed") : rawDeliverables;
    return prepareSortable(base).sort(compareLens);
  }, [rawDeliverables, filter]);

  const deliverables = rawDeliverables; // alias for tab count / recentDeliverables below

  const recentDeliverables = useMemo(() => {
    return prepareSortable(rawDeliverables.filter((d) => d.state !== "killed"))
      .sort(compareLens)
      .slice(0, 3);
  }, [rawDeliverables]);

  const filteredDetails = useMemo(() => {
    const q = stakeholderSearch.trim().toLowerCase();
    if (!q) return details;
    return details.filter((d) => {
      const s = d.stakeholder;
      return (
        s.name.toLowerCase().includes(q) ||
        s.role.toLowerCase().includes(q) ||
        s.email.toLowerCase().includes(q)
      );
    });
  }, [details, stakeholderSearch]);

  const stakeholderByEmail = useMemo(() => {
    const map = new Map<string, Stakeholder>();
    for (const d of details) {
      if (d.stakeholder.email) map.set(d.stakeholder.email.toLowerCase(), d.stakeholder);
    }
    return map;
  }, [details]);

  const nextCalendarEvent = useMemo(() => {
    const now = new Date();
    return (
      calendarEvents
        .filter((e) => {
          const t = e.start_datetime ? new Date(e.start_datetime) : new Date(e.start_date);
          return t >= now;
        })
        .sort((a, b) => {
          const da = a.start_datetime ? new Date(a.start_datetime).getTime() : new Date(a.start_date).getTime();
          const db = b.start_datetime ? new Date(b.start_datetime).getTime() : new Date(b.start_date).getTime();
          return da - db;
        })[0] ?? null
    );
  }, [calendarEvents]);

  // Navigate away if stakeholderId is invalid after details load
  useEffect(() => {
    if (!isLoading && stakeholderId && !details.some((d) => d.stakeholder.id === stakeholderId)) {
      navigate("/stakeholders", { replace: true });
    }
  }, [isLoading, details, stakeholderId, navigate]);

  // Reset tab + email page when switching stakeholders
  useEffect(() => {
    setActiveTab("overview");
    setEmailPage(1);
  }, [stakeholderId]);

  async function handleCreate(input: UpdateStakeholderInput) {
    try {
      setMessage(null);
      setIsSaving(true);
      const created = await createStakeholder(input);
      void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all });
      navigate(`/stakeholders/${created.id}`);
      setCreateDialogOpen(false);
      setMessage(`Created ${created.name}.`);
    } finally {
      setIsSaving(false);
    }
  }

  async function handleUpdate(id: string, input: UpdateStakeholderInput) {
    try {
      setMessage(null);
      setIsSaving(true);
      const updated = await updateStakeholder(id, input);
      void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all });
      setEditingStakeholder(null);
      setMessage(`Saved ${updated.name}.`);
    } finally {
      setIsSaving(false);
    }
  }

  async function handleDelete(stakeholder: Stakeholder) {
    await deleteStakeholder(stakeholder.id);
    setBriefings((prev) => { const next = new Map(prev); next.delete(stakeholder.id); return next; });
    void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all });
    navigate("/stakeholders", { replace: true });
    setPendingDelete(null);
    setMessage(`Deleted ${stakeholder.name}.`);
  }

  async function handleDelinkThread(thread: GmailLocalThread) {
    if (!stakeholderId) return;
    try {
      setIsDelinking(true);
      await gmailExcludeThreadFromStakeholder(thread.thread_id, stakeholderId);
      void queryClient.invalidateQueries({ queryKey: qk.gmail.stakeholderThreads(stakeholderId, 100) });
      setPendingDelinkThread(null);
      setMessage(`Removed "${thread.subject || thread.thread_id}" from this stakeholder's threads.`);
    } finally {
      setIsDelinking(false);
    }
  }

  async function handleCopyVisible() {
    if (!selectedDetail) return;
    const markdown = visibleDeliverables.length
      ? visibleDeliverables
          .map((d) => `- [${deliverableTypeLabels[d.type]}] ${d.title} - ${d.claim || "No claim yet."}`)
          .join("\n")
      : "- No visible deliverables.";
    await copyToClipboard(`## ${selectedDetail.stakeholder.name} briefing\n\n${markdown}`);
    setMessage("Copied visible deliverables as markdown.");
  }

  async function handleGenerateBriefing() {
    if (!selectedDetail) return;
    try {
      setError(null);
      setMessage(null);
      setIsGenerating(true);
      const generated = await generateStakeholderBriefing(selectedDetail.stakeholder.id);
      setBriefings((prev) => new Map(prev).set(selectedDetail.stakeholder.id, generated));
      setBriefingDialogOpen(true);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsGenerating(false);
    }
  }

  useEffect(() => {
    if (!stakeholderId || briefing || isGenerating || !nextCalendarEvent) return;
    if (autoTriggeredRef.current.has(stakeholderId)) return;
    const eventTime = nextCalendarEvent.start_datetime
      ? new Date(nextCalendarEvent.start_datetime).getTime()
      : new Date(nextCalendarEvent.start_date).getTime();
    const hoursUntil = (eventTime - Date.now()) / (1000 * 60 * 60);
    if (hoursUntil >= 0 && hoursUntil <= 48) {
      autoTriggeredRef.current.add(stakeholderId);
      void handleGenerateBriefing();
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nextCalendarEvent, stakeholderId]);

  const tabs: { id: Tab; label: string; count?: number }[] = [
    { id: "overview", label: "Overview" },
    { id: "email", label: "Email", count: emailThreads.length },
    { id: "calendar", label: "Calendar", count: calendarEvents.length },
    { id: "deliverables", label: "Deliverables", count: deliverables.filter((d) => d.state !== "killed").length },
    { id: "files", label: "Files" },
  ];

  return (
    <div className="mx-auto grid min-h-full max-w-7xl gap-5 px-5 py-6 xl:grid-cols-[288px_minmax(0,1fr)]">

      {/* ── Sidebar ── */}
      <aside className="min-w-0">
        <div className="mb-4 flex items-end justify-between gap-3">
          <div>
            <p className="page-kicker">Audience context</p>
            <h1 className="text-2xl font-semibold tracking-normal text-zinc-950">Stakeholders</h1>
          </div>
          <div className="flex gap-2">
            <button
              className="btn h-8 w-8 px-0"
              onClick={() => void queryClient.invalidateQueries({ queryKey: qk.stakeholders.all })}
              title="Refresh"
              type="button"
            >
              <RefreshCw aria-hidden="true" size={14} />
            </button>
            <button
              className="btn btn-primary h-8 w-8 px-0"
              onClick={() => setCreateDialogOpen(true)}
              title="New stakeholder"
              type="button"
            >
              <Plus aria-hidden="true" size={14} />
            </button>
          </div>
        </div>

        {error ? <div className="mb-3 notice notice-error text-xs">{error}</div> : null}
        {message ? <div className="mb-3 notice notice-success text-xs">{message}</div> : null}

        <div className="relative mb-3">
          <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400" size={13} />
          <input
            className="field-control h-9 pl-9 text-sm"
            onChange={(e) => setStakeholderSearch(e.currentTarget.value)}
            placeholder="Search people, roles…"
            value={stakeholderSearch}
          />
        </div>

        {isLoading ? (
          <div className="space-y-2">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="h-[60px] animate-pulse rounded-xl bg-zinc-100" />
            ))}
          </div>
        ) : filteredDetails.length === 0 ? (
          <div className="rounded-xl border border-dashed border-zinc-200 p-5 text-center text-sm text-zinc-400">
            No stakeholders match.
          </div>
        ) : (
          <nav className="space-y-1.5">
            {filteredDetails.map((detail) => {
              const s = detail.stakeholder;
              const selected = s.id === stakeholderId;
              return (
                <Link
                  className={[
                    "group flex items-center gap-3 rounded-xl border p-3 transition-all duration-150",
                    selected
                      ? "border-zinc-200 bg-white shadow-sm"
                      : "border-transparent hover:border-zinc-100 hover:bg-white hover:shadow-sm",
                  ].join(" ")}
                  key={s.id}
                  to={`/stakeholders/${s.id}`}
                >
                  <Avatar name={s.name} size="sm" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-semibold text-zinc-900">{s.name}</p>
                    <p className="truncate text-xs text-zinc-400">{s.role || "No role set"}</p>
                  </div>
                  {detail.in_flight_count > 0 && (
                    <span className="shrink-0 rounded-full bg-sky-50 px-2 py-0.5 text-[11px] font-semibold text-sky-600">
                      {detail.in_flight_count}
                    </span>
                  )}
                </Link>
              );
            })}
          </nav>
        )}
      </aside>

      {/* ── Main panel ── */}
      <main className="min-w-0">
        {!selectedDetail ? (
          <div className="flex min-h-[28rem] items-center justify-center rounded-2xl border border-dashed border-zinc-200 bg-white">
            <div className="text-center">
              <UsersRound className="mx-auto mb-3 text-zinc-200" size={36} />
              <p className="text-sm font-semibold text-zinc-700">Select a stakeholder</p>
              <p className="mt-1 max-w-xs text-xs leading-5 text-zinc-400">
                Review in-flight work, email history, and upcoming meetings in one place.
              </p>
            </div>
          </div>
        ) : (
          <div className="space-y-4">

            {/* ── Profile card ── */}
            <section className="rounded-2xl border border-zinc-100 bg-white p-5 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
              <div className="flex flex-wrap items-start justify-between gap-4">
                {/* Avatar + identity */}
                <div className="flex items-center gap-4">
                  <Avatar name={selectedDetail.stakeholder.name} size="lg" />
                  <div>
                    <h2 className="text-2xl font-semibold tracking-tight text-zinc-950">
                      {selectedDetail.stakeholder.name}
                    </h2>
                    <p className="text-sm text-zinc-500">
                      {selectedDetail.stakeholder.role || "Role not set"}
                    </p>
                    {selectedDetail.stakeholder.email && (
                      <p className="mt-0.5 text-xs text-zinc-400">
                        {selectedDetail.stakeholder.email}
                      </p>
                    )}
                  </div>
                </div>

                {/* Actions */}
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    className="btn"
                    onClick={() => setEditingStakeholder(selectedDetail.stakeholder)}
                    type="button"
                  >
                    <Pencil size={13} />
                    Edit
                  </button>
                  <button className="btn" onClick={() => void handleCopyVisible()} type="button">
                    <Clipboard size={13} />
                    Copy
                  </button>
                  <button
                    className="btn btn-danger"
                    onClick={() => setPendingDelete(selectedDetail.stakeholder)}
                    type="button"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>

              {/* Stats strip */}
              <div className="mt-4 flex flex-wrap items-center gap-x-4 gap-y-1 text-sm text-zinc-500">
                <span>
                  <span className="font-semibold text-zinc-900">{selectedDetail.deliverable_count}</span>{" "}
                  deliverables
                </span>
                <span className="text-zinc-200">·</span>
                <span>
                  <span className="font-semibold text-zinc-900">{selectedDetail.in_flight_count}</span>{" "}
                  in-flight
                </span>
                <span className="text-zinc-200">·</span>
                <span>
                  <span className="font-semibold text-zinc-900">
                    {selectedDetail.days_since_last_delivery === null
                      ? "Never"
                      : `${selectedDetail.days_since_last_delivery}d`}
                  </span>{" "}
                  since last delivery
                </span>
              </div>

              {/* Notes */}
              {selectedDetail.stakeholder.notes && (
                <p className="mt-4 rounded-xl bg-zinc-50 px-4 py-3 text-sm leading-6 text-zinc-600">
                  {selectedDetail.stakeholder.notes}
                </p>
              )}

              {/* Brief me row */}
              <div className="mt-4 flex flex-wrap items-center gap-2.5">
                <button
                  className="btn btn-primary"
                  disabled={isGenerating}
                  onClick={() => briefing ? setBriefingDialogOpen(true) : void handleGenerateBriefing()}
                  type="button"
                >
                  <Sparkles size={13} />
                  {isGenerating ? "Generating briefing…" : briefing ? "View briefing" : "Brief me"}
                </button>
                {briefing && !isGenerating && (
                  <span className="text-xs text-zinc-400">
                    {briefing.generated_with} · {new Date(briefing.generated_at).toLocaleTimeString(undefined, {
                      hour: "numeric",
                      minute: "2-digit",
                    })}
                  </span>
                )}
                {nextCalendarEvent && !briefing && !isGenerating && (
                  <span className="flex items-center gap-1 rounded-full bg-sky-50 px-2.5 py-1 text-xs font-medium text-sky-600">
                    <CalendarDays size={11} />
                    Meeting soon — auto-briefing
                  </span>
                )}
              </div>
            </section>

            {/* ── Tabbed content card ── */}
            <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
              {/* Tab bar */}
              <div className="relative border-b border-zinc-100">
                <nav className="flex overflow-x-auto">
                  {tabs.map((tab) => (
                    <button
                      className="relative shrink-0 px-5 py-3.5 text-sm transition-colors"
                      key={tab.id}
                      onClick={() => setActiveTab(tab.id)}
                      type="button"
                    >
                      <span
                        className={
                          activeTab === tab.id
                            ? "font-semibold text-zinc-950"
                            : "text-zinc-400 hover:text-zinc-700"
                        }
                      >
                        {tab.label}
                      </span>
                      {tab.count !== undefined && tab.count > 0 && (
                        <span className="ml-1.5 rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                          {tab.count}
                        </span>
                      )}
                      {activeTab === tab.id && (
                        <motion.div
                          className="absolute bottom-0 left-0 right-0 h-0.5 bg-zinc-900"
                          layoutId="tab-indicator"
                          transition={MOTION.spring}
                        />
                      )}
                    </button>
                  ))}
                </nav>
              </div>

              {/* Tab content */}
              <AnimatePresence mode="wait">
                <motion.div
                  key={activeTab}
                  {...fadeY}
                >
                  {/* ── Overview ── */}
                  {activeTab === "overview" && (
                    <div className="p-5 space-y-4">
                      <div className="grid gap-4 sm:grid-cols-2">
                        {/* Email health card */}
                        <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
                          <div className="mb-3 flex items-center gap-2">
                            <Mail className="text-zinc-400" size={13} />
                            <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                              Email relationship
                            </span>
                          </div>
                          {emailHealth ? (
                            <>
                              <div className="flex items-baseline gap-1">
                                <span className="text-3xl font-bold text-zinc-950">
                                  {emailHealth.health_score}
                                </span>
                                <span className="text-sm text-zinc-400">/100</span>
                              </div>
                              <p className="mt-1.5 text-xs text-zinc-500">
                                {emailHealth.days_since_last_email === null
                                  ? "Never contacted"
                                  : `Last contact ${emailHealth.days_since_last_email}d ago`}
                                {" · "}
                                {emailHealth.thread_count} threads
                              </p>
                            </>
                          ) : (
                            <p className="text-sm text-zinc-400">No email data yet</p>
                          )}
                        </div>

                        {/* Next meeting card */}
                        <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
                          <div className="mb-3 flex items-center gap-2">
                            <CalendarDays className="text-zinc-400" size={13} />
                            <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                              Next meeting
                            </span>
                          </div>
                          {nextCalendarEvent ? (
                            <>
                              <p className="text-sm font-semibold text-zinc-900 leading-snug">
                                {nextCalendarEvent.title}
                              </p>
                              <p className="mt-1 text-xs text-zinc-500">
                                {nextCalendarEvent.start_datetime
                                  ? new Date(nextCalendarEvent.start_datetime).toLocaleString(
                                      undefined,
                                      { month: "short", day: "numeric", weekday: "short", hour: "numeric", minute: "2-digit" },
                                    )
                                  : nextCalendarEvent.start_date}
                              </p>
                              {(() => {
                                const videoUri = parseGCalConferenceData(nextCalendarEvent)?.video_uri;
                                return videoUri ? (
                                  <button
                                    className="mt-2.5 flex items-center gap-1.5 rounded-lg bg-sky-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-sky-700 transition-colors"
                                    onClick={() => void openExternalUrl(videoUri)}
                                    type="button"
                                  >
                                    <Video size={12} />
                                    Join meeting
                                  </button>
                                ) : null;
                              })()}
                            </>
                          ) : (
                            <p className="text-sm text-zinc-400">No upcoming meetings</p>
                          )}
                        </div>
                      </div>

                      {/* Recent deliverables */}
                      <div className="overflow-hidden rounded-xl border border-zinc-100">
                        <div className="flex items-center justify-between border-b border-zinc-50 px-4 py-3">
                          <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                            Recent deliverables
                          </span>
                          {deliverables.length > 3 && (
                            <button
                              className="text-xs font-medium text-zinc-400 hover:text-zinc-700 transition-colors"
                              onClick={() => setActiveTab("deliverables")}
                              type="button"
                            >
                              See all {deliverables.filter((d) => d.state !== "killed").length} →
                            </button>
                          )}
                        </div>
                        {recentDeliverables.length === 0 ? (
                          <EmptyState variant="inline" icon={Package} title="No deliverables yet" />
                        ) : (
                          <div className="divide-y divide-zinc-50">
                            {recentDeliverables.map((d) => (
                              <DeliverableRow deliverable={d} key={d.id} />
                            ))}
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                  {/* ── Email ── */}
                  {activeTab === "email" && (
                    <div>
                      {emailHealth && (
                        <div className="grid gap-px border-b border-zinc-100 bg-zinc-100 sm:grid-cols-4">
                          {[
                            {
                              label: "Last email",
                              value:
                                emailHealth.days_since_last_email === null
                                  ? "Never"
                                  : `${emailHealth.days_since_last_email}d`,
                            },
                            { label: "Threads", value: emailHealth.thread_count },
                            { label: "Sent", value: emailHealth.sent_count },
                            { label: "Received", value: emailHealth.received_count },
                          ].map((stat) => (
                            <div className="bg-white px-5 py-3.5" key={stat.label}>
                              <p className="text-[11px] font-medium text-zinc-400">{stat.label}</p>
                              <p className="mt-0.5 text-lg font-semibold text-zinc-950">{stat.value}</p>
                            </div>
                          ))}
                        </div>
                      )}
                      {emailThreads.length === 0 ? (
                        <div className="px-5 py-10 text-center">
                          <Mail className="mx-auto mb-2 text-zinc-200" size={24} />
                          <p className="text-sm text-zinc-400">No synced threads for this stakeholder.</p>
                        </div>
                      ) : (
                        <>
                          <div className="divide-y divide-zinc-50">
                            {emailThreads
                              .slice((emailPage - 1) * THREADS_PER_PAGE, emailPage * THREADS_PER_PAGE)
                              .map((thread) => {
                                const threadStakeholders = thread.participants
                                  .map((p) => stakeholderByEmail.get(p.email.toLowerCase()))
                                  .filter((s): s is Stakeholder => s !== undefined);
                                return (
                                  <div
                                    className="group relative flex items-start gap-3 px-5 py-4 transition-colors hover:bg-zinc-50"
                                    key={thread.thread_id}
                                  >
                                    <Link
                                      className="min-w-0 flex-1"
                                      to={`/email?category=${thread.ai_category}&thread=${thread.thread_id}`}
                                    >
                                      {(() => {
                                        const m = thread.ai_title?.match(/^\[([^\]]+)\]\s*(.+)$/);
                                        const label = m ? m[1] : null;
                                        const title = m ? m[2] : (thread.ai_title ?? thread.subject);
                                        return (
                                          <>
                                            <div className="flex items-start justify-between gap-2">
                                              <div className="flex min-w-0 items-center gap-1.5">
                                                {label && (
                                                  <span className="shrink-0 rounded px-1.5 py-0.5 text-[10px] font-semibold bg-violet-50 text-violet-600">
                                                    {label}
                                                  </span>
                                                )}
                                                <h3 className="truncate text-sm font-semibold text-zinc-900">
                                                  {title}
                                                </h3>
                                              </div>
                                              <time className="shrink-0 text-[11px] text-zinc-400">
                                                {thread.last_message_at
                                                  ? formatRelativeDate(thread.last_message_at)
                                                  : "—"}
                                              </time>
                                            </div>
                                            <p className="mt-1 line-clamp-1 text-xs leading-5 text-zinc-500">
                                              {thread.summary ? (
                                                <>
                                                  <span className="mr-1 text-violet-400">✦</span>
                                                  {thread.summary
                                                    .replace(/^\s*[*\-]\s*/gm, "")
                                                    .replace(/\n+/g, " · ")
                                                    .trim()}
                                                </>
                                              ) : (
                                                thread.snippet
                                              )}
                                            </p>
                                            {threadStakeholders.length > 0 && (
                                              <div className="mt-1.5">
                                                <ThreadStakeholderChips stakeholders={threadStakeholders} />
                                              </div>
                                            )}
                                          </>
                                        );
                                      })()}
                                    </Link>
                                    <button
                                      className="mt-0.5 shrink-0 rounded p-1 text-zinc-200 opacity-0 transition-opacity hover:bg-red-50 hover:text-red-500 group-hover:opacity-100"
                                      onClick={() => setPendingDelinkThread(thread)}
                                      title="Remove from this stakeholder"
                                      type="button"
                                    >
                                      <Unlink size={12} />
                                    </button>
                                  </div>
                                );
                              })}
                          </div>
                          {emailThreads.length > THREADS_PER_PAGE && (
                            <div className="flex items-center justify-between border-t border-zinc-50 px-5 py-3">
                              <span className="text-xs text-zinc-400">
                                {(emailPage - 1) * THREADS_PER_PAGE + 1}–
                                {Math.min(emailPage * THREADS_PER_PAGE, emailThreads.length)} of{" "}
                                {emailThreads.length}
                              </span>
                              <div className="flex gap-1">
                                {Array.from(
                                  { length: Math.ceil(emailThreads.length / THREADS_PER_PAGE) },
                                  (_, i) => i + 1,
                                ).map((page) => (
                                  <button
                                    className={`h-7 min-w-[1.75rem] rounded px-1.5 text-xs font-medium transition-colors ${
                                      page === emailPage
                                        ? "bg-zinc-900 text-white"
                                        : "text-zinc-500 hover:bg-zinc-100"
                                    }`}
                                    key={page}
                                    onClick={() => setEmailPage(page)}
                                    type="button"
                                  >
                                    {page}
                                  </button>
                                ))}
                              </div>
                            </div>
                          )}
                        </>
                      )}
                    </div>
                  )}

                  {/* ── Calendar ── */}
                  {activeTab === "calendar" && (
                    <div>
                      {calendarEvents.length === 0 ? (
                        <div className="px-5 py-10 text-center">
                          <CalendarDays className="mx-auto mb-2 text-zinc-200" size={24} />
                          <p className="text-sm text-zinc-400">
                            {selectedDetail.stakeholder.email
                              ? "No calendar events found (±30 days)."
                              : "Add an email address to match calendar events."}
                          </p>
                        </div>
                      ) : (
                        <div className="divide-y divide-zinc-50">
                          {calendarEvents.map((event) => {
                            const videoUri = parseGCalConferenceData(event)?.video_uri ?? undefined;
                            const when = event.is_all_day
                              ? event.start_date
                              : event.start_datetime
                                ? new Date(event.start_datetime).toLocaleString(undefined, {
                                    month: "short",
                                    day: "numeric",
                                    weekday: "short",
                                    hour: "numeric",
                                    minute: "2-digit",
                                  })
                                : event.start_date;
                            const isPast = (() => {
                              const t = event.start_datetime
                                ? new Date(event.start_datetime)
                                : new Date(event.start_date);
                              return t < new Date();
                            })();
                            return (
                              <div
                                className={`flex items-start justify-between gap-3 px-5 py-4 transition-colors hover:bg-zinc-50 ${isPast ? "opacity-50" : ""}`}
                                key={event.id}
                              >
                                <div className="min-w-0">
                                  <p className="truncate text-sm font-semibold text-zinc-900">
                                    {event.title}
                                  </p>
                                  <p className="mt-0.5 text-xs text-zinc-500">{when}</p>
                                  {event.location && (
                                    <p className="mt-0.5 truncate text-xs text-zinc-400">
                                      {event.location}
                                    </p>
                                  )}
                                </div>
                                <div className="flex shrink-0 items-center gap-1.5">
                                  {videoUri && !isPast && (
                                    <button
                                      className="flex items-center gap-1 rounded-lg bg-sky-600 px-2.5 py-1.5 text-xs font-semibold text-white hover:bg-sky-700 transition-colors"
                                      onClick={() => void openExternalUrl(videoUri)}
                                      type="button"
                                    >
                                      <Video size={11} />
                                      Join
                                    </button>
                                  )}
                                  {event.html_link && (
                                    <button
                                      className="rounded-lg p-1.5 text-zinc-300 hover:bg-zinc-100 hover:text-zinc-600 transition-colors"
                                      onClick={() => void openExternalUrl(event.html_link!)}
                                      title="Open in Google Calendar"
                                      type="button"
                                    >
                                      <ExternalLink size={13} />
                                    </button>
                                  )}
                                </div>
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  )}

                  {/* ── Deliverables ── */}
                  {activeTab === "deliverables" && (
                    <div>
                      <div className="flex items-center justify-between border-b border-zinc-50 px-5 py-3">
                        <span className="text-xs text-zinc-400">
                          {visibleDeliverables.length} visible
                        </span>
                        <div className="flex rounded-lg border border-zinc-100 bg-zinc-50 p-0.5">
                          {(Object.keys(filterLabels) as LensFilter[]).map((f) => (
                            <button
                              className={[
                                "h-7 rounded-md px-3 text-xs font-medium transition-colors",
                                filter === f
                                  ? "bg-white text-zinc-950 shadow-sm"
                                  : "text-zinc-400 hover:text-zinc-700",
                              ].join(" ")}
                              key={f}
                              onClick={() => setFilter(f)}
                              type="button"
                            >
                              {filterLabels[f]}
                            </button>
                          ))}
                        </div>
                      </div>
                      {isLoadingDeliverables ? (
                        <div className="space-y-2 p-5">
                          {[...Array(3)].map((_, i) => (
                            <div key={i} className="h-16 animate-pulse rounded-xl bg-zinc-100" />
                          ))}
                        </div>
                      ) : visibleDeliverables.length === 0 ? (
                        <EmptyState variant="inline" icon={FileText} title="No deliverables match this filter" />
                      ) : (
                        <div className="divide-y divide-zinc-50">
                          {visibleDeliverables.map((d) => (
                            <DeliverableRow deliverable={d} key={d.id} />
                          ))}
                        </div>
                      )}
                    </div>
                  )}

                  {/* ── Files ── */}
                  {activeTab === "files" && (
                    <div className="p-5">
                      <EntityFilesPanel
                        entityId={selectedDetail.stakeholder.id}
                        entityKind="stakeholder"
                      />
                    </div>
                  )}
                </motion.div>
              </AnimatePresence>
            </section>
          </div>
        )}
      </main>

      {/* ── Dialogs ── */}
      <StakeholderDialog
        isSubmitting={isSaving}
        onOpenChange={setCreateDialogOpen}
        onSubmit={handleCreate}
        open={createDialogOpen}
        submitLabel="Create stakeholder"
        title="New stakeholder"
      />
      <StakeholderDialog
        initialValue={editingStakeholder ?? undefined}
        isSubmitting={isSaving}
        onOpenChange={(open) => { if (!open) setEditingStakeholder(null); }}
        onSubmit={(input) =>
          editingStakeholder ? handleUpdate(editingStakeholder.id, input) : Promise.resolve()
        }
        open={Boolean(editingStakeholder)}
        submitLabel="Save profile"
        title="Edit stakeholder"
      />
      <DeleteStakeholderDialog
        onCancel={() => setPendingDelete(null)}
        onConfirm={() => pendingDelete ? handleDelete(pendingDelete) : Promise.resolve()}
        stakeholder={pendingDelete}
      />
      {briefing && selectedDetail && (
        <BriefingDialog
          briefing={briefing}
          isRegenerating={isGenerating}
          onOpenChange={setBriefingDialogOpen}
          onRegenerate={() => void handleGenerateBriefing()}
          open={briefingDialogOpen}
          stakeholderId={selectedDetail.stakeholder.id}
          stakeholderName={selectedDetail.stakeholder.name}
        />
      )}

      <Dialog.Root
        onOpenChange={(open) => { if (!open) setPendingDelinkThread(null); }}
        open={Boolean(pendingDelinkThread)}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="fixed inset-0 z-50 bg-zinc-950/20 backdrop-blur-sm" />
          <Dialog.Content className="fixed left-1/2 top-1/2 z-50 w-[400px] max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2 rounded-2xl border border-zinc-100 bg-white p-6 shadow-2xl">
            <div className="mb-4 flex items-start gap-3">
              <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-amber-50">
                <AlertTriangle className="text-amber-500" size={16} />
              </div>
              <div>
                <Dialog.Title className="text-sm font-semibold text-zinc-950">
                  Remove email from this stakeholder?
                </Dialog.Title>
                <Dialog.Description className="mt-1 text-sm leading-5 text-zinc-500">
                  This will hide{" "}
                  <span className="font-medium text-zinc-700">
                    "{pendingDelinkThread?.subject || pendingDelinkThread?.thread_id}"
                  </span>{" "}
                  from {selectedDetail?.stakeholder.name}'s email history. The email stays in your inbox.
                </Dialog.Description>
              </div>
            </div>
            <div className="flex justify-end gap-2">
              <button
                className="btn"
                disabled={isDelinking}
                onClick={() => setPendingDelinkThread(null)}
                type="button"
              >
                Cancel
              </button>
              <button
                className="btn btn-danger"
                disabled={isDelinking}
                onClick={() =>
                  pendingDelinkThread ? void handleDelinkThread(pendingDelinkThread) : undefined
                }
                type="button"
              >
                <Unlink size={12} />
                {isDelinking ? "Removing…" : "Remove"}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
    </div>
  );
}

function StakeholderForm({
  initialValue,
  isSubmitting,
  onSubmit,
  submitLabel,
}: {
  initialValue?: Pick<Stakeholder, "name" | "email" | "role" | "notes">;
  isSubmitting: boolean;
  submitLabel: string;
  onSubmit: (input: UpdateStakeholderInput) => Promise<void>;
}) {
  const [name, setName] = useState(initialValue?.name ?? "");
  const [email, setEmail] = useState(initialValue?.email ?? "");
  const [role, setRole] = useState(initialValue?.role ?? "");
  const [notes, setNotes] = useState(initialValue?.notes ?? "");

  useEffect(() => {
    setName(initialValue?.name ?? "");
    setEmail(initialValue?.email ?? "");
    setRole(initialValue?.role ?? "");
    setNotes(initialValue?.notes ?? "");
  }, [initialValue]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await onSubmit({ name, email, role, notes });
    if (!initialValue) {
      setName(""); setEmail(""); setRole(""); setNotes("");
    }
  }

  return (
    <form className="space-y-3" onSubmit={handleSubmit}>
      <label className="block space-y-1.5">
        <span className="field-label">Name</span>
        <input
          className="field-control"
          maxLength={120}
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="CEO, mentor, content team"
          required
          value={name}
        />
      </label>
      <label className="block space-y-1.5">
        <span className="field-label">Role</span>
        <input
          className="field-control"
          maxLength={160}
          onChange={(e) => setRole(e.currentTarget.value)}
          placeholder="Decision maker, reviewer, collaborator"
          value={role}
        />
      </label>
      <label className="block space-y-1.5">
        <span className="field-label">Email</span>
        <input
          className="field-control"
          maxLength={180}
          onChange={(e) => setEmail(e.currentTarget.value)}
          placeholder="name@company.com"
          type="email"
          value={email}
        />
      </label>
      <label className="block space-y-1.5">
        <span className="field-label">Notes</span>
        <textarea
          className="field-control min-h-24"
          onChange={(e) => setNotes(e.currentTarget.value)}
          placeholder="Context to remember before reviews or 1:1s"
          value={notes}
        />
      </label>
      <button className="btn btn-primary" disabled={isSubmitting} type="submit">
        <Check size={14} />
        {submitLabel}
      </button>
    </form>
  );
}

function StakeholderDialog({
  initialValue,
  isSubmitting,
  onOpenChange,
  onSubmit,
  open,
  submitLabel,
  title,
}: {
  initialValue?: Pick<Stakeholder, "name" | "email" | "role" | "notes">;
  isSubmitting: boolean;
  onOpenChange: (open: boolean) => void;
  onSubmit: (input: UpdateStakeholderInput) => Promise<void>;
  open: boolean;
  submitLabel: string;
  title: string;
}) {
  return (
    <Dialog.Root onOpenChange={onOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-zinc-950/20 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-12 z-50 max-h-[calc(100vh-6rem)] w-[min(92vw,34rem)] -translate-x-1/2 overflow-y-auto rounded-2xl border border-zinc-100 bg-white p-5 shadow-2xl">
          <div className="mb-4 flex items-start justify-between gap-4">
            <div>
              <p className="page-kicker">Profile</p>
              <Dialog.Title className="text-lg font-semibold text-zinc-950">{title}</Dialog.Title>
              <Dialog.Description className="mt-1 text-sm leading-6 text-zinc-500">
                Keep the stakeholder profile current so deliverables and email context stay easy to review.
              </Dialog.Description>
            </div>
            <Dialog.Close aria-label="Close" className="btn h-8 w-8 px-0">
              <X size={14} />
            </Dialog.Close>
          </div>
          <StakeholderForm
            initialValue={initialValue}
            isSubmitting={isSubmitting}
            onSubmit={onSubmit}
            submitLabel={submitLabel}
          />
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function DeleteStakeholderDialog({
  onCancel,
  onConfirm,
  stakeholder,
}: {
  onCancel: () => void;
  onConfirm: () => Promise<void>;
  stakeholder: Stakeholder | null;
}) {
  return (
    <Dialog.Root onOpenChange={(open) => !open && onCancel()} open={Boolean(stakeholder)}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-zinc-950/20 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-24 z-50 w-[min(92vw,28rem)] -translate-x-1/2 rounded-2xl border border-zinc-100 bg-white p-5 shadow-2xl">
          <div className="mb-4 flex items-start gap-3">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-xl bg-red-50 text-red-500">
              <AlertTriangle size={16} />
            </div>
            <div>
              <Dialog.Title className="text-base font-semibold text-zinc-950">
                Delete stakeholder?
              </Dialog.Title>
              <Dialog.Description className="mt-1 text-sm leading-6 text-zinc-500">
                Deliverables aimed at {stakeholder?.name ?? "this stakeholder"} will keep their work
                but lose the stakeholder link.
              </Dialog.Description>
            </div>
          </div>
          <div className="flex justify-end gap-2">
            <button className="btn" onClick={onCancel} type="button">
              Cancel
            </button>
            <button className="btn btn-danger" onClick={() => void onConfirm()} type="button">
              <Trash2 size={13} />
              Delete
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function DeliverableRow({ deliverable }: { deliverable: Deliverable }) {
  return (
    <Link
      className="block px-5 py-4 transition-colors hover:bg-zinc-50"
      to={`/deliverables/${deliverable.id}`}
    >
      <div className="mb-1.5 flex flex-wrap items-center gap-1.5">
        <span className="rounded-md bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold text-zinc-600">
          {deliverableTypeLabels[deliverable.type]}
        </span>
        <StateBadge state={deliverable.state} />
        <span className="text-[11px] text-zinc-400">{formatLensDate(deliverable)}</span>
      </div>
      <h3 className="text-sm font-semibold text-zinc-950">{deliverable.title}</h3>
      {deliverable.claim && (
        <p className="mt-1 line-clamp-2 text-xs leading-5 text-zinc-500">{deliverable.claim}</p>
      )}
      {deliverable.initiatives.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {deliverable.initiatives.map((initiative) => (
            <span
              className="rounded-md bg-accent-50 px-2 py-0.5 text-[10px] font-medium text-accent-700"
              key={initiative.id}
            >
              {initiative.title}
            </span>
          ))}
        </div>
      )}
    </Link>
  );
}

function StateBadge({ state }: { state: DeliverableState }) {
  const cls =
    state === "shipped"
      ? "bg-emerald-50 text-emerald-700"
      : state === "in_review"
        ? "bg-sky-50 text-sky-700"
        : state === "killed"
          ? "bg-zinc-100 text-zinc-400"
          : "bg-amber-50 text-amber-700";
  return (
    <span className={`rounded-md px-2 py-0.5 text-[10px] font-semibold ${cls}`}>
      {deliverableStateLabels[state]}
    </span>
  );
}

function formatLensDate(deliverable: Deliverable) {
  if (deliverable.shipped_at) return `Done ${formatDateTime(deliverable.shipped_at)}`;
  return `Updated ${formatDateTime(deliverable.updated_at)}`;
}


function ThreadStakeholderChips({ stakeholders }: { stakeholders: Stakeholder[] }) {
  const visible = stakeholders.slice(0, 4);
  const overflow = stakeholders.length - visible.length;
  return (
    <div className="flex items-center">
      {visible.map((s, i) => (
        <div
          className="group/chip relative"
          key={s.id}
          style={{ marginLeft: i === 0 ? 0 : "-5px", zIndex: visible.length - i }}
        >
          <div
            className={`flex h-5 w-5 items-center justify-center rounded-full border border-white text-[9px] font-bold ${avatarColor(s.name).bg} ${avatarColor(s.name).text}`}
            title={s.name}
          >
            {initials(s.name)}
          </div>
        </div>
      ))}
      {overflow > 0 && (
        <div
          className="flex h-5 w-5 items-center justify-center rounded-full border border-white bg-zinc-100 text-[9px] font-bold text-zinc-500"
          style={{ marginLeft: "-5px" }}
        >
          +{overflow}
        </div>
      )}
    </div>
  );
}

async function copyToClipboard(value: string) {
  await navigator.clipboard.writeText(value);
}

function SourceChip({ source }: { source?: string }) {
  if (!source) return null;
  const { label, cls } =
    source === "email"
      ? { label: "email", cls: "bg-violet-50 text-violet-600" }
      : source === "meeting"
        ? { label: "mtg", cls: "bg-sky-50 text-sky-600" }
        : { label: "del", cls: "bg-zinc-100 text-zinc-500" };
  return (
    <span className={`ml-1.5 inline-block rounded px-1 py-0.5 text-[9px] font-semibold uppercase leading-none ${cls}`}>
      {label}
    </span>
  );
}

function CommentableItem({
  bullet,
  item,
  onSubmit,
}: {
  bullet: ReactNode;
  item: BriefingItem;
  onSubmit: (comment: string) => Promise<string>;
}) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState("");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  async function handleSubmit() {
    if (!text.trim() || loading) return;
    setLoading(true);
    try {
      const res = await onSubmit(text.trim());
      setResult(res);
      setText("");
      setOpen(false);
    } finally {
      setLoading(false);
    }
  }

  return (
    <li className="space-y-1.5">
      <div className="flex items-start gap-2">
        {bullet}
        <span className="flex-1 text-xs leading-5 text-zinc-600">
          {item.text}
          <SourceChip source={item.source} />
          {result && (
            <span className="ml-1.5 inline-flex items-center gap-0.5 text-[10px] text-emerald-600">
              <Check size={9} />
              {result}
            </span>
          )}
          <button
            className="ml-1 inline-flex items-center rounded p-0.5 text-zinc-200 transition-colors hover:text-violet-400"
            onClick={() => setOpen((o) => !o)}
            title="Add note"
            type="button"
          >
            <MessageCircle size={10} />
          </button>
        </span>
      </div>
      {open && (
        <div className="flex items-center gap-1.5 pl-4">
          <input
            ref={inputRef}
            className="h-6 flex-1 rounded border border-zinc-200 bg-zinc-50 px-2 text-[11px] text-zinc-700 placeholder:text-zinc-300 focus:border-violet-400 focus:outline-none"
            disabled={loading}
            onChange={(e) => setText(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSubmit();
              if (e.key === "Escape") setOpen(false);
            }}
            placeholder="done, rescheduled, defer…"
            value={text}
          />
          <button
            className="h-6 rounded bg-violet-500 px-2 text-[11px] font-semibold text-white transition-colors hover:bg-violet-600 disabled:opacity-40"
            disabled={loading || !text.trim()}
            onClick={() => void handleSubmit()}
            type="button"
          >
            {loading ? "…" : "→"}
          </button>
          <button
            className="h-6 rounded px-1.5 text-[11px] text-zinc-400 transition-colors hover:bg-zinc-100"
            onClick={() => setOpen(false)}
            type="button"
          >
            ✕
          </button>
        </div>
      )}
    </li>
  );
}

function BriefingDialog({
  briefing,
  isRegenerating,
  onOpenChange,
  onRegenerate,
  open,
  stakeholderId,
  stakeholderName,
}: {
  briefing: StakeholderBriefing;
  isRegenerating: boolean;
  onOpenChange: (open: boolean) => void;
  onRegenerate: () => void;
  open: boolean;
  stakeholderId: string;
  stakeholderName: string;
}) {
  const { sections } = briefing;
  const hasCommitments =
    sections.open_commitments.length > 0 || sections.waiting_on_them.length > 0;

  function makeSubmit(section: string, item: BriefingItem) {
    return (comment: string) =>
      commentOnBriefingItem(stakeholderId, section, item.text, item.source, comment);
  }

  return (
    <Dialog.Root onOpenChange={onOpenChange} open={open}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-50 bg-zinc-950/20 backdrop-blur-sm" />
        <Dialog.Content className="fixed left-1/2 top-[4vh] z-50 flex max-h-[92vh] w-[min(92vw,44rem)] -translate-x-1/2 flex-col overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-2xl">
          {/* Header */}
          <div className="flex shrink-0 items-center gap-2.5 border-b border-zinc-100 px-5 py-3.5">
            <Sparkles className="text-violet-500" size={14} />
            <div className="min-w-0 flex-1">
              <Dialog.Title className="text-sm font-semibold text-zinc-950">
                {stakeholderName}
              </Dialog.Title>
              <p className="text-[11px] text-zinc-400">
                {briefing.generated_with} · {new Date(briefing.generated_at).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}
              </p>
            </div>
            <div className="flex items-center gap-1">
              <button
                className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-medium text-zinc-500 transition-colors hover:bg-zinc-100 hover:text-zinc-800 disabled:opacity-40"
                disabled={isRegenerating}
                onClick={onRegenerate}
                type="button"
              >
                <RefreshCw size={11} className={isRegenerating ? "animate-spin" : ""} />
                {isRegenerating ? "Redoing…" : "Redo"}
              </button>
              <Dialog.Close className="rounded-lg p-1.5 text-zinc-300 transition-colors hover:bg-zinc-100 hover:text-zinc-600">
                <X size={14} />
              </Dialog.Close>
            </div>
          </div>

          {/* Scrollable body */}
          <div className="min-h-0 overflow-y-auto">
            {/* TLDR */}
            <div className="border-b border-zinc-50 bg-zinc-50 px-5 py-4">
              <p className="text-sm leading-6 text-zinc-700">{sections.tldr}</p>
            </div>

            <div className="divide-y divide-zinc-50">
              {/* Commitments — 2 columns */}
              {hasCommitments && (
                <div className="grid gap-px bg-zinc-100 sm:grid-cols-2">
                  <div className="bg-white px-5 py-4">
                    <p className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                      You owe them
                      {sections.open_commitments.length > 0 && (
                        <span className="ml-1 font-normal text-zinc-300">
                          ({sections.open_commitments.length})
                        </span>
                      )}
                    </p>
                    {sections.open_commitments.length === 0 ? (
                      <EmptyState variant="inline" icon={Inbox} title="Nothing tracked" />
                    ) : (
                      <ul className="space-y-2.5">
                        {sections.open_commitments.map((item, i) => (
                          <CommentableItem
                            key={i}
                            bullet={<span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-400" />}
                            item={item}
                            onSubmit={makeSubmit("open_commitments", item)}
                          />
                        ))}
                      </ul>
                    )}
                  </div>
                  <div className="bg-white px-5 py-4">
                    <p className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                      They owe you
                      {sections.waiting_on_them.length > 0 && (
                        <span className="ml-1 font-normal text-zinc-300">
                          ({sections.waiting_on_them.length})
                        </span>
                      )}
                    </p>
                    {sections.waiting_on_them.length === 0 ? (
                      <EmptyState variant="inline" icon={Inbox} title="Nothing tracked" />
                    ) : (
                      <ul className="space-y-2.5">
                        {sections.waiting_on_them.map((item, i) => (
                          <CommentableItem
                            key={i}
                            bullet={<span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-sky-400" />}
                            item={item}
                            onSubmit={makeSubmit("waiting_on_them", item)}
                          />
                        ))}
                      </ul>
                    )}
                  </div>
                </div>
              )}

              {/* Recent wins + In flight */}
              {(sections.recent_wins.length > 0 || sections.in_flight.length > 0) && (
                <div className={`grid gap-px bg-zinc-100 ${sections.recent_wins.length > 0 && sections.in_flight.length > 0 ? "sm:grid-cols-2" : ""}`}>
                  {sections.recent_wins.length > 0 && (
                    <div className="bg-white px-5 py-4">
                      <p className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                        Recent wins
                      </p>
                      <ul className="space-y-2.5">
                        {sections.recent_wins.map((item, i) => (
                          <CommentableItem
                            key={i}
                            bullet={<Check className="mt-0.5 shrink-0 text-emerald-500" size={12} />}
                            item={item}
                            onSubmit={makeSubmit("recent_wins", item)}
                          />
                        ))}
                      </ul>
                    </div>
                  )}
                  {sections.in_flight.length > 0 && (
                    <div className="bg-white px-5 py-4">
                      <p className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                        In flight
                      </p>
                      <ul className="space-y-2.5">
                        {sections.in_flight.map((item, i) => (
                          <CommentableItem
                            key={i}
                            bullet={<span className="mt-2 h-1.5 w-1.5 shrink-0 rounded-full bg-zinc-300" />}
                            item={item}
                            onSubmit={makeSubmit("in_flight", item)}
                          />
                        ))}
                      </ul>
                    </div>
                  )}
                </div>
              )}

              {/* Talking points — comment inline per point */}
              {sections.talking_points.length > 0 && (
                <div className="px-5 py-4">
                  <p className="mb-3 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                    Talking points
                  </p>
                  <ol className="space-y-2.5">
                    {sections.talking_points.map((point, i) => (
                      <CommentableItem
                        key={i}
                        bullet={
                          <span className="shrink-0 pt-0.5 text-[10px] font-bold tabular-nums text-zinc-300">
                            {i + 1}.
                          </span>
                        }
                        item={{ text: point }}
                        onSubmit={(comment) =>
                          commentOnBriefingItem(stakeholderId, "talking_points", point, undefined, comment)
                        }
                      />
                    ))}
                  </ol>
                </div>
              )}

              {/* Watch out */}
              {sections.watch_out && (
                <div className="flex items-start gap-2.5 bg-amber-50 px-5 py-4">
                  <AlertTriangle className="mt-0.5 shrink-0 text-amber-500" size={13} />
                  <p className="text-xs leading-5 text-amber-700">{sections.watch_out}</p>
                </div>
              )}
            </div>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
