import { useCallback, useEffect, useMemo, useState } from "react";
import { Check, ChevronRight, Inbox, RefreshCw, X } from "lucide-react";
import {
  getBrainLearningSummary,
  listBrainInferences,
  reviewInference,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type {
  BrainInferenceFilter,
  BrainInferenceListResult,
  BrainInferenceRow,
  InferenceThresholdSummary,
} from "../lib/types";
import { InferenceRowView } from "./InferenceRow";

/**
 * Section 6.2 — paginated pending-inference review queue.
 *
 * Filters by template (chips populated from `inference_thresholds`), accepts
 * or rejects rows in-place with optimistic removal, and shows the threshold
 * delta in a sonner toast for the affected template.
 */
export function InferenceReviewQueue() {
  const [data, setData] = useState<BrainInferenceListResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [templates, setTemplates] = useState<InferenceThresholdSummary[]>([]);
  const [templateFilter, setTemplateFilter] = useState<string | "all">("all");
  const [cursor, setCursor] = useState<string | null>(null);

  const load = useCallback(
    async (opts?: { append?: boolean; templateOverride?: string | "all" }) => {
      setLoading(true);
      try {
        const filter: BrainInferenceFilter = {
          status: "pending",
          template:
            (opts?.templateOverride ?? templateFilter) === "all"
              ? null
              : ((opts?.templateOverride ?? templateFilter) as string),
          limit: 25,
          before_updated_at: opts?.append ? cursor : null,
        };
        const result = await listBrainInferences(filter);
        if (opts?.append && data) {
          setData({
            ...result,
            items: [...data.items, ...result.items],
          });
        } else {
          setData(result);
        }
        setCursor(result.next_cursor);
      } catch {
        // toasted
      } finally {
        setLoading(false);
      }
    },
    [cursor, data, templateFilter],
  );

  useEffect(() => {
    void load();
  }, []); // initial only — pagination + filter triggers explicit reload

  useEffect(() => {
    (async () => {
      try {
        const summary = await getBrainLearningSummary();
        setTemplates(summary.inference_thresholds);
      } catch {
        // toasted
      }
    })();
  }, []);

  const onReview = useCallback(
    async (row: BrainInferenceRow, decision: "accepted" | "rejected") => {
      setBusy(row.id);
      // Optimistic remove
      setData((prev) =>
        prev
          ? {
              ...prev,
              items: prev.items.filter((item) => item.id !== row.id),
              total_pending: Math.max(0, prev.total_pending - 1),
            }
          : prev,
      );
      try {
        const result = await reviewInference(row.id, decision);
        const verb = decision === "accepted" ? "Accepted" : "Rejected";
        const templateLabel = result.template ?? "unknown";
        const beforePct = result.threshold_before;
        const afterPct = result.threshold_after;
        const movedLine =
          beforePct != null && afterPct != null && Math.abs(beforePct - afterPct) > 0.0001
            ? ` · threshold ${beforePct.toFixed(2)} → ${afterPct.toFixed(2)}`
            : "";
        const supersededLine =
          result.superseded_inference_ids.length > 0
            ? ` · ${result.superseded_inference_ids.length} superseded`
            : "";
        toast.success(`${verb} · ${templateLabel}${movedLine}${supersededLine}`);
      } catch (error) {
        // Roll back optimistic remove
        setData((prev) => {
          if (!prev) return prev;
          if (prev.items.some((item) => item.id === row.id)) return prev;
          return {
            ...prev,
            items: [row, ...prev.items],
            total_pending: prev.total_pending + 1,
          };
        });
        toast.error(`Review failed: ${formatError(error)}`);
      } finally {
        setBusy(null);
      }
    },
    [],
  );

  const items = data?.items ?? [];

  const templateOptions = useMemo(
    () => [
      { value: "all" as const, label: "All" },
      ...templates.map((t) => ({ value: t.template, label: t.template })),
    ],
    [templates],
  );

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Inbox className="text-violet-500" size={14} />
          <span className="page-kicker">Inference review queue</span>
          {data ? (
            <span className="text-[11px] text-zinc-400">
              {data.total_pending} pending · {data.total_accepted_7d} accepted 7d ·{" "}
              {data.total_rejected_7d} rejected 7d
            </span>
          ) : null}
        </div>
        <button
          aria-label="Refresh queue"
          className="btn h-7 w-7 px-0"
          disabled={loading}
          onClick={() => {
            setCursor(null);
            void load();
          }}
          type="button"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      <div className="mb-3 flex flex-wrap gap-1">
        {templateOptions.map((opt) => (
          <button
            key={opt.value}
            className={[
              "btn h-7 px-2 text-[11px]",
              templateFilter === opt.value ? "bg-zinc-900 text-white" : "",
            ].join(" ")}
            onClick={() => {
              setTemplateFilter(opt.value);
              setCursor(null);
              void load({ templateOverride: opt.value });
            }}
            type="button"
          >
            {opt.label}
          </button>
        ))}
      </div>

      {!data && loading ? (
        <div className="space-y-2">
          {[0, 1, 2].map((i) => (
            <div key={i} className="h-20 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : items.length === 0 ? (
        <div className="flex flex-col items-center gap-2 py-8 text-zinc-300">
          <Inbox size={28} />
          <span className="text-[12px] text-zinc-400">
            No pending inferences{templateFilter !== "all" ? ` for ${templateFilter}` : ""}
          </span>
        </div>
      ) : (
        <div className="space-y-2">
          {items.map((row) => (
            <InferenceRowView
              key={row.id}
              row={row}
              trailing={
                <div className="flex items-center gap-1">
                  <button
                    aria-label="Accept inference"
                    className="btn h-7 w-7 px-0 text-emerald-600 hover:bg-emerald-50"
                    disabled={busy === row.id}
                    onClick={() => void onReview(row, "accepted")}
                    type="button"
                  >
                    <Check size={13} />
                  </button>
                  <button
                    aria-label="Reject inference"
                    className="btn h-7 w-7 px-0 text-rose-600 hover:bg-rose-50"
                    disabled={busy === row.id}
                    onClick={() => void onReview(row, "rejected")}
                    type="button"
                  >
                    <X size={13} />
                  </button>
                </div>
              }
            />
          ))}
          {data?.has_more ? (
            <button
              className="btn flex w-full items-center justify-center gap-1 text-[12px]"
              disabled={loading}
              onClick={() => void load({ append: true })}
              type="button"
            >
              Load more <ChevronRight size={12} />
            </button>
          ) : null}
        </div>
      )}
    </section>
  );
}

function formatError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
