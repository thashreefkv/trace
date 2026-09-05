import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { ChevronDown, ChevronRight, Sparkles } from "lucide-react";
import type { ScoredBrainNode, SearchResult } from "../lib/types";

/**
 * Section 6.2 — "Why this answer?" expander.
 *
 * Renders the per-cited-node retrieval signal breakdown captured during the
 * Ask agent loop. Each row shows the five signal bars (BM25, cosine,
 * recency, node weight, focus proximity), the learned-factor chip, and the
 * final blended score. Rows that match a citation in `refs` get the
 * citation's display label + a click-through link.
 *
 * Mirrors the disclosure pattern from `ReferenceDisclosure` so the styling
 * stays consistent with the rest of the Ask turn. Pure presentational — no
 * IPC, no state beyond expand/collapse.
 */
export function WhyThisAnswerPanel({
  scored,
  refs,
  query,
}: {
  scored: ScoredBrainNode[];
  refs: SearchResult[];
  query?: string | null;
}) {
  const [expanded, setExpanded] = useState(false);

  // Sort by blended score descending so the most-influential rows surface
  // first. Stable sort preserves order when scores tie.
  const sorted = useMemo(
    () => [...scored].sort((a, b) => b.blended_score - a.blended_score),
    [scored],
  );

  const refByEntityId = useMemo(() => {
    const map = new Map<string, SearchResult>();
    for (const ref of refs) map.set(ref.entity_id, ref);
    return map;
  }, [refs]);

  if (sorted.length === 0) return null;

  return (
    <div className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <button
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left"
        onClick={() => setExpanded((value) => !value)}
        type="button"
      >
        <Sparkles size={14} className="text-violet-500" />
        <span className="text-[12px] font-semibold text-zinc-950">Why this answer?</span>
        <span className="flex min-w-0 flex-1 items-center gap-2 overflow-hidden">
          <span className="text-[11px] text-zinc-500">
            {sorted.length} node{sorted.length === 1 ? "" : "s"} scored
          </span>
          {query ? (
            <span
              className="truncate rounded-md bg-violet-50 px-1.5 py-0.5 text-[11px] font-medium text-violet-700"
              title={query}
            >
              {query}
            </span>
          ) : null}
        </span>
        <ChevronDown
          size={15}
          className={["text-zinc-400 transition-transform", expanded ? "rotate-180" : ""].join(" ")}
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
            <div className="divide-y divide-zinc-100">
              {sorted.map((node) => (
                <ScoredNodeRow
                  key={node.node_id}
                  node={node}
                  matchingRef={refByEntityId.get(node.entity_id)}
                />
              ))}
            </div>
            <div className="border-t border-zinc-100 bg-zinc-50/60 px-3 py-2 text-[10.5px] leading-relaxed text-zinc-500">
              <span className="font-semibold uppercase tracking-wider text-zinc-400">
                Signal legend.{" "}
              </span>
              <span className="text-zinc-500">
                BM25 = lexical overlap with the query. Cosine = semantic
                similarity via embeddings. Recency = age of the entity.
                Weight = node importance in the brain graph. Focus = 1.0 if
                the node is the focus anchor. × factor = learned per-entity
                multiplier from `node_importance`.
              </span>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function ScoredNodeRow({
  node,
  matchingRef,
}: {
  node: ScoredBrainNode;
  matchingRef?: SearchResult;
}) {
  const factor = node.learned_factor ?? 1.0;
  const factorBadge =
    factor > 1.05 ? "amplified" : factor < 0.95 ? "dampened" : "neutral";
  const factorTone =
    factorBadge === "amplified"
      ? "bg-emerald-50 text-emerald-700"
      : factorBadge === "dampened"
        ? "bg-amber-50 text-amber-700"
        : "bg-zinc-100 text-zinc-500";

  return (
    <div className="px-3 py-2.5">
      <div className="flex items-baseline gap-2">
        {matchingRef ? (
          <a
            href={`#${matchingRef.route}`}
            className="truncate text-[12px] font-semibold text-zinc-950 hover:text-violet-700"
            title={matchingRef.title}
          >
            {matchingRef.title}
            <ChevronRight size={11} className="ml-0.5 inline -translate-y-px text-zinc-300" />
          </a>
        ) : (
          <span
            className="truncate text-[12px] font-semibold text-zinc-700"
            title={node.entity_id}
          >
            {node.entity_id}
          </span>
        )}
        <span className="shrink-0 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium uppercase tracking-wider text-zinc-500">
          {node.kind}
        </span>
        <span className="ml-auto flex items-center gap-1.5">
          <span className={["rounded-md px-1.5 py-0.5 text-[10px] font-medium", factorTone].join(" ")}>
            × {factor.toFixed(2)}
          </span>
          <span className="rounded-md bg-violet-50 px-1.5 py-0.5 text-[11px] font-semibold text-violet-700">
            {node.blended_score.toFixed(2)}
          </span>
        </span>
      </div>
      <div className="mt-2 grid grid-cols-5 gap-2">
        <SignalBar label="BM25" value={node.bm25_norm} tone="sky" />
        <SignalBar label="Cosine" value={node.cosine} tone="indigo" />
        <SignalBar
          label="Recency"
          value={Math.max(0, Math.min(1, (node.recency_multiplier - 1.0) / 0.5))}
          tone="amber"
          hint={`${node.recency_multiplier.toFixed(2)}×`}
        />
        <SignalBar label="Weight" value={node.node_weight_norm} tone="emerald" />
        <SignalBar label="Focus" value={node.focus_proximity} tone="rose" />
      </div>
    </div>
  );
}

const TONE_CLASSES: Record<string, string> = {
  sky: "bg-sky-400",
  indigo: "bg-indigo-400",
  amber: "bg-amber-400",
  emerald: "bg-emerald-400",
  rose: "bg-rose-400",
};

function SignalBar({
  label,
  value,
  tone,
  hint,
}: {
  label: string;
  value: number;
  tone: string;
  hint?: string;
}) {
  const pct = Math.max(0, Math.min(1, value)) * 100;
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between gap-1 text-[10px] font-medium">
        <span className="text-zinc-500">{label}</span>
        <span className="text-zinc-400">{hint ?? value.toFixed(2)}</span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-zinc-100">
        <div
          className={["h-full rounded-full", TONE_CLASSES[tone] ?? "bg-zinc-300"].join(" ")}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  );
}
