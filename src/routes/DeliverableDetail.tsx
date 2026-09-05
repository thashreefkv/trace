import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  ArrowLeft,
  AlertTriangle,
  ExternalLink,
  Link2,
  Mail,
  Pencil,
  RefreshCw,
  Star,
  Tag,
  Trash2,
  X,
} from "lucide-react";
import {
  assignLabelToDeliverable,
  createStakeholder,
  deleteDeliverable,
  getDeliverable,
  gmailLinkThreadToDeliverable,
  gmailListLocalThreads,
  gmailSuggestThreadsForDeliverable,
  listInitiatives,
  listLabels,
  listStakeholders,
  removeLabelFromDeliverable,
  updateDeliverable,
  updateDeliverableMetadata,
  setDeliverableFocus,
} from "../lib/ipc";
import type {
  CreateDeliverableInput,
  Deliverable,
  DeliverablePriority,
  GmailLocalThread,
  Initiative,
  Label,
  Stakeholder,
  UpdateDeliverableMetadataInput,
} from "../lib/types";
import { labelColors, priorityColors, priorityLabels } from "../lib/types";
import { formatDateTime } from "../lib/format";
import { safeExternalUrl } from "../lib/urlSafety";
import { DeliverableForm } from "../components/DeliverableForm";
import { StatePill } from "../components/StatePill";
import { DeliverableTypeBadge } from "../components/DeliverableTypeBadge";
import { DeliverableTasks } from "../components/DeliverableTasks";
import { DeliverableNotes } from "../components/DeliverableNotes";
import { EntityFilesPanel } from "../components/files/EntityFilesPanel";

type Tab = "overview" | "tasks" | "notes" | "email" | "files";

