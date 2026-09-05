// Pure helpers extracted from AskWorkspace.tsx: storage loaders, turn-tree
// navigation, type guards, server↔local converters, formatters, and small
// utilities. Nothing in here renders or hooks into React — keep it that way so
// components can call freely without provoking re-renders.

import type { AskChatRecord, AskTurnRecord, SearchResult } from "../../lib/types";
import {
  ASK_ACTIVE_CHAT_ID_KEY,
  ASK_AGENT_MEMORY_EVENTS_KEY,
  ASK_AGENT_MODE_KEY,
  ASK_REASONING_DEPTH_STORAGE_ID,
  ASK_AUTO_CONFIRM_TOOLS_KEY,
  ASK_CHAT_SESSIONS_KEY,
  ASK_GENERATE_MEMORY_ENABLED_KEY,
  ASK_HISTORY_KEY,
  ASK_MEMORY_ENTRIES_KEY,
  ASK_PERMISSION_MODE_KEY,
  ASK_PREFERRED_CHILD_STORAGE_ID,
  ASK_PROJECT_MEMORY_KEY,
  ASK_SEARCH_CHATS_ENABLED_KEY,
  ASK_TASKS_KEY,
  ASK_TURNS_KEY,
  type AgentMemoryEvent,
  type AgentMemoryKind,
  type AgentMode,
  type AskChatSession,
  type AskStep,
  type AskTask,
  type AskTaskStatus,
  type AskTurn,
  type ReasoningDepth,
  type DrawerKind,
  type MemoryEntry,
  type MemoryType,
  type PermissionMode,
} from "./state";
import {
  AGENT_MODES,
  AGENT_MODE_CONTEXT,
  MENTION_TARGETS,
  PERMISSION_MODE_CONTEXT,
} from "./constants";

// ── Storage loaders ──────────────────────────────────────────────────────────

