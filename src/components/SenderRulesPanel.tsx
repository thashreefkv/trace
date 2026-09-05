import { useCallback, useEffect, useState } from "react";
import {
  ChevronDown,
  Filter,
  Plus,
  Power,
  PowerOff,
  Trash2,
} from "lucide-react";
import {
  gmailCalibrationReport,
  gmailCreateSenderRule,
  gmailDeleteSenderRule,
  gmailListSenderRules,
  gmailToggleSenderRule,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type {
  CalibrationReport,
  CreateSenderRuleInput,
  SenderRule,
  WorkMailAttentionState,
  WorkMailMessageType,
  WorkMailRelevance,
} from "../lib/types";

const CATEGORY_OPTIONS = [
  { value: "", label: "(no change)" },
  { value: "work", label: "Work" },
  { value: "personal", label: "Personal" },
  { value: "newsletter", label: "Newsletter" },
  { value: "receipt", label: "Receipt" },
  { value: "meeting", label: "Meeting" },
  { value: "action_required", label: "Action required" },
  { value: "archive", label: "Archive" },
  { value: "spam", label: "Spam" },
  { value: "other", label: "Other" },
];

const PRIORITY_OPTIONS = [
  { value: "", label: "(no change)" },
  { value: "urgent", label: "Urgent" },
  { value: "high", label: "High" },
  { value: "medium", label: "Medium" },
  { value: "low", label: "Low" },
];

const WORK_RELEVANCE_OPTIONS = [
  { value: "", label: "(no change)" },
  { value: "work", label: "Work" },
  { value: "linked_external", label: "Linked external" },
  { value: "promoted", label: "Promoted" },
  { value: "excluded", label: "Excluded" },
  { value: "non_work", label: "Non-work" },
  { value: "unknown", label: "Unknown" },
];

const ATTENTION_OPTIONS = [
  { value: "", label: "(no change)" },
  { value: "needs_me", label: "Needs me" },
  { value: "waiting", label: "Waiting" },
  { value: "review", label: "Review" },
  { value: "fyi", label: "FYI" },
  { value: "scheduled", label: "Scheduled" },
  { value: "resolved", label: "Resolved" },
];

const MESSAGE_TYPE_OPTIONS = [
  { value: "", label: "(no change)" },
  { value: "conversation", label: "Conversation" },
  { value: "file_share", label: "File share" },
  { value: "meeting", label: "Meeting" },
  { value: "announcement", label: "Announcement" },
  { value: "notification", label: "Notification" },
  { value: "newsletter", label: "Newsletter" },
  { value: "promotion", label: "Promotion" },
  { value: "receipt", label: "Receipt" },
  { value: "system", label: "System" },
  { value: "other", label: "Other" },
];

const PATTERN_KIND_OPTIONS = [
  {
    value: "glob",
    label: "Glob",
    hint: "wildcards: *@example.com, notifications@*",
  },
  { value: "domain", label: "Domain", hint: "matches @domain only" },
  { value: "exact", label: "Exact", hint: "full email address" },
];

export function SenderRulesPanel() {
  const [rules, setRules] = useState<SenderRule[]>([]);
  const [calibration, setCalibration] = useState<CalibrationReport | null>(null);
  const [loading, setLoading] = useState(false);
  const [creating, setCreating] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [rs, cal] = await Promise.all([
        gmailListSenderRules(),
        gmailCalibrationReport().catch(() => null),
      ]);
      setRules(rs);
      setCalibration(cal);
    } catch {
      // ipc toasts
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleDelete = async (id: string) => {
    if (!confirm("Delete this rule?")) return;
    try {
      await gmailDeleteSenderRule(id);
      await load();
    } catch {
      // toasted
    }
  };

  const handleToggle = async (rule: SenderRule) => {
    try {
      await gmailToggleSenderRule(rule.id, !rule.enabled);
      await load();
    } catch {
      // toasted
    }
  };

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <Filter size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">
              Email sender rules
            </h2>
            <p className="text-[11px] text-zinc-400">
              Deterministic shortcuts that beat the LLM classifier when a sender matches.
            </p>
          </div>
        </div>
        <button
          className="btn"
          onClick={() => setCreating((v) => !v)}
          type="button"
        >
          <Plus size={14} />
          New rule
        </button>
      </div>

      {creating && (
        <NewRuleForm
          onCancel={() => setCreating(false)}
          onCreated={async () => {
            setCreating(false);
            await load();
          }}
        />
      )}

      {calibration && calibration.total_overrides > 0 && (
        <div className="border-b border-zinc-100 bg-zinc-50/50 px-5 py-3 text-[11px]">
          <div className="flex items-center justify-between">
            <span className="page-kicker">Classifier calibration</span>
            <span className="text-zinc-400">
              {calibration.total_overrides} correction
              {calibration.total_overrides === 1 ? "" : "s"} recorded
            </span>
          </div>
          <p className="mt-1 text-zinc-500">{calibration.note}</p>
          {calibration.by_dimension.length > 0 && (
            <ul className="mt-2 space-y-0.5 text-zinc-600">
              {calibration.by_dimension.slice(0, 8).map((entry, i) => {
                const rate = Math.round(entry.rate * 100);
                const lo = Math.round(entry.rate_lo * 100);
                const hi = Math.round(entry.rate_hi * 100);
                return (
                  <li key={i} className="font-mono text-[10px]">
                    {entry.dimension} · {entry.original} → {entry.corrected} ·{" "}
                    <span className="text-zinc-700">×{entry.count}</span>
                    {entry.rate > 0 && (
                      <span className="text-zinc-400">
                        {" "}
                        · {rate}% (CI {lo}–{hi}%)
                      </span>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      )}

      {loading && rules.length === 0 ? (
        <div className="space-y-2 p-5">
          {Array.from({ length: 2 }).map((_, i) => (
            <div key={i} className="h-10 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : rules.length === 0 ? (
        <div className="px-5 py-10 text-center">
          <Filter className="mx-auto mb-2 text-zinc-200" size={24} />
          <p className="text-sm text-zinc-400">No rules yet.</p>
          <p className="mt-1 text-xs text-zinc-300">
            Add a rule to auto-classify mail from a sender or domain.
          </p>
        </div>
      ) : (
        <ul className="divide-y divide-zinc-50">
          {rules.map((rule) => (
            <li
              key={rule.id}
              className={`flex items-center gap-3 px-5 py-3 ${
                rule.enabled ? "" : "opacity-50"
              }`}
            >
              <button
                aria-label={rule.enabled ? "Disable rule" : "Enable rule"}
                className="btn h-7 w-7 px-0"
                onClick={() => void handleToggle(rule)}
                type="button"
              >
                {rule.enabled ? <Power size={12} /> : <PowerOff size={12} />}
              </button>
              <div className="min-w-0 flex-1">
                <div className="flex flex-wrap items-baseline gap-2">
                  <span className="font-mono text-[12px] font-medium text-zinc-900">
                    {rule.pattern}
                  </span>
                  <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                    {rule.pattern_kind}
                  </span>
                  {rule.category && (
                    <span className="rounded-md bg-sky-50 px-1.5 py-0.5 text-[10px] font-medium text-sky-700">
                      → {rule.category}
                    </span>
                  )}
                  {rule.priority && (
                    <span className="rounded-md bg-amber-50 px-1.5 py-0.5 text-[10px] font-medium text-amber-700">
                      → {rule.priority}
                    </span>
                  )}
                  {rule.work_relevance && (
                    <span className="rounded-md bg-emerald-50 px-1.5 py-0.5 text-[10px] font-medium text-emerald-700">
                      scope → {rule.work_relevance}
                    </span>
                  )}
                  {rule.attention_state && (
                    <span className="rounded-md bg-rose-50 px-1.5 py-0.5 text-[10px] font-medium text-rose-700">
                      attention → {rule.attention_state}
                    </span>
                  )}
                  {rule.message_type && (
                    <span className="rounded-md bg-violet-50 px-1.5 py-0.5 text-[10px] font-medium text-violet-700">
                      type → {rule.message_type}
                    </span>
                  )}
                </div>
                {rule.note && (
                  <p className="text-[11px] text-zinc-400">{rule.note}</p>
                )}
              </div>
              <span className="text-[10px] text-zinc-400">
                {rule.applied_count} applied
              </span>
              <button
                aria-label="Delete rule"
                className="btn h-7 w-7 px-0 text-zinc-400 hover:text-red-600"
                onClick={() => void handleDelete(rule.id)}
                type="button"
              >
                <Trash2 size={12} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function NewRuleForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: () => void | Promise<void>;
}) {
  const [pattern, setPattern] = useState("");
  const [patternKind, setPatternKind] = useState<"exact" | "glob" | "domain">(
    "glob",
  );
  const [category, setCategory] = useState("");
  const [priority, setPriority] = useState("");
  const [workRelevance, setWorkRelevance] = useState<WorkMailRelevance | "">("");
  const [attentionState, setAttentionState] = useState<WorkMailAttentionState | "">("");
  const [messageType, setMessageType] = useState<WorkMailMessageType | "">("");
  const [note, setNote] = useState("");
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pattern.trim()) {
      toast.warning("Pattern is required");
      return;
    }
    if (!category && !priority && !workRelevance && !attentionState && !messageType) {
      toast.warning("Rule must set at least one classification dimension");
      return;
    }
    setSaving(true);
    try {
      const input: CreateSenderRuleInput = {
        pattern: pattern.trim(),
        pattern_kind: patternKind,
        category: category || null,
        priority: priority || null,
        work_relevance: workRelevance || null,
        attention_state: attentionState || null,
        message_type: messageType || null,
        note: note.trim() || null,
      };
      await gmailCreateSenderRule(input);
      await onCreated();
      setPattern("");
      setCategory("");
      setPriority("");
      setWorkRelevance("");
      setAttentionState("");
      setMessageType("");
      setNote("");
    } catch {
      // toasted
    } finally {
      setSaving(false);
    }
  };

  const activeKind = PATTERN_KIND_OPTIONS.find((p) => p.value === patternKind);

  return (
    <form
      className="space-y-3 border-b border-zinc-100 bg-zinc-50/50 px-5 py-4 text-[12px]"
      onSubmit={(e) => void handleSubmit(e)}
    >
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex-1 min-w-[200px] space-y-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Pattern
          </span>
          <input
            aria-label="Pattern"
            className="field-control font-mono text-[12px]"
            onChange={(e) => setPattern(e.target.value)}
            placeholder={
              patternKind === "domain"
                ? "example.com"
                : patternKind === "exact"
                  ? "alice@example.com"
                  : "*@example.com"
            }
            value={pattern}
          />
        </label>
        <label className="space-y-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Kind
          </span>
          <div className="relative">
            <select
              aria-label="Pattern kind"
              className="appearance-none rounded-lg border border-zinc-200 bg-white px-2 py-1.5 pr-7"
              onChange={(e) =>
                setPatternKind(e.target.value as "exact" | "glob" | "domain")
              }
              value={patternKind}
            >
              {PATTERN_KIND_OPTIONS.map((p) => (
                <option key={p.value} value={p.value}>
                  {p.label}
                </option>
              ))}
            </select>
            <ChevronDown
              className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 text-zinc-400"
              size={12}
            />
          </div>
        </label>
      </div>
      {activeKind && (
        <p className="text-[10px] text-zinc-400">{activeKind.hint}</p>
      )}
      <div className="grid grid-cols-2 gap-2">
        <label className="space-y-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Category
          </span>
          <select
            aria-label="Category"
            className="field-control"
            onChange={(e) => setCategory(e.target.value)}
            value={category}
          >
            {CATEGORY_OPTIONS.map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
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
            className="field-control"
            onChange={(e) => setPriority(e.target.value)}
            value={priority}
          >
            {PRIORITY_OPTIONS.map((p) => (
              <option key={p.value} value={p.value}>
                {p.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="grid gap-2 md:grid-cols-3">
        <RuleSelect
          label="Work scope"
          onChange={(value) => setWorkRelevance(value as WorkMailRelevance | "")}
          options={WORK_RELEVANCE_OPTIONS}
          value={workRelevance}
        />
        <RuleSelect
          label="Attention"
          onChange={(value) => setAttentionState(value as WorkMailAttentionState | "")}
          options={ATTENTION_OPTIONS}
          value={attentionState}
        />
        <RuleSelect
          label="Message type"
          onChange={(value) => setMessageType(value as WorkMailMessageType | "")}
          options={MESSAGE_TYPE_OPTIONS}
          value={messageType}
        />
      </div>
      <input
        aria-label="Note"
        className="field-control"
        onChange={(e) => setNote(e.target.value)}
        placeholder="Notes (optional)"
        value={note}
      />
      <div className="flex justify-end gap-2">
        <button className="btn" onClick={onCancel} type="button">
          Cancel
        </button>
        <button className="btn btn-primary" disabled={saving} type="submit">
          {saving ? "Saving…" : "Create rule"}
        </button>
      </div>
    </form>
  );
}

function RuleSelect({
  label,
  onChange,
  options,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  options: Array<{ value: string; label: string }>;
  value: string;
}) {
  return (
    <label className="space-y-1">
      <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </span>
      <select
        aria-label={label}
        className="field-control"
        onChange={(event) => onChange(event.currentTarget.value)}
        value={value}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}