export function DeliverableDetail() {
  const { deliverableId } = useParams();
  const navigate = useNavigate();
  const [deliverable, setDeliverable] = useState<Deliverable | null>(null);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [isEditing, setIsEditing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("overview");

  // Metadata inline edit state
  const [metaEdit, setMetaEdit] = useState(false);
  const [deadline, setDeadline] = useState("");
  const [effort, setEffort] = useState<string>("");
  const [impact, setImpact] = useState<string>("");
  const [blockerReason, setBlockerReason] = useState("");
  const [priority, setPriority] = useState<DeliverablePriority | "">("");
  const [isSavingMeta, setIsSavingMeta] = useState(false);

  // Labels state
  const [allLabels, setAllLabels] = useState<Label[]>([]);
  const [labelsUpdating, setLabelsUpdating] = useState(false);
  const [linkedEmailThreads, setLinkedEmailThreads] = useState<GmailLocalThread[]>([]);
  const [suggestedEmailThreads, setSuggestedEmailThreads] = useState<GmailLocalThread[]>([]);
  const [emailLoading, setEmailLoading] = useState(false);
  const [emailMessage, setEmailMessage] = useState<string | null>(null);

  useEffect(() => {
    if (!deliverableId) return;
    void loadDeliverable(deliverableId);
    void loadEmailContext(deliverableId);
    void listLabels().then(setAllLabels).catch(() => {});
  }, [deliverableId]);

  const initialFormValue = useMemo<CreateDeliverableInput | undefined>(() => {
    if (!deliverable) return undefined;
    return {
      title: deliverable.title,
      type: deliverable.type,
      state: deliverable.state,
      claim: deliverable.claim,
      artifact_url: deliverable.artifact_url,
      conversation_id: deliverable.conversation_id,
      stakeholder_id: deliverable.stakeholder_id,
      stakeholder_ids: deliverable.stakeholders.map((stakeholder) => stakeholder.id),
      initiative_ids: deliverable.initiatives.map((i) => i.id),
    };
  }, [deliverable]);

  async function loadDeliverable(id: string) {
    try {
      setError(null);
      setIsLoading(true);
      const [nextDeliverable, nextInitiatives, nextStakeholders] = await Promise.all([
        getDeliverable(id),
        listInitiatives(),
        listStakeholders(),
      ]);
      setDeliverable(nextDeliverable);
      setInitiatives(nextInitiatives);
      setStakeholders(nextStakeholders);
      syncMetaState(nextDeliverable);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function loadEmailContext(id: string) {
    try {
      setEmailLoading(true);
      const [linked, suggested] = await Promise.all([
        gmailListLocalThreads({ deliverable_id: id, limit: 24 }).catch(() => []),
        gmailSuggestThreadsForDeliverable(id, 12).catch(() => []),
      ]);
      const linkedIds = new Set(linked.map((thread) => thread.thread_id));
      setLinkedEmailThreads(linked);
      setSuggestedEmailThreads(
        suggested.filter((thread) => !linkedIds.has(thread.thread_id)),
      );
    } finally {
      setEmailLoading(false);
    }
  }

  function syncMetaState(d: Deliverable) {
    setDeadline(d.deadline ?? "");
    setEffort(d.effort != null ? String(d.effort) : "");
    setImpact(d.impact != null ? String(d.impact) : "");
    setBlockerReason(d.blocker_reason ?? "");
    setPriority(d.priority ?? "");
  }

  async function handleUpdate(input: CreateDeliverableInput) {
    if (!deliverable) return;
    try {
      setError(null);
      setIsSaving(true);
      const updated = await updateDeliverable(deliverable.id, input);
      setDeliverable(updated);
      setIsEditing(false);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSaving(false);
    }
  }

  async function handleSaveMeta() {
    if (!deliverable) return;
    const input: UpdateDeliverableMetadataInput = {
      deadline: deadline || null,
      effort: effort ? Number(effort) : null,
      impact: impact ? Number(impact) : null,
      blocker_reason: blockerReason || null,
      priority: (priority as DeliverablePriority) || null,
    };
    try {
      setIsSavingMeta(true);
      const updated = await updateDeliverableMetadata(deliverable.id, input);
      setDeliverable(updated);
      setMetaEdit(false);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsSavingMeta(false);
    }
  }

  async function handleToggleFocus() {
    if (!deliverable) return;
    try {
      const updated = await setDeliverableFocus(deliverable.id, !deliverable.is_focused);
      setDeliverable(updated);
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleCreateStakeholder(name: string) {
    const created = await createStakeholder({ name });
    setStakeholders((current) => [...current, created]);
    return created;
  }

  async function handleToggleLabel(labelId: string) {
    if (!deliverable) return;
    const assigned = deliverable.labels.some((l) => l.id === labelId);
    try {
      setLabelsUpdating(true);
      if (assigned) {
        await removeLabelFromDeliverable(deliverable.id, labelId);
      } else {
        await assignLabelToDeliverable(deliverable.id, labelId);
      }
      const updated = await getDeliverable(deliverable.id);
      setDeliverable(updated);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setLabelsUpdating(false);
    }
  }

  async function handleLinkEmailThread(threadId: string) {
    if (!deliverable) return;
    try {
      setEmailMessage(null);
      await gmailLinkThreadToDeliverable(threadId, deliverable.id);
      await loadEmailContext(deliverable.id);
      setEmailMessage("Email thread linked.");
    } catch (caught) {
      setError(String(caught));
    }
  }

  async function handleDelete() {
    if (!deliverable) return;
    const confirmed = window.confirm(`Delete "${deliverable.title}"?`);
    if (!confirmed) return;
    try {
      setError(null);
      await deleteDeliverable(deliverable.id);
      navigate("/deliverables");
    } catch (caught) {
      setError(String(caught));
    }
  }

  return (
    <div className="mx-auto min-h-full max-w-4xl px-5 py-6">
      <Link
        className="mb-5 inline-flex items-center gap-2 text-sm font-medium text-neutral-600 hover:text-neutral-950 dark:text-neutral-400 dark:hover:text-neutral-100"
        to="/deliverables"
      >
        <ArrowLeft aria-hidden="true" size={16} />
        Deliverables
      </Link>

      {error ? <div className="mb-4 notice notice-error">{error}</div> : null}

      {isLoading ? (
        <p className="text-sm text-zinc-500 dark:text-neutral-400">Loading deliverable…</p>
      ) : deliverable ? (
        <article className="space-y-6">
          {/* Header */}
          <header className="border-b border-zinc-100 pb-5 dark:border-zinc-700">
            <div className="mb-3 flex flex-wrap items-center gap-2">
              <DeliverableTypeBadge type={deliverable.type} />
              <StatePill kind="deliverable" state={deliverable.state} />
              {deliverable.blocker_reason && (
                <span className="inline-flex items-center gap-1 rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-semibold text-amber-700 dark:bg-amber-900/30 dark:text-amber-400">
                  <AlertTriangle size={11} />
                  Blocked
                </span>
              )}
              <span className="font-mono text-xs text-zinc-500">
                Updated {formatDateTime(deliverable.updated_at)}
              </span>
            </div>

            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="min-w-0">
                <h1 className="break-words text-3xl font-semibold tracking-normal text-neutral-950 dark:text-neutral-50">
                  {deliverable.title}
                </h1>
                <p className="mt-2 text-sm text-zinc-500 dark:text-neutral-400">
                  {deliverable.stakeholder_name ?? "No stakeholder"} · Created{" "}
                  {formatDateTime(deliverable.created_at)}
                </p>
              </div>

              <div className="flex flex-wrap gap-2">
                <button
                  className={[
                    "btn",
                    deliverable.is_focused
                      ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
                      : "",
                  ].join(" ")}
                  onClick={() => void handleToggleFocus()}
                  title={deliverable.is_focused ? "Remove focus" : "Set as focus"}
                  type="button"
                >
                  <Star
                    aria-hidden="true"
                    className={deliverable.is_focused ? "fill-amber-500 text-amber-500" : ""}
                    size={16}
                  />
                  {deliverable.is_focused ? "Focused" : "Focus"}
                </button>
                {safeExternalUrl(deliverable.artifact_url) ? (
                  <a className="btn" href={safeExternalUrl(deliverable.artifact_url)!} rel="noopener noreferrer" target="_blank">
                    <ExternalLink aria-hidden="true" size={16} />
                    Artifact
                  </a>
                ) : null}
                {safeExternalUrl(deliverable.conversation_url) ? (
                  <a className="btn" href={safeExternalUrl(deliverable.conversation_url)!} rel="noopener noreferrer" target="_blank">
                    <ExternalLink aria-hidden="true" size={16} />
                    Claude
                  </a>
                ) : null}
                <button className="btn" onClick={() => setIsEditing(true)} type="button">
                  <Pencil aria-hidden="true" size={16} />
                  Edit
                </button>
                <button className="btn btn-danger" onClick={() => void handleDelete()} type="button">
                  <Trash2 aria-hidden="true" size={16} />
                  Delete
                </button>
              </div>
            </div>

            {/* Metadata row */}
            <div className="mt-4">
              {metaEdit ? (
                <div className="flex flex-wrap items-end gap-3">
                  <label className="space-y-1">
                    <span className="field-label">Deadline</span>
                    <input
                      className="field-control w-36"
                      onChange={(e) => setDeadline(e.currentTarget.value)}
                      type="date"
                      value={deadline}
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="field-label">Effort (1–5)</span>
                    <input
                      className="field-control w-20"
                      max={5}
                      min={1}
                      onChange={(e) => setEffort(e.currentTarget.value)}
                      type="number"
                      value={effort}
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="field-label">Impact (1–5)</span>
                    <input
                      className="field-control w-20"
                      max={5}
                      min={1}
                      onChange={(e) => setImpact(e.currentTarget.value)}
                      type="number"
                      value={impact}
                    />
                  </label>
                  <label className="space-y-1">
                    <span className="field-label">Priority</span>
                    <select
                      className="field-control w-24"
                      onChange={(e) => setPriority(e.currentTarget.value as DeliverablePriority | "")}
                      value={priority}
                    >
                      <option value="">None</option>
                      <option value="p1">P1</option>
                      <option value="p2">P2</option>
                      <option value="p3">P3</option>
                    </select>
                  </label>
                  <label className="flex-1 space-y-1">
                    <span className="field-label">Blocker reason</span>
                    <input
                      className="field-control"
                      onChange={(e) => setBlockerReason(e.currentTarget.value)}
                      placeholder="Leave empty if not blocked"
                      type="text"
                      value={blockerReason}
                    />
                  </label>
                  <button
                    className="btn btn-primary"
                    disabled={isSavingMeta}
                    onClick={() => void handleSaveMeta()}
                    type="button"
                  >
                    Save
                  </button>
                  <button
                    className="btn"
                    onClick={() => { syncMetaState(deliverable); setMetaEdit(false); }}
                    type="button"
                  >
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="flex flex-wrap items-center gap-4">
                  {deliverable.priority && (
                    <span className={`rounded px-1.5 py-0.5 text-xs font-bold ${priorityColors[deliverable.priority]}`}>
                      {priorityLabels[deliverable.priority]}
                    </span>
                  )}
                  {deliverable.deadline && (
                    <MetaChip label="Deadline" value={deliverable.deadline} />
                  )}
                  {deliverable.effort != null && (
                    <MetaChip label="Effort" value={`${deliverable.effort}/5`} />
                  )}
                  {deliverable.impact != null && (
                    <MetaChip label="Impact" value={`${deliverable.impact}/5`} />
                  )}
                  {deliverable.blocker_reason && (
                    <MetaChip label="Blocked" value={deliverable.blocker_reason} warn />
                  )}
                  <button
                    className="text-xs text-neutral-400 hover:text-neutral-700 dark:hover:text-neutral-300"
                    onClick={() => setMetaEdit(true)}
                    type="button"
                  >
                    {deliverable.deadline || deliverable.effort != null || deliverable.blocker_reason || deliverable.priority
                      ? "Edit metadata"
                      : "+ Add deadline, effort, priority…"}
                  </button>
                </div>
              )}
            </div>
          </header>

          {/* Edit form */}
          {isEditing && initialFormValue ? (
            <section className="rounded-md border border-zinc-100 bg-white p-5 dark:border-zinc-700 dark:bg-zinc-900">
              <h2 className="mb-4 text-sm font-semibold">Edit deliverable</h2>
              <DeliverableForm
                initialValue={initialFormValue}
                initiatives={initiatives}
                isSubmitting={isSaving}
                onCancel={() => setIsEditing(false)}
                onCreateStakeholder={handleCreateStakeholder}
                onSubmit={handleUpdate}
                stakeholders={stakeholders}
                submitLabel="Save changes"
              />
            </section>
          ) : (
            <>
              {/* Tabs */}
              <div className="border-b border-zinc-100 dark:border-zinc-700">
                <nav className="-mb-px flex gap-6">
                  {(["overview", "tasks", "notes", "email", "files"] as Tab[]).map((tab) => (
                    <button
                      className={[
                        "border-b-2 pb-3 text-sm font-medium transition-colors",
                        activeTab === tab
                          ? "border-neutral-900 text-neutral-900 dark:border-neutral-100 dark:text-neutral-100"
                          : "border-transparent text-zinc-500 hover:text-neutral-700 dark:text-neutral-400 dark:hover:text-neutral-300",
                      ].join(" ")}
                      key={tab}
                      onClick={() => setActiveTab(tab)}
                      type="button"
                    >
                      {tab.charAt(0).toUpperCase() + tab.slice(1)}
                    </button>
                  ))}
                </nav>
              </div>

              {/* Tab content */}
              {activeTab === "overview" && (
                <div className="space-y-6">
                  <section>
                    <h2 className="mb-2 text-sm font-semibold text-neutral-950 dark:text-neutral-50">
                      Claim
                    </h2>
                    <p className="whitespace-pre-wrap text-base leading-7 text-neutral-700 dark:text-neutral-300">
                      {deliverable.claim}
                    </p>
                  </section>

                  {/* Labels */}
                  <section>
                    <h2 className="mb-2 flex items-center gap-1.5 text-sm font-semibold text-neutral-950 dark:text-neutral-50">
                      <Tag size={13} />
                      Labels
                    </h2>
                    <div className="flex flex-wrap gap-2">
                      {/* Assigned labels */}
                      {deliverable.labels.map((lbl) => {
                        const lc = labelColors[lbl.color] ?? labelColors["zinc"];
                        return (
                          <button
                            key={lbl.id}
                            className={`inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium ${lc.bg} ${lc.text} opacity-100 hover:opacity-80`}
                            disabled={labelsUpdating}
                            onClick={() => void handleToggleLabel(lbl.id)}
                            title="Remove label"
                            type="button"
                          >
                            {lbl.name}
                            <X size={10} />
                          </button>
                        );
                      })}
                      {/* Available but unassigned labels */}
                      {allLabels
                        .filter((l) => !deliverable.labels.some((al) => al.id === l.id))
                        .map((lbl) => {
                          const lc = labelColors[lbl.color] ?? labelColors["zinc"];
                          return (
                            <button
                              key={lbl.id}
                              className={`inline-flex items-center gap-1 rounded-full border border-dashed px-2.5 py-0.5 text-xs font-medium opacity-50 hover:opacity-100 ${lc.text}`}
                              disabled={labelsUpdating}
                              onClick={() => void handleToggleLabel(lbl.id)}
                              title="Add label"
                              type="button"
                            >
                              + {lbl.name}
                            </button>
                          );
                        })}
                      {allLabels.length === 0 && deliverable.labels.length === 0 && (
                        <p className="text-xs text-neutral-400">No labels — create them in Settings.</p>
                      )}
                    </div>
                  </section>

                  <section>
                    <h2 className="mb-2 text-sm font-semibold text-neutral-950 dark:text-neutral-50">
                      Initiatives
                    </h2>
                    <div className="flex flex-wrap gap-2">
                      {deliverable.initiatives.map((initiative) => (
                        <Link
                          className="rounded-md bg-accent-50 px-2 py-1 text-sm text-accent-700 hover:bg-accent-100 dark:bg-accent-700/20 dark:text-accent-100"
                          key={initiative.id}
                          to={`/initiatives/${initiative.id}`}
                        >
                          {initiative.title}
                        </Link>
                      ))}
                    </div>
                  </section>
                </div>
              )}

              {activeTab === "tasks" && (
                <DeliverableTasks deliverableId={deliverable.id} />
              )}

              {activeTab === "notes" && (
                <DeliverableNotes deliverableId={deliverable.id} />
              )}

              {activeTab === "email" && (
                <EmailThreadPanel
                  linkedThreads={linkedEmailThreads}
                  loading={emailLoading}
                  message={emailMessage}
                  onLink={(threadId) => void handleLinkEmailThread(threadId)}
                  onRefresh={() => void loadEmailContext(deliverable.id)}
                  suggestedThreads={suggestedEmailThreads}
                />
              )}

              {activeTab === "files" && (
                <EntityFilesPanel entityKind="deliverable" entityId={deliverable.id} />
              )}
            </>
          )}
        </article>
      ) : (
        <p className="text-sm text-zinc-500 dark:text-neutral-400">Deliverable not found.</p>
      )}
    </div>
  );
}

function EmailThreadPanel({
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
    <section className="space-y-5">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-neutral-950 dark:text-neutral-50">
            Email context
          </h2>
          <p className="mt-1 text-xs text-zinc-500 dark:text-neutral-400">
            Gmail threads linked to this deliverable plus local suggestions by title, artifact URL, and participants.
          </p>
        </div>
        <button className="btn" disabled={loading} onClick={onRefresh} type="button">
          <RefreshCw aria-hidden="true" className={loading ? "animate-spin" : ""} size={16} />
          Refresh
        </button>
      </div>

      {message ? <div className="notice notice-success">{message}</div> : null}

      <div className="rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
        <div className="border-b border-zinc-100 px-4 py-3 dark:border-zinc-700">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Linked threads
          </h3>
        </div>
        {linkedThreads.length === 0 ? (
          <EmptyEmailState text="No email threads linked yet." />
        ) : (
          <div className="divide-y divide-neutral-200 dark:divide-neutral-800">
            {linkedThreads.map((thread) => (
              <EmailThreadRow key={thread.thread_id} thread={thread} />
            ))}
          </div>
        )}
      </div>

      <div className="rounded-xl border border-zinc-100 bg-white dark:border-zinc-700 dark:bg-zinc-900">
        <div className="border-b border-zinc-100 px-4 py-3 dark:border-zinc-700">
          <h3 className="text-xs font-semibold uppercase tracking-wide text-zinc-500">
            Suggested from Gmail
          </h3>
        </div>
        {suggestedThreads.length === 0 ? (
          <EmptyEmailState text="No suggested threads from the local Gmail index." />
        ) : (
          <div className="divide-y divide-neutral-200 dark:divide-neutral-800">
            {suggestedThreads.map((thread) => (
              <div className="flex flex-wrap items-start justify-between gap-3 px-4 py-3" key={thread.thread_id}>
                <div className="min-w-0 flex-1">
                  <EmailThreadSummary thread={thread} />
                </div>
                <button className="btn btn-primary" onClick={() => onLink(thread.thread_id)} type="button">
                  <Link2 aria-hidden="true" size={16} />
                  Link
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function EmailThreadRow({ thread }: { thread: GmailLocalThread }) {
  return (
    <Link
      className="block px-4 py-3 transition-colors hover:bg-neutral-50 dark:hover:bg-neutral-800/60"
      to={`/email?thread=${thread.thread_id}`}
    >
      <EmailThreadSummary thread={thread} />
    </Link>
  );
}

function EmailThreadSummary({ thread }: { thread: GmailLocalThread }) {
  return (
    <div className="min-w-0">
      <div className="mb-1 flex flex-wrap items-center gap-2">
        <h4 className="min-w-0 truncate text-sm font-semibold text-neutral-950 dark:text-neutral-50">
          {thread.subject || "(no subject)"}
        </h4>
        {thread.has_unread ? (
          <span className="rounded-full bg-blue-50 px-2 py-0.5 text-[11px] font-semibold text-blue-700">
            Unread
          </span>
        ) : null}
        {thread.artifact_urls.length > 0 ? (
          <span className="rounded-full bg-emerald-50 px-2 py-0.5 text-[11px] font-semibold text-emerald-700">
            Artifact
          </span>
        ) : null}
      </div>
      <p className="line-clamp-2 text-sm leading-6 text-neutral-600 dark:text-neutral-300">
        {thread.snippet || participantLine(thread)}
      </p>
      <p className="mt-1 text-xs text-neutral-400">
        {participantLine(thread)} · {thread.message_count} messages · {formatThreadDate(thread.last_message_at)}
      </p>
    </div>
  );
}

function EmptyEmailState({ text }: { text: string }) {
  return (
    <div className="flex items-center gap-3 px-4 py-5 text-sm text-zinc-500">
      <Mail aria-hidden="true" className="text-neutral-300" size={18} />
      {text}
    </div>
  );
}

function participantLine(thread: GmailLocalThread) {
  const participants = thread.participants
    .slice(0, 4)
    .map((participant) => participant.name || participant.email)
    .filter(Boolean);
  return participants.length ? participants.join(", ") : "No participants";
}

function formatThreadDate(value: number | null) {
  if (!value) {
    return "No date";
  }
  return formatDateTime(new Date(value * 1000).toISOString());
}

function MetaChip({
  label,
  value,
  warn,
}: {
  label: string;
  value: string;
  warn?: boolean;
}) {
  return (
    <span
      className={[
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium",
        warn
          ? "bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400"
          : "bg-neutral-100 text-neutral-600 dark:bg-neutral-800 dark:text-neutral-400",
      ].join(" ")}
    >
      <span className="font-normal opacity-70">{label}</span>
      {value}
    </span>
  );
}
