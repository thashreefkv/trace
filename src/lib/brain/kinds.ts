import type { BrainTemplateKind, MemoryKind } from "../types";

export const baseKindLabels: Record<string, string> = {
  initiative: "Initiatives",
  deliverable: "Deliverables",
  stakeholder: "Stakeholders",
  conversation: "Conversations",
  capture: "Captures",
  memory: "Memory",
  task: "Tasks",
  note: "Notes",
  label: "Labels",
  meeting: "Meetings",
  meeting_action: "Actions",
  initiative_note: "Initiative notes",
  week_day: "Week plan",
  email_thread: "Email threads",
  email_participant: "Email people",
  email_draft: "Drafts",
  email_suggestion: "Email suggestions",
  ask_chat: "Ask chats",
  work_intake_suggestion: "Work intake",
  blocker: "Blockers",
  file: "Files",
};

export interface KindPalette {
  fill: string;
  stroke: string;
  text: string;
}

export const baseKindColors: Record<string, KindPalette> = {
  initiative: { fill: "#e0f2fe", stroke: "#0284c7", text: "#075985" },
  deliverable: { fill: "#ecfdf5", stroke: "#059669", text: "#065f46" },
  stakeholder: { fill: "#fff7ed", stroke: "#ea580c", text: "#9a3412" },
  conversation: { fill: "#f5f3ff", stroke: "#7c3aed", text: "#5b21b6" },
  capture: { fill: "#fefce8", stroke: "#ca8a04", text: "#854d0e" },
  memory: { fill: "#ccfbf1", stroke: "#0f766e", text: "#115e59" },
  task: { fill: "#f0fdf4", stroke: "#16a34a", text: "#166534" },
  note: { fill: "#fafafa", stroke: "#737373", text: "#404040" },
  meeting: { fill: "#fef2f2", stroke: "#dc2626", text: "#991b1b" },
  meeting_action: { fill: "#fff1f2", stroke: "#e11d48", text: "#9f1239" },
  email_thread: { fill: "#eff6ff", stroke: "#2563eb", text: "#1e40af" },
  email_participant: { fill: "#f0f9ff", stroke: "#0ea5e9", text: "#0369a1" },
  ask_chat: { fill: "#eef2ff", stroke: "#4f46e5", text: "#3730a3" },
  blocker: { fill: "#fee2e2", stroke: "#ef4444", text: "#991b1b" },
  file: { fill: "#fff7ed", stroke: "#f97316", text: "#9a3412" },
};

export const fallbackKindColors: KindPalette[] = [
  { fill: "#f0fdfa", stroke: "#0d9488", text: "#115e59" },
  { fill: "#fdf4ff", stroke: "#c026d3", text: "#86198f" },
  { fill: "#f8fafc", stroke: "#64748b", text: "#334155" },
  { fill: "#fefce8", stroke: "#ca8a04", text: "#854d0e" },
  { fill: "#fff7ed", stroke: "#f97316", text: "#9a3412" },
];

export function colorForKind(kind: string): KindPalette {
  const direct = baseKindColors[kind];
  if (direct) return direct;
  let h = 0;
  for (let i = 0; i < kind.length; i += 1) {
    h = (h * 31 + kind.charCodeAt(i)) >>> 0;
  }
  return fallbackKindColors[h % fallbackKindColors.length] ?? fallbackKindColors[0];
}

export function labelForKind(kind: string): string {
  return baseKindLabels[kind] ?? kind.replace(/_/g, " ");
}

export const memoryKindLabels: Record<MemoryKind, string> = {
  episodic: "Episodic",
  semantic: "Semantic",
  procedural: "Procedural",
};

export const brainTemplateOptions: Array<{ value: BrainTemplateKind; label: string }> = [
  { value: "focus_today", label: "Focus today" },
  { value: "blocked_work", label: "Blocked" },
  { value: "email_followups", label: "Follow-ups" },
  { value: "stale_work", label: "Stale work" },
  { value: "stakeholder_context", label: "Stakeholders" },
];

export const KNOWN_RELATION_LABELS: Record<string, string> = {
  owns: "owns",
  belongs_to: "in",
  about: "about",
  references: "references",
  related: "related",
  participant: "participant",
  thread_initiative: "in initiative",
  thread_deliverable: "about deliverable",
  thread_stakeholder: "with",
  promoted_from: "promoted from",
  inferred: "inferred",
  followup: "follow-up",
  blocks: "blocks",
  blocked_by: "blocked by",
};
