// Empty-state hero, agent-mode toggle, and the composer assist popover that
// surfaces slash commands / @mention targets / recent prompts. Extracted from
// AskWorkspace.tsx (E8).

import { AtSign, Command, History, Network } from "lucide-react";

import {
  AGENT_MODES,
  COMPOSER_COMMANDS,
  MENTION_TARGETS,
  SAMPLE_PROMPTS,
} from "./constants";
import { findActiveMention } from "./utils";
import { TraceIconContent } from "./icons";
import type { AgentMode, ReasoningDepth } from "./state";

export function EmptyState({ onPrompt }: { onPrompt: (prompt: string) => Promise<void> }) {
  return (
    <div className="mx-auto flex min-h-[52vh] max-w-2xl flex-col items-center justify-center py-14 text-center">
      <div className="mb-4 flex h-14 w-14 items-center justify-center rounded-2xl bg-gradient-to-br from-orange-400 to-orange-600 text-white shadow-sm">
        <TraceIconContent searchSize={26} sparkleSize={14} />
      </div>
      <h2 className="text-2xl font-semibold tracking-tight text-zinc-950">
        How can Trace help?
      </h2>
      <p className="mt-2 max-w-md text-sm leading-6 text-zinc-500">
        Research your workspace, recall memory, run tools, and get source-backed answers.
      </p>
      <div className="mt-6 flex flex-wrap justify-center gap-2">
        {SAMPLE_PROMPTS.map((prompt) => (
          <button
            className="rounded-xl border border-zinc-100 bg-white px-3.5 py-2 text-[12px] font-medium text-zinc-600 shadow-sm transition-all duration-150 hover:border-zinc-100 hover:shadow-[0_4px_20px_rgba(0,0,0,0.09)] hover:text-zinc-900"
            key={prompt}
            onClick={() => void onPrompt(prompt)}
            type="button"
          >
            {prompt}
          </button>
        ))}
      </div>
    </div>
  );
}

export function AgentModeSelector({
  mode,
  onModeChange,
}: {
  mode: AgentMode;
  onModeChange: (mode: AgentMode) => void;
}) {
  return (
    <div
      aria-label="Agent mode"
      className="inline-flex rounded-lg border border-zinc-100 bg-zinc-50 p-0.5"
      role="group"
    >
      {AGENT_MODES.map((item) => (
        <button
          className={[
            "inline-flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[11px] font-semibold transition-colors",
            mode === item.key
              ? "bg-white text-zinc-900 shadow-sm"
              : "text-zinc-400 hover:text-zinc-600",
          ].join(" ")}
          key={item.key}
          onClick={() => onModeChange(item.key)}
          title={item.title}
          type="button"
        >
          {item.icon}
          {item.label}
        </button>
      ))}
    </div>
  );
}

export function DeepReasoningToggle({
  depth,
  onChange,
}: {
  depth: ReasoningDepth;
  onChange: (depth: ReasoningDepth) => void;
}) {
  const enabled = depth === "deep";
  return (
    <button
      aria-pressed={enabled}
      className={[
        "ml-2 inline-flex h-8 items-center gap-1.5 rounded-lg border px-2.5 text-[11px] font-semibold transition-colors",
        enabled
          ? "border-violet-200 bg-violet-50 text-violet-700"
          : "border-zinc-100 bg-zinc-50 text-zinc-400 hover:text-zinc-600",
      ].join(" ")}
      onClick={() => onChange(enabled ? "standard" : "deep")}
      title="Use Gemini Pro reasoning with cited graph evidence and review-gated assertions"
      type="button"
    >
      <Network size={12} />
      Deep reasoning
    </button>
  );
}

export function ComposerAssist({
  input,
  onCommand,
  onMention,
  promptHistory,
}: {
  input: string;
  onCommand: (name: string) => void;
  onMention: (token: string) => void;
  promptHistory: string[];
}) {
  const trimmed = input.trim();
  const activeMention = findActiveMention(input);

  if (trimmed.startsWith("/") && !/\s/.test(trimmed.slice(1))) {
    const query = trimmed.slice(1).toLowerCase();
    const commands = COMPOSER_COMMANDS.filter((command) =>
      command.name.includes(query) ||
      command.description.toLowerCase().includes(query) ||
      command.badge.includes(query),
    ).slice(0, 7);

    return (
      <div className="mb-2 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
        <div className="flex items-center gap-2 border-b border-zinc-100 px-3 py-2 text-[11px] font-semibold uppercase tracking-widest text-zinc-400">
          <Command size={12} />
          Commands
        </div>
        {commands.map((command) => (
          <button
            className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-white"
            key={command.name}
            onClick={() => onCommand(command.name)}
            type="button"
          >
            <span className="w-24 shrink-0 font-mono text-[12px] font-semibold text-zinc-950">
              {command.label}
            </span>
            <span className="rounded-full bg-white px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-zinc-400">
              {command.badge}
            </span>
            <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-500">{command.description}</span>
          </button>
        ))}
      </div>
    );
  }

  if (activeMention) {
    const query = activeMention.query.toLowerCase();
    const targets = MENTION_TARGETS.filter((target) =>
      target.token.toLowerCase().includes(query) ||
      target.label.toLowerCase().includes(query) ||
      target.description.toLowerCase().includes(query),
    ).slice(0, 7);

    return (
      <div className="mb-2 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
        <div className="flex items-center gap-2 border-b border-zinc-100 px-3 py-2 text-[11px] font-semibold uppercase tracking-widest text-zinc-400">
          <AtSign size={12} />
          Mentions
        </div>
        {targets.map((target) => (
          <button
            className="flex w-full items-center gap-2 px-3 py-2 text-left transition-colors hover:bg-white"
            key={target.token}
            onClick={() => onMention(target.token)}
            type="button"
          >
            <span className="text-zinc-500">{target.icon}</span>
            <span className="w-24 shrink-0 font-mono text-[12px] font-semibold text-zinc-950">
              {target.token}
            </span>
            <span className="min-w-0 flex-1 truncate text-[12px] text-zinc-500">{target.description}</span>
          </button>
        ))}
      </div>
    );
  }

  if (!input.trim() && promptHistory.length > 0) {
    return (
      <div className="mb-2 flex items-center gap-2 rounded-xl border border-zinc-100 bg-zinc-50 px-3 py-2 text-[11px] text-zinc-400">
        <History size={12} />
        Press Up to reuse: <span className="min-w-0 flex-1 truncate font-medium text-zinc-600">{promptHistory[0]}</span>
      </div>
    );
  }

  return null;
}
