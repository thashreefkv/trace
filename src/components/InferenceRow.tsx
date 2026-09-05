import { memo } from "react";
import { ArrowRight, FileText, Hash, Sparkles } from "lucide-react";
import type { BrainInferenceRow } from "../lib/types";

/**
 * Section 6.2 — shared inference row.
 *
 * Renders subject → relation badge → target, plus the rationale, evidence
 * preview, and confidence vs. template threshold. Used by both
 * `InferenceReviewQueue` and `SupersessionLog`.
 */
export const InferenceRowView = memo(function InferenceRowView({
  row,
  trailing,
  muted = false,
}: {
  row: BrainInferenceRow;
  trailing?: React.ReactNode;
  muted?: boolean;
}) {
  const subject = row.subject_label ?? prettyEntityId(row.source_id);
  const target = row.target_label ?? prettyEntityId(row.target_id);
  const relation = row.relation_kind.split("_").join(" ").toLowerCase();
  const confidence = row.confidence;
  const threshold = row.threshold;
  const meetsThreshold = threshold == null ? true : confidence >= threshold;

  return (
    <div
      className={[
        "rounded-xl border p-3",
        muted
          ? "border-zinc-100 bg-zinc-50/60 text-zinc-500"
          : "border-zinc-100 bg-white",
      ].join(" ")}
    >
      <div className="flex flex-wrap items-center gap-2">
        <EntityChip kind={row.source_kind} label={subject} />
        <span className="rounded-md bg-violet-50 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider text-violet-700">
          {relation}
        </span>
        <ArrowRight size={12} className="text-zinc-300" />
        <EntityChip kind={row.target_kind} label={target} />
        {row.template ? (
          <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
            {row.template}
          </span>
        ) : null}
        <div className="ml-auto flex items-center gap-2">
          <ConfidenceBar
            confidence={confidence}
            threshold={threshold}
            highlight={meetsThreshold}
          />
          {trailing}
        </div>
      </div>
      {row.rationale ? (
        <p className="mt-2 text-[12px] leading-snug text-zinc-600">{row.rationale}</p>
      ) : null}
      {row.evidence_json && row.evidence_json !== "{}" ? (
        <EvidencePreview evidenceJson={row.evidence_json} />
      ) : null}
    </div>
  );
});

function EntityChip({ kind, label }: { kind: string; label: string }) {
  return (
    <span
      className="inline-flex max-w-[14rem] items-center gap-1 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[11px] font-medium text-zinc-700"
      title={label}
    >
      <Hash size={10} className="shrink-0 text-zinc-400" />
      <span className="truncate">{label}</span>
      <span className="shrink-0 text-[9px] uppercase tracking-wider text-zinc-400">
        {kind.split("_").join(" ")}
      </span>
    </span>
  );
}

function ConfidenceBar({
  confidence,
  threshold,
  highlight,
}: {
  confidence: number;
  threshold: number | null;
  highlight: boolean;
}) {
  const pct = Math.max(0, Math.min(1, confidence)) * 100;
  const thresholdPct =
    threshold == null ? null : Math.max(0, Math.min(1, threshold)) * 100;
  return (
    <div className="flex w-32 flex-col gap-0.5" title={`Confidence ${(confidence * 100).toFixed(0)}%`}>
      <div className="relative h-1.5 overflow-hidden rounded-full bg-zinc-100">
        <div
          className={[
            "h-full rounded-full",
            highlight ? "bg-violet-400" : "bg-amber-300",
          ].join(" ")}
          style={{ width: `${pct}%` }}
        />
        {thresholdPct != null ? (
          <span
            className="absolute top-0 h-full w-px bg-zinc-700"
            style={{ left: `${thresholdPct}%` }}
            title={`Threshold ${(threshold ?? 0).toFixed(2)}`}
          />
        ) : null}
      </div>
      <div className="flex justify-between text-[10px] font-medium text-zinc-400">
        <span>{(confidence * 100).toFixed(0)}%</span>
        {threshold != null ? (
          <span title="Current template threshold">τ {(threshold * 100).toFixed(0)}%</span>
        ) : null}
      </div>
    </div>
  );
}

function EvidencePreview({ evidenceJson }: { evidenceJson: string }) {
  let parsed: Record<string, unknown> | null = null;
  try {
    parsed = JSON.parse(evidenceJson) as Record<string, unknown>;
  } catch {
    parsed = null;
  }
  if (!parsed) return null;
  const entries = Object.entries(parsed).filter(
    ([key, value]) =>
      key !== "source" && typeof value === "string" && value.trim().length > 0,
  ) as [string, string][];
  if (entries.length === 0) return null;
  return (
    <div className="mt-2 flex flex-wrap gap-1 text-[11px] text-zinc-500">
      <Sparkles size={11} className="mt-0.5 text-zinc-300" />
      {entries.slice(0, 3).map(([key, value]) => (
        <span
          key={key}
          className="rounded-md bg-zinc-50 px-1.5 py-0.5"
          title={value}
        >
          <span className="font-semibold text-zinc-400">{key}:</span>{" "}
          <span className="text-zinc-600">{truncate(value, 100)}</span>
        </span>
      ))}
    </div>
  );
}

function truncate(text: string, max: number) {
  if (text.length <= max) return text;
  return `${text.slice(0, max - 1)}…`;
}

function prettyEntityId(id: string) {
  // Strip the ulid-y suffix when present so the row reads cleaner. Falls
  // back to the raw id when the heuristic doesn't apply.
  if (id.length > 28 && id.includes("_")) {
    const parts = id.split("_");
    return parts.slice(0, -1).join("_");
  }
  return id;
}

// Suppress unused-import warning when consumers don't need the FileText icon —
// we keep the import so future evidence types can render it inline.
const _filetextRef: typeof FileText = FileText;
void _filetextRef;
