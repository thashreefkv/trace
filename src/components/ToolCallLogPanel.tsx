import { memo, useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  RefreshCw,
  ScrollText,
} from "lucide-react";
import { listToolCallLog } from "../lib/ipc";
import type { ToolCallLogEntry, ToolCallLogSnapshot } from "../lib/types";

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

function prettyJson(raw: string | null): string {
  if (!raw) return "";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

export function ToolCallLogPanel() {
  const [snapshot, setSnapshot] = useState<ToolCallLogSnapshot | null>(null);
  const [loading, setLoading] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [sourceFilter, setSourceFilter] = useState<string>("all");
  const [toolFilter, setToolFilter] = useState<string>("");
  const [onlyErrors, setOnlyErrors] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const data = await listToolCallLog({
        source: sourceFilter === "all" ? null : sourceFilter,
        tool: toolFilter.trim() === "" ? null : toolFilter.trim(),
        only_errors: onlyErrors || null,
        limit: 100,
      });
      setSnapshot(data);
    } catch {
      // ipc wrapper toasts; nothing else to do
    } finally {
      setLoading(false);
    }
  }, [sourceFilter, toolFilter, onlyErrors]);

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

  const stats = useMemo(() => {
    if (!snapshot) return null;
    const errorRate =
      snapshot.total_calls_24h > 0
        ? Math.round(
            (snapshot.error_calls_24h / snapshot.total_calls_24h) * 100,
          )
        : 0;
    return { ...snapshot, errorRate };
  }, [snapshot]);

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <ScrollText size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">
              Tool calls
            </h2>
            <p className="text-[11px] text-zinc-400">
              Audit log of every tool the AI has invoked.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          {stats && (
            <span className="rounded-full bg-zinc-100 px-2.5 py-1 text-[11px] font-medium text-zinc-600">
              {stats.total_calls_24h} in 24h
              {stats.error_calls_24h > 0 ? (
                <span className="ml-1.5 text-red-600">
                  · {stats.error_calls_24h} err
                </span>
              ) : null}
            </span>
          )}
          <button
            aria-label="Refresh tool calls"
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
          <option value="ask">Ask</option>
          <option value="mcp">MCP</option>
        </select>
        <input
          aria-label="Tool name"
          className="flex-1 min-w-[160px] rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5 text-[12px] text-zinc-700 placeholder-zinc-400"
          onChange={(e) => setToolFilter(e.target.value)}
          placeholder="Filter by tool name…"
          value={toolFilter}
        />
        <label className="flex items-center gap-1.5 text-[12px] text-zinc-600">
          <input
            checked={onlyErrors}
            onChange={(e) => setOnlyErrors(e.target.checked)}
            type="checkbox"
          />
          Errors only
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
          <div className="px-5 py-10 text-center">
            <ScrollText className="mx-auto mb-2 text-zinc-200" size={24} />
            <p className="text-sm text-zinc-400">No tool calls yet.</p>
            <p className="mt-1 text-xs text-zinc-300">
              The AI will record entries here as it works.
            </p>
          </div>
        ) : (
          <ul className="divide-y divide-zinc-50">
            {snapshot.entries.map((entry) => (
              <ToolCallRow
                key={entry.id}
                entry={entry}
                expanded={expanded.has(entry.id)}
                onToggle={() => toggle(entry.id)}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}

const ToolCallRow = memo(function ToolCallRow({
  entry,
  expanded,
  onToggle,
}: {
  entry: ToolCallLogEntry;
  expanded: boolean;
  onToggle: () => void;
}) {
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
        {entry.ok ? (
          <CheckCircle2 className="text-emerald-500" size={14} />
        ) : (
          <AlertCircle className="text-red-500" size={14} />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="font-mono text-[12px] font-medium text-zinc-900">
              {entry.tool}
            </span>
            <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
              {entry.source}
            </span>
            <span className="text-[11px] text-zinc-400">
              {entry.latency_ms}ms · {timeAgo(entry.ts)}
            </span>
          </div>
          {entry.result_summary ? (
            <p className="truncate text-[11px] text-zinc-500">
              {entry.result_summary}
            </p>
          ) : entry.error ? (
            <p className="truncate text-[11px] text-red-600">{entry.error}</p>
          ) : null}
        </div>
      </button>
      {expanded && (
        <div className="space-y-3 border-t border-zinc-50 bg-zinc-50/50 px-5 py-4 text-[11px]">
          <ToolCallSection label="Args" value={prettyJson(entry.args_json)} />
          {entry.result_json && (
            <ToolCallSection
              label="Result"
              value={prettyJson(entry.result_json)}
            />
          )}
          {entry.error && (
            <ToolCallSection label="Error" value={entry.error} tone="error" />
          )}
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-[10px] text-zinc-400">
            {entry.run_id && <span>run_id: {entry.run_id}</span>}
            {entry.call_id && <span>call_id: {entry.call_id}</span>}
            <span>ts: {new Date(entry.ts).toLocaleString()}</span>
          </div>
        </div>
      )}
    </li>
  );
});

function ToolCallSection({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "error";
}) {
  return (
    <div>
      <p className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </p>
      <pre
        className={`max-h-48 overflow-auto rounded-lg border px-3 py-2 font-mono text-[11px] leading-relaxed ${
          tone === "error"
            ? "border-red-100 bg-red-50 text-red-800"
            : "border-zinc-100 bg-white text-zinc-700"
        }`}
      >
        {value}
      </pre>
    </div>
  );
}
