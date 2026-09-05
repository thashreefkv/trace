// Brand mark + per-turn avatars + the inner Trace icon glyph used in
// AskWorkspace. Extracted so submodules (Turn.tsx, AgentMode.tsx, etc.) can
// reuse without depending on the route file directly.

import { motion } from "framer-motion";
import {
  Brain,
  CheckCircle2,
  CircleAlert,
  CircleDot,
  Clock3,
  History,
  ListTodo,
  MessageSquareText,
  Search,
  Sparkles,
  UserRound,
  Wrench,
} from "lucide-react";

import { streamPulse } from "../../lib/motion";
import type { AgentMemoryKind, AskTaskStatus } from "./state";

export function TraceIconContent({ searchSize, sparkleSize }: { searchSize: number; sparkleSize: number }) {
  return (
    <span className="relative flex items-center justify-center">
      <Search size={searchSize} strokeWidth={1.75} />
      <Sparkles
        size={sparkleSize}
        strokeWidth={2.5}
        className="absolute -right-1 -top-1 text-white/90"
      />
    </span>
  );
}

export function Avatar({ tone }: { tone: "user" | "trace" }) {
  if (tone === "user") {
    return (
      <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-zinc-700 to-zinc-950 text-white shadow-sm">
        <UserRound size={15} strokeWidth={2} />
      </div>
    );
  }
  return (
    <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-gradient-to-br from-orange-400 to-orange-600 text-white shadow-sm">
      <TraceIconContent searchSize={13} sparkleSize={8} />
    </div>
  );
}

export function TaskStatusIcon({ status }: { status: AskTaskStatus }) {
  if (status === "completed") {
    return <CheckCircle2 size={15} className="mt-0.5 shrink-0 text-emerald-600" />;
  }
  if (status === "blocked") {
    return <CircleAlert size={15} className="mt-0.5 shrink-0 text-amber-600" />;
  }
  if (status === "running") {
    return (
      <motion.span {...streamPulse} className="mt-0.5 shrink-0">
        <CircleDot size={15} className="text-amber-500" />
      </motion.span>
    );
  }
  return <Clock3 size={15} className="mt-0.5 shrink-0 text-zinc-400" />;
}

export function agentMemoryIcon(kind: AgentMemoryKind) {
  if (kind === "ask") {
    return <Sparkles size={13} />;
  }
  if (kind === "answer") {
    return <MessageSquareText size={13} />;
  }
  if (kind === "tool") {
    return <Wrench size={13} />;
  }
  if (kind === "memory") {
    return <Brain size={13} />;
  }
  if (kind === "task") {
    return <ListTodo size={13} />;
  }
  if (kind === "session") {
    return <History size={13} />;
  }
  if (kind === "clarification") {
    return <UserRound size={13} />;
  }
  return <CircleAlert size={13} />;
}

export function BrandMark({ small = false }: { small?: boolean }) {
  return (
    <span
      className={[
        "inline-flex items-center justify-center rounded-xl bg-gradient-to-br from-orange-400 to-orange-600 text-white shadow-sm",
        small ? "h-5 w-5" : "h-7 w-7",
      ].join(" ")}
    >
      <TraceIconContent searchSize={small ? 10 : 13} sparkleSize={small ? 6 : 8} />
    </span>
  );
}
