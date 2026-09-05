import { memo, useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import { EmptyState } from "./EmptyState";
import {
  listPromptInjectionLog,
  type PromptInjectionEntry,
  type PromptInjectionSnapshot,
} from "../lib/ipc";

function timeAgo(ms: number): string {
  const diff = Date.now() - ms;
  if (diff < 1000) return "just now";
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

function parseFlags(raw: string): string[] {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : [];
  } catch {
    return [];
  }
}

const SOURCE_LABELS: Record<string, string> = {
  email: "Email",
  web: "Web",
  capture: "Capture",
  memory: "Memory",
  tool_confirm: "Tool ✓",
  tool_reject: "Tool ✗",
};

const ACTION_LABELS: Record<string, { label: string; tone: "ok" | "warn" | "bad" }> = {
  sanitized: { label: "sanitized", tone: "ok" },
  flagged: { label: "flagged", tone: "warn" },
  truncated: { label: "truncated", tone: "warn" },
  refused: { label: "refused", tone: "bad" },
  confirmed: { label: "confirmed", tone: "ok" },
  rejected: { label: "rejected", tone: "bad" },
};

export function PromptInjectionLogPanel() {
  const [snapshot, setSnapshot] = useState<PromptInjectionSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [actionFilter, setActionFilter] = useState<string>("all");
  const [onlyWithFlags, setOnlyWithFlags] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listPromptInjectionLog({
        source: sourceFilter === "all" ? undefined : sourceFilter,
        action: actionFilter === "all" ? undefined : actionFilter,
        only_with_flags: onlyWithFlags || undefined,
        limit: 100,
      });
      setSnapshot(data);
    } catch {
      /* ipc wrapper toasts */
    } finally {
      setLoading(false);
    }
  }, [sourceFilter, actionFilter, onlyWithFlags]);

  useEffect(() => {
    void load();
  }, [load]);

  const toggle = (id: string) => {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const stats = useMemo(() => snapshot, [snapshot]);

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-amber-50 text-amber-600">
            <ShieldAlert size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">Prompt injection log</h2>
            <p className="text-[11px] text-zinc-400">
              Sanitized, flagged, truncated, and refused untrusted content.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {stats && (
            <span className="rounded-full bg-zinc-100 px-2.5 py-1 text-[11px] font-medium text-zinc-600">
              {stats.total_24h} in 24h
              {stats.flagged_24h > 0 ? (
                <span className="ml-1.5 text-amber-700">· {stats.flagged_24h} flagged</span>
              ) : null}
              {stats.refusals_24h > 0 ? (
                <span className="ml-1.5 text-red-600">· {stats.refusals_24h} refused</span>
              ) : null}
            </span>
          )}
          <button
            aria-label="Refresh log"
            className="btn h-8 w-8 px-0"
            disabled={loading}
            onClick={() => void load()}
            type="button"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2 border-b border-zinc-100 px-5 py-3">
        <select
          aria-label="Source"
          className="rounded-lg border border-zinc-200 bg-white px-2 py-1.5 text-[12px] text-zinc-700"
          onChange={(e) => setSourceFilter(e.target.value)}
          value={sourceFilter}
        >
          <option value="all">All sources</option>
          <option value="email">Email</option>
          <option value="web">Web fetch</option>
          <option value="capture">Capture</option>
          <option value="memory">Memory</option>
          <option value="tool_confirm">Tool confirm</option>
          <option value="tool_reject">Tool reject</option>
        </select>
        <select
          aria-label="Action"
          className="rounded-lg border border-zinc-200 bg-white px-2 py-1.5 text-[12px] text-zinc-700"
          onChange={(e) => setActionFilter(e.target.value)}
          value={actionFilter}
        >
          <option value="all">All actions</option>
          <option value="flagged">Flagged</option>
          <option value="truncated">Truncated</option>
          <option value="sanitized">Sanitized</option>
          <option value="refused">Refused</option>
          <option value="confirmed">Confirmed</option>
          <option value="rejected">Rejected</option>
        </select>
        <label className="flex items-center gap-1.5 text-[12px] text-zinc-600">
          <input
            checked={onlyWithFlags}
            onChange={(e) => setOnlyWithFlags(e.target.checked)}
            type="checkbox"
          />
          Only with flags
        </label>
      </div>

      <div className="max-h-[480px] overflow-y-auto">
        {!snapshot ? (
          <div className="space-y-2 p-5">
            {Array.from({ length: 4 }).map((_, i) => (
              <div key={i} className="h-12 animate-pulse rounded-xl bg-zinc-100" />
            ))}
          </div>
        ) : snapshot.entries.length === 0 ? (
          <EmptyState
            variant="inline"
            icon={ShieldCheck}
            title="Nothing flagged yet"
            description="Sanitized, flagged, or truncated content will appear here."
          />
        ) : (
          <ul className="divide-y divide-zinc-50">
            {snapshot.entries.map((entry) => (
              <PromptInjectionRow
                entry={entry}
                expanded={expanded.has(entry.id)}
                key={entry.id}
                onToggle={() => toggle(entry.id)}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

const PromptInjectionRow = memo(function PromptInjectionRow({
  entry,
  expanded,
  onToggle,
}: {
  entry: PromptInjectionEntry;
  expanded: boolean;
  onToggle: () => void;
}) {
  const flags = parseFlags(entry.flags_json);
  const action = ACTION_LABELS[entry.action_taken] ?? { label: entry.action_taken, tone: "warn" as const };
  return (
    <li>
      <button
        aria-expanded={expanded}
        className="flex w-full items-center gap-3 px-5 py-3 text-left transition-colors hover:bg-zinc-50"
        onClick={onToggle}
        type="button"
      >
        {expanded ? (
          <ChevronDown className="text-zinc-300" size={14} />
        ) : (
          <ChevronRight className="text-zinc-300" size={14} />
        )}
        {action.tone === "ok" ? (
          <CheckCircle2 className="text-emerald-500" size={14} />
        ) : action.tone === "bad" ? (
          <AlertCircle className="text-red-500" size={14} />
        ) : (
          <ShieldAlert className="text-amber-500" size={14} />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
              {SOURCE_LABELS[entry.source] ?? entry.source}
            </span>
            <span
              className={[
                "rounded-md px-1.5 py-0.5 text-[10px] font-semibold",
                action.tone === "ok"
                  ? "bg-emerald-50 text-emerald-700"
                  : action.tone === "bad"
                  ? "bg-red-50 text-red-700"
                  : "bg-amber-50 text-amber-700",
              ].join(" ")}
            >
              {action.label}
            </span>
            {entry.tool ? (
              <span className="font-mono text-[11px] text-zinc-600">{entry.tool}</span>
            ) : null}
            <span className="ml-auto text-[11px] text-zinc-400">{timeAgo(entry.ts)}</span>
          </div>
          {entry.reason ? (
            <p className="mt-0.5 truncate text-[11px] text-zinc-500">{entry.reason}</p>
          ) : null}
          {flags.length > 0 ? (
            <div className="mt-1 flex flex-wrap gap-1">
              {flags.map((flag) => (
                <span
                  className="rounded-md bg-amber-50 px-1.5 py-0.5 text-[10px] font-medium text-amber-700"
                  key={flag}
                >
                  {flag}
                </span>
              ))}
            </div>
          ) : null}
        </div>
      </button>
      {expanded ? (
        <div className="space-y-3 border-t border-zinc-50 bg-zinc-50/50 px-5 py-4 text-[11px]">
          {entry.content_excerpt ? (
            <div>
              <p className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Excerpt
              </p>
              <pre className="max-h-48 overflow-auto whitespace-pre-wrap rounded-lg border border-zinc-100 bg-white px-3 py-2 font-mono text-[11px] leading-relaxed text-zinc-700">
                {entry.content_excerpt}
              </pre>
            </div>
          ) : null}
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-zinc-400">
            {entry.origin_kind ? <span>kind: {entry.origin_kind}</span> : null}
            {entry.origin_id ? <span>origin: {entry.origin_id}</span> : null}
            {entry.run_id ? <span>run_id: {entry.run_id}</span> : null}
            {entry.call_id ? <span>call_id: {entry.call_id}</span> : null}
            {entry.original_bytes > 0 ? (
              <span>
                {entry.original_bytes}B → {entry.sanitized_bytes}B
              </span>
            ) : null}
            <span>ts: {new Date(entry.ts).toLocaleString()}</span>
          </div>
        </div>
      ) : null}
    </li>
  );
});
