// Tool-call surface inside an Ask turn: status icon row, confirmation prompt
// for destructive tool calls, expandable reasoning panel. Extracted from
// AskWorkspace.tsx (E4).

import { useMemo, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Brain,
  CheckCircle2,
  ChevronDown,
  CircleAlert,
  ShieldAlert,
  Sparkles,
  X,
} from "lucide-react";

import { confirmAskToolDecision } from "../../lib/ipc";
import { AGENT_MODES, TOOL_SPECS } from "./constants";
import type { AgentMode, AskStep, AskTurn } from "./state";

export function ToolSummary({
  mode,
  steps,
  status,
  autoConfirmTools,
  onToggleAutoConfirm,
}: {
  mode: AgentMode;
  steps: AskStep[];
  status: AskTurn["status"];
  autoConfirmTools: string[];
  onToggleAutoConfirm: (tool: string, enabled: boolean) => void;
}) {
  const inProgress = status === "running" || status === "streaming";
  const hasAwaiting = useMemo(
    () => steps.some((step) => step.status === "awaiting"),
    [steps],
  );
  const [expanded, setExpanded] = useState(false);
  const showExpanded = expanded || hasAwaiting;
  const grouped = useMemo(() => groupSteps(steps), [steps]);
  const latestRunning = useMemo(
    () => [...steps].reverse().find((step) => step.status === "running"),
    [steps],
  );
  const label = AGENT_MODES.find((item) => item.key === mode)?.label ?? "Research";

  if (steps.length === 0 && !inProgress) {
    return null;
  }

  return (
    <div className="rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <button
        className="flex w-full items-center gap-3 px-3 py-2.5 text-left"
        onClick={() => setExpanded((v) => !v)}
        type="button"
      >
        <ToolStatusIcon running={inProgress && Boolean(latestRunning)} />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[12px] font-semibold text-zinc-950">
            {inProgress && latestRunning
              ? latestRunning.label
              : inProgress
              ? `Starting ${label.toLowerCase()} run`
              : `Activity · ${steps.length} tool ${steps.length === 1 ? "use" : "uses"}`}
          </p>
          <p className="truncate text-[11px] text-zinc-400">
            {grouped.map((step) => `${step.label}${step.count > 1 ? ` x${step.count}` : ""}`).join(" · ") || "Preparing"}
          </p>
        </div>
        <ChevronDown
          size={15}
          className={["text-zinc-400 transition-transform", showExpanded ? "rotate-180" : ""].join(" ")}
        />
      </button>
      <AnimatePresence initial={false}>
        {showExpanded ? (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            className="overflow-hidden border-t border-zinc-100"
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
          >
            <div className="max-h-72 space-y-1.5 overflow-y-auto bg-zinc-50 px-3 py-2.5">
              {steps.length === 0 ? (
                <ToolLine label="Starting agent loop" status="running" />
              ) : (
                steps.map((step) => (
                  <ToolLine
                    icon={toolIcon(step.tool)}
                    key={step.id}
                    label={step.label}
                    rationale={step.rationale}
                    summary={step.summary}
                    status={step.status}
                    tool={step.tool}
                    runId={step.runId}
                    callId={step.callId}
                    riskReason={step.riskReason}
                    argsPreview={step.argsPreview}
                    autoConfirmTools={autoConfirmTools}
                    onToggleAutoConfirm={onToggleAutoConfirm}
                  />
                ))
              )}
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

function ToolLine({
  icon,
  label,
  rationale,
  summary,
  status,
  tool,
  runId,
  callId,
  riskReason,
  argsPreview,
  autoConfirmTools,
  onToggleAutoConfirm,
}: {
  icon?: ReactNode;
  label: string;
  rationale?: string | null;
  summary?: string | null;
  status: AskStep["status"];
  tool?: string;
  runId?: string;
  callId?: string;
  riskReason?: string | null;
  argsPreview?: string | null;
  autoConfirmTools?: string[];
  onToggleAutoConfirm?: (tool: string, enabled: boolean) => void;
}) {
  if (status === "awaiting" && tool && runId && callId) {
    return (
      <ToolConfirmRow
        icon={icon}
        label={label}
        summary={summary}
        tool={tool}
        runId={runId}
        callId={callId}
        riskReason={riskReason}
        argsPreview={argsPreview}
        autoConfirmTools={autoConfirmTools ?? []}
        onToggleAutoConfirm={onToggleAutoConfirm}
      />
    );
  }
  return (
    <div className="flex items-start gap-2 text-[12px]">
      <ToolStatusIcon running={status === "running"} error={status === "error" || status === "denied"} small />
      <span className="mt-0.5 text-violet-400">{icon ?? <Sparkles size={12} />}</span>
      <div className="min-w-0 flex-1">
        <span
          className={[
            "block truncate",
            status === "error" ? "text-red-600" : status === "denied" ? "text-zinc-400" : "text-zinc-600",
          ].join(" ")}
        >
          {label}
          {status === "denied" ? <span className="ml-1 text-zinc-400">· rejected</span> : null}
        </span>
        {rationale ? <span className="block text-[11px] text-zinc-400">{rationale}</span> : null}
        {summary && status !== "running" ? (
          <span className="block text-[11px] text-zinc-400">→ {summary}</span>
        ) : null}
      </div>
    </div>
  );
}

function ToolConfirmRow({
  icon,
  label,
  summary,
  tool,
  runId,
  callId,
  riskReason,
  argsPreview,
  autoConfirmTools,
  onToggleAutoConfirm,
}: {
  icon?: ReactNode;
  label: string;
  summary?: string | null;
  tool: string;
  runId: string;
  callId: string;
  riskReason?: string | null;
  argsPreview?: string | null;
  autoConfirmTools: string[];
  onToggleAutoConfirm?: (tool: string, enabled: boolean) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [alwaysAllow, setAlwaysAllow] = useState(false);
  const respond = async (approved: boolean) => {
    if (busy) return;
    setBusy(true);
    try {
      if (approved && alwaysAllow && onToggleAutoConfirm) {
        onToggleAutoConfirm(tool, true);
      }
      await confirmAskToolDecision(runId, callId, approved);
    } catch (e) {
      void e;
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="rounded-xl border border-amber-200 bg-amber-50 p-3 text-[12px]">
      <div className="mb-1 flex items-center gap-2">
        <ShieldAlert size={13} className="text-amber-600" />
        <span className="font-semibold text-amber-900">Confirm destructive tool</span>
        <span className="ml-auto text-[10px] text-amber-700">{riskReason ?? "destructive"}</span>
      </div>
      <div className="mb-2 flex items-start gap-2">
        <span className="mt-0.5 text-violet-400">{icon ?? <Sparkles size={12} />}</span>
        <div className="min-w-0">
          <p className="truncate font-medium text-zinc-900">{summary ?? label}</p>
          {argsPreview ? (
            <p className="line-clamp-2 text-[11px] text-zinc-500">{argsPreview}</p>
          ) : null}
        </div>
      </div>
      <div className="flex items-center gap-2">
        <button
          className="btn btn-primary h-7 px-3 text-xs"
          disabled={busy}
          onClick={() => void respond(true)}
          type="button"
        >
          <CheckCircle2 size={12} />
          Allow
        </button>
        <button
          className="btn h-7 px-3 text-xs"
          disabled={busy}
          onClick={() => void respond(false)}
          type="button"
        >
          <X size={12} />
          Reject
        </button>
        <label className="ml-auto flex items-center gap-1.5 text-[11px] text-zinc-600">
          <input
            checked={alwaysAllow}
            className="h-3 w-3 accent-zinc-900"
            disabled={busy || !onToggleAutoConfirm}
            onChange={(e) => setAlwaysAllow(e.target.checked)}
            type="checkbox"
          />
          Always allow this tool
        </label>
      </div>
      {onToggleAutoConfirm && autoConfirmTools.includes(tool) ? (
        <p className="mt-2 text-[10px] text-amber-700">
          Currently on the auto-allow list — remove it in Settings to re-prompt next time.
        </p>
      ) : null}
    </div>
  );
}

export function ReasoningPanel({ reasoning }: { reasoning: string }) {
  const [open, setOpen] = useState(false);
  if (!reasoning.trim()) return null;
  return (
    <div className="rounded-2xl border border-dashed border-violet-100 bg-violet-50/40">
      <button
        className="flex w-full items-center gap-2 px-3 py-2 text-[12px] text-zinc-400"
        onClick={() => setOpen((value) => !value)}
        type="button"
      >
        <Brain size={13} className="text-violet-400" />
        <span className="flex-1 text-left">Reasoning</span>
        <ChevronDown
          size={13}
          className={["transition-transform", open ? "rotate-180" : ""].join(" ")}
        />
      </button>
      {open ? (
        <pre className="max-h-48 overflow-y-auto whitespace-pre-wrap border-t border-violet-100 px-3 py-2 font-sans text-[12px] leading-5 text-zinc-600">
          {reasoning.trim()}
        </pre>
      ) : null}
    </div>
  );
}

export function ToolStatusIcon({
  running,
  error = false,
  small = false,
}: {
  running: boolean;
  error?: boolean;
  small?: boolean;
}) {
  const h = small ? 10 : 13;
  if (running) {
    return (
      <span className="flex items-end gap-[2px]" style={{ height: h }}>
        {[0, 1, 2].map((i) => (
          <motion.span
            key={i}
            animate={{ scaleY: [0.25, 1, 0.25] }}
            className="w-[3px] rounded-full bg-violet-500"
            style={{ height: h, originY: 1 }}
            transition={{ delay: i * 0.18, duration: 0.65, ease: "easeInOut", repeat: Infinity }}
          />
        ))}
      </span>
    );
  }
  if (error) {
    return <CircleAlert size={small ? 11 : 13} className="text-red-600" />;
  }
  return (
    <span className="flex items-end gap-[2px]" style={{ height: h }}>
      {[0.45, 1, 0.65].map((scale, i) => (
        <span
          key={i}
          className="w-[3px] rounded-full bg-emerald-500"
          style={{ height: h * scale }}
        />
      ))}
    </span>
  );
}

export function toolIcon(toolName: string) {
  return TOOL_SPECS.find((tool) => tool.name === toolName)?.icon ?? <Sparkles size={12} />;
}

export function groupSteps(steps: AskStep[]) {
  const groups: { key: string; label: string; count: number }[] = [];
  for (const step of steps) {
    const existing = groups.find((group) => group.key === `${step.tool}:${step.label}`);
    if (existing) {
      existing.count += 1;
    } else {
      groups.push({ key: `${step.tool}:${step.label}`, label: step.label, count: 1 });
    }
  }
  return groups;
}
