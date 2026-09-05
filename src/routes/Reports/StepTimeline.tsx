import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertCircle,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Clock,
  HelpCircle,
  Loader2,
  RotateCcw,
  XCircle,
} from "lucide-react";

import { answerReportClarification, rerunReportStep } from "../../lib/ipc";
import type { ReportStep } from "../../lib/types";
import { ClarifyPrompt } from "../../components/ClarifyPrompt";

const STATUS_ICON: Record<string, React.ReactNode> = {
  queued: <Clock className="text-zinc-400" size={14} />,
  running: <Loader2 className="animate-spin text-violet-500" size={14} />,
  awaiting_clarification: <HelpCircle className="text-amber-500" size={14} />,
  done: <CheckCircle2 className="text-emerald-500" size={14} />,
  error: <AlertCircle className="text-red-500" size={14} />,
  cancelled: <XCircle className="text-zinc-400" size={14} />,
};

const STEP_LABEL: Record<string, string> = {
  resolve_scope: "Resolve scope",
  plan_sections: "Plan sections",
  draft_section: "Draft section",
  critique: "Critique",
};

interface Props {
  reportRunId: string;
  steps: ReportStep[];
}

export function StepTimeline({ reportRunId, steps }: Props) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());

  function toggle(id: string) {
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }

  if (steps.length === 0) {
    return (
      <p className="px-3 py-6 text-center text-xs text-zinc-400">
        No steps yet. Run the pipeline to begin.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-1 pb-2">
      {steps.map((step) => (
        <StepRow
          expanded={expanded.has(step.id)}
          key={step.id}
          reportRunId={reportRunId}
          step={step}
          onToggle={() => toggle(step.id)}
        />
      ))}
    </div>
  );
}

function StepRow({
  step,
  expanded,
  reportRunId,
  onToggle,
}: {
  step: ReportStep;
  expanded: boolean;
  reportRunId: string;
  onToggle: () => void;
}) {
  const icon = STATUS_ICON[step.status] ?? <Clock className="text-zinc-400" size={14} />;
  const label =
    step.step_name === "draft_section" && step.section_index !== null
      ? `Draft section ${step.section_index + 1}`
      : (STEP_LABEL[step.step_name] ?? step.step_name);

  const clarification = parseClarification(step.clarification_json);

  async function handleClarificationAnswer(selectedLabels: string[], freeText?: string) {
    await answerReportClarification(step.id, selectedLabels, freeText);
  }

  async function handleRerun() {
    await rerunReportStep(
      reportRunId,
      step.step_name,
      step.step_name === "draft_section" ? String(step.section_index) : undefined,
    );
  }

  return (
    <div
      className={[
        "rounded-xl border transition-colors",
        step.status === "running"
          ? "border-violet-200 bg-violet-50/40 ring-1 ring-violet-100"
          : "border-zinc-100 bg-zinc-50",
      ].join(" ")}
    >
      <button
        className="flex w-full items-center gap-2 px-3 py-3 text-left"
        onClick={onToggle}
        type="button"
      >
        <span className="shrink-0">{icon}</span>
        <span className="min-w-0 flex-1">
          <span className="truncate text-xs font-medium text-zinc-800">{label}</span>
          {step.ticker_label && step.status === "running" ? (
            <span className="ml-2 text-[10px] text-violet-500">{step.ticker_label}</span>
          ) : null}
        </span>
        <span className="flex shrink-0 items-center gap-1.5">
          {step.latency_ms ? (
            <span className="text-[10px] text-zinc-400">
              {(step.latency_ms / 1000).toFixed(1)} s
            </span>
          ) : null}
          {step.model ? (
            <span className="rounded bg-zinc-200 px-1.5 py-0.5 text-[9px] font-mono text-zinc-600">
              {modelBadge(step.model)}
            </span>
          ) : null}
          {step.cache_hit ? (
            <span className="rounded bg-emerald-100 px-1.5 py-0.5 text-[9px] text-emerald-700">
              cached
            </span>
          ) : null}
          {expanded ? (
            <ChevronDown className="text-zinc-400" size={12} />
          ) : (
            <ChevronRight className="text-zinc-400" size={12} />
          )}
        </span>
      </button>

      <AnimatePresence>
        {expanded ? (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            className="overflow-hidden border-t border-zinc-100"
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.18 }}
          >
            <div className="space-y-3 px-3 py-3">
              {step.status === "awaiting_clarification" && clarification ? (
                <ClarifyPrompt
                  freeTextAllowed={clarification.free_text_allowed}
                  onAnswer={(labels, text) => void handleClarificationAnswer(labels, text)}
                  options={clarification.options}
                  question={clarification.question}
                />
              ) : null}

              {step.error_text ? (
                <div className="rounded-xl border border-red-100 bg-red-50 px-3 py-2">
                  <p className="text-xs text-red-700">{step.error_text}</p>
                </div>
              ) : null}

              <details className="group">
                <summary className="cursor-pointer select-none text-[10px] text-zinc-400 hover:text-zinc-600">
                  Input / Output JSON
                </summary>
                <div className="mt-2 space-y-1.5">
                  <pre className="max-h-36 overflow-auto rounded-xl border border-zinc-100 bg-zinc-100 p-3 text-[10px] leading-relaxed text-zinc-600">
                    {formatJson(step.input_json)}
                  </pre>
                  <pre className="max-h-36 overflow-auto rounded-xl border border-zinc-100 bg-zinc-100 p-3 text-[10px] leading-relaxed text-zinc-600">
                    {formatJson(step.output_json)}
                  </pre>
                </div>
              </details>

              {["done", "error", "cancelled"].includes(step.status) ? (
                <div className="flex gap-1.5">
                  <button
                    className="inline-flex h-7 items-center gap-1 rounded-lg border border-zinc-200 px-2.5 text-[11px] text-zinc-600 transition-colors hover:bg-white"
                    onClick={() => void handleRerun()}
                    type="button"
                  >
                    <RotateCcw size={11} /> Re-run
                  </button>
                  <button
                    className="inline-flex h-7 items-center gap-1 rounded-lg border border-zinc-200 px-2.5 text-[11px] text-zinc-600 transition-colors hover:bg-white"
                    onClick={() =>
                      rerunReportStep(
                        reportRunId,
                        step.step_name,
                        step.step_name === "draft_section"
                          ? String(step.section_index)
                          : undefined,
                        true,
                      )
                    }
                    type="button"
                  >
                    Re-run (bypass cache)
                  </button>
                </div>
              ) : null}
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

interface ParsedClarification {
  question: string;
  options: Array<{ label: string; description?: string }>;
  free_text_allowed: boolean;
}

function parseClarification(raw: string | null): ParsedClarification | null {
  if (!raw) return null;
  try {
    return JSON.parse(raw) as ParsedClarification;
  } catch {
    return null;
  }
}

function formatJson(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
}

function modelBadge(model: string): string {
  return model.replace("gemini-", "").replace("-preview", "");
}
