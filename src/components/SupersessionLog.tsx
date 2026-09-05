import { useCallback, useEffect, useState } from "react";
import { History, RefreshCw, Undo2 } from "lucide-react";
import { EmptyState } from "./EmptyState";
import { listInferenceSupersessions, revertInferenceSupersession } from "../lib/ipc";
import { toast } from "../lib/toast";
import type { SupersessionRecord } from "../lib/types";
import { InferenceRowView } from "./InferenceRow";

/**
 * Section 6.2 — recent supersessions log.
 *
 * Lists (loser, winner) pairs ordered by recency. Each row has a Revert
 * button that clears the supersede metadata on the loser (status stays
 * put). Confirmation is inline — clicking Revert twice within 3s confirms.
 */
export function SupersessionLog() {
  const [rows, setRows] = useState<SupersessionRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setRows(await listInferenceSupersessions(50));
    } catch {
      // toasted
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const onRevert = useCallback(
    async (record: SupersessionRecord) => {
      const loserId = record.loser.id;
      if (confirmingId !== loserId) {
        setConfirmingId(loserId);
        window.setTimeout(() => {
          setConfirmingId((current) => (current === loserId ? null : current));
        }, 3000);
        return;
      }
      setConfirmingId(null);
      setBusyId(loserId);
      // Optimistic removal — server-side state already supports re-fetch.
      setRows((prev) => prev.filter((r) => r.loser.id !== loserId));
      try {
        await revertInferenceSupersession(loserId);
        toast.success("Supersession reverted");
      } catch (error) {
        toast.error(`Revert failed: ${formatError(error)}`);
        // Repopulate from server on failure
        try {
          setRows(await listInferenceSupersessions(50));
        } catch {
          // ignore
        }
      } finally {
        setBusyId(null);
      }
    },
    [confirmingId],
  );

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <History className="text-amber-500" size={14} />
          <span className="page-kicker">Recent supersessions</span>
          {rows.length > 0 ? (
            <span className="text-[11px] text-zinc-400">{rows.length} pairs</span>
          ) : null}
        </div>
        <button
          aria-label="Refresh supersessions"
          className="btn h-7 w-7 px-0"
          disabled={loading}
          onClick={() => void load()}
          type="button"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {loading && rows.length === 0 ? (
        <div className="space-y-2">
          {[0, 1].map((i) => (
            <div key={i} className="h-24 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : rows.length === 0 ? (
        <EmptyState variant="inline" icon={History} title="No supersessions yet" description="Overridden inference decisions will appear here." />
      ) : (
        <div className="space-y-3">
          {rows.map((record) => (
            <div
              key={record.loser.id}
              className="rounded-xl border border-zinc-100 bg-zinc-50/40 p-3"
            >
              <div className="mb-2 flex items-center justify-between gap-2">
                <div className="flex items-center gap-2 text-[11px] text-zinc-500">
                  <span className="rounded-md bg-amber-50 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-amber-700">
                    {record.supersede_reason || "auto"}
                  </span>
                  <span className="text-zinc-400">
                    {formatRelative(record.superseded_at)}
                  </span>
                </div>
                <button
                  className="btn flex h-7 items-center gap-1 px-2 text-[11px]"
                  disabled={busyId === record.loser.id}
                  onClick={() => void onRevert(record)}
                  type="button"
                >
                  <Undo2 size={11} />
                  {confirmingId === record.loser.id ? "Confirm revert" : "Revert"}
                </button>
              </div>
              <div className="space-y-2">
                <div>
                  <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-emerald-600">
                    Winner (accepted)
                  </div>
                  <InferenceRowView row={record.winner} />
                </div>
                <div>
                  <div className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                    Loser (superseded)
                  </div>
                  <InferenceRowView row={record.loser} muted />
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

function formatRelative(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const diffMs = Date.now() - date.getTime();
  const sec = Math.round(diffMs / 1000);
  if (sec < 60) return `${sec}s ago`;
  const min = Math.round(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.round(hr / 24);
  return `${day}d ago`;
}

function formatError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
