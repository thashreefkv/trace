import { useCallback, useEffect, useState } from "react";
import {
  Brain,
  CheckCircle2,
  RefreshCw,
  Repeat,
  Scale,
  TrendingUp,
  XCircle,
} from "lucide-react";
import { getRLDigest } from "../lib/ipc";
import type { RLDigest } from "../lib/types";

/**
 * Section 6.2 — weekly RL digest.
 *
 * 6-tile dashboard: inferences generated / accepted / rejected, acceptance
 * rate, supersessions, ask-feedback rate. Below: "Top template" callout
 * + threshold drift number. Window toggle 7d / 30d.
 *
 * Pattern mirrors `InboxDashboard.tsx` — single panel, header with
 * refresh, grid of stat tiles, optional callout below.
 */
export function RLDigestPanel() {
  const [data, setData] = useState<RLDigest | null>(null);
  const [loading, setLoading] = useState(false);
  const [windowDays, setWindowDays] = useState<7 | 30>(7);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setData(await getRLDigest(windowDays));
    } catch {
      // toasted
    } finally {
      setLoading(false);
    }
  }, [windowDays]);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Brain className="text-violet-500" size={14} />
          <span className="page-kicker">Brain learning · {windowDays}d</span>
        </div>
        <div className="flex items-center gap-1">
          <button
            className={[
              "btn h-7 px-2 text-[11px]",
              windowDays === 7 ? "bg-zinc-900 text-white" : "",
            ].join(" ")}
            onClick={() => setWindowDays(7)}
            type="button"
          >
            7d
          </button>
          <button
            className={[
              "btn h-7 px-2 text-[11px]",
              windowDays === 30 ? "bg-zinc-900 text-white" : "",
            ].join(" ")}
            onClick={() => setWindowDays(30)}
            type="button"
          >
            30d
          </button>
          <button
            aria-label="Refresh digest"
            className="btn h-7 w-7 px-0"
            disabled={loading}
            onClick={() => void load()}
            type="button"
          >
            <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {!data ? (
        <div className="grid grid-cols-3 gap-2">
          {[0, 1, 2, 3, 4, 5].map((i) => (
            <div
              key={i}
              className="h-16 animate-pulse rounded-xl bg-zinc-100"
            />
          ))}
        </div>
      ) : (
        <>
          <div className="grid grid-cols-3 gap-2">
            <Tile
              icon={<TrendingUp size={14} />}
              label="Generated"
              value={data.inferences_generated}
            />
            <Tile
              icon={<CheckCircle2 size={14} />}
              label="Accepted"
              value={data.inferences_accepted}
              tone={data.inferences_accepted > 0 ? "success" : undefined}
            />
            <Tile
              icon={<XCircle size={14} />}
              label="Rejected"
              value={data.inferences_rejected}
              tone={data.inferences_rejected > 0 ? "warning" : undefined}
            />
            <Tile
              icon={<Scale size={14} />}
              label="Acceptance rate"
              value={percent(data.acceptance_rate)}
              hint={`${data.inferences_accepted + data.inferences_rejected} reviewed`}
            />
            <Tile
              icon={<Repeat size={14} />}
              label="Supersessions"
              value={data.supersessions}
              tone={data.supersessions > 0 ? "info" : undefined}
            />
            <Tile
              icon={<Scale size={14} />}
              label="Ask feedback"
              value={percent(data.ask_feedback_rate)}
              hint={`${data.ask_useful} useful · ${data.ask_wrong} wrong`}
            />
          </div>

          <div className="mt-3 grid grid-cols-1 gap-2 sm:grid-cols-2">
            {data.top_template ? (
              <div className="rounded-xl border border-violet-100 bg-violet-50/40 p-3">
                <div className="text-[10px] font-semibold uppercase tracking-wider text-violet-500">
                  Top template
                </div>
                <div className="mt-1 truncate text-[13px] font-semibold text-violet-900">
                  {data.top_template.name}
                </div>
                <div className="mt-0.5 text-[11px] text-violet-700">
                  {data.top_template.accepted} accepted · {data.top_template.rejected} rejected
                  · {percent(data.top_template.acceptance_rate)}
                </div>
              </div>
            ) : (
              <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3 text-[11px] text-zinc-400">
                No template events in window
              </div>
            )}
            <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
              <div className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                Threshold drift
              </div>
              <div className="mt-1 text-[13px] font-semibold text-zinc-950">
                ±{(data.threshold_drift * 100).toFixed(1)}pp
              </div>
              <div className="mt-0.5 text-[11px] text-zinc-500">
                Mean delta from baseline across recomputed templates
              </div>
            </div>
          </div>

          <div className="mt-3 text-[11px] text-zinc-400">
            New embeddings: {data.embeddings_added} ·{" "}
            <span className="text-zinc-500">
              Window {data.window_days} day{data.window_days === 1 ? "" : "s"}
            </span>
          </div>
        </>
      )}
    </section>
  );
}

function percent(value: number) {
  if (!Number.isFinite(value)) return "—";
  return `${(value * 100).toFixed(0)}%`;
}

function Tile({
  icon,
  label,
  value,
  tone,
  hint,
}: {
  icon: React.ReactNode;
  label: string;
  value: number | string;
  tone?: "info" | "warning" | "error" | "success";
  hint?: string;
}) {
  const cls =
    tone === "error"
      ? "border-red-100 bg-red-50 text-red-700"
      : tone === "warning"
        ? "border-amber-100 bg-amber-50 text-amber-700"
        : tone === "info"
          ? "border-sky-100 bg-sky-50 text-sky-700"
          : tone === "success"
            ? "border-emerald-100 bg-emerald-50 text-emerald-700"
            : "border-zinc-100 bg-zinc-50 text-zinc-700";
  return (
    <div className={`rounded-xl border p-3 ${cls}`}>
      <div className="flex items-center gap-1.5 text-zinc-400">
        {icon}
        <span className="text-[10px] font-semibold uppercase tracking-wider">{label}</span>
      </div>
      <p className={`mt-1 text-lg font-bold ${tone ? "" : "text-zinc-950"}`}>{value}</p>
      {hint ? <p className="text-[10px] text-zinc-400">{hint}</p> : null}
    </div>
  );
}
