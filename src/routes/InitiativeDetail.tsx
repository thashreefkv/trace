import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  BarChart2,
  Check,
  ChevronRight,
  Link2,
  List,
  Mail,
  Paperclip,
  Pencil,
  RefreshCw,
  Target,
  Trash2,
  X,
  MessageSquare,
} from "lucide-react";
import { EmptyState } from "../components/EmptyState";
import { motion, AnimatePresence } from "framer-motion";
import { MOTION, fadeY } from "../lib/motion";
import {
  createInitiativeNote,
  deleteDeliverable,
  deleteInitiative,
  deleteInitiativeNote,
  getInitiative,
  gmailLinkThreadToInitiative,
  gmailListLocalThreads,
  listDeliverablesForInitiative,
  listInitiativeNotes,
  updateInitiative,
} from "../lib/ipc";
import type {
  CreateInitiativeInput,
  Deliverable,
  GmailLocalThread,
  Initiative,
  InitiativeNote,
} from "../lib/types";
import { formatDateTime } from "../lib/format";
import { InitiativeForm } from "../components/InitiativeForm";
import { StatePill } from "../components/StatePill";
import { DeliverableTypeBadge } from "../components/DeliverableTypeBadge";
import { EntityFilesPanel } from "../components/files/EntityFilesPanel";
import { GanttView } from "../components/GanttView";
import { InitiativeIcon } from "../components/InitiativeIcon";

type Tab = "deliverables" | "gantt" | "email" | "files" | "notes";

