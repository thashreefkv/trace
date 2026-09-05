import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent,
} from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useIpcQuery, qk } from "../../lib/queries";
import { queryClient } from "../../lib/queryClient";
import { listen } from "@tauri-apps/api/event";
import { useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowUpRight,
  BookOpen,
  Download,
  Command,
  MessageSquareText,
  Mic,
  Paperclip,
  Plus,
  Send,
  Settings2,
  ShieldCheck,
  Square,
  Wrench,
} from "lucide-react";
import {
  appendAskTurn,
  askSearch,
  cancelAskRun,
  deleteAskChat,
  getAskChat,
  listAskChats,
  retrieveMemories,
  transcribeVoiceInput,
  startAskRun,
  upsertAskChat,
  type AskAttachmentInput,
} from "../../lib/ipc";
import type {
  AskSearchResult,
  AskUserQuestion,
} from "../../lib/types";
import {
  ACCEPTED_ATTACHMENT_MIMES,
  ASK_ACTIVE_CHAT_ID_KEY,
  ASK_AGENT_MEMORY_EVENTS_KEY,
  ASK_AGENT_MODE_KEY,
  ASK_REASONING_DEPTH_STORAGE_ID,
  ASK_AUTO_CONFIRM_TOOLS_KEY,
  ASK_GENERATE_MEMORY_ENABLED_KEY,
  ASK_HISTORY_KEY,
  ASK_MEMORY_ENTRIES_KEY,
  ASK_PERMISSION_MODE_KEY,
  ASK_PREFERRED_CHILD_STORAGE_ID,
  ASK_PROJECT_MEMORY_KEY,
  ASK_SEARCH_CHATS_ENABLED_KEY,
  ASK_TASKS_KEY,
  ASK_TURNS_KEY,
  MAX_ATTACHMENT_BYTES,
  MAX_ATTACHMENTS,
  type AgentMemoryEvent,
  type AgentMode,
  type AskAttachment,
  type AskChatSession,
  type AskProgressPayload,
  type AskRunEventPayload,
  type AskStep,
  type AskTask,
  type AskTaskStatus,
  type AskTurn,
  type DrawerKind,
  type MemoryEntry,
  type PermissionMode,
  type ReasoningDepth,
} from "./state";
import {
  buildSessionContext,
  buildSessionSummary,
  chatToMarkdown,
  collectAncestors,
  collectRefs,
  computeActivePath,
  countTools,
  createRunTask,
  fileToBase64,
  findActiveMention,
  indexChildren,
  isTauriRuntime,
  loadActiveChatId,
  loadAgentMemoryEvents,
  loadAgentMode,
  loadAskTasks,
  loadAutoConfirmTools,
  loadChatSessions,
  loadGenerateMemoryEnabled,
  loadMemoryEntries,
  loadPermissionMode,
  loadReasoningDepth,
  loadPreferredChild,
  loadProjectMemory,
  loadPromptHistory,
  loadSearchChatsEnabled,
  loadStoredTurns,
  makeChatSummary,
  makeChatTitle,
  makeLocalId,
  makeMemoryTitle,
  makeTurnId,
  mergeChatSessions,
  mergeVoiceTranscript,
  migrateTurnChain,
  pushAgentMemoryEvent,
  pushPromptHistory,
  rebuildPreferredChildFromChain,
  serverChatToSession,
  serverTurnToAskTurn,
  slugifyFilename,
  taskStatusLabel,
  truncateText,
  upsertChatSession,
  waitForTurnSettlement,
} from "./utils";
import {
  AGENT_MODES,
  PERMISSION_MODE_CONTEXT,
  TOOL_SPECS,
} from "./constants";
import { ComposerAttachmentChip, TurnView } from "./Turn";
import { BrandMark } from "./icons";
import { AgentModeSelector, ComposerAssist, DeepReasoningToggle, EmptyState } from "./AgentMode";
import { SideDrawer } from "./SideDrawer";

// Types and storage keys moved to `./AskWorkspace/state`.

// `kindIcon` lives in ./AskWorkspace/Citations alongside its only consumers.

