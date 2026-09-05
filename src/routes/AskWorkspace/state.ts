// Shared types and localStorage / sessionStorage keys for the AskWorkspace
// route. Extracted from AskWorkspace.tsx so submodules (utils, Turn, ToolPanel,
// SideDrawer, etc.) can import without pulling in the whole route file.

import type { ReactNode } from "react";
import type {
  AskSearchResult,
  AskUserQuestion,
  ScoredBrainNode,
  SearchResult,
} from "../../lib/types";

// ── Storage keys ─────────────────────────────────────────────────────────────

export const ASK_TURNS_KEY = "trace.ask.turns.v1";
export const ASK_PREFERRED_CHILD_STORAGE_ID = "trace.ask.preferredChild.v1";

export const MAX_ATTACHMENTS = 6;
export const MAX_ATTACHMENT_BYTES = 8 * 1024 * 1024; // 8 MB per image
export const ACCEPTED_ATTACHMENT_MIMES = "image/png,image/jpeg,image/gif,image/webp";

export const ASK_PROJECT_MEMORY_KEY = "trace.ask.projectMemory.v1";
export const ASK_AGENT_MODE_KEY = "trace.ask.agentMode.v1";
export const ASK_REASONING_DEPTH_STORAGE_ID = "trace.ask.reasoningDepth.v1";
export const ASK_MEMORY_ENTRIES_KEY = "trace.ask.memoryEntries.v1";
export const ASK_TASKS_KEY = "trace.ask.tasks.v1";
export const ASK_HISTORY_KEY = "trace.ask.promptHistory.v1";
export const ASK_PERMISSION_MODE_KEY = "trace.ask.permissionMode.v1";
export const ASK_AUTO_CONFIRM_TOOLS_KEY = "trace.ask.autoConfirmTools.v1";
export const ASK_CHAT_SESSIONS_KEY = "trace.ask.chatSessions.v1";
export const ASK_ACTIVE_CHAT_ID_KEY = "trace.ask.activeChatId.v1";
export const ASK_AGENT_MEMORY_EVENTS_KEY = "trace.ask.agentMemoryEvents.v1";
export const ASK_SEARCH_CHATS_ENABLED_KEY = "trace.ask.searchChatsEnabled.v1";
export const ASK_GENERATE_MEMORY_ENABLED_KEY = "trace.ask.generateMemoryEnabled.v1";

// ── Types ────────────────────────────────────────────────────────────────────

export interface AskStep {
  id: string;
  tool: string;
  label: string;
  status: "running" | "ok" | "error" | "awaiting" | "denied";
  rationale?: string | null;
  argsPreview?: string | null;
  summary?: string | null;
  runId?: string;
  callId?: string;
  riskReason?: string | null;
}

export interface AskProgressPayload {
  kind: string;
  tool: string;
  label: string;
}

export type AskRunEventPayload =
  | { kind: "started"; run_id: string }
  | { kind: "text_delta"; run_id: string; delta: string }
  | { kind: "reasoning_delta"; run_id: string; delta: string }
  | {
      kind: "tool_call_started";
      run_id: string;
      call_id: string;
      tool: string;
      label: string;
      rationale: string | null;
      args_preview: string | null;
    }
  | {
      kind: "tool_call_done";
      run_id: string;
      call_id: string;
      tool: string;
      ok: boolean;
      summary: string;
    }
  | {
      kind: "awaiting_confirmation";
      run_id: string;
      call_id: string;
      tool: string;
      label: string;
      summary: string;
      args_preview: string | null;
      risk_reason: string;
    }
  | {
      kind: "tool_denied";
      run_id: string;
      call_id: string;
      tool: string;
      reason: string;
    }
  | { kind: "turn_complete"; run_id: string; iteration: number }
  | { kind: "done"; run_id: string; result: AskSearchResult }
  | { kind: "cancelled"; run_id: string }
  | { kind: "error"; run_id: string; message: string };

export interface AskAttachmentImage {
  id: string;
  kind: "image";
  mimeType: string;
  /** Base64 (no data: prefix). */
  data: string;
  filename?: string;
  /** Bytes of the original file, for telemetry / size limits. */
  size?: number;
}

export type AskAttachment = AskAttachmentImage;

export interface AskTurn {
  id: string;
  /** Previous turn in the conversation tree. `null` for the first turn in a chat. */
  parentId: string | null;
  /** Original turn this is a retry/edit of, for analytics. Sibling turns share a parentId. */
  forkOf?: string | null;
  runId?: string;
  mode?: AgentMode;
  reasoningDepth?: ReasoningDepth;
  question: string;
  attachments?: AskAttachment[];
  answer: string;
  reasoning: string;
  refs: SearchResult[];
  questions: AskUserQuestion[];
  steps: AskStep[];
  status: "running" | "streaming" | "done" | "error" | "cancelled";
  error: string | null;
  /** Section 6.2 — per-node retrieval score breakdown for "Why this answer?". */
  scoredNodes?: ScoredBrainNode[];
  /** Section 6.2 — the query the brain actually saw (may differ from `question`). */
  retrievalQuery?: string | null;
}

export type DrawerKind =
  | "tools"
  | "memory"
  | "tasks"
  | "commands"
  | "history"
  | "chats"
  | "settings"
  | "ingest"
  | null;
export type ToolCategory = "read" | "write" | "email" | "memory" | "files" | "clarify" | "web";
export type AgentMode = "ask" | "research" | "act";
export type ReasoningDepth = "standard" | "deep";
export type PermissionMode = "confirm" | "auto_read" | "auto_safe";
export type MemoryType = "user" | "feedback" | "project" | "reference";
export type AskTaskStatus = "pending" | "running" | "completed" | "blocked";
export type AgentMemoryKind =
  | "ask"
  | "answer"
  | "tool"
  | "memory"
  | "task"
  | "session"
  | "clarification"
  | "error";

export interface MemoryEntry {
  id: string;
  type: MemoryType;
  title: string;
  body: string;
  updatedAt: string;
}

export interface AskTask {
  id: string;
  turnId?: string;
  mode: AgentMode;
  status: AskTaskStatus;
  title: string;
  detail: string;
  createdAt: string;
  updatedAt: string;
}

export interface AskChatSession {
  id: string;
  title: string;
  mode: AgentMode;
  turns: AskTurn[];
  createdAt: string;
  updatedAt: string;
  summary: string;
}

export interface AgentMemoryEvent {
  id: string;
  kind: AgentMemoryKind;
  title: string;
  detail: string;
  mode?: AgentMode;
  turnId?: string;
  sessionId?: string;
  tool?: string;
  refs?: number;
  createdAt: string;
}

export interface ToolSpec {
  name: string;
  label: string;
  category: ToolCategory;
  icon: ReactNode;
}

export interface ComposerCommand {
  name: string;
  label: string;
  description: string;
  badge: string;
}

export interface MentionTarget {
  token: string;
  label: string;
  description: string;
  context: string;
  icon: ReactNode;
}
