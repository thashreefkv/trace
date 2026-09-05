import { useCallback, useEffect, useState } from "react";
import { Pencil, RotateCcw, Save } from "lucide-react";
import {
  gmailClearThreadOverride,
  gmailCreateSenderRule,
  gmailGetEffectiveClassification,
  gmailGetThreadOverride,
  gmailSetThreadOverride,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type {
  EffectiveClassification,
  UserClassification,
  WorkMailAttentionState,
  WorkMailMessageType,
  WorkMailRelevance,
} from "../lib/types";

const CATEGORY_VALUES = [
  "work",
  "personal",
  "newsletter",
  "receipt",
  "meeting",
  "action_required",
  "archive",
  "spam",
  "other",
];
const PRIORITY_VALUES = ["urgent", "high", "medium", "low"];
const INTENT_VALUES = [
  "asking",
  "informing",
  "requesting_decision",
  "scheduling",
  "acknowledging",
  "venting",
  "other",
];
const THREAD_STATE_VALUES = [
  "waiting_on_you",
  "waiting_on_them",
  "resolved",
  "dormant",
];
const WORK_RELEVANCE_VALUES = [
  "work",
  "linked_external",
  "promoted",
  "excluded",
  "non_work",
  "unknown",
];
const ATTENTION_STATE_VALUES = [
  "needs_me",
  "waiting",
  "review",
  "fyi",
  "scheduled",
  "resolved",
];
const MESSAGE_TYPE_VALUES = [
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
];

interface Props {
  threadId: string;
  senderEmail?: string | null;
  /** LLM-set reasons in JSON (from gmail_threads.ai_category_reasons). */
  reasons?: string[] | null;
  /** New multi-dimensional classification fields, shown read-only. */
  intent?: string | null;
  actionRequired?: boolean;
  predictedAction?: string | null;
  threadState?: string | null;
  dimensionsConfidence?: Record<string, number> | null;
  bundleSize?: number;
  onChange?: () => void;
}

function humanize(value: string): string {
  return value
    .replace(/_/g, " ")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function ThreadClassificationEditor({
  threadId,
  senderEmail,
  reasons,
  intent,
  actionRequired,
  predictedAction,
  threadState,
  bundleSize,
  onChange,
}: Props) {
  const [effective, setEffective] = useState<EffectiveClassification | null>(
    null,
  );
  const [override, setOverride] = useState<UserClassification | null>(null);
  const [editing, setEditing] = useState(false);
  const [draftCategory, setDraftCategory] = useState<string>("");
  const [draftPriority, setDraftPriority] = useState<string>("");
  const [draftIntent, setDraftIntent] = useState<string>("");
  const [draftActionRequired, setDraftActionRequired] = useState<boolean>(false);
  const [draftThreadState, setDraftThreadState] = useState<string>("");
  const [draftWorkRelevance, setDraftWorkRelevance] = useState<WorkMailRelevance | "">("");
  const [draftAttentionState, setDraftAttentionState] = useState<WorkMailAttentionState | "">("");
  const [draftMessageType, setDraftMessageType] = useState<WorkMailMessageType | "">("");
  const [draftNote, setDraftNote] = useState("");
  const [saveAsRule, setSaveAsRule] = useState(false);
  const [saving, setSaving] = useState(false);

  const load = useCallback(async () => {
    try {
      const [eff, ovr] = await Promise.all([
        gmailGetEffectiveClassification(threadId),
        gmailGetThreadOverride(threadId),
      ]);
      setEffective(eff);
      setOverride(ovr);
      setDraftCategory(eff.category);
      setDraftPriority(eff.priority);
      setDraftIntent(eff.intent ?? "");
      setDraftActionRequired(eff.action_required);
      setDraftThreadState(eff.thread_state ?? "");
      setDraftWorkRelevance(eff.work_relevance);
      setDraftAttentionState(eff.attention_state);
      setDraftMessageType(eff.message_type);
      setDraftNote(ovr?.note ?? "");
    } catch {
      // toasted
    }
  }, [threadId]);

  useEffect(() => {
    void load();
  }, [load]);

  const handleSave = async () => {
    setSaving(true);
    try {
      await gmailSetThreadOverride({
        thread_id: threadId,
        category: draftCategory || null,
        priority: draftPriority || null,
        intent: draftIntent || null,
        action_required: draftActionRequired,
        thread_state: draftThreadState || null,
        work_relevance: draftWorkRelevance || null,
        attention_state: draftAttentionState || null,
        message_type: draftMessageType || null,
        note: draftNote.trim() || null,
      });
      if (saveAsRule && senderEmail) {
        const domain = senderEmail.split("@")[1];
        try {
          await gmailCreateSenderRule({
            pattern: domain ? `*@${domain}` : senderEmail,
            pattern_kind: domain ? "glob" : "exact",
            category: draftCategory || null,
            priority: draftPriority || null,
            work_relevance: draftWorkRelevance || null,
            attention_state: draftAttentionState || null,
            message_type: draftMessageType || null,
            note: `Auto-created from thread override`,
          });
          toast.success("Override saved · sender rule created");
        } catch {
          // toasted
        }
      } else {
        toast.success("Override saved");
      }
      setEditing(false);
      setSaveAsRule(false);
      await load();
      onChange?.();
    } catch {
      // toasted
    } finally {
      setSaving(false);
    }
  };

  const handleClear = async () => {
    if (!confirm("Remove this thread's override?")) return;
    try {
      await gmailClearThreadOverride(threadId);
      await load();
      onChange?.();
      toast.success("Override removed");
    } catch {
      // toasted
    }
  };

  if (!effective) {
    return <div className="h-10 animate-pulse rounded-xl bg-zinc-100" />;
  }

  const priorityValue =
    effective.recency_adjusted_priority !== effective.priority
      ? `${effective.recency_adjusted_priority} (was ${effective.priority})`
      : effective.priority;

  return (
    <div className="space-y-3">
      {!editing && (
        <div className="flex items-start justify-between gap-3">
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-4 gap-y-1.5 text-[11px]">
            <KeyValue label="Category" value={humanize(effective.category)} />
            <KeyValue
              label="Priority"
              value={priorityValue}
              tone={priorityTextTone(effective.recency_adjusted_priority)}
            />
            <KeyValue label="Work scope" value={humanize(effective.work_relevance)} />
            <KeyValue label="Attention" value={humanize(effective.attention_state)} />
            <KeyValue label="Type" value={humanize(effective.message_type)} />
            {intent && (
              <KeyValue label="Intent" value={humanize(intent)} />
            )}
            {threadState && (
              <KeyValue
                label="State"
                value={humanize(threadState)}
                tone={threadStateTextTone(threadState)}
              />
            )}
            {actionRequired && (
              <span className="font-semibold text-amber-700">
                Action required
                {predictedAction ? ` · ${humanize(predictedAction)}` : ""}
              </span>
            )}
            {bundleSize !== undefined && bundleSize > 1 && (
              <span className="text-zinc-500">
                {bundleSize} in conversation
              </span>
            )}
            {effective.source !== "llm" && (
              <span
                className={`rounded-md px-1.5 py-0.5 text-[10px] font-semibold ${
                  effective.source === "override"
                    ? "bg-amber-50 text-amber-700"
                    : "bg-sky-50 text-sky-700"
                }`}
              >
                {effective.source === "override" ? "Your override" : "From rule"}
              </span>
            )}
          </div>
          <div className="flex shrink-0 items-center gap-1">
            {override && (
              <button
                aria-label="Clear override"
                className="rounded-md p-1.5 text-zinc-400 hover:bg-zinc-50 hover:text-rose-600"
                onClick={() => void handleClear()}
                title="Clear override"
                type="button"
              >
                <RotateCcw size={12} />
              </button>
            )}
            <button
              className="flex items-center gap-1 rounded-md px-2 py-1 text-[11px] font-medium text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
              onClick={() => setEditing(true)}
              type="button"
            >
              <Pencil size={12} />
              Edit Trace
            </button>
          </div>
        </div>
      )}

      {effective.override_note && !editing && (
        <p className="text-[11px] italic text-zinc-500">
          “{effective.override_note}”
        </p>
      )}
      {effective.recency_decay_note && !editing && (
        <p className="text-[10px] text-zinc-400">{effective.recency_decay_note}</p>
      )}

      {!editing && reasons && reasons.length > 0 && (
        <details className="group">
          <summary className="cursor-pointer text-[11px] text-zinc-500 hover:text-zinc-900">
            Why this classification?
          </summary>
          <ul className="mt-2 space-y-1 pl-3 text-[11px] text-zinc-600">
            {reasons.map((reason, i) => (
              <li className="list-disc" key={i}>
                {reason}
              </li>
            ))}
          </ul>
        </details>
      )}

      {editing && (
        <div className="space-y-2 rounded-xl border border-zinc-100 bg-zinc-50 p-4 text-[12px]">
          <div className="grid grid-cols-2 gap-2">
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Category
              </span>
              <select
                aria-label="Category"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftCategory(e.target.value)}
                value={draftCategory}
              >
                {CATEGORY_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Priority
              </span>
              <select
                aria-label="Priority"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftPriority(e.target.value)}
                value={draftPriority}
              >
                {PRIORITY_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {v}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Intent
              </span>
              <select
                aria-label="Intent"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftIntent(e.target.value)}
                value={draftIntent}
              >
                <option value="">(no override)</option>
                {INTENT_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {humanize(v)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Thread state
              </span>
              <select
                aria-label="Thread state"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftThreadState(e.target.value)}
                value={draftThreadState}
              >
                <option value="">(no override)</option>
                {THREAD_STATE_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {humanize(v)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Work scope
              </span>
              <select
                aria-label="Work scope"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftWorkRelevance(e.target.value as WorkMailRelevance)}
                value={draftWorkRelevance}
              >
                {WORK_RELEVANCE_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {humanize(v)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Attention
              </span>
              <select
                aria-label="Attention"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftAttentionState(e.target.value as WorkMailAttentionState)}
                value={draftAttentionState}
              >
                {ATTENTION_STATE_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {humanize(v)}
                  </option>
                ))}
              </select>
            </label>
            <label className="space-y-1">
              <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Message type
              </span>
              <select
                aria-label="Message type"
                className="w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5"
                onChange={(e) => setDraftMessageType(e.target.value as WorkMailMessageType)}
                value={draftMessageType}
              >
                {MESSAGE_TYPE_VALUES.map((v) => (
                  <option key={v} value={v}>
                    {humanize(v)}
                  </option>
                ))}
              </select>
            </label>
          </div>
          <label className="flex items-center gap-2 text-[11px] text-zinc-700">
            <input
              checked={draftActionRequired}
              onChange={(e) => setDraftActionRequired(e.target.checked)}
              type="checkbox"
            />
            Action required from me
          </label>
          <input
            aria-label="Override note"
            className="w-full rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5"
            onChange={(e) => setDraftNote(e.target.value)}
            placeholder="Why? (optional, helps the brain learn)"
            value={draftNote}
          />
          {senderEmail && (
            <label className="flex items-center gap-2 text-[11px] text-zinc-600">
              <input
                checked={saveAsRule}
                onChange={(e) => setSaveAsRule(e.target.checked)}
                type="checkbox"
              />
              Also create a sender rule for{" "}
              <span className="font-mono">
                {senderEmail.includes("@")
                  ? `*@${senderEmail.split("@")[1]}`
                  : senderEmail}
              </span>
            </label>
          )}
          <div className="flex justify-end gap-2">
            <button
              className="btn"
              onClick={() => {
                setEditing(false);
                setSaveAsRule(false);
              }}
              type="button"
            >
              Cancel
            </button>
            <button
              className="btn btn-primary"
              disabled={saving}
              onClick={() => void handleSave()}
              type="button"
            >
              <Save size={12} />
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function KeyValue({
  label,
  tone,
  value,
}: {
  label: string;
  tone?: string;
  value: string;
}) {
  return (
    <span className="inline-flex items-baseline gap-1.5">
      <span className="text-[9px] font-bold uppercase tracking-wider text-zinc-400">
        {label}
      </span>
      <span className={`font-medium ${tone || "text-zinc-700"}`}>{value}</span>
    </span>
  );
}

function priorityTextTone(level: string): string {
  const lower = level?.toLowerCase() ?? "";
  if (lower === "urgent" || lower === "high") return "text-rose-600";
  if (lower === "low") return "text-zinc-500";
  return "text-zinc-700";
}

function threadStateTextTone(state: string): string {
  if (state === "waiting_on_you") return "text-rose-700";
  if (state === "waiting_on_them") return "text-sky-700";
  if (state === "resolved") return "text-emerald-700";
  return "text-zinc-500";
}
