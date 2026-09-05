import { useEffect, useState } from "react";
import { AlertCircle, Loader2, RotateCcw, X } from "lucide-react";

import { getReportScopePreview, replaceScopeExclusions } from "../../lib/ipc";
import type { ReportRun, ReportScopeEntry, ReportScopePreview } from "../../lib/types";

const KIND_LABEL: Record<string, string> = {
  initiative: "Initiative",
  deliverable: "Deliverable",
  stakeholder: "Stakeholder",
  email_thread: "Email",
  meeting: "Meeting",
  file: "File",
  calendar_event: "Calendar",
};

interface Props {
  report: ReportRun;
  onUpdate: (report: ReportRun) => void;
}

export function ScopePreview({ report, onUpdate }: Props) {
  const [preview, setPreview] = useState<ReportScopePreview | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [excluded, setExcluded] = useState<Set<string>>(
    () => new Set(parseExclusions(report.scope_exclusions_json)),
  );

  useEffect(() => {
    setExcluded(new Set(parseExclusions(report.scope_exclusions_json)));
  }, [report.scope_exclusions_json]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    getReportScopePreview(report.id)
      .then((data) => { setPreview(data); setLoading(false); })
      .catch((err) => { setError(String(err)); setLoading(false); });
  }, [report.id]);

  async function toggleExclude(entryId: string) {
    const next = new Set(excluded);
    if (next.has(entryId)) next.delete(entryId);
    else next.add(entryId);
    setExcluded(next);
    const updated = await replaceScopeExclusions(report.id, Array.from(next));
    onUpdate(updated);
  }

  async function resetExclusions() {
    setExcluded(new Set());
    const updated = await replaceScopeExclusions(report.id, []);
    onUpdate(updated);
  }

  if (loading) {
    return (
      <div className="flex items-center gap-2 py-4 text-xs text-zinc-400">
        <Loader2 className="animate-spin" size={13} /> Resolving scope…
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center gap-2 py-2 text-xs text-red-500">
        <AlertCircle size={13} /> {error}
      </div>
    );
  }

  if (!preview || preview.total_entries === 0) {
    return <EmptyScope />;
  }

  const grouped = groupByKind(preview.entries);
  const excludedCount = excluded.size;

  return (
    <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
      <div className="mb-3 flex items-center justify-between">
        <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-500">
          Evidence scope · {preview.total_entries} entities
        </span>
        {excludedCount > 0 ? (
          <button
            className="flex items-center gap-1 text-[11px] text-violet-600 hover:underline"
            onClick={() => void resetExclusions()}
            type="button"
          >
            <RotateCcw size={11} />
            {excludedCount} excluded — reset
          </button>
        ) : null}
      </div>

      <div className="space-y-3">
        {Object.entries(grouped).map(([kind, entries]) => (
          <div key={kind}>
            <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
              {KIND_LABEL[kind] ?? kind} ({entries.length})
            </div>
            <div className="flex flex-wrap gap-1.5">
              {entries.map((entry) => {
                const isExcluded = excluded.has(entry.id);
                return (
                  <button
                    className={[
                      "group inline-flex max-w-[220px] items-center gap-1 truncate rounded-lg border px-2.5 py-1.5 text-[11px] transition-colors",
                      isExcluded
                        ? "border-zinc-200 bg-white text-zinc-400 line-through"
                        : "border-zinc-200 bg-white text-zinc-700 hover:border-zinc-300 hover:bg-zinc-50",
                    ].join(" ")}
                    key={entry.id}
                    onClick={() => void toggleExclude(entry.id)}
                    title={entry.title}
                    type="button"
                  >
                    <span className="truncate">{entry.title}</span>
                    {!isExcluded ? (
                      <X
                        className="shrink-0 text-zinc-300 opacity-0 group-hover:opacity-100 transition-opacity"
                        size={10}
                      />
                    ) : null}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export function EmptyScope() {
  return (
    <div className="rounded-2xl border border-dashed border-zinc-200 p-10 text-center">
      <AlertCircle className="mx-auto text-zinc-200" size={32} />
      <p className="mt-3 text-sm font-medium text-zinc-500">No linked work found for this scope.</p>
      <p className="mt-1 text-xs text-zinc-400">
        Link emails, meetings, or deliverables to the initiative to generate a report.
      </p>
    </div>
  );
}

function groupByKind(entries: ReportScopeEntry[]): Record<string, ReportScopeEntry[]> {
  const order = ["initiative", "deliverable", "stakeholder", "email_thread", "meeting", "file", "calendar_event"];
  const result: Record<string, ReportScopeEntry[]> = {};
  for (const entry of entries) {
    (result[entry.kind] ??= []).push(entry);
  }
  return Object.fromEntries(
    order.filter((k) => result[k]).map((k) => [k, result[k]]),
  );
}

function parseExclusions(raw: string): string[] {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as string[]) : [];
  } catch {
    return [];
  }
}