export function InitiativeDetail() {
  const { initiativeId } = useParams();
  const navigate = useNavigate();
  const [initiative, setInitiative] = useState<Initiative | null>(null);
  const [deliverables, setDeliverables] = useState<Deliverable[]>([]);
  const [notes, setNotes] = useState<InitiativeNote[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("deliverables");
  const [noteBody, setNoteBody] = useState("");
  const [linkedEmailThreads, setLinkedEmailThreads] = useState<GmailLocalThread[]>([]);
  const [suggestedEmailThreads, setSuggestedEmailThreads] = useState<GmailLocalThread[]>([]);
  const [emailLoading, setEmailLoading] = useState(false);
  const [emailMessage, setEmailMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!initiativeId) return;
    void loadInitiative(initiativeId);
  }, [initiativeId]);

  const initialFormValue = useMemo<CreateInitiativeInput | undefined>(() => {
    if (!initiative) return undefined;
    return { title: initiative.title, framing: initiative.framing, status: initiative.status, icon: initiative.icon, icon_color: initiative.icon_color };
  }, [initiative]);

  const tabs: { id: Tab; label: string; icon: React.ReactNode; count?: number }[] = [
    { id: "deliverables", label: "Deliverables", icon: <List size={13} />, count: deliverables.length },
    { id: "gantt", label: "Gantt", icon: <BarChart2 size={13} /> },
    { id: "email", label: "Email", icon: <Mail size={13} />, count: linkedEmailThreads.length },
    { id: "files", label: "Files", icon: <Paperclip size={13} /> },
    { id: "notes", label: "Notes", icon: <MessageSquare size={13} />, count: notes.length },
  ];

  async function loadInitiative(id: string) {
    try {
      setError(null);
      setIsLoading(true);
      const [nextInitiative, nextDeliverables, nextNotes] = await Promise.all([
        getInitiative(id),
        listDeliverablesForInitiative(id),
        listInitiativeNotes(id),
      ]);
      setInitiative(nextInitiative);
      setDeliverables(nextDeliverables);
      setNotes(nextNotes);
      void loadEmailContext(nextInitiative.id, nextInitiative.title);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleUpdate(input: CreateInitiativeInput) {
    if (!initiative) return;
    try {
      setError(null);
      setIsSaving(true);
      const updated = await updateInitiative(initiative.id, input);
      setInitiative(updated);
      setIsEditing(false);
      void loadEmailContext(updated.id, updated.title);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  async function loadEmailContext(id: string, title: string) {
    try {
      setEmailLoading(true);
      const [linked, suggested] = await Promise.all([
        gmailListLocalThreads({ initiative_id: id, limit: 24 }).catch(() => []),
        gmailListLocalThreads({ query: title, limit: 12 }).catch(() => []),
      ]);
      const linkedIds = new Set(linked.map((t) => t.thread_id));
      setLinkedEmailThreads(linked);
      setSuggestedEmailThreads(suggested.filter((t) => !linkedIds.has(t.thread_id)));
    } finally {
      setEmailLoading(false);
    }
  }

  async function handleLinkEmailThread(threadId: string) {
    if (!initiative) return;
    try {
      setEmailMessage(null);
      await gmailLinkThreadToInitiative(threadId, initiative.id);
      await loadEmailContext(initiative.id, initiative.title);
      setEmailMessage("Email thread linked.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleDeleteDeliverable(id: string, title: string) {
    if (!window.confirm(`Delete "${title}"?`)) return;
    try {
      setError(null);
      await deleteDeliverable(id);
      setDeliverables((cur) => cur.filter((d) => d.id !== id));
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleDelete() {
    if (!initiative) return;
    if (!window.confirm(`Delete "${initiative.title}"?`)) return;
    try {
      setError(null);
      await deleteInitiative(initiative.id);
      navigate("/initiatives");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleAddNote() {
    if (!initiative || !noteBody.trim()) return;
    try {
      const note = await createInitiativeNote(initiative.id, noteBody.trim());
      setNotes((n) => [note, ...n]);
      setNoteBody("");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleDeleteNote(id: string) {
    try {
      await deleteInitiativeNote(id);
      setNotes((n) => n.filter((note) => note.id !== id));
    } catch (caught) {
      setError(String(caught));
    }
  }

  return (
    <div className="mx-auto min-h-screen max-w-4xl px-5 py-6">
      {/* Back link */}
      <Link
        className="mb-5 inline-flex items-center gap-1.5 text-xs font-medium text-zinc-400 transition-colors hover:text-zinc-700"
        to="/initiatives"
      >
        <ArrowLeft size={13} />
        Initiatives
      </Link>

      {error && <div className="mb-4 notice notice-error text-sm">{error}</div>}

      {isLoading ? (
        <div className="space-y-4">
          <div className="h-[140px] animate-pulse rounded-2xl bg-zinc-100" />
          <div className="h-[400px] animate-pulse rounded-2xl bg-zinc-100" />
        </div>
      ) : initiative ? (
        <div className="space-y-4">

          {/* ── Header card ── */}
          <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
            {isEditing && initialFormValue ? (
              <div className="p-5">
                <div className="mb-4 flex items-center justify-between">
                  <p className="text-sm font-semibold text-zinc-950">Edit initiative</p>
                  <button
                    className="btn h-7 w-7 px-0"
                    onClick={() => setIsEditing(false)}
                    type="button"
                  >
                    <X size={13} />
                  </button>
                </div>
                <InitiativeForm
                  initialValue={initialFormValue}
                  isSubmitting={isSaving}
                  onCancel={() => setIsEditing(false)}
                  onSubmit={handleUpdate}
                  submitLabel="Save changes"
                />
              </div>
            ) : (
              <div className="p-5">
                <div className="flex flex-wrap items-start justify-between gap-4">
                  {/* Icon + title */}
                  <div className="flex items-center gap-4">
                    <div
                      className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl shadow-sm"
                      style={{ backgroundColor: `${initiative.icon_color || "#6366f1"}15`, color: initiative.icon_color || "#6366f1" }}
                    >
                      <InitiativeIcon name={initiative.icon} color={initiative.icon_color || "#6366f1"} size={24} />
                    </div>
                    <div>
                      <div className="mb-1 flex flex-wrap items-center gap-2">
                        <StatePill kind="initiative" status={initiative.status} />
                        <span className="text-xs text-zinc-400">
                          Created {formatDateTime(initiative.created_at)}
                        </span>
                      </div>
                      <h1 className="text-2xl font-semibold tracking-tight text-zinc-950">
                        {initiative.title}
                      </h1>
                      <p className="mt-0.5 text-xs text-zinc-400">
                        Updated {formatDateTime(initiative.updated_at)}
                      </p>
                    </div>
                  </div>

                  {/* Actions */}
                  <div className="flex gap-2">
                    <button
                      className="btn"
                      onClick={() => setIsEditing(true)}
                      type="button"
                    >
                      <Pencil size={13} />
                      Edit
                    </button>
                    <button
                      className="btn btn-danger"
                      onClick={() => void handleDelete()}
                      type="button"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                </div>

                {/* Framing */}
                {initiative.framing && (
                  <p className="mt-4 rounded-xl border border-zinc-100 bg-zinc-50 px-4 py-3 text-sm leading-6 text-zinc-600">
                    {initiative.framing}
                  </p>
                )}
              </div>
            )}
          </section>

          {/* ── Tabbed content card ── */}
          <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
            {/* Tab bar */}
            <div className="relative border-b border-zinc-100">
              <nav className="flex overflow-x-auto">
                {tabs.map((t) => (
                  <button
                    className="relative flex shrink-0 items-center gap-1.5 px-5 py-3.5 text-sm transition-colors"
                    key={t.id}
                    onClick={() => setTab(t.id)}
                    type="button"
                  >
                    <span className={tab === t.id ? "text-zinc-600" : "text-zinc-300"}>
                      {t.icon}
                    </span>
                    <span className={tab === t.id ? "font-semibold text-zinc-950" : "text-zinc-400 hover:text-zinc-700"}>
                      {t.label}
                    </span>
                    {t.count !== undefined && t.count > 0 && (
                      <span className="rounded-full bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                        {t.count}
                      </span>
                    )}
                    {tab === t.id && (
                      <motion.div
                        className="absolute bottom-0 left-0 right-0 h-0.5 bg-zinc-900"
                        layoutId="initiative-tab-indicator"
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
                key={tab}
                {...fadeY}
              >
                {/* ── Deliverables ── */}
                {tab === "deliverables" && (
                  <div>
                    {deliverables.length === 0 ? (
                      <EmptyState variant="inline" icon={Target} title="No deliverables attached" description="Link deliverables to this initiative to track progress." />
                    ) : (
                      <div className="relative ml-8 border-l border-zinc-100 py-4 pr-5">
                        <div className="space-y-0">
                          {deliverables.map((deliverable) => (
                            <div className="group relative pl-6" key={deliverable.id}>
                              {/* Timeline dot */}
                              <div className="absolute -left-[5px] top-5 h-2.5 w-2.5 rounded-full border-2 border-white bg-zinc-200 shadow-sm ring-1 ring-zinc-200" />
                              <div className="flex items-start gap-2 border-b border-zinc-50 py-4">
                                <Link
                                  className="min-w-0 flex-1"
                                  to={`/deliverables/${deliverable.id}`}
                                >
                                  <div className="mb-2 flex flex-wrap items-center gap-2">
                                    <DeliverableTypeBadge type={deliverable.type} />
                                    <StatePill kind="deliverable" state={deliverable.state} />
                                    {deliverable.stakeholder_name && (
                                      <span className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
                                        · {deliverable.stakeholder_name}
                                      </span>
                                    )}
                                  </div>
                                  <h3 className="text-sm font-semibold text-zinc-900 transition-colors group-hover:text-zinc-600">
                                    {deliverable.title}
                                  </h3>
                                  {deliverable.claim && (
                                    <p className="mt-1 line-clamp-2 text-xs leading-relaxed text-zinc-500">
                                      {deliverable.claim}
                                    </p>
                                  )}
                                  {deliverable.deadline && (
                                    <p className="mt-1 text-[11px] text-zinc-400">
                                      Due {deliverable.deadline}
                                    </p>
                                  )}
                                </Link>
                                <button
                                  className="mt-1 shrink-0 rounded p-1.5 text-zinc-200 opacity-0 transition-all hover:bg-red-50 hover:text-red-500 group-hover:opacity-100"
                                  onClick={() => void handleDeleteDeliverable(deliverable.id, deliverable.title)}
                                  title="Delete deliverable"
                                  type="button"
                                >
                                  <Trash2 size={13} />
                                </button>
                              </div>
                            </div>
                          ))}
                        </div>
                      </div>
                    )}
                  </div>
                )}

                {/* ── Gantt ── */}
                {tab === "gantt" && (
                  <div className="p-5">
                    <GanttView initiativeId={initiative.id} />
                  </div>
                )}

                {/* ── Email ── */}
                {tab === "email" && (
                  <div className="p-5">
                    <InitiativeEmailPanel
                      linkedThreads={linkedEmailThreads}
                      loading={emailLoading}
                      message={emailMessage}
                      onLink={(threadId) => void handleLinkEmailThread(threadId)}
                      onRefresh={() => void loadEmailContext(initiative.id, initiative.title)}
                      suggestedThreads={suggestedEmailThreads}
                    />
                  </div>
                )}

                {/* ── Files ── */}
                {tab === "files" && (
                  <div className="p-5">
                    <EntityFilesPanel entityId={initiative.id} entityKind="initiative" />
                  </div>
                )}

                {/* ── Notes ── */}
                {tab === "notes" && (
                  <div className="p-5">
                    <div className="mb-5 flex items-center justify-between">
                      <h2 className="text-sm font-semibold text-zinc-950">Notes</h2>
                      <p className="text-xs text-zinc-400">
                        Cmd+Enter to save
                      </p>
                    </div>

                    {/* Composer */}
                    <div className="mb-6 flex gap-2">
                      <textarea
                        className="field-control flex-1 resize-none text-sm"
                        onChange={(e) => setNoteBody(e.currentTarget.value)}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void handleAddNote();
                        }}
                        placeholder="Add a note…"
                        rows={2}
                        value={noteBody}
                      />
                      <button
                        className="btn btn-primary self-start"
                        disabled={!noteBody.trim()}
                        onClick={() => void handleAddNote()}
                        type="button"
                      >
                        <Check size={13} />
                        Add
                      </button>
                    </div>

                    {/* Notes list */}
                    {notes.length === 0 ? (
                      <div className="flex min-h-[12rem] items-center justify-center rounded-xl border border-dashed border-zinc-200 bg-zinc-50/50">
                        <div className="text-center">
                          <MessageSquare className="mx-auto mb-2 text-zinc-200" size={24} />
                          <p className="text-sm text-zinc-400">No notes yet.</p>
                        </div>
                      </div>
                    ) : (
                      <div className="space-y-3">
                        {notes.map((note) => (
                          <div
                            className="group flex items-start gap-3 rounded-xl border border-zinc-100 bg-zinc-50 px-4 py-4"
                            key={note.id}
                          >
                            <p className="flex-1 whitespace-pre-wrap text-sm leading-6 text-zinc-700">
                              {note.body}
                            </p>
                            <div className="flex shrink-0 flex-col items-end gap-1.5">
                              <span className="text-[10px] text-zinc-400">
                                {formatDateTime(note.created_at)}
                              </span>
                              <button
                                className="rounded p-1 text-zinc-300 opacity-0 transition-all hover:bg-red-50 hover:text-red-500 group-hover:opacity-100"
                                onClick={() => void handleDeleteNote(note.id)}
                                type="button"
                              >
                                <Trash2 size={13} />
                              </button>
                            </div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </motion.div>
            </AnimatePresence>
          </section>
        </div>
      ) : (
        <div className="flex min-h-[20rem] items-center justify-center rounded-2xl border border-dashed border-zinc-200 bg-white">
          <div className="text-center">
            <Target className="mx-auto mb-3 text-zinc-200" size={32} />
            <p className="text-sm text-zinc-400">Initiative not found.</p>
          </div>
        </div>
      )}
    </div>
  );
}

function InitiativeEmailPanel({
  linkedThreads,
  loading,
  message,
  onLink,
  onRefresh,
  suggestedThreads,
}: {
  linkedThreads: GmailLocalThread[];
  loading: boolean;
  message: string | null;
  onLink: (threadId: string) => void;
  onRefresh: () => void;
  suggestedThreads: GmailLocalThread[];
}) {
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-zinc-950">Email context</h2>
          <p className="mt-0.5 text-xs text-zinc-400">
            Linked threads and title-matched suggestions from your local Gmail index.
          </p>
        </div>
        <button className="btn" disabled={loading} onClick={onRefresh} type="button">
          <RefreshCw className={loading ? "animate-spin" : ""} size={13} />
          Refresh
        </button>
      </div>

      {message && <div className="notice notice-success text-sm">{message}</div>}

      <EmailThreadGroup
        count={linkedThreads.length}
        emptyText="No threads linked to this initiative."
        title="Linked"
      >
        {linkedThreads.map((thread) => (
          <Link
            className="group block px-4 py-3.5 transition-colors hover:bg-zinc-50"
            key={thread.thread_id}
            to={`/email?thread=${thread.thread_id}`}
          >
            <EmailThreadSummary showArrow thread={thread} />
          </Link>
        ))}
      </EmailThreadGroup>

      <EmailThreadGroup
        count={suggestedThreads.length}
        emptyText="No suggestions from the local Gmail index."
        title="Suggested"
      >
        {suggestedThreads.map((thread) => (
          <div
            className="flex items-start gap-3 px-4 py-3.5"
            key={thread.thread_id}
          >
            <div className="min-w-0 flex-1">
              <EmailThreadSummary thread={thread} />
            </div>
            <button
              className="btn btn-primary mt-0.5 shrink-0"
              onClick={() => onLink(thread.thread_id)}
              type="button"
            >
              <Link2 size={13} />
              Link
            </button>
          </div>
        ))}
      </EmailThreadGroup>
    </div>
  );
}

function EmailThreadGroup({
  children,
  count,
  emptyText,
  title,
}: {
  children: React.ReactNode;
  count?: number;
  emptyText: string;
  title: string;
}) {
  const items = Array.isArray(children) ? children.filter(Boolean) : [children].filter(Boolean);
  return (
    <div className="overflow-hidden rounded-xl border border-zinc-100">
      <div className="flex items-center gap-2 border-b border-zinc-100 bg-zinc-50 px-4 py-2.5">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
          {title}
        </span>
        {count != null && count > 0 && (
          <span className="rounded-full bg-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-500">
            {count}
          </span>
        )}
      </div>
      {items.length === 0 ? (
        <div className="flex items-center gap-2 px-4 py-5 text-sm text-zinc-400">
          <Mail className="text-zinc-200" size={16} />
          {emptyText}
        </div>
      ) : (
        <div className="divide-y divide-zinc-50">{children}</div>
      )}
    </div>
  );
}

function EmailThreadSummary({
  showArrow,
  thread,
}: {
  showArrow?: boolean;
  thread: GmailLocalThread;
}) {
  const participants = thread.participants
    .slice(0, 3)
    .map((p) => p.name || p.email)
    .filter(Boolean)
    .join(", ");
  const msgCount = thread.message_count;
  const dateStr = thread.last_message_at
    ? formatDateTime(new Date(thread.last_message_at * 1000).toISOString())
    : null;

  return (
    <div className="min-w-0">
      <div className="mb-1 flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-1 items-center gap-2">
          {thread.has_unread && (
            <span className="mt-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-sky-500" />
          )}
          <h4 className="truncate text-sm font-semibold text-zinc-950">
            {thread.subject || "(no subject)"}
          </h4>
        </div>
        {showArrow && (
          <ChevronRight className="shrink-0 text-zinc-300 transition-colors group-hover:text-zinc-400" size={14} />
        )}
      </div>
      {(thread.snippet || participants) ? (
        <p className="line-clamp-1 text-xs leading-5 text-zinc-500">
          {thread.snippet || participants}
        </p>
      ) : null}
      <div className="mt-1 flex flex-wrap items-center gap-x-1.5 text-[11px] text-zinc-400">
        {participants && <span>{participants}</span>}
        <span>·</span>
        <span>{msgCount} {msgCount === 1 ? "message" : "messages"}</span>
        {dateStr && (
          <>
            <span>·</span>
            <span>{dateStr}</span>
          </>
        )}
      </div>
    </div>
  );
}