export function AskWorkspace() {
  const [params] = useSearchParams();
  const [turns, setTurns] = useState<AskTurn[]>(loadStoredTurns);
  const [preferredChild, setPreferredChild] = useState<Record<string, string>>(loadPreferredChild);
  const [editingTurnId, setEditingTurnId] = useState<string | null>(null);
  const [projectMemory, setProjectMemory] = useState(loadProjectMemory);
  const [memoryEntries, setMemoryEntries] = useState<MemoryEntry[]>(loadMemoryEntries);
  const [tasks, setTasks] = useState<AskTask[]>(loadAskTasks);
  const [promptHistory, setPromptHistory] = useState<string[]>(loadPromptHistory);
  const [chatSessions, setChatSessions] = useState<AskChatSession[]>(loadChatSessions);
  const [activeChatId, setActiveChatId] = useState(loadActiveChatId);
  const [agentMemoryEvents, setAgentMemoryEvents] = useState<AgentMemoryEvent[]>(loadAgentMemoryEvents);
  const [searchChatsEnabled, setSearchChatsEnabled] = useState(loadSearchChatsEnabled);
  const [generateMemoryEnabled, setGenerateMemoryEnabled] = useState(loadGenerateMemoryEnabled);
  const [agentMode, setAgentMode] = useState<AgentMode>(loadAgentMode);
  const [reasoningDepth, setReasoningDepth] = useState<ReasoningDepth>(loadReasoningDepth);
  const [permissionMode, setPermissionMode] = useState<PermissionMode>(loadPermissionMode);
  const [autoConfirmTools, setAutoConfirmTools] = useState<string[]>(loadAutoConfirmTools);
  const [drawer, setDrawer] = useState<DrawerKind>(null);
  const [input, setInput] = useState("");
  const [composerAttachments, setComposerAttachments] = useState<AskAttachment[]>([]);
  const [composerError, setComposerError] = useState<string | null>(null);
  const [voiceState, setVoiceState] = useState<"idle" | "listening" | "transcribing">("idle");
  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const audioChunksRef = useRef<Blob[]>([]);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [routeError, setRouteError] = useState<string | null>(null);
  const activeTurnIdRef = useRef<string | null>(null);
  const stepCounterRef = useRef(0);
  const lastRouteQueryRef = useRef<string | null>(null);
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const filePickerRef = useRef<HTMLInputElement | null>(null);
  /** Maps stream runId → React turn id so events can find their target. */
  const runToTurnRef = useRef<Map<string, string>>(new Map());
  /** Maps stream runId → step counter so call_id renders stably. */
  const stepIndexRef = useRef<Map<string, Map<string, string>>>(new Map());
  /** Turn ids already persisted to the server (terminal status reached). */
  const persistedTurnsRef = useRef<Set<string>>(new Set());
  /** Chats already upserted to the server in this session. */
  const persistedChatsRef = useRef<Set<string>>(new Set());
  const [activeRunId, setActiveRunId] = useState<string | null>(null);

  // ── React-query: server-backed chat list ────────────────────────────────────
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);
  const { data: serverChats } = useIpcQuery(
    qk.ask.chats(60),
    () => listAskChats({ limit: 60 }),
    { enabled: isTauriRuntime() },
  );
  // ──────────────────────────────────────────────────────────────────────────

  useEffect(() => {
    sessionStorage.setItem(ASK_TURNS_KEY, JSON.stringify(turns.slice(-40)));
  }, [turns]);

  useEffect(() => {
    sessionStorage.setItem(ASK_PREFERRED_CHILD_STORAGE_ID, JSON.stringify(preferredChild));
  }, [preferredChild]);

  // Merge server chat list into local chatSessions whenever the query updates
  useEffect(() => {
    if (!serverChats) return;
    const remoteSessions: AskChatSession[] = serverChats.map(serverChatToSession);
    setChatSessions((current) => mergeChatSessions(remoteSessions, current));
  }, [serverChats]);

  useEffect(() => {
    localStorage.setItem(ASK_PROJECT_MEMORY_KEY, projectMemory);
  }, [projectMemory]);

  useEffect(() => {
    localStorage.setItem(ASK_MEMORY_ENTRIES_KEY, JSON.stringify(memoryEntries.slice(0, 80)));
  }, [memoryEntries]);

  useEffect(() => {
    localStorage.setItem(ASK_TASKS_KEY, JSON.stringify(tasks.slice(0, 100)));
  }, [tasks]);

  useEffect(() => {
    localStorage.setItem(ASK_HISTORY_KEY, JSON.stringify(promptHistory.slice(0, 60)));
  }, [promptHistory]);

  useEffect(() => {
    localStorage.setItem(ASK_ACTIVE_CHAT_ID_KEY, activeChatId);
  }, [activeChatId]);

  useEffect(() => {
    localStorage.setItem(ASK_AGENT_MEMORY_EVENTS_KEY, JSON.stringify(agentMemoryEvents.slice(0, 300)));
  }, [agentMemoryEvents]);

  useEffect(() => {
    localStorage.setItem(ASK_SEARCH_CHATS_ENABLED_KEY, searchChatsEnabled ? "true" : "false");
  }, [searchChatsEnabled]);

  useEffect(() => {
    localStorage.setItem(ASK_GENERATE_MEMORY_ENABLED_KEY, generateMemoryEnabled ? "true" : "false");
  }, [generateMemoryEnabled]);

  useEffect(() => {
    localStorage.setItem(ASK_AGENT_MODE_KEY, agentMode);
  }, [agentMode]);

  useEffect(() => {
    localStorage.setItem(ASK_REASONING_DEPTH_STORAGE_ID, reasoningDepth);
  }, [reasoningDepth]);

  useEffect(() => {
    localStorage.setItem(ASK_PERMISSION_MODE_KEY, permissionMode);
  }, [permissionMode]);

  useEffect(() => {
    localStorage.setItem(ASK_AUTO_CONFIRM_TOOLS_KEY, JSON.stringify(autoConfirmTools));
  }, [autoConfirmTools]);

  // Invalidate the server chat list whenever a run completes (activeRunId → null)
  const prevRunIdRef = useRef<string | null>(null);
  useEffect(() => {
    const prev = prevRunIdRef.current;
    prevRunIdRef.current = activeRunId;
    if (prev !== null && activeRunId === null && isTauriRuntime()) {
      void queryClient.invalidateQueries({ queryKey: qk.ask.all });
    }
  }, [activeRunId]);

  const toggleAutoConfirmTool = useCallback((tool: string, enabled: boolean) => {
    setAutoConfirmTools((current) => {
      const set = new Set(current);
      if (enabled) {
        set.add(tool);
      } else {
        set.delete(tool);
      }
      return Array.from(set).sort();
    });
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) {
      return;
    }

    let unlisten: (() => void) | undefined;
    void listen<AskProgressPayload>("ask:progress", (event) => {
      if (event.payload.kind !== "tool_call") {
        return;
      }
      const turnId = activeTurnIdRef.current;
      if (!turnId) {
        return;
      }
      stepCounterRef.current += 1;
      const step: AskStep = {
        id: `${turnId}-${stepCounterRef.current}`,
        tool: event.payload.tool,
        label: event.payload.label,
        status: "ok",
      };
      setTurns((current) =>
        current.map((turn) =>
          turn.id === turnId ? { ...turn, steps: [...turn.steps, step] } : turn,
        ),
      );
      setAgentMemoryEvents((current) =>
        generateMemoryEnabled
          ? pushAgentMemoryEvent(current, {
              kind: "tool",
              title: event.payload.label,
              detail: `Tool call: ${event.payload.tool}`,
              sessionId: activeChatId,
              tool: event.payload.tool,
              turnId,
            })
          : current,
      );
    })
      .then((nextUnlisten) => {
        unlisten = nextUnlisten;
      })
      .catch((caught) => setRouteError(String(caught)));

    return () => unlisten?.();
  }, [activeChatId, generateMemoryEnabled]);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;

    function findTurnId(runId: string) {
      return runToTurnRef.current.get(runId) ?? null;
    }

    function patchTurn(turnId: string, updater: (turn: AskTurn) => AskTurn) {
      setTurns((current) =>
        current.map((turn) => (turn.id === turnId ? updater(turn) : turn)),
      );
    }

    void listen<AskRunEventPayload>("ask:event", (event) => {
      const payload = event.payload;
      const turnId = findTurnId(payload.run_id);
      if (!turnId) return;

      switch (payload.kind) {
        case "started":
          patchTurn(turnId, (turn) => ({ ...turn, status: "streaming" }));
          break;
        case "text_delta":
          patchTurn(turnId, (turn) => ({
            ...turn,
            answer: turn.answer + payload.delta,
            status: "streaming",
          }));
          break;
        case "reasoning_delta":
          patchTurn(turnId, (turn) => ({
            ...turn,
            reasoning: turn.reasoning + payload.delta,
          }));
          break;
        case "tool_call_started": {
          const stepMap =
            stepIndexRef.current.get(payload.run_id) ?? new Map<string, string>();
          const stepId = `${turnId}-step-${stepMap.size + 1}`;
          stepMap.set(payload.call_id, stepId);
          stepIndexRef.current.set(payload.run_id, stepMap);
          patchTurn(turnId, (turn) => ({
            ...turn,
            steps: [
              ...turn.steps,
              {
                id: stepId,
                tool: payload.tool,
                label: payload.label,
                status: "running",
                rationale: payload.rationale,
                argsPreview: payload.args_preview,
                summary: null,
              },
            ],
          }));
          break;
        }
        case "tool_call_done": {
          const stepMap = stepIndexRef.current.get(payload.run_id);
          const stepId = stepMap?.get(payload.call_id);
          if (!stepId) return;
          patchTurn(turnId, (turn) => ({
            ...turn,
            steps: turn.steps.map((step) =>
              step.id === stepId
                ? {
                    ...step,
                    status: payload.ok ? "ok" : "error",
                    summary: payload.summary,
                  }
                : step,
            ),
          }));
          break;
        }
        case "awaiting_confirmation": {
          const stepMap = stepIndexRef.current.get(payload.run_id);
          const stepId = stepMap?.get(payload.call_id);
          if (!stepId) return;
          patchTurn(turnId, (turn) => ({
            ...turn,
            steps: turn.steps.map((step) =>
              step.id === stepId
                ? {
                    ...step,
                    status: "awaiting",
                    riskReason: payload.risk_reason,
                    runId: payload.run_id,
                    callId: payload.call_id,
                    summary: payload.summary,
                    argsPreview: payload.args_preview,
                  }
                : step,
            ),
          }));
          break;
        }
        case "tool_denied": {
          const stepMap = stepIndexRef.current.get(payload.run_id);
          const stepId = stepMap?.get(payload.call_id);
          if (!stepId) return;
          patchTurn(turnId, (turn) => ({
            ...turn,
            steps: turn.steps.map((step) =>
              step.id === stepId
                ? {
                    ...step,
                    status: "denied",
                    summary: payload.reason,
                  }
                : step,
            ),
          }));
          break;
        }
        case "done":
          patchTurn(turnId, (turn) => ({
            ...turn,
            answer: payload.result.answer,
            refs: payload.result.refs ?? [],
            questions: payload.result.questions ?? [],
            // Section 6.2 — capture retrieval breakdown for "Why this answer?".
            scoredNodes:
              payload.result.scored_nodes && payload.result.scored_nodes.length > 0
                ? payload.result.scored_nodes
                : turn.scoredNodes,
            retrievalQuery: payload.result.retrieval_query ?? turn.retrievalQuery ?? null,
            status: "done",
          }));
          runToTurnRef.current.delete(payload.run_id);
          stepIndexRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        case "cancelled":
          patchTurn(turnId, (turn) => ({
            ...turn,
            status: "cancelled",
          }));
          runToTurnRef.current.delete(payload.run_id);
          stepIndexRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        case "error":
          patchTurn(turnId, (turn) => ({
            ...turn,
            status: "error",
            error: payload.message,
          }));
          runToTurnRef.current.delete(payload.run_id);
          stepIndexRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        case "turn_complete":
          // Iteration boundary; nothing to render but useful for diagnostics.
          break;
      }
    })
      .then((nextUnlisten) => {
        unlisten = nextUnlisten;
      })
      .catch((caught) => setRouteError(String(caught)));

    return () => unlisten?.();
  }, []);

  const stopActiveRun = useCallback(async () => {
    const runId = activeRunId;
    if (!runId) return;
    try {
      await cancelAskRun(runId);
    } catch (caught) {
      setRouteError(String(caught));
    }
  }, [activeRunId]);

  useEffect(() => {
    if (turns.length === 0) {
      return;
    }
    setChatSessions((current) => upsertChatSession(current, activeChatId, turns, agentMode));
  }, [activeChatId, agentMode, turns]);

  // Server persistence: upsert chat header + append finalized turns whenever a turn
  // reaches a terminal status. Outside Tauri this is a no-op.
  useEffect(() => {
    if (!isTauriRuntime() || turns.length === 0) return;
    const terminal = turns.filter(
      (turn) =>
        (turn.status === "done" ||
          turn.status === "error" ||
          turn.status === "cancelled") &&
        !persistedTurnsRef.current.has(turn.id),
    );
    if (terminal.length === 0) return;

    const chatId = activeChatId;
    const title = makeChatTitle(turns);
    const summary = makeChatSummary(turns);
    const mode = agentMode;

    void (async () => {
      try {
        if (!persistedChatsRef.current.has(chatId)) {
          await upsertAskChat({ id: chatId, title, mode, summary });
          persistedChatsRef.current.add(chatId);
        } else {
          // Refresh title/summary occasionally; cheap upsert.
          await upsertAskChat({ id: chatId, title, mode, summary });
        }
        for (const turn of terminal) {
          await appendAskTurn({
            chat_id: chatId,
            turn_id: turn.id,
            parent_id: turn.parentId,
            fork_of: turn.forkOf ?? null,
            mode: turn.mode ?? mode,
            question: turn.question,
            answer: turn.answer,
            reasoning: turn.reasoning,
            status: turn.status,
            error: turn.error,
            refs: turn.refs,
            questions: turn.questions,
            steps: turn.steps as unknown[],
            attachments:
              turn.attachments?.map((attachment) => ({
                mime_type: attachment.mimeType,
                filename: attachment.filename ?? null,
                data: attachment.data,
                size: attachment.size ?? null,
              })) ?? [],
            // Section 6.2 — persist the retrieval breakdown for "Why this answer?".
            scored_nodes: turn.scoredNodes ?? [],
            retrieval_query: turn.retrievalQuery ?? null,
          });
          persistedTurnsRef.current.add(turn.id);
        }
      } catch (caught) {
        console.warn("appendAskTurn failed", caught);
      }
    })();
  }, [activeChatId, agentMode, turns]);

  const activePath = useMemo(
    () => computeActivePath(turns, preferredChild),
    [turns, preferredChild],
  );
  const childrenIndex = useMemo(() => indexChildren(turns), [turns]);
  const memoryRefs = useMemo(() => collectRefs(activePath).slice(0, 8), [activePath]);
  const usedToolCounts = useMemo(() => countTools(activePath), [activePath]);

  // ── Virtualizer for the turns list ─────────────────────────────────────────
  const turnVirtualizer = useVirtualizer({
    count: activePath.length,
    getScrollElement: () => scrollContainerRef.current,
    estimateSize: () => 240,
    overscan: 4,
    measureElement: (el) => el?.getBoundingClientRect().height ?? 240,
  });
  const sessionSummary = useMemo(
    () => buildSessionSummary(activePath, memoryRefs),
    [activePath, memoryRefs],
  );

  const recordAgentEvent = useCallback(
    (event: Omit<AgentMemoryEvent, "id" | "createdAt" | "sessionId"> & { sessionId?: string }) => {
      if (!generateMemoryEnabled) {
        return;
      }
      setAgentMemoryEvents((current) =>
        pushAgentMemoryEvent(current, {
          ...event,
          sessionId: event.sessionId ?? activeChatId,
        }),
      );
    },
    [activeChatId, generateMemoryEnabled],
  );

  const addMemoryEntry = useCallback((entry: Omit<MemoryEntry, "id" | "updatedAt">) => {
    const now = new Date().toISOString();
    const nextEntry: MemoryEntry = {
      ...entry,
      id: makeLocalId("mem"),
      updatedAt: now,
    };
    setMemoryEntries((current) => [nextEntry, ...current].slice(0, 80));
    recordAgentEvent({
      kind: "memory",
      title: `Saved ${entry.type} memory`,
      detail: `${entry.title}: ${entry.body}`,
    });
  }, [recordAgentEvent]);

  const removeMemoryEntry = useCallback((id: string) => {
    setMemoryEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  const updateTaskStatus = useCallback((id: string, status: AskTaskStatus) => {
    setTasks((current) =>
      current.map((task) =>
        task.id === id ? { ...task, status, updatedAt: new Date().toISOString() } : task,
      ),
    );
    recordAgentEvent({
      kind: "task",
      title: `Task ${taskStatusLabel(status).toLowerCase()}`,
      detail: id,
    });
  }, [recordAgentEvent]);

  const removeTask = useCallback((id: string) => {
    setTasks((current) => current.filter((task) => task.id !== id));
  }, []);

  const submitQuestion = useCallback(
    async (
      rawQuestion: string,
      modeOverride?: AgentMode,
      branchOptions?: {
        parentId?: string | null;
        forkOf?: string | null;
        attachments?: AskAttachment[];
      },
    ) => {
      const question = rawQuestion.trim();
      if (!question || isSubmitting) {
        return;
      }

      const effectiveMode = modeOverride ?? agentMode;
      if (modeOverride && modeOverride !== agentMode) {
        setAgentMode(modeOverride);
      }

      const explicitParent = branchOptions?.parentId;
      const parentId =
        explicitParent !== undefined
          ? explicitParent
          : activePath[activePath.length - 1]?.id ?? null;
      const id = makeTurnId();
      const turnAttachments = branchOptions?.attachments ?? composerAttachments;
      const nextTurn: AskTurn = {
        id,
        parentId,
        forkOf: branchOptions?.forkOf ?? null,
        mode: effectiveMode,
        reasoningDepth,
        question,
        attachments: turnAttachments.length > 0 ? turnAttachments : undefined,
        answer: "",
        reasoning: "",
        refs: [],
        questions: [],
        steps: [],
        status: "running",
        error: null,
      };
      const runTask = createRunTask(id, question, effectiveMode);
      const durableMemory = generateMemoryEnabled
        ? await retrieveMemories({ query: question, limit: 16 }).catch(() => null)
        : null;
      // Build context from the path that THIS turn is about to extend.
      const ancestors = collectAncestors(turns, parentId);
      const context = buildSessionContext({
        turns: ancestors,
        projectMemory,
        memoryEntries,
        durableMemoryContext: durableMemory?.context ?? "",
        sessionSummary,
        agentMode: effectiveMode,
        permissionMode,
        tasks,
        agentMemoryEvents,
        chatSessions,
        activeChatId,
        searchChatsEnabled,
        generateMemoryEnabled,
        question,
      });

      stepCounterRef.current = 0;
      activeTurnIdRef.current = id;
      setIsSubmitting(true);
      setInput("");
      // Clear composer attachments only when this submit consumed them (not on retry/edit
      // which supplies its own array via branchOptions).
      if (!branchOptions?.attachments) {
        setComposerAttachments([]);
      }
      setComposerError(null);
      setRouteError(null);
      setTurns((current) => [...current, nextTurn]);
      setPreferredChild((current) => ({
        ...current,
        [parentId ?? "root"]: id,
      }));
      setTasks((current) => [runTask, ...current].slice(0, 100));
      setPromptHistory((current) => pushPromptHistory(current, question));
      recordAgentEvent({
        kind: "ask",
        title: `${AGENT_MODES.find((item) => item.key === effectiveMode)?.label ?? "Ask"} request`,
        detail: question,
        mode: effectiveMode,
        turnId: id,
      });

      try {
        if (isTauriRuntime()) {
          const attachmentsForBackend: AskAttachmentInput[] = turnAttachments.map(
            (attachment) => ({
              kind: "image",
              mime_type: attachment.mimeType,
              data: attachment.data,
              filename: attachment.filename ?? null,
            }),
          );
          const runId = await startAskRun(
            question,
            context,
            attachmentsForBackend,
            effectiveMode,
            permissionMode,
            autoConfirmTools,
            reasoningDepth,
          );
          runToTurnRef.current.set(runId, id);
          setActiveRunId(runId);
          setTurns((current) =>
            current.map((turn) => (turn.id === id ? { ...turn, runId } : turn)),
          );
          // Wait for the run to terminate before unblocking the composer. We watch
          // the turn status set by the streaming event listener.
          const finalTurn = await waitForTurnSettlement(setTurns, id);
          if (finalTurn.status === "done") {
            recordAgentEvent({
              kind: "answer",
              title: `Answered: ${truncateText(question, 80)}`,
              detail: truncateText(finalTurn.answer.replace(/\s+/g, " ").trim(), 900),
              mode: effectiveMode,
              refs: finalTurn.refs?.length ?? 0,
              turnId: id,
            });
            updateTaskStatus(runTask.id, "completed");
          } else if (finalTurn.status === "cancelled") {
            recordAgentEvent({
              kind: "error",
              title: `Ask cancelled: ${truncateText(question, 80)}`,
              detail: "User stopped the run.",
              mode: effectiveMode,
              turnId: id,
            });
            updateTaskStatus(runTask.id, "blocked");
          } else if (finalTurn.status === "error") {
            recordAgentEvent({
              kind: "error",
              title: `Ask failed: ${truncateText(question, 80)}`,
              detail: finalTurn.error ?? "unknown error",
              mode: effectiveMode,
              turnId: id,
            });
            updateTaskStatus(runTask.id, "blocked");
          }
        } else {
          // Non-Tauri runtime fallback (preview/dev): use legacy non-streaming command.
          const result: AskSearchResult = await askSearch(question, context);
          setTurns((current) =>
            current.map((turn) =>
              turn.id === id
                ? {
                    ...turn,
                    answer: result.answer,
                    refs: result.refs ?? [],
                    questions: result.questions ?? [],
                    status: "done",
                  }
                : turn,
            ),
          );
          recordAgentEvent({
            kind: "answer",
            title: `Answered: ${truncateText(question, 80)}`,
            detail: truncateText(result.answer.replace(/\s+/g, " ").trim(), 900),
            mode: effectiveMode,
            refs: result.refs?.length ?? 0,
            turnId: id,
          });
          updateTaskStatus(runTask.id, "completed");
        }
      } catch (caught) {
        const error = String(caught);
        setTurns((current) =>
          current.map((turn) =>
            turn.id === id ? { ...turn, status: "error", error } : turn,
          ),
        );
        recordAgentEvent({
          kind: "error",
          title: `Ask failed: ${truncateText(question, 80)}`,
          detail: error,
          mode: effectiveMode,
          turnId: id,
        });
        updateTaskStatus(runTask.id, "blocked");
      } finally {
        activeTurnIdRef.current = null;
        setIsSubmitting(false);
        inputRef.current?.focus();
      }
    },
    [
      activePath,
      agentMode,
      composerAttachments,
      isSubmitting,
      memoryEntries,
      agentMemoryEvents,
      activeChatId,
      chatSessions,
      generateMemoryEnabled,
      permissionMode,
      reasoningDepth,
      projectMemory,
      recordAgentEvent,
      searchChatsEnabled,
      sessionSummary,
      tasks,
      turns,
      updateTaskStatus,
    ],
  );

  const retryTurn = useCallback(
    async (target: AskTurn) => {
      if (isSubmitting) return;
      await submitQuestion(target.question, target.mode, {
        parentId: target.parentId,
        forkOf: target.id,
        attachments: target.attachments ?? [],
      });
    },
    [isSubmitting, submitQuestion],
  );

  const submitEditedTurn = useCallback(
    async (target: AskTurn, newQuestion: string) => {
      const trimmed = newQuestion.trim();
      if (!trimmed || isSubmitting) return;
      setEditingTurnId(null);
      await submitQuestion(trimmed, target.mode, {
        parentId: target.parentId,
        forkOf: target.id,
        attachments: target.attachments ?? [],
      });
    },
    [isSubmitting, submitQuestion],
  );

  const ingestAttachmentFiles = useCallback(async (files: File[]) => {
    if (files.length === 0) return;
    const accepted: AskAttachment[] = [];
    const errors: string[] = [];
    for (const file of files) {
      if (!file.type.startsWith("image/")) {
        errors.push(`${file.name || "file"}: only images are supported`);
        continue;
      }
      if (file.size > MAX_ATTACHMENT_BYTES) {
        errors.push(`${file.name || "image"}: over ${Math.round(MAX_ATTACHMENT_BYTES / 1_000_000)} MB`);
        continue;
      }
      try {
        const data = await fileToBase64(file);
        accepted.push({
          id: makeLocalId("att"),
          kind: "image",
          mimeType: file.type,
          data,
          filename: file.name || undefined,
          size: file.size,
        });
      } catch (caught) {
        errors.push(`${file.name || "image"}: ${String(caught)}`);
      }
    }
    setComposerAttachments((current) => [...current, ...accepted].slice(0, MAX_ATTACHMENTS));
    setComposerError(errors[0] ?? null);
  }, []);

  const removeComposerAttachment = useCallback((id: string) => {
    setComposerAttachments((current) => current.filter((attachment) => attachment.id !== id));
  }, []);

  const handleComposerPaste = useCallback(
    (event: React.ClipboardEvent<HTMLTextAreaElement>) => {
      const items = event.clipboardData?.items;
      if (!items) return;
      const files: File[] = [];
      for (let i = 0; i < items.length; i += 1) {
        const item = items[i];
        if (item.kind === "file") {
          const file = item.getAsFile();
          if (file && file.type.startsWith("image/")) {
            files.push(file);
          }
        }
      }
      if (files.length > 0) {
        event.preventDefault();
        void ingestAttachmentFiles(files);
      }
    },
    [ingestAttachmentFiles],
  );

  const handleComposerDrop = useCallback(
    (event: React.DragEvent<HTMLDivElement>) => {
      event.preventDefault();
      const files = Array.from(event.dataTransfer.files ?? []);
      if (files.length > 0) {
        void ingestAttachmentFiles(files);
      }
    },
    [ingestAttachmentFiles],
  );

  const stopVoiceInput = useCallback(() => {
    mediaRecorderRef.current?.stop();
  }, []);

  const startVoiceInput = useCallback(async () => {
    // If already recording, stop it
    if (voiceState === "listening") {
      stopVoiceInput();
      return;
    }

    setComposerError(null);
    audioChunksRef.current = [];

    let stream: MediaStream;
    try {
      stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    } catch {
      setComposerError("Microphone access denied. Allow microphone in System Settings.");
      return;
    }

    const mimeType = MediaRecorder.isTypeSupported("audio/webm;codecs=opus")
      ? "audio/webm;codecs=opus"
      : MediaRecorder.isTypeSupported("audio/mp4")
      ? "audio/mp4"
      : "audio/webm";

    const recorder = new MediaRecorder(stream, { mimeType });
    mediaRecorderRef.current = recorder;

    recorder.ondataavailable = (e) => {
      if (e.data.size > 0) audioChunksRef.current.push(e.data);
    };

    recorder.onstop = async () => {
      stream.getTracks().forEach((t) => t.stop());
      setVoiceState("transcribing");
      try {
        const blob = new Blob(audioChunksRef.current, { type: mimeType });
        const arrayBuffer = await blob.arrayBuffer();
        const uint8 = new Uint8Array(arrayBuffer);
        const binary = uint8.reduce((s, b) => s + String.fromCharCode(b), "");
        const audio_base64 = btoa(binary);
        const text = await transcribeVoiceInput({ audio_base64, mime_type: mimeType });
        setInput((current) => mergeVoiceTranscript(current, text.trim()));
      } catch (err) {
        setComposerError(`Transcription failed: ${String(err)}`);
      } finally {
        setVoiceState("idle");
      }
    };

    recorder.start();
    setVoiceState("listening");
  }, [voiceState, stopVoiceInput]);

  const switchVariant = useCallback(
    (current: AskTurn, direction: -1 | 1) => {
      const siblings = childrenIndex.get(current.parentId ?? null) ?? [];
      if (siblings.length <= 1) return;
      const idx = siblings.findIndex((turn) => turn.id === current.id);
      if (idx === -1) return;
      const nextIdx = (idx + direction + siblings.length) % siblings.length;
      const nextTurn = siblings[nextIdx];
      setPreferredChild((existing) => ({
        ...existing,
        [current.parentId ?? "root"]: nextTurn.id,
      }));
    },
    [childrenIndex],
  );

  useEffect(() => {
    const routeQuery = params.get("q")?.trim() ?? "";
    if (!routeQuery || lastRouteQueryRef.current === routeQuery) {
      return;
    }
    lastRouteQueryRef.current = routeQuery;
    void submitQuestion(routeQuery);
  }, [params, submitQuestion]);

  useEffect(() => {
    if (activePath.length > 0) {
      turnVirtualizer.scrollToIndex(activePath.length - 1, { align: "end" });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [turns]);

  function runComposerCommand(value: string) {
    const trimmed = value.trim();
    if (!trimmed.startsWith("/")) {
      return false;
    }

    const commandToken = trimmed.slice(1).split(/\s+/, 1)[0]?.toLowerCase() ?? "";
    const rest = trimmed.slice(commandToken.length + 1).trim();

    if (commandToken === "ask" || commandToken === "research" || commandToken === "act") {
      const nextMode = commandToken as AgentMode;
      if (rest) {
        void submitQuestion(rest, nextMode);
      } else {
        setAgentMode(nextMode);
        setInput("");
      }
      return true;
    }

    if (commandToken === "remember") {
      if (!rest) {
        setInput("/remember ");
        inputRef.current?.focus();
        return true;
      }
      addMemoryEntry({
        type: "project",
        title: makeMemoryTitle(rest),
        body: rest,
      });
      setInput("");
      setDrawer("memory");
      return true;
    }

    const drawerCommands: Record<string, DrawerKind> = {
      chats: "chats",
      forget: "memory",
      help: "commands",
      history: "history",
      memory: "memory",
      past: "chats",
      settings: "settings",
      tasks: "tasks",
      tools: "tools",
    };

    const nextDrawer = drawerCommands[commandToken];
    if (nextDrawer) {
      setDrawer(nextDrawer);
      setInput("");
      return true;
    }

    if (commandToken === "compact") {
      setTurns((current) => current.slice(-6));
      setInput("");
      return true;
    }

    if (commandToken === "new") {
      startNewChat();
      return true;
    }

    if (commandToken === "clear") {
      clearSession();
      setInput("");
      return true;
    }

    return false;
  }

  function submitComposerValue(value: string) {
    if (runComposerCommand(value)) {
      return;
    }
    void submitQuestion(value);
  }

  function applyCommand(name: string) {
    if (name === "ask" || name === "research" || name === "act" || name === "remember") {
      setInput(`/${name} `);
      inputRef.current?.focus();
      return;
    }
    runComposerCommand(`/${name}`);
  }

  function applyMention(token: string) {
    const activeMention = findActiveMention(input);
    if (!activeMention) {
      setInput((current) => `${current}${current.endsWith(" ") || !current ? "" : " "}${token} `);
      inputRef.current?.focus();
      return;
    }
    setInput(`${input.slice(0, activeMention.start)}${token} ${input.slice(activeMention.end)}`);
    inputRef.current?.focus();
  }

  function reusePrompt(prompt: string) {
    setInput(prompt);
    setDrawer(null);
    inputRef.current?.focus();
  }

  function startNewChat() {
    if (turns.length > 0) {
      setChatSessions((current) => upsertChatSession(current, activeChatId, turns, agentMode));
    }
    const nextChatId = makeLocalId("chat");
    setActiveChatId(nextChatId);
    setTurns([]);
    setPreferredChild({});
    setInput("");
    setDrawer(null);
    sessionStorage.removeItem(ASK_TURNS_KEY);
    sessionStorage.removeItem(ASK_PREFERRED_CHILD_STORAGE_ID);
    // New chat → no turns persisted yet, but don't drop the global set since
    // other in-memory chats may have terminal turns we still want to skip-on-rerender.
    recordAgentEvent({
      kind: "session",
      title: "Started new chat",
      detail: "Archived the current Ask chat and opened a clean thread.",
      sessionId: nextChatId,
    });
    inputRef.current?.focus();
  }

  async function openChatSession(session: AskChatSession) {
    if (turns.length > 0) {
      setChatSessions((current) => upsertChatSession(current, activeChatId, turns, agentMode));
    }
    setActiveChatId(session.id);
    setAgentMode(session.mode);
    let restored = migrateTurnChain(session.turns);
    // Hydrate from server if the in-memory copy is empty (or stale).
    if (isTauriRuntime() && restored.length === 0) {
      try {
        const detail = await getAskChat(session.id);
        restored = detail.turns.map(serverTurnToAskTurn);
        // Mark already-persisted turns so we don't re-persist on render.
        for (const turn of restored) persistedTurnsRef.current.add(turn.id);
        persistedChatsRef.current.add(session.id);
      } catch (caught) {
        console.warn("getAskChat failed", caught);
      }
    }
    setTurns(restored);
    setPreferredChild(rebuildPreferredChildFromChain(restored));
    setInput("");
    setDrawer(null);
    recordAgentEvent({
      kind: "session",
      title: "Opened past chat",
      detail: session.title,
      sessionId: session.id,
    });
    inputRef.current?.focus();
  }

  function deleteChatSession(id: string) {
    setChatSessions((current) => current.filter((session) => session.id !== id));
    if (id === activeChatId) {
      const nextChatId = makeLocalId("chat");
      setActiveChatId(nextChatId);
      setTurns([]);
      setPreferredChild({});
      sessionStorage.removeItem(ASK_TURNS_KEY);
      sessionStorage.removeItem(ASK_PREFERRED_CHILD_STORAGE_ID);
    }
    if (isTauriRuntime()) {
      void deleteAskChat(id).catch((caught) => console.warn("deleteAskChat failed", caught));
    }
    persistedChatsRef.current.delete(id);
  }

  function clearAgentMemory() {
    setAgentMemoryEvents([]);
    localStorage.removeItem(ASK_AGENT_MEMORY_EVENTS_KEY);
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    submitComposerValue(input);
  }

  function handleComposerKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      setInput("/");
      inputRef.current?.focus();
      return;
    }

    if (event.key === "ArrowUp" && !input.trim() && promptHistory.length > 0) {
      event.preventDefault();
      setInput(promptHistory[0] ?? "");
      return;
    }

    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submitComposerValue(input);
    }
  }

  function answerClarification(question: AskUserQuestion, answer: string) {
    if (answer.trim() === "Other:") {
      setInput(`Answer to "${question.question}": `);
      inputRef.current?.focus();
      return;
    }
    recordAgentEvent({
      kind: "clarification",
      title: question.question,
      detail: answer,
    });
    void submitQuestion(`Answer to "${question.question}": ${answer}`);
  }

  function clearSession() {
    setTurns([]);
    setPreferredChild({});
    setChatSessions((current) => current.filter((session) => session.id !== activeChatId));
    setActiveChatId(makeLocalId("chat"));
    sessionStorage.removeItem(ASK_TURNS_KEY);
    sessionStorage.removeItem(ASK_PREFERRED_CHILD_STORAGE_ID);
  }

  function exportCurrentChatMarkdown() {
    if (activePath.length === 0) {
      setRouteError("Nothing to export — chat is empty.");
      return;
    }
    const markdown = chatToMarkdown(activePath, makeChatTitle(turns), agentMode);
    const blob = new Blob([markdown], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `${slugifyFilename(makeChatTitle(turns))}.md`;
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  }

  return (
    <div className="min-h-full bg-white text-zinc-950">
      <div className="mx-auto flex min-h-[calc(100vh-65px)] max-w-5xl flex-col px-4 sm:px-6">
        <div className="flex items-center justify-between gap-4 border-b border-zinc-100 py-3">
          <div className="flex items-center gap-2.5">
            <BrandMark />
            <h1 className="text-sm font-semibold text-zinc-950">Ask Trace</h1>
          </div>
          <div className="flex items-center gap-0.5">
            <button className="ask-toolbar-button gap-1.5 px-2.5" onClick={startNewChat} type="button">
              <Plus size={13} />
              New
            </button>
            <button className="icon-btn" onClick={() => setDrawer("chats")} title="Past chats" type="button">
              <MessageSquareText size={15} />
            </button>
            <button className="icon-btn" onClick={() => setDrawer("memory")} title="Memory" type="button">
              <BookOpen size={15} />
            </button>
            <button className="icon-btn" onClick={() => setDrawer("ingest")} title="Ingest conversation" type="button">
              <Download size={15} />
            </button>
            <button
              className="icon-btn"
              disabled={activePath.length === 0}
              onClick={exportCurrentChatMarkdown}
              title="Export as Markdown"
              type="button"
            >
              <ArrowUpRight size={15} />
            </button>
            <button className="icon-btn" onClick={() => setDrawer("settings")} title="Agent settings" type="button">
              <Settings2 size={15} />
            </button>
          </div>
        </div>

        {routeError ? <div className="mx-auto mb-3 w-full max-w-3xl notice notice-error">{routeError}</div> : null}
        {!isTauriRuntime() ? (
          <div className="mx-auto mb-3 w-full max-w-3xl rounded-xl border border-amber-200/70 bg-amber-50/80 px-3 py-2 text-[12px] font-medium text-amber-900">
            Trace Ask needs the macOS app runtime for local workspace tools.
          </div>
        ) : null}

        <div ref={scrollContainerRef} className="min-h-0 flex-1 overflow-y-auto px-1 pb-4 pt-2">
          {activePath.length === 0 ? (
            <AnimatePresence mode="wait">
              <motion.div
                key="empty"
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0, y: -4 }}
                initial={{ opacity: 0, y: 8 }}
                transition={{ duration: 0.16, ease: "easeOut" }}
              >
                <EmptyState onPrompt={submitQuestion} />
              </motion.div>
            </AnimatePresence>
          ) : (
            <div className="mx-auto max-w-3xl py-4" style={{ height: `${turnVirtualizer.getTotalSize()}px`, position: "relative" }}>
              {turnVirtualizer.getVirtualItems().map((vItem) => {
                const turn = activePath[vItem.index];
                const siblings = childrenIndex.get(turn.parentId ?? null) ?? [];
                const variantIndex =
                  siblings.findIndex((sibling) => sibling.id === turn.id) + 1;
                return (
                  <div
                    key={turn.id}
                    data-index={vItem.index}
                    ref={turnVirtualizer.measureElement}
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${vItem.start}px)`,
                      paddingBottom: "1.5rem",
                    }}
                  >
                    <TurnView
                      autoConfirmTools={autoConfirmTools}
                      isEditing={editingTurnId === turn.id}
                      isSubmitting={isSubmitting}
                      onAnswerClarification={answerClarification}
                      onCancelEdit={() => setEditingTurnId(null)}
                      onEdit={() => setEditingTurnId(turn.id)}
                      onRetry={() => void retryTurn(turn)}
                      onSubmitEdit={(value) => void submitEditedTurn(turn, value)}
                      onSwitchVariant={(direction) => switchVariant(turn, direction)}
                      onToggleAutoConfirm={toggleAutoConfirmTool}
                      siblingCount={siblings.length}
                      turn={turn}
                      variantIndex={variantIndex}
                    />
                  </div>
                );
              })}
            </div>
          )}
        </div>

        <form
          className="sticky bottom-0 bg-white px-1 pb-4 pt-3"
          onSubmit={handleSubmit}
        >
          <div
            className="mx-auto max-w-3xl rounded-2xl border border-zinc-100 bg-white p-3 shadow-[0_2px_12px_rgba(0,0,0,0.06)] transition-shadow focus-within:border-zinc-100 focus-within:shadow-[0_4px_20px_rgba(0,0,0,0.09)]"
            onDragOver={(event) => event.preventDefault()}
            onDrop={handleComposerDrop}
          >
            <ComposerAssist
              input={input}
              onCommand={applyCommand}
              onMention={applyMention}
              promptHistory={promptHistory}
            />
            {composerAttachments.length > 0 ? (
              <div className="mb-2 flex flex-wrap gap-2">
                {composerAttachments.map((attachment) => (
                  <ComposerAttachmentChip
                    attachment={attachment}
                    key={attachment.id}
                    onRemove={() => removeComposerAttachment(attachment.id)}
                  />
                ))}
              </div>
            ) : null}
            {composerError ? (
              <p className="mb-2 text-[11px] text-red-600">{composerError}</p>
            ) : null}
            <textarea
              ref={inputRef}
              className="max-h-52 min-h-[5rem] w-full resize-none bg-transparent px-1 py-1 text-[14px] leading-6 text-zinc-950 outline-none placeholder:text-zinc-400"
              disabled={isSubmitting}
              onChange={(event) => setInput(event.currentTarget.value)}
              onKeyDown={handleComposerKeyDown}
              onPaste={handleComposerPaste}
              placeholder={
                agentMode === "research"
                  ? "Research your workspace — I'll cite every source…"
                  : "Ask anything about your work — I'll cite sources"
              }
              rows={2}
              value={input}
            />
            <input
              accept={ACCEPTED_ATTACHMENT_MIMES}
              className="hidden"
              multiple
              onChange={(event) => {
                const files = Array.from(event.currentTarget.files ?? []);
                if (files.length > 0) {
                  void ingestAttachmentFiles(files);
                }
                event.currentTarget.value = "";
              }}
              ref={filePickerRef}
              type="file"
            />
            <div className="mt-2 flex items-center justify-between gap-2 border-t border-zinc-100 pt-2">
              <div className="flex items-center">
                <AgentModeSelector mode={agentMode} onModeChange={setAgentMode} />
                <DeepReasoningToggle depth={reasoningDepth} onChange={setReasoningDepth} />
              </div>
              <div className="flex items-center gap-0.5">
                <button
                  aria-label="Attach image"
                  className="icon-btn"
                  disabled={isSubmitting || composerAttachments.length >= MAX_ATTACHMENTS}
                  onClick={() => filePickerRef.current?.click()}
                  title="Attach image — paste, drop, or click"
                  type="button"
                >
                  <Paperclip size={14} />
                </button>
                <button
                  aria-label={voiceState === "listening" ? "Stop recording" : "Start voice input"}
                  aria-pressed={voiceState === "listening"}
                  className={[
                    "icon-btn",
                    voiceState === "listening" ? "!bg-red-50 !text-red-500 hover:!bg-red-100" : "",
                    voiceState === "transcribing" ? "opacity-50" : "",
                  ].join(" ")}
                  disabled={voiceState === "transcribing"}
                  onClick={startVoiceInput}
                  title={
                    voiceState === "listening"
                      ? "Stop recording"
                      : voiceState === "transcribing"
                      ? "Transcribing…"
                      : "Voice input"
                  }
                  type="button"
                >
                  <Mic size={14} />
                </button>
                <button
                  className="icon-btn"
                  onClick={() => setDrawer("commands")}
                  title="Commands ( / )"
                  type="button"
                >
                  <Command size={14} />
                </button>
                <button
                  className="icon-btn"
                  onClick={() => setDrawer("settings")}
                  title={PERMISSION_MODE_CONTEXT[permissionMode]}
                  type="button"
                >
                  <ShieldCheck size={14} />
                </button>
                <button
                  className="icon-btn"
                  onClick={() => setDrawer("tools")}
                  title={`${TOOL_SPECS.length} tools`}
                  type="button"
                >
                  <Wrench size={14} />
                </button>
                {isSubmitting && activeRunId ? (
                  <button
                    className="ml-1 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-red-500 text-white transition-colors hover:bg-red-600"
                    onClick={() => void stopActiveRun()}
                    title="Stop"
                    type="button"
                  >
                    <Square fill="currentColor" size={10} />
                  </button>
                ) : (
                  <button
                    className="ml-1 inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-zinc-950 text-white transition-colors hover:bg-zinc-800 disabled:bg-zinc-100 disabled:text-zinc-400"
                    disabled={(!input.trim() && composerAttachments.length === 0) || isSubmitting}
                    title="Send"
                    type="submit"
                  >
                    <Send size={14} />
                  </button>
                )}
              </div>
            </div>
          </div>
        </form>
      </div>

      <SideDrawer
        activeChatId={activeChatId}
        agentMemoryEvents={agentMemoryEvents}
        chatSessions={chatSessions}
        drawer={drawer}
        memoryEntries={memoryEntries}
        memoryRefs={memoryRefs}
        onAddMemory={addMemoryEntry}
        onClearAgentMemory={clearAgentMemory}
        onClearSession={clearSession}
        onClose={() => setDrawer(null)}
        onDeleteChat={deleteChatSession}
        onOpenChat={openChatSession}
        onRemoveMemory={removeMemoryEntry}
        onRemoveTask={removeTask}
        onReusePrompt={reusePrompt}
        onSetPermissionMode={setPermissionMode}
        onUpdateTaskStatus={updateTaskStatus}
        permissionMode={permissionMode}
        promptHistory={promptHistory}
        projectMemory={projectMemory}
        searchChatsEnabled={searchChatsEnabled}
        setGenerateMemoryEnabled={setGenerateMemoryEnabled}
        setSearchChatsEnabled={setSearchChatsEnabled}
        sessionSummary={sessionSummary}
        generateMemoryEnabled={generateMemoryEnabled}
        setProjectMemory={setProjectMemory}
        tasks={tasks}
        turns={turns}
        usedToolCounts={usedToolCounts}
      />
    </div>
  );
}


// Storage loaders, type guards, tree helpers, server↔local converters, and
// misc utilities live in `./AskWorkspace/utils`.