export function loadStoredTurns(): AskTurn[] {
  try {
    const raw = sessionStorage.getItem(ASK_TURNS_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as AskTurn[];
    if (!Array.isArray(parsed)) return [];
    return migrateTurnChain(parsed);
  } catch {
    return [];
  }
}

export function loadPreferredChild(): Record<string, string> {
  try {
    const raw = sessionStorage.getItem(ASK_PREFERRED_CHILD_STORAGE_ID);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as Record<string, string>;
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    return {};
  }
}

export function loadProjectMemory() {
  return localStorage.getItem(ASK_PROJECT_MEMORY_KEY) ?? "";
}

export function loadMemoryEntries(): MemoryEntry[] {
  try {
    const raw = localStorage.getItem(ASK_MEMORY_ENTRIES_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as MemoryEntry[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(isMemoryEntry).slice(0, 80);
  } catch {
    return [];
  }
}

export function loadAskTasks(): AskTask[] {
  try {
    const raw = localStorage.getItem(ASK_TASKS_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as AskTask[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(isAskTask).slice(0, 100);
  } catch {
    return [];
  }
}

export function loadPromptHistory(): string[] {
  try {
    const raw = localStorage.getItem(ASK_HISTORY_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as string[];
    return Array.isArray(parsed) ? parsed.filter((item) => typeof item === "string").slice(0, 60) : [];
  } catch {
    return [];
  }
}

export function loadChatSessions(): AskChatSession[] {
  try {
    const raw = localStorage.getItem(ASK_CHAT_SESSIONS_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as AskChatSession[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(isAskChatSession).slice(0, 60);
  } catch {
    return [];
  }
}

export function loadActiveChatId() {
  return localStorage.getItem(ASK_ACTIVE_CHAT_ID_KEY) || makeLocalId("chat");
}

export function loadAgentMemoryEvents(): AgentMemoryEvent[] {
  try {
    const raw = localStorage.getItem(ASK_AGENT_MEMORY_EVENTS_KEY);
    if (!raw) {
      return [];
    }
    const parsed = JSON.parse(raw) as AgentMemoryEvent[];
    if (!Array.isArray(parsed)) {
      return [];
    }
    return parsed.filter(isAgentMemoryEvent).slice(0, 300);
  } catch {
    return [];
  }
}

export function loadSearchChatsEnabled() {
  return localStorage.getItem(ASK_SEARCH_CHATS_ENABLED_KEY) !== "false";
}

export function loadGenerateMemoryEnabled() {
  return localStorage.getItem(ASK_GENERATE_MEMORY_ENABLED_KEY) !== "false";
}

export function loadAgentMode(): AgentMode {
  const raw = localStorage.getItem(ASK_AGENT_MODE_KEY);
  return raw === "ask" || raw === "research" || raw === "act" ? raw : "research";
}

export function loadReasoningDepth(): ReasoningDepth {
  return localStorage.getItem(ASK_REASONING_DEPTH_STORAGE_ID) === "deep" ? "deep" : "standard";
}

export function loadPermissionMode(): PermissionMode {
  const raw = localStorage.getItem(ASK_PERMISSION_MODE_KEY);
  return raw === "auto_read" || raw === "auto_safe" || raw === "confirm" ? raw : "confirm";
}

export function loadAutoConfirmTools(): string[] {
  try {
    const raw = localStorage.getItem(ASK_AUTO_CONFIRM_TOOLS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === "string") : [];
  } catch {
    return [];
  }
}

// ── Turn-tree navigation ─────────────────────────────────────────────────────

/** Walk parentId pointers from `leafId` back to root, returning ancestors in order. */
export function collectAncestors(turns: AskTurn[], leafId: string | null): AskTurn[] {
  if (!leafId) return [];
  const byId = new Map(turns.map((turn) => [turn.id, turn]));
  const chain: AskTurn[] = [];
  const guard = new Set<string>();
  let cursor: string | null = leafId;
  while (cursor) {
    const turn = byId.get(cursor);
    if (!turn || guard.has(turn.id)) break;
    guard.add(turn.id);
    chain.push(turn);
    cursor = turn.parentId ?? null;
  }
  return chain.reverse();
}

/** Build a parentId → child turns map. */
export function indexChildren(turns: AskTurn[]): Map<string | null, AskTurn[]> {
  const map = new Map<string | null, AskTurn[]>();
  for (const turn of turns) {
    const key = turn.parentId ?? null;
    const list = map.get(key) ?? [];
    list.push(turn);
    map.set(key, list);
  }
  return map;
}

/**
 * Walks the turn tree from root to a leaf using `preferredChild` as a guide. When
 * no preference exists for a slot we pick the most recently created child. The
 * resulting list is the visible thread; switching variants just rewrites the map.
 */
export function computeActivePath(
  turns: AskTurn[],
  preferredChild: Record<string, string>,
): AskTurn[] {
  if (turns.length === 0) return [];
  const children = indexChildren(turns);
  const path: AskTurn[] = [];
  let parent: string | null = null;
  const guard = new Set<string>();
  while (true) {
    const candidates: AskTurn[] = children.get(parent) ?? [];
    if (candidates.length === 0) break;
    const preferredId = preferredChild[parent ?? "root"];
    const preferred = candidates.find((turn: AskTurn) => turn.id === preferredId);
    const next: AskTurn = preferred ?? candidates[candidates.length - 1];
    if (guard.has(next.id)) break;
    guard.add(next.id);
    path.push(next);
    parent = next.id;
  }
  return path;
}

/** Build a preferredChild map from a linear chain so loaded sessions render correctly. */
export function rebuildPreferredChildFromChain(turns: AskTurn[]): Record<string, string> {
  const map: Record<string, string> = {};
  for (const turn of turns) {
    map[turn.parentId ?? "root"] = turn.id;
  }
  return map;
}

/** Backfill `parentId` chain on legacy turns saved before branching landed. */
export function migrateTurnChain(turns: AskTurn[]): AskTurn[] {
  let previous: string | null = null;
  return turns.map((turn) => {
    const next: AskTurn = {
      ...turn,
      reasoning: turn.reasoning ?? "",
      steps: (turn.steps ?? []).map((step) => ({
        ...step,
        status: step.status ?? "ok",
      })),
      parentId: turn.parentId === undefined ? previous : turn.parentId,
    };
    previous = next.id;
    return next;
  });
}

// ── Type guards ──────────────────────────────────────────────────────────────

export function isMemoryEntry(value: MemoryEntry) {
  return (
    typeof value?.id === "string" &&
    (value.type === "user" || value.type === "feedback" || value.type === "project" || value.type === "reference") &&
    typeof value.title === "string" &&
    typeof value.body === "string" &&
    typeof value.updatedAt === "string"
  );
}

export function isAskTask(value: AskTask) {
  return (
    typeof value?.id === "string" &&
    (value.mode === "ask" || value.mode === "research" || value.mode === "act") &&
    (value.status === "pending" || value.status === "running" || value.status === "completed" || value.status === "blocked") &&
    typeof value.title === "string" &&
    typeof value.detail === "string" &&
    typeof value.createdAt === "string" &&
    typeof value.updatedAt === "string"
  );
}

export function isAskChatSession(value: AskChatSession) {
  return (
    typeof value?.id === "string" &&
    typeof value.title === "string" &&
    (value.mode === "ask" || value.mode === "research" || value.mode === "act") &&
    Array.isArray(value.turns) &&
    typeof value.createdAt === "string" &&
    typeof value.updatedAt === "string" &&
    typeof value.summary === "string"
  );
}

export function isAgentMemoryEvent(value: AgentMemoryEvent) {
  return (
    typeof value?.id === "string" &&
    isAgentMemoryKind(value.kind) &&
    typeof value.title === "string" &&
    typeof value.detail === "string" &&
    typeof value.createdAt === "string"
  );
}

export function isAgentMemoryKind(value: unknown): value is AgentMemoryKind {
  return (
    value === "ask" ||
    value === "answer" ||
    value === "tool" ||
    value === "memory" ||
    value === "task" ||
    value === "session" ||
    value === "clarification" ||
    value === "error"
  );
}

// ── Composer mention parsing ─────────────────────────────────────────────────

/** Detects an active `@mention` token at the cursor — used by the composer
 *  assist popover to surface mention targets. Returns null if the trailing
 *  segment is not a valid mention. */
export function findActiveMention(input: string) {
  const atIndex = input.lastIndexOf("@");
  if (atIndex < 0) {
    return null;
  }
  if (atIndex > 0 && !/\s/.test(input[atIndex - 1] ?? "")) {
    return null;
  }
  const query = input.slice(atIndex + 1);
  if (!/^[a-zA-Z0-9_-]*$/.test(query)) {
    return null;
  }
  return { start: atIndex, end: input.length, query };
}

// ── Identifiers + runtime detection ──────────────────────────────────────────

export function makeTurnId() {
  return `ask-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function makeLocalId(prefix: string) {
  return `${prefix}-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

// ── Server ↔ local converters ────────────────────────────────────────────────

/** Convert a server-side `AskChatRecord` into the existing `AskChatSession` shape so the
 *  past-chats drawer keeps working without a UI rewrite. Turns are loaded lazily on open. */
export function serverChatToSession(record: AskChatRecord): AskChatSession {
  return {
    id: record.id,
    title: record.title,
    mode: (record.mode as AgentMode) ?? "ask",
    turns: [],
    createdAt: record.created_at,
    updatedAt: record.updated_at,
    summary: record.summary,
  };
}

/** Convert a server `AskTurnRecord` back into the local `AskTurn` shape. */
export function serverTurnToAskTurn(record: AskTurnRecord): AskTurn {
  return {
    id: record.id,
    parentId: record.parent_id ?? null,
    forkOf: record.fork_of ?? null,
    mode: (record.mode as AgentMode | undefined) ?? undefined,
    question: record.question,
    answer: record.answer,
    reasoning: record.reasoning,
    refs: record.refs ?? [],
    questions: record.questions ?? [],
    steps: ((record.steps as AskStep[] | undefined) ?? []).map((step) => ({
      ...step,
      status: step.status ?? "ok",
    })),
    status: (record.status as AskTurn["status"]) ?? "done",
    error: record.error ?? null,
    attachments:
      record.attachments?.map((attachment) => ({
        id: attachment.id,
        kind: "image" as const,
        mimeType: attachment.mime_type,
        data: attachment.data_b64,
        filename: attachment.filename ?? undefined,
        size: attachment.size_bytes ?? undefined,
      })) ?? undefined,
    scoredNodes: record.scored_nodes ?? undefined,
    retrievalQuery: record.retrieval_query ?? null,
  };
}

/** Merge server-derived sessions ahead of local ones, deduping by id. */
export function mergeChatSessions(
  remote: AskChatSession[],
  local: AskChatSession[],
): AskChatSession[] {
  const byId = new Map<string, AskChatSession>();
  for (const session of remote) byId.set(session.id, session);
  for (const session of local) {
    if (!byId.has(session.id)) byId.set(session.id, session);
  }
  return Array.from(byId.values()).sort(
    (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
  );
}

// ── Misc helpers ─────────────────────────────────────────────────────────────

export function slugifyFilename(value: string) {
  const base = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);
  return base || "trace-chat";
}

export function chatToMarkdown(turns: AskTurn[], title: string, mode: AgentMode) {
  const lines: string[] = [
    `# ${title}`,
    "",
    `_Mode: ${mode} · Exported: ${new Date().toISOString()}_`,
    "",
  ];
  turns.forEach((turn, index) => {
    const heading = `## Turn ${index + 1}`;
    lines.push(heading);
    lines.push("");
    lines.push("**You**");
    lines.push("");
    lines.push(turn.question);
    if (turn.attachments && turn.attachments.length > 0) {
      lines.push("");
      lines.push(
        `_Attachments: ${turn.attachments
          .map((attachment) => attachment.filename ?? attachment.mimeType)
          .join(", ")}_`,
      );
    }
    lines.push("");
    lines.push("**Trace**");
    lines.push("");
    if (turn.status === "error") {
      lines.push(`_Error: ${turn.error ?? "unknown"}_`);
    } else if (turn.status === "cancelled") {
      lines.push("_Cancelled by user._");
      if (turn.answer) lines.push(turn.answer);
    } else {
      lines.push(turn.answer || "_(no response)_");
    }
    if (turn.refs.length > 0) {
      lines.push("");
      lines.push("_Sources:_");
      turn.refs.forEach((ref, refIdx) => {
        const route = ref.route;
        lines.push(`- [${refIdx + 1}] ${ref.kind}: ${ref.title}${route ? ` — ${route}` : ""}`);
      });
    }
    if (turn.steps.length > 0) {
      lines.push("");
      lines.push("_Tool activity:_");
      turn.steps.forEach((step) => {
        const status = step.status === "running" ? "…" : step.status === "error" ? "✗" : "✓";
        lines.push(`- ${status} ${step.label}${step.summary ? ` — ${step.summary}` : ""}`);
      });
    }
    lines.push("");
  });
  return lines.join("\n");
}

export function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      const result = reader.result;
      if (typeof result !== "string") {
        reject(new Error("unexpected reader output"));
        return;
      }
      const commaIdx = result.indexOf(",");
      resolve(commaIdx >= 0 ? result.slice(commaIdx + 1) : result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("read failed"));
    reader.readAsDataURL(file);
  });
}

// ── Session aggregation + formatting ─────────────────────────────────────────

export function countTools(turns: AskTurn[]) {
  const counts = new Map<string, number>();
  for (const turn of turns) {
    for (const step of turn.steps) {
      counts.set(step.tool, (counts.get(step.tool) ?? 0) + 1);
    }
  }
  return counts;
}

export function collectRefs(turns: AskTurn[]) {
  const seen = new Set<string>();
  const refs: SearchResult[] = [];
  for (const turn of [...turns].reverse()) {
    for (const ref of turn.refs) {
      const key = `${ref.kind}:${ref.entity_id}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      refs.push(ref);
    }
  }
  return refs;
}

export function buildSessionSummary(turns: AskTurn[], refs: SearchResult[]) {
  const completed = turns.filter((turn) => turn.status === "done");
  if (completed.length === 0) {
    return "";
  }

  const latest = completed[completed.length - 1];
  const answer = latest?.answer.replace(/\s+/g, " ").trim().slice(0, 700);
  const latestLine = latest
    ? [`Last asked: ${latest.question}`, answer ? `Last answer: ${answer}` : ""]
        .filter(Boolean)
        .join("\n")
    : "";
  const refsLine = refs.length
    ? `Recalled records: ${refs.map((ref) => `${ref.kind}:${ref.title}`).join(", ")}`
    : "Recalled records: none";
  return ["Current thread", latestLine, refsLine].filter(Boolean).join("\n");
}

export function buildSessionContext({
  turns,
  projectMemory,
  memoryEntries,
  durableMemoryContext,
  sessionSummary,
  agentMode,
  permissionMode,
  tasks,
  agentMemoryEvents,
  chatSessions,
  activeChatId,
  searchChatsEnabled,
  generateMemoryEnabled,
  question,
}: {
  turns: AskTurn[];
  projectMemory: string;
  memoryEntries: MemoryEntry[];
  durableMemoryContext: string;
  sessionSummary: string;
  agentMode: AgentMode;
  permissionMode: PermissionMode;
  tasks: AskTask[];
  agentMemoryEvents: AgentMemoryEvent[];
  chatSessions: AskChatSession[];
  activeChatId: string;
  searchChatsEnabled: boolean;
  generateMemoryEnabled: boolean;
  question: string;
}) {
  const completedTurns = turns.filter((turn) => turn.status === "done").slice(-6);
  const activeTasks = tasks
    .filter((task) => task.status === "pending" || task.status === "running" || task.status === "blocked")
    .slice(0, 12);
  const mentionContext = buildMentionContext(question);
  const pastChatContext = searchChatsEnabled ? formatPastChatsForContext(chatSessions, activeChatId) : "";
  const parts = [
    `Agent mode: ${agentMode}\n${AGENT_MODE_CONTEXT[agentMode]}`,
    `Permission mode: ${permissionMode}\n${PERMISSION_MODE_CONTEXT[permissionMode]}`,
    searchChatsEnabled ? "Past chat search: enabled. Use past-chat summaries only when relevant and cite concrete records when available." : "Past chat search: disabled. Do not use saved past-chat summaries.",
    generateMemoryEnabled ? "Generated memory: enabled. Use durable memory as context, but verify stale claims against current records." : "Generated memory: paused. Do not use durable generated memory or create new generated memory.",
    searchChatsEnabled && pastChatContext ? `Past chat summaries:\n${pastChatContext}` : "",
    durableMemoryContext.trim() ? `First-class durable memory:\n${durableMemoryContext.trim()}` : "",
    generateMemoryEnabled && memoryEntries.length ? `Structured memory:\n${formatMemoryEntries(memoryEntries)}` : "",
    generateMemoryEnabled && projectMemory.trim() ? `Scratch memory:\n${projectMemory.trim()}` : "",
    sessionSummary.trim() ? `Session memory:\n${sessionSummary.trim()}` : "",
    activeTasks.length ? `Active task state:\n${formatTasksForContext(activeTasks)}` : "",
    generateMemoryEnabled && agentMemoryEvents.length ? `Long-term agent activity memory:\n${formatAgentMemoryForContext(agentMemoryEvents)}` : "",
    mentionContext ? `Explicit mentions:\n${mentionContext}` : "",
    completedTurns.length
      ? completedTurns
          .map((turn, index) => {
            const refs = turn.refs.map((ref) => `${ref.kind}:${ref.title} (${ref.route})`).join(", ");
            return [
              `Turn ${index + 1}`,
              `User: ${turn.question}`,
              `Assistant: ${turn.answer}`,
              refs ? `References: ${refs}` : "References: none",
            ].join("\n");
          })
          .join("\n\n")
      : "",
  ];

  return parts.filter(Boolean).join("\n\n");
}

export function createRunTask(turnId: string, question: string, mode: AgentMode): AskTask {
  const now = new Date().toISOString();
  return {
    id: makeLocalId("task"),
    turnId,
    mode,
    status: "running",
    title: `${AGENT_MODES.find((item) => item.key === mode)?.label ?? "Ask"}: ${truncateText(question, 72)}`,
    detail: question,
    createdAt: now,
    updatedAt: now,
  };
}

export function formatMemoryEntries(entries: MemoryEntry[]) {
  return entries
    .slice(0, 40)
    .map((entry) =>
      [
        `[${entry.type}] ${entry.title}`,
        entry.body,
        `updated_at: ${entry.updatedAt}`,
      ].join("\n"),
    )
    .join("\n\n");
}

export function formatTasksForContext(tasks: AskTask[]) {
  return tasks
    .map((task) => `- [${task.status}] ${task.title}: ${task.detail}`)
    .join("\n");
}

export function formatAgentMemoryForContext(events: AgentMemoryEvent[]) {
  return events
    .slice(0, 36)
    .map((event) => {
      const meta = [
        event.mode ? `mode=${event.mode}` : "",
        event.tool ? `tool=${event.tool}` : "",
        typeof event.refs === "number" ? `refs=${event.refs}` : "",
      ].filter(Boolean).join(", ");
      return `- [${event.kind}] ${event.title}${meta ? ` (${meta})` : ""}: ${event.detail}`;
    })
    .join("\n");
}

export function formatPastChatsForContext(sessions: AskChatSession[], activeChatId: string) {
  return sessions
    .filter((session) => session.id !== activeChatId)
    .sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime())
    .slice(0, 12)
    .map((session) => `- ${session.title} (${session.mode}, ${session.turns.length} turns, updated ${session.updatedAt}): ${session.summary}`)
    .join("\n");
}

export function buildMentionContext(question: string) {
  const lower = question.toLowerCase();
  return MENTION_TARGETS.filter((target) => lower.includes(target.token.toLowerCase()))
    .map((target) => `- ${target.token}: ${target.context}`)
    .join("\n");
}

export function upsertChatSession(
  sessions: AskChatSession[],
  id: string,
  turns: AskTurn[],
  mode: AgentMode,
) {
  const now = new Date().toISOString();
  const existing = sessions.find((session) => session.id === id);
  const nextSession: AskChatSession = {
    id,
    title: makeChatTitle(turns),
    mode,
    turns: turns.slice(-40),
    createdAt: existing?.createdAt ?? now,
    updatedAt: now,
    summary: makeChatSummary(turns),
  };
  return [nextSession, ...sessions.filter((session) => session.id !== id)].slice(0, 60);
}

export function makeChatTitle(turns: AskTurn[]) {
  const firstQuestion = turns.find((turn) => turn.question.trim())?.question ?? "New chat";
  return truncateText(firstQuestion.replace(/\s+/g, " ").trim(), 72);
}

export function makeChatSummary(turns: AskTurn[]) {
  const latest = [...turns].reverse().find((turn) => turn.status === "done" || turn.status === "error") ?? turns[turns.length - 1];
  if (!latest) {
    return "Empty chat";
  }
  const answer = latest.answer || latest.error || latest.question;
  return truncateText(answer.replace(/\s+/g, " ").trim(), 180);
}

export function pushAgentMemoryEvent(
  current: AgentMemoryEvent[],
  event: Omit<AgentMemoryEvent, "id" | "createdAt">,
) {
  const nextEvent: AgentMemoryEvent = {
    ...event,
    id: makeLocalId("event"),
    createdAt: new Date().toISOString(),
  };
  return [nextEvent, ...current].slice(0, 300);
}

export function pushPromptHistory(current: string[], prompt: string) {
  const normalized = prompt.trim();
  if (!normalized) {
    return current;
  }
  return [normalized, ...current.filter((item) => item !== normalized)].slice(0, 60);
}

export function groupMemoryEntries(entries: MemoryEntry[]) {
  const grouped: Record<MemoryType, MemoryEntry[]> = {
    feedback: [],
    project: [],
    reference: [],
    user: [],
  };
  for (const entry of entries) {
    grouped[entry.type].push(entry);
  }
  return grouped;
}

export function getMemoryUpdatedAt(entries: MemoryEntry[], events: AgentMemoryEvent[]) {
  const timestamps = [
    ...entries.map((entry) => entry.updatedAt),
    ...events.map((event) => event.createdAt),
  ]
    .map((value) => new Date(value).getTime())
    .filter((value) => !Number.isNaN(value));
  if (timestamps.length === 0) {
    return null;
  }
  return new Date(Math.max(...timestamps)).toISOString();
}

export function makeMemoryTitle(body: string) {
  return truncateText(body.replace(/\s+/g, " ").trim(), 64) || "Memory";
}

export function truncateText(text: string, maxLength: number) {
  return text.length > maxLength ? `${text.slice(0, maxLength - 1)}…` : text;
}

export function drawerTitles(drawer: Exclude<DrawerKind, null>) {
  const titles: Record<Exclude<DrawerKind, null>, { kicker: string; title: string }> = {
    chats: { kicker: "Threads", title: "Past chats" },
    commands: { kicker: "Shortcuts", title: "Commands and mentions" },
    history: { kicker: "Recall", title: "Prompt history" },
    ingest: { kicker: "Conversation ingest", title: "Backfill Claude work" },
    memory: { kicker: "Memory", title: "Memory system" },
    settings: { kicker: "Guardrails", title: "Agent settings" },
    tasks: { kicker: "Runs", title: "Task tracker" },
    tools: { kicker: "Harness", title: "Tool harness" },
  };
  return titles[drawer];
}

export function permissionModeLabel(mode: PermissionMode) {
  if (mode === "auto_read") {
    return "Auto read";
  }
  if (mode === "auto_safe") {
    return "Auto safe";
  }
  return "Confirm";
}

export function taskStatusLabel(status: AskTaskStatus) {
  if (status === "running") {
    return "Running";
  }
  if (status === "completed") {
    return "Done";
  }
  if (status === "blocked") {
    return "Blocked";
  }
  return "Pending";
}

export function formatRelativeDate(value: string) {
  const time = new Date(value).getTime();
  if (Number.isNaN(time)) {
    return "recently";
  }
  const delta = Math.max(0, Date.now() - time);
  const minutes = Math.floor(delta / 60000);
  if (minutes < 1) {
    return "just now";
  }
  if (minutes < 60) {
    return `${minutes}m ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}h ago`;
  }
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

// `AgentMemoryKind` discriminant value-set used by isAgentMemoryKind below.
export type _AgentMemoryKind = AgentMemoryKind;

export function mergeVoiceTranscript(current: string, transcript: string) {
  const trimmedTranscript = transcript.trim();
  if (!trimmedTranscript) return current;
  if (!current) return trimmedTranscript;
  return `${current.replace(/\s+$/, "")} ${trimmedTranscript}`;
}

/**
 * Resolves once the streaming event listener has set the turn to a terminal status
 * (`done` / `error` / `cancelled`). Polls the React state via the setter's read pattern.
 */
export function waitForTurnSettlement(
  setter: (updater: (current: AskTurn[]) => AskTurn[]) => void,
  turnId: string,
): Promise<AskTurn> {
  return new Promise((resolve) => {
    const interval = window.setInterval(() => {
      setter((current) => {
        const target = current.find((turn) => turn.id === turnId);
        if (target && (target.status === "done" || target.status === "error" || target.status === "cancelled")) {
          window.clearInterval(interval);
          resolve(target);
        }
        return current;
      });
    }, 75);
  });
}
