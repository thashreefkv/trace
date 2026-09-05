import { useCallback, useEffect, useMemo, useState } from "react";
import { Brain, ChevronDown, RefreshCw, RotateCcw } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import {
  getBrainLearningSummary,
  getTemplateDetail,
  resetBrainTemplateLearning,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type {
  BrainLearningSummary,
  BrainPolicySummary,
  InferenceThresholdSummary,
  TemplateDetail,
} from "../lib/types";

/**
 * Section 6.2 — per-template learning settings.
 *
 * One card per RL template surfaced via `get_brain_learning_summary`.
 * `node_importance` is omitted (item-keyed, not feature-vectored — needs a
 * different card design). Each card shows observations, current threshold
 * (when applicable), top-N feature weights as horizontal bars, recent
 * events (collapsed by default), and a Reset button that calls
 * `reset_brain_template_learning` after inline confirmation.
 */
export function RLLearningSettings() {
  const [summary, setSummary] = useState<BrainLearningSummary | null>(null);
  const [loading, setLoading] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setSummary(await getBrainLearningSummary());
    } catch {
      // toasted
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const policies = useMemo(() => {
    if (!summary) return [] as BrainPolicySummary[];
    return summary.policies.filter((p) => p.template !== "node_importance");
  }, [summary]);

  const thresholdMap = useMemo(() => {
    const map = new Map<string, InferenceThresholdSummary>();
    summary?.inference_thresholds.forEach((t) => map.set(t.template, t));
    return map;
  }, [summary]);

  // Thresholds without policies (e.g. inference-only templates).
  const standaloneThresholds = useMemo(() => {
    if (!summary) return [] as InferenceThresholdSummary[];
    const seen = new Set(policies.map((p) => p.template));
    return summary.inference_thresholds.filter((t) => !seen.has(t.template));
  }, [policies, summary]);

  return (
    <section className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] p-4">
      <div className="mb-3 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Brain className="text-violet-500" size={14} />
          <span className="page-kicker">Learning policies</span>
        </div>
        <button
          aria-label="Refresh policies"
          className="btn h-7 w-7 px-0"
          disabled={loading}
          onClick={() => void load()}
          type="button"
        >
          <RefreshCw size={12} className={loading ? "animate-spin" : ""} />
        </button>
      </div>

      {!summary && loading ? (
        <div className="space-y-2">
          {[0, 1, 2].map((i) => (
            <div key={i} className="h-24 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : policies.length === 0 && standaloneThresholds.length === 0 ? (
        <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3 text-[12px] text-zinc-400">
          No learned policies yet. The bandit starts learning after the first
          feedback event.
        </div>
      ) : (
        <div className="space-y-3">
          {policies.map((policy) => (
            <TemplateCard
              key={policy.template}
              policy={policy}
              threshold={thresholdMap.get(policy.template) ?? null}
              onReset={() => void load()}
            />
          ))}
          {standaloneThresholds.map((threshold) => (
            <TemplateCard
              key={threshold.template}
              policy={null}
              threshold={threshold}
              onReset={() => void load()}
            />
          ))}
        </div>
      )}
    </section>
  );
}

function TemplateCard({
  policy,
  threshold,
  onReset,
}: {
  policy: BrainPolicySummary | null;
  threshold: InferenceThresholdSummary | null;
  onReset: () => void;
}) {
  const template = policy?.template ?? threshold?.template ?? "";
  const [detail, setDetail] = useState<TemplateDetail | null>(null);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const loadDetail = useCallback(async () => {
    if (detail) return;
    setLoadingDetail(true);
    try {
      setDetail(await getTemplateDetail(template));
    } catch {
      // toasted
    } finally {
      setLoadingDetail(false);
    }
  }, [detail, template]);

  const handleToggle = () => {
    setExpanded((value) => {
      const next = !value;
      if (next) void loadDetail();
      return next;
    });
  };

  const handleReset = useCallback(async () => {
    if (!confirming) {
      setConfirming(true);
      window.setTimeout(() => setConfirming(false), 4000);
      return;
    }
    setBusy(true);
    try {
      await resetBrainTemplateLearning(template);
      toast.success(`Reset ${template}`);
      setDetail(null);
      onReset();
    } catch (error) {
      toast.error(`Reset failed: ${formatError(error)}`);
    } finally {
      setBusy(false);
      setConfirming(false);
    }
  }, [confirming, onReset, template]);

  const featureCoefficients = detail?.coefficient_summary ?? [];
  const top = useMemo(
    () =>
      [...featureCoefficients]
        .sort((a, b) => b.abs_max - a.abs_max)
        .slice(0, 5),
    [featureCoefficients],
  );
  const maxAbs = Math.max(0.0001, ...top.map((c) => c.abs_max));

  const isBlend = template === "retrieval_blend";

  return (
    <div className="rounded-xl border border-zinc-100 bg-zinc-50/40">
      <button
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left"
        onClick={handleToggle}
        type="button"
      >
        <span className="text-[13px] font-semibold text-zinc-950">{template}</span>
        <div className="flex flex-wrap items-center gap-2 text-[11px] text-zinc-500">
          {policy ? (
            <span>
              <span className="font-semibold text-zinc-700">{policy.observations}</span>{" "}
              observations
            </span>
          ) : null}
          {threshold ? (
            <>
              <span>
                threshold{" "}
                <span className="font-semibold text-zinc-700">
                  {threshold.threshold.toFixed(2)}
                </span>
              </span>
              <span>·</span>
              <span>
                <span className="font-semibold text-zinc-700">{threshold.sample_count}</span>{" "}
                samples
              </span>
            </>
          ) : null}
        </div>
        <ChevronDown
          size={14}
          className={[
            "ml-auto text-zinc-400 transition-transform",
            expanded ? "rotate-180" : "",
          ].join(" ")}
        />
      </button>

      <AnimatePresence initial={false}>
        {expanded ? (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            className="overflow-hidden border-t border-zinc-100"
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
          >
            <div className="space-y-3 p-3">
              {loadingDetail && !detail ? (
                <div className="h-20 animate-pulse rounded-xl bg-zinc-100" />
              ) : detail ? (
                <>
                  {top.length > 0 ? (
                    <div>
                      <div className="mb-1.5 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                        Top feature weights
                      </div>
                      <div className="space-y-1.5">
                        {top.map((coef) => (
                          <FeatureBar
                            key={coef.feature}
                            label={coef.feature}
                            value={coef.mean}
                            max={maxAbs}
                          />
                        ))}
                      </div>
                    </div>
                  ) : (
                    <div className="text-[11px] text-zinc-400">
                      No feature weights yet — the policy hasn't run a least-squares
                      update.
                    </div>
                  )}

                  <RecentEventsList events={detail.recent_events} />
                </>
              ) : (
                <div className="text-[11px] text-zinc-400">Failed to load detail.</div>
              )}

              <div className="flex items-center justify-between border-t border-zinc-100 pt-3">
                <div className="text-[11px] text-zinc-500">
                  {isBlend
                    ? "Reset reverts retrieval to baseline blend until ~20 new feedback events accrue."
                    : "Reset wipes A/b matrices and per-item scores. Audit trail (events) is preserved."}
                </div>
                <button
                  className="btn flex h-7 items-center gap-1 px-2 text-[11px]"
                  disabled={busy}
                  onClick={() => void handleReset()}
                  type="button"
                >
                  <RotateCcw size={11} />
                  {confirming ? "Confirm reset" : "Reset learning"}
                </button>
              </div>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function FeatureBar({
  label,
  value,
  max,
}: {
  label: string;
  value: number;
  max: number;
}) {
  const magnitude = Math.min(1, Math.abs(value) / max);
  const positive = value >= 0;
  return (
    <div className="flex items-center gap-2">
      <span className="w-28 truncate text-[11px] text-zinc-500" title={label}>
        {label}
      </span>
      <div className="relative h-1.5 flex-1 overflow-hidden rounded-full bg-zinc-100">
        <div
          className={[
            "absolute top-0 h-full rounded-full",
            positive ? "left-1/2 bg-emerald-400" : "right-1/2 bg-rose-400",
          ].join(" ")}
          style={{ width: `${(magnitude * 100) / 2}%` }}
        />
        <span className="absolute left-1/2 top-0 h-full w-px bg-zinc-300" />
      </div>
      <span className="w-12 text-right text-[10px] font-medium text-zinc-500">
        {value.toFixed(2)}
      </span>
    </div>
  );
}

function RecentEventsList({
  events,
}: {
  events: TemplateDetail["recent_events"];
}) {
  if (events.length === 0) {
    return (
      <div className="text-[11px] text-zinc-400">No recent events.</div>
    );
  }
  return (
    <details className="rounded-xl border border-zinc-100 bg-white p-2">
      <summary className="cursor-pointer text-[11px] font-medium text-zinc-500">
        Last {events.length} events
      </summary>
      <ul className="mt-2 space-y-1 text-[11px]">
        {events.map((event) => {
          const reward = event.reward;
          const rewardTone =
            reward > 0.01 ? "text-emerald-600" : reward < -0.01 ? "text-rose-600" : "text-zinc-500";
          return (
            <li key={event.id} className="flex items-center gap-2">
              <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-600">
                {event.event_type}
              </span>
              <span className={`font-mono text-[10px] ${rewardTone}`}>
                {reward >= 0 ? "+" : ""}
                {reward.toFixed(2)}
              </span>
              <span className="truncate text-zinc-400" title={event.item_id}>
                {event.item_id}
              </span>
              <span className="ml-auto shrink-0 text-[10px] text-zinc-400">
                {formatRelative(event.created_at)}
              </span>
            </li>
          );
        })}
      </ul>
    </details>
  );
}

function formatRelative(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  const diffMs = Date.now() - date.getTime();
  const sec = Math.round(diffMs / 1000);
  if (sec < 60) return `${sec}s`;
  const min = Math.round(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.round(min / 60);
  if (hr < 24) return `${hr}h`;
  return `${Math.round(hr / 24)}d`;
}

function formatError(error: unknown) {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return String(error);
}
