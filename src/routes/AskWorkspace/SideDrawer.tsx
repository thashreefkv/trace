// Right-anchored slide-in drawer with sub-panels (tools, memory, tasks,
// commands, history, chats, settings, ingest). Extracted from
// AskWorkspace.tsx (E7).

import { useMemo, useState, type FormEvent, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Brain,
  CheckCircle2,
  ChevronDown,
  Clock3,
  CornerDownLeft,
  ListTodo,
  MessageSquareText,
  Plus,
  Search,
  ShieldCheck,
  Trash2,
  X,
} from "lucide-react";

import { searchAskChats } from "../../lib/ipc";
import type {
  AskChatSearchHit,
  SearchResult,
} from "../../lib/types";

import { COMPOSER_COMMANDS, MEMORY_TYPE_META, MENTION_TARGETS, PERMISSION_MODE_CONTEXT, TOOL_SPECS } from "./constants";
import { ReferenceLink } from "./Citations";
import { IngestPanel } from "./Ingest";
import { Metric, PanelRow, PanelSurface } from "./panels";
import { TaskStatusIcon, agentMemoryIcon } from "./icons";
import {
  drawerTitles,
  formatRelativeDate,
  getMemoryUpdatedAt,
  groupMemoryEntries,
  isTauriRuntime,
  makeMemoryTitle,
  permissionModeLabel,
  taskStatusLabel,
} from "./utils";
import type {
  AgentMemoryEvent,
  AskChatSession,
  AskTask,
  AskTaskStatus,
  AskTurn,
  ComposerCommand,
  DrawerKind,
  MemoryEntry,
  MemoryType,
  PermissionMode,
  ToolCategory,
} from "./state";

export function SideDrawer({
  activeChatId,
  agentMemoryEvents,
  chatSessions,
  drawer,
  memoryEntries,
  memoryRefs,
  onAddMemory,
  onClearAgentMemory,
  onClearSession,
  onClose,
  onDeleteChat,
  onOpenChat,
  onRemoveMemory,
  onRemoveTask,
  onReusePrompt,
  onSetPermissionMode,
  onUpdateTaskStatus,
  permissionMode,
  promptHistory,
  projectMemory,
  searchChatsEnabled,
  setGenerateMemoryEnabled,
  setSearchChatsEnabled,
  generateMemoryEnabled,
  sessionSummary,
  setProjectMemory,
  tasks,
  turns,
  usedToolCounts,
}: {
  activeChatId: string;
  agentMemoryEvents: AgentMemoryEvent[];
  chatSessions: AskChatSession[];
  drawer: DrawerKind;
  memoryEntries: MemoryEntry[];
  memoryRefs: SearchResult[];
  onAddMemory: (entry: Omit<MemoryEntry, "id" | "updatedAt">) => void;
  onClearAgentMemory: () => void;
  onClearSession: () => void;
  onClose: () => void;
  onDeleteChat: (id: string) => void;
  onOpenChat: (session: AskChatSession) => void;
  onRemoveMemory: (id: string) => void;
  onRemoveTask: (id: string) => void;
  onReusePrompt: (prompt: string) => void;
  onSetPermissionMode: (mode: PermissionMode) => void;
  onUpdateTaskStatus: (id: string, status: AskTaskStatus) => void;
  permissionMode: PermissionMode;
  promptHistory: string[];
  projectMemory: string;
  searchChatsEnabled: boolean;
  setGenerateMemoryEnabled: (enabled: boolean) => void;
  setSearchChatsEnabled: (enabled: boolean) => void;
  generateMemoryEnabled: boolean;
  sessionSummary: string;
  setProjectMemory: (value: string) => void;
  tasks: AskTask[];
  turns: AskTurn[];
  usedToolCounts: Map<string, number>;
}) {
  const title = drawer ? drawerTitles(drawer) : null;

  return (
    <AnimatePresence>
      {drawer ? (
        <>
          <motion.button
            animate={{ opacity: 1 }}
            aria-label="Close side panel"
            className="fixed inset-0 z-50 cursor-default bg-black/10"
            exit={{ opacity: 0 }}
            initial={{ opacity: 0 }}
            onClick={onClose}
            type="button"
          />
          <motion.aside
            aria-labelledby="ask-side-panel-title"
            aria-modal="true"
            animate={{ x: 0 }}
            className="fixed bottom-0 right-0 top-0 z-50 flex w-full max-w-[440px] flex-col border-l border-zinc-100 bg-white shadow-2xl"
            exit={{ x: "100%" }}
            initial={{ x: "100%" }}
            role="dialog"
            transition={{ type: "spring", stiffness: 420, damping: 38 }}
          >
            <div className="flex items-center justify-between border-b border-zinc-100 px-4 py-3">
              <div>
                <p className="page-kicker">{title?.kicker}</p>
                <h2 className="text-[15px] font-semibold text-zinc-950" id="ask-side-panel-title">
                  {title?.title}
                </h2>
              </div>
              <button
                aria-label={`Close ${title?.title ?? "side panel"}`}
                className="inline-flex h-8 w-8 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700"
                onClick={onClose}
                type="button"
              >
                <X size={16} />
              </button>
            </div>
            <div className="min-h-0 flex-1 overflow-y-auto p-4">
              {drawer === "tools" ? <ToolHarness usedToolCounts={usedToolCounts} /> : null}
              {drawer === "memory" ? (
                <MemoryPanel
                  agentMemoryEvents={agentMemoryEvents}
                  generateMemoryEnabled={generateMemoryEnabled}
                  memoryEntries={memoryEntries}
                  onAddMemory={onAddMemory}
                  onClearAgentMemory={onClearAgentMemory}
                  onClearSession={onClearSession}
                  onRemoveMemory={onRemoveMemory}
                  projectMemory={projectMemory}
                  refs={memoryRefs}
                  searchChatsEnabled={searchChatsEnabled}
                  setGenerateMemoryEnabled={setGenerateMemoryEnabled}
                  setSearchChatsEnabled={setSearchChatsEnabled}
                  sessionSummary={sessionSummary}
                  setProjectMemory={setProjectMemory}
                  turns={turns}
                />
              ) : null}
              {drawer === "tasks" ? (
                <TaskPanel
                  onRemoveTask={onRemoveTask}
                  onUpdateTaskStatus={onUpdateTaskStatus}
                  tasks={tasks}
                />
              ) : null}
              {drawer === "commands" ? <CommandPanel /> : null}
              {drawer === "history" ? (
                <HistoryPanel onReusePrompt={onReusePrompt} promptHistory={promptHistory} />
              ) : null}
              {drawer === "chats" ? (
                <ChatSessionsPanel
                  activeChatId={activeChatId}
                  chatSessions={chatSessions}
                  onDeleteChat={onDeleteChat}
                  onOpenChat={onOpenChat}
                />
              ) : null}
              {drawer === "settings" ? (
                <SettingsPanel
                  permissionMode={permissionMode}
                  setPermissionMode={onSetPermissionMode}
                />
              ) : null}
              {drawer === "ingest" ? <IngestPanel onClose={onClose} /> : null}
            </div>
          </motion.aside>
        </>
      ) : null}
    </AnimatePresence>
  );
}

function ToolHarness({ usedToolCounts }: { usedToolCounts: Map<string, number> }) {
  const categories: { key: ToolCategory; label: string }[] = [
    { key: "memory", label: "Memory" },
    { key: "read", label: "Read" },
    { key: "email", label: "Email" },
    { key: "write", label: "Write" },
    { key: "files", label: "Files" },
    { key: "web", label: "Web (Research)" },
    { key: "clarify", label: "Ask user" },
  ];

  return (
    <PanelSurface>
      <PanelRow>
        <div className="grid grid-cols-3 gap-4">
          <Metric label="Tools" value={String(TOOL_SPECS.length)} />
          <Metric label="Used" value={String(usedToolCounts.size)} />
          <Metric label="Calls" value={String([...usedToolCounts.values()].reduce((sum, value) => sum + value, 0))} />
        </div>
      </PanelRow>
      {categories.map((category) => (
        <PanelRow key={category.key}>
          <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-zinc-400">
            {category.label}
          </p>
          <div className="flex flex-wrap gap-1.5">
            {TOOL_SPECS.filter((tool) => tool.category === category.key).map((tool) => {
              const count = usedToolCounts.get(tool.name) ?? 0;
              return (
                <span
                  className={[
                    "inline-flex items-center gap-1 rounded-md border px-2 py-1 text-[11px] font-medium",
                    count > 0
                      ? "border-violet-200 bg-violet-50 text-violet-700"
                      : "border-zinc-100 bg-white text-zinc-500",
                  ].join(" ")}
                  key={tool.name}
                  title={tool.name}
                >
                  {tool.icon}
                  {tool.label}
                  {count > 0 ? <span className="text-[10px] text-violet-400">{count}</span> : null}
                </span>
              );
            })}
          </div>
        </PanelRow>
      ))}
    </PanelSurface>
  );
}

function MemoryPanel({
  agentMemoryEvents,
  generateMemoryEnabled,
  memoryEntries,
  onAddMemory,
  onClearAgentMemory,
  onClearSession,
  onRemoveMemory,
  projectMemory,
  refs,
  searchChatsEnabled,
  setGenerateMemoryEnabled,
  setSearchChatsEnabled,
  sessionSummary,
  setProjectMemory,
  turns,
}: {
  agentMemoryEvents: AgentMemoryEvent[];
  generateMemoryEnabled: boolean;
  memoryEntries: MemoryEntry[];
  onAddMemory: (entry: Omit<MemoryEntry, "id" | "updatedAt">) => void;
  onClearAgentMemory: () => void;
  onClearSession: () => void;
  onRemoveMemory: (id: string) => void;
  projectMemory: string;
  refs: SearchResult[];
  searchChatsEnabled: boolean;
  setGenerateMemoryEnabled: (enabled: boolean) => void;
  setSearchChatsEnabled: (enabled: boolean) => void;
  sessionSummary: string;
  setProjectMemory: (value: string) => void;
  turns: AskTurn[];
}) {
  const [manageOpen, setManageOpen] = useState(false);
  const [draftType, setDraftType] = useState<MemoryType>("project");
  const [draftTitle, setDraftTitle] = useState("");
  const [draftBody, setDraftBody] = useState("");
  const entriesByType = useMemo(() => groupMemoryEntries(memoryEntries), [memoryEntries]);
  const updatedAt = getMemoryUpdatedAt(memoryEntries, agentMemoryEvents);

  function submitMemory(event: FormEvent) {
    event.preventDefault();
    const title = draftTitle.trim() || makeMemoryTitle(draftBody);
    const body = draftBody.trim();
    if (!title || !body) {
      return;
    }
    onAddMemory({ type: draftType, title, body });
    setDraftTitle("");
    setDraftBody("");
  }

  return (
    <div className="space-y-4">
      <PanelSurface>
        <MemorySettingRow
          description="Allow Trace to search for relevant details in past chats."
          enabled={searchChatsEnabled}
          learnMoreHref="https://support.claude.com/en/articles/11817273-use-claude-s-chat-search-and-memory-to-build-on-previous-context"
          onChange={setSearchChatsEnabled}
          title="Search and reference chats"
        />
        <MemorySettingRow
          description="Allow Trace to remember relevant context from chats and project work."
          enabled={generateMemoryEnabled}
          learnMoreHref="https://support.claude.com/en/articles/11817273-use-claude-s-chat-search-and-memory-to-build-on-previous-context"
          onChange={setGenerateMemoryEnabled}
          title="Generate memory from chat history"
        />
      </PanelSurface>

      <button
        className="flex w-full items-center gap-2 rounded-xl border border-zinc-100 bg-white px-4 py-3 text-left shadow-[0_2px_12px_rgba(0,0,0,0.06)] transition-colors hover:bg-zinc-50"
        onClick={() => setManageOpen((value) => !value)}
        type="button"
      >
        <Brain className="shrink-0 text-violet-400" size={14} />
        <span className="min-w-0 flex-1 text-[12px] font-semibold text-zinc-900">
          View and manage memory
        </span>
        <span className="text-[11px] text-zinc-400">{updatedAt ? formatRelativeDate(updatedAt) : "No saved memory"}</span>
        <ChevronDown
          className={["shrink-0 text-zinc-400 transition-transform", manageOpen ? "rotate-180" : ""].join(" ")}
          size={14}
        />
      </button>

      {manageOpen ? (
        <PanelSurface>
          <PanelRow>
            <div className="flex items-start justify-between gap-4">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-semibold text-zinc-950">Session memory</p>
                <p className="mt-1 text-[12px] leading-5 text-zinc-500">
                  {sessionSummary || "The session summary appears after your first completed answer."}
                </p>
              </div>
              <div className="grid shrink-0 grid-cols-2 gap-4">
                <Metric label="Turns" value={String(turns.length)} />
                <Metric label="Refs" value={String(refs.length)} />
              </div>
            </div>
            <button
              className="mt-3 text-[12px] font-medium text-zinc-500 underline decoration-zinc-200 underline-offset-2 hover:text-zinc-800"
              onClick={onClearSession}
              type="button"
            >
              Clear chat session
            </button>
          </PanelRow>

          <PanelRow>
            <div className="mb-3 flex items-start justify-between gap-4">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-semibold text-zinc-950">Agent activity memory</p>
                <p className="mt-1 text-[12px] leading-5 text-zinc-500">
                  Durable log of asks, answers, tool calls, actions, saved memories, and clarifications.
                </p>
              </div>
              <button
                className="text-[12px] font-medium text-zinc-500 underline decoration-zinc-200 underline-offset-2 hover:text-zinc-800"
                onClick={onClearAgentMemory}
                type="button"
              >
                Clear
              </button>
            </div>
            <div className="mb-3 grid grid-cols-3 gap-4">
              <Metric label="Events" value={String(agentMemoryEvents.length)} />
              <Metric label="Asks" value={String(agentMemoryEvents.filter((event) => event.kind === "ask").length)} />
              <Metric label="Tools" value={String(agentMemoryEvents.filter((event) => event.kind === "tool").length)} />
            </div>
            {agentMemoryEvents.length === 0 ? (
              <p className="text-[12px] leading-5 text-zinc-400">
                No long-term agent activity yet.
              </p>
            ) : (
              <div className="max-h-72 divide-y divide-zinc-100 overflow-y-auto">
                {agentMemoryEvents.slice(0, 18).map((event) => (
                  <AgentMemoryEventRow event={event} key={event.id} />
                ))}
              </div>
            )}
          </PanelRow>

          <PanelRow>
            <div className="mb-3 flex items-center justify-between gap-4">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] font-semibold text-zinc-950">Structured memory</p>
                <p className="mt-1 text-[12px] leading-5 text-zinc-500">
                  Claude-style user, feedback, project, and reference memory.
                </p>
              </div>
              <span className="rounded-full bg-zinc-100 px-2 py-1 text-[11px] font-semibold text-zinc-500">
                {memoryEntries.length}
              </span>
            </div>
            <form className="space-y-2" onSubmit={submitMemory}>
              <div className="grid grid-cols-[120px_1fr] gap-2">
                <select
                  className="h-9 rounded-md border border-zinc-100 bg-white px-2 text-[12px] font-medium text-zinc-700 outline-none focus:border-violet-300"
                  onChange={(event) => setDraftType(event.currentTarget.value as MemoryType)}
                  value={draftType}
                >
                  {Object.entries(MEMORY_TYPE_META).map(([type, meta]) => (
                    <option key={type} value={type}>
                      {meta.label}
                    </option>
                  ))}
                </select>
                <input
                  className="h-9 rounded-md border border-zinc-100 bg-white px-2 text-[12px] text-zinc-700 outline-none focus:border-violet-300"
                  onChange={(event) => setDraftTitle(event.currentTarget.value)}
                  placeholder="Short title"
                  value={draftTitle}
                />
              </div>
              <textarea
                className="min-h-20 w-full resize-y rounded-md border border-zinc-100 bg-white px-2 py-2 text-[12px] leading-5 text-zinc-700 outline-none focus:border-violet-300"
                onChange={(event) => setDraftBody(event.currentTarget.value)}
                placeholder="What should Trace remember for future Ask runs?"
                value={draftBody}
              />
              <button
                className="inline-flex h-8 items-center gap-1.5 rounded-md bg-zinc-900 px-2.5 text-[12px] font-semibold text-white hover:bg-zinc-700 disabled:bg-zinc-200"
                disabled={!draftBody.trim()}
                type="submit"
              >
                <Plus size={13} />
                Save memory
              </button>
            </form>

            <div className="mt-4 divide-y divide-zinc-100">
              {(Object.keys(MEMORY_TYPE_META) as MemoryType[]).map((type) => {
                const meta = MEMORY_TYPE_META[type];
                const entries = entriesByType[type] ?? [];
                return (
                  <details className="py-2" key={type} open={entries.length > 0}>
                    <summary className="flex cursor-pointer list-none items-center gap-2">
                      <span className="text-zinc-500">{meta.icon}</span>
                      <span className="flex-1 text-[12px] font-semibold text-zinc-800">{meta.label}</span>
                      <span className="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold text-zinc-500">
                        {entries.length}
                      </span>
                    </summary>
                    <div className="pt-2">
                      <p className="mb-2 text-[11px] leading-5 text-zinc-400">{meta.description}</p>
                      {entries.length === 0 ? (
                        <p className="text-[12px] text-zinc-400">No saved entries.</p>
                      ) : (
                        <div className="divide-y divide-zinc-100">
                          {entries.map((entry) => (
                            <MemoryEntryRow entry={entry} key={entry.id} onRemove={onRemoveMemory} />
                          ))}
                        </div>
                      )}
                    </div>
                  </details>
                );
              })}
            </div>
          </PanelRow>

          <PanelRow>
            <p className="text-[13px] font-semibold text-zinc-950">Scratch memory</p>
            <textarea
              className="mt-2 min-h-36 w-full resize-y rounded-md border border-zinc-100 bg-white px-3 py-2 text-[12px] leading-5 text-zinc-700 outline-none focus:border-violet-300"
              onChange={(event) => setProjectMemory(event.currentTarget.value)}
              placeholder="Durable instructions, preferences, project facts, and recurring workflow notes..."
              value={projectMemory}
            />
            <p className="mt-2 text-[11px] leading-5 text-zinc-400">
              Legacy freeform memory, still included in every Ask follow-up.
            </p>
          </PanelRow>

          <PanelRow>
            <p className="mb-2 text-[13px] font-semibold text-zinc-950">Recalled records</p>
            {refs.length === 0 ? (
              <p className="text-[12px] leading-5 text-zinc-400">No referenced workspace records yet.</p>
            ) : (
              <div className="divide-y divide-zinc-100">
                {refs.map((ref) => (
                  <ReferenceLink key={`memory-${ref.kind}-${ref.entity_id}`} refItem={ref} />
                ))}
              </div>
            )}
          </PanelRow>
        </PanelSurface>
      ) : null}
    </div>
  );
}

function MemorySettingRow({
  description,
  enabled,
  learnMoreHref,
  onChange,
  title,
}: {
  description: string;
  enabled: boolean;
  learnMoreHref: string;
  onChange: (enabled: boolean) => void;
  title: string;
}) {
  return (
    <PanelRow className="flex items-center gap-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-semibold text-zinc-950">{title}</p>
        <p className="mt-0.5 text-[12px] leading-5 text-zinc-500">
          {description}{" "}
          <a
            className="font-medium text-sky-600 underline decoration-sky-200 underline-offset-2 hover:text-sky-800"
            href={learnMoreHref}
            rel="noreferrer"
            target="_blank"
          >
            Learn more
          </a>
        </p>
      </div>
      <ToggleSwitch checked={enabled} onChange={onChange} />
    </PanelRow>
  );
}

function ToggleSwitch({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <button
      aria-checked={checked}
      className={[
        "relative inline-flex h-7 w-12 shrink-0 items-center rounded-full transition-colors",
        checked ? "bg-sky-500" : "bg-zinc-200",
      ].join(" ")}
      onClick={() => onChange(!checked)}
      role="switch"
      type="button"
    >
      <span
        className={[
          "inline-block h-6 w-6 rounded-full bg-white shadow transition-transform",
          checked ? "translate-x-5" : "translate-x-0.5",
        ].join(" ")}
      />
    </button>
  );
}

function MemoryEntryRow({ entry, onRemove }: { entry: MemoryEntry; onRemove: (id: string) => void }) {
  return (
    <div className="py-2">
      <div className="flex items-start gap-2">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[12px] font-semibold text-zinc-800">{entry.title}</p>
          <p className="mt-1 text-[11px] leading-5 text-zinc-500">{entry.body}</p>
          <p className="mt-1 text-[10px] text-zinc-400">Updated {formatRelativeDate(entry.updatedAt)}</p>
        </div>
        <button
          aria-label={`Remove memory ${entry.title}`}
          className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-700"
          onClick={() => onRemove(entry.id)}
          type="button"
        >
          <Trash2 size={13} />
        </button>
      </div>
    </div>
  );
}

function AgentMemoryEventRow({ event }: { event: AgentMemoryEvent }) {
  return (
    <div className="py-2">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 text-zinc-400">{agentMemoryIcon(event.kind)}</span>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="min-w-0 flex-1 truncate text-[12px] font-semibold text-zinc-800">{event.title}</p>
            <span className="rounded-full bg-zinc-100 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-zinc-400">
              {event.kind}
            </span>
          </div>
          <p className="mt-1 line-clamp-3 text-[11px] leading-5 text-zinc-500">{event.detail}</p>
          <div className="mt-1 flex flex-wrap gap-2 text-[10px] text-zinc-400">
            <span>{formatRelativeDate(event.createdAt)}</span>
            {event.mode ? <span>{event.mode}</span> : null}
            {event.tool ? <span>{event.tool}</span> : null}
            {typeof event.refs === "number" ? <span>{event.refs} refs</span> : null}
          </div>
        </div>
      </div>
    </div>
  );
}

function ChatSessionsPanel({
  activeChatId,
  chatSessions,
  onDeleteChat,
  onOpenChat,
}: {
  activeChatId: string;
  chatSessions: AskChatSession[];
  onDeleteChat: (id: string) => void;
  onOpenChat: (session: AskChatSession) => void;
}) {
  const [searchInput, setSearchInput] = useState("");
  const [searchHits, setSearchHits] = useState<AskChatSearchHit[] | null>(null);
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);

  const sortedSessions = [...chatSessions].sort(
    (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime(),
  );

  async function runSearch() {
    const query = searchInput.trim();
    if (!query) {
      setSearchHits(null);
      return;
    }
    if (!isTauriRuntime()) {
      setSearchError("Search requires the desktop app.");
      return;
    }
    try {
      setSearchBusy(true);
      setSearchError(null);
      const hits = await searchAskChats(query, 60);
      setSearchHits(hits);
    } catch (caught) {
      setSearchError(String(caught));
    } finally {
      setSearchBusy(false);
    }
  }

  return (
    <PanelSurface>
      <PanelRow>
        <div className="grid grid-cols-3 gap-4">
          <Metric label="Chats" value={String(sortedSessions.length)} />
          <Metric label="Turns" value={String(sortedSessions.reduce((sum, session) => sum + session.turns.length, 0))} />
          <Metric label="Modes" value={[...new Set(sortedSessions.map((s) => s.mode))].join(" / ") || "—"} />
        </div>
      </PanelRow>
      <PanelRow>
        <div className="flex items-center gap-2">
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-400" size={12} />
            <input
              className="field-control h-8 pl-7 text-[12px]"
              onChange={(event) => {
                setSearchInput(event.currentTarget.value);
                if (event.currentTarget.value.trim().length === 0) {
                  setSearchHits(null);
                  setSearchError(null);
                }
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  void runSearch();
                }
              }}
              placeholder="Search across all chats…"
              value={searchInput}
            />
          </div>
          <button
            className="btn h-8 shrink-0 px-3 text-[11px]"
            disabled={searchBusy || searchInput.trim().length === 0}
            onClick={() => void runSearch()}
            type="button"
          >
            {searchBusy ? "Searching…" : "Search"}
          </button>
        </div>
        {searchError ? <p className="mt-1.5 text-[11px] text-rose-500">{searchError}</p> : null}
      </PanelRow>
      {searchHits !== null ? (
        searchHits.length === 0 ? (
          <div className="px-5 py-8 text-center">
            <Search className="mx-auto mb-2 text-zinc-200" size={22} />
            <p className="text-[12px] text-zinc-400">No matches for that query.</p>
          </div>
        ) : (
          searchHits.map((hit) => {
            const session = chatSessions.find((s) => s.id === hit.chat_id);
            return (
              <PanelRow key={`${hit.chat_id}-${hit.turn_id}`}>
                <div className="flex items-start gap-2.5">
                  <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-zinc-50 text-zinc-400">
                    <MessageSquareText size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-2">
                      <p className="min-w-0 flex-1 truncate text-[12px] font-semibold text-zinc-900">
                        {hit.chat_title}
                      </p>
                      <button
                        className="shrink-0 rounded-md border border-zinc-100 bg-white px-2 py-1 text-[11px] font-medium text-zinc-600 transition-colors hover:border-zinc-100 hover:bg-zinc-50"
                        onClick={() =>
                          onOpenChat(
                            session ?? {
                              id: hit.chat_id,
                              title: hit.chat_title,
                              mode: "ask",
                              turns: [],
                              createdAt: hit.created_at,
                              updatedAt: hit.created_at,
                              summary: "",
                            },
                          )
                        }
                        type="button"
                      >
                        Open
                      </button>
                    </div>
                    <p className="mt-1 line-clamp-2 text-[11px] leading-5 text-zinc-500">
                      {highlightSnippet(hit.question_snippet)}
                    </p>
                    <p className="mt-1 line-clamp-2 text-[11px] leading-5 text-zinc-400">
                      {highlightSnippet(hit.answer_snippet)}
                    </p>
                    <p className="mt-1 text-[10px] text-zinc-400">{formatRelativeDate(hit.created_at)}</p>
                  </div>
                </div>
              </PanelRow>
            );
          })
        )
      ) : sortedSessions.length === 0 ? (
        <div className="px-5 py-8 text-center">
          <MessageSquareText className="mx-auto mb-2 text-zinc-200" size={22} />
          <p className="text-[12px] text-zinc-400">No saved chats yet.</p>
          <p className="mt-1 text-[11px] text-zinc-300">Chats are saved after your first Ask turn.</p>
        </div>
      ) : (
        <>
          {sortedSessions.map((session) => (
            <button
              className={[
                "group flex w-full items-start gap-2.5 px-4 py-3 text-left transition-colors",
                session.id === activeChatId ? "bg-sky-50" : "hover:bg-zinc-50",
              ].join(" ")}
              key={session.id}
              onClick={() => onOpenChat(session)}
              type="button"
            >
              <div className={[
                "mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-lg transition-colors",
                session.id === activeChatId ? "bg-sky-100 text-sky-500" : "bg-zinc-50 text-zinc-400",
              ].join(" ")}>
                <MessageSquareText size={13} />
              </div>
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <p className="min-w-0 flex-1 truncate text-[12px] font-semibold text-zinc-900">
                    {session.title}
                  </p>
                  {session.id === activeChatId ? (
                    <span className="shrink-0 rounded-full bg-sky-100 px-2 py-0.5 text-[10px] font-semibold text-sky-600">
                      Active
                    </span>
                  ) : null}
                  <button
                    aria-label={`Delete chat ${session.title}`}
                    className="shrink-0 rounded-md p-1 text-zinc-300 opacity-0 transition-all hover:bg-zinc-100 hover:text-zinc-500 group-hover:opacity-100"
                    onClick={(e) => { e.stopPropagation(); onDeleteChat(session.id); }}
                    type="button"
                  >
                    <Trash2 size={11} />
                  </button>
                </div>
                <p className="mt-0.5 line-clamp-1 text-[11px] leading-5 text-zinc-400">{session.summary}</p>
                <div className="mt-0.5 flex flex-wrap gap-2 text-[10px] text-zinc-400">
                  <span>{session.turns.length} turns</span>
                  <span>·</span>
                  <span>{session.mode}</span>
                  <span>·</span>
                  <span>{formatRelativeDate(session.updatedAt)}</span>
                </div>
              </div>
            </button>
          ))}
        </>
      )}
    </PanelSurface>
  );
}

function highlightSnippet(snippet: string): ReactNode[] {
  // FTS wraps matches in `[` and `]`. Rendering nodes keeps database content
  // in React's escaped text path instead of creating an HTML injection sink.
  const nodes: ReactNode[] = [];
  let text = "";
  let highlighted = false;
  let key = 0;

  for (const character of snippet) {
    if (character === "[" && !highlighted) {
      if (text) nodes.push(text);
      text = "";
      highlighted = true;
    } else if (character === "]" && highlighted) {
      nodes.push(
        <mark className="rounded bg-amber-100 px-0.5 text-amber-800" key={key++}>
          {text}
        </mark>,
      );
      text = "";
      highlighted = false;
    } else {
      text += character;
    }
  }
  if (text) nodes.push(highlighted ? `[${text}` : text);
  return nodes;
}

function TaskPanel({
  onRemoveTask,
  onUpdateTaskStatus,
  tasks,
}: {
  onRemoveTask: (id: string) => void;
  onUpdateTaskStatus: (id: string, status: AskTaskStatus) => void;
  tasks: AskTask[];
}) {
  const active = tasks.filter((task) => task.status === "pending" || task.status === "running");
  const done = tasks.filter((task) => task.status === "completed");
  const blocked = tasks.filter((task) => task.status === "blocked");

  return (
    <PanelSurface>
      <PanelRow>
        <div className="grid grid-cols-3 gap-4">
          <Metric label="Active" value={String(active.length)} />
          <Metric label="Done" value={String(done.length)} />
          <Metric label="Blocked" value={String(blocked.length)} />
        </div>
      </PanelRow>
      {tasks.length === 0 ? (
        <div className="px-5 py-8 text-center">
          <ListTodo className="mx-auto mb-2 text-zinc-200" size={22} />
          <p className="text-[12px] text-zinc-400">No tasks yet.</p>
          <p className="mt-1 text-[11px] text-zinc-300">Ask runs appear here as lightweight agent tasks.</p>
        </div>
      ) : (
        <>
          {tasks.map((task) => (
            <PanelRow key={task.id}>
              <div className="flex items-start gap-2">
                <TaskStatusIcon status={task.status} />
                <div className="min-w-0 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <p className="min-w-0 flex-1 truncate text-[13px] font-semibold text-zinc-900">
                      {task.title}
                    </p>
                    <span className="rounded-full bg-zinc-100 px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-zinc-500">
                      {task.mode}
                    </span>
                  </div>
                  <p className="mt-1 text-[12px] leading-5 text-zinc-500">{task.detail}</p>
                  <p className="mt-1 text-[10px] text-zinc-400">Updated {formatRelativeDate(task.updatedAt)}</p>
                </div>
                <button
                  aria-label={`Remove task ${task.title}`}
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-700"
                  onClick={() => onRemoveTask(task.id)}
                  type="button"
                >
                  <Trash2 size={13} />
                </button>
              </div>
              <div className="mt-3 flex flex-wrap gap-1.5">
                {(["pending", "running", "completed", "blocked"] as AskTaskStatus[]).map((status) => (
                  <button
                    className={[
                      "rounded-md border px-2 py-1 text-[11px] font-medium transition-colors",
                      task.status === status
                        ? "border-zinc-900 bg-zinc-900 text-white"
                        : "border-zinc-100 bg-white text-zinc-500 hover:bg-zinc-50",
                    ].join(" ")}
                    key={status}
                    onClick={() => onUpdateTaskStatus(task.id, status)}
                    type="button"
                  >
                    {taskStatusLabel(status)}
                  </button>
                ))}
              </div>
            </PanelRow>
          ))}
        </>
      )}
    </PanelSurface>
  );
}

function CommandPanel() {
  return (
    <PanelSurface>
      <PanelRow className="text-[12px] leading-5 text-zinc-600">
        Type <code className="rounded border border-zinc-100 bg-zinc-50 px-1 py-0.5 font-mono text-[11px] text-zinc-700">/</code> in the composer for commands, or <code className="rounded border border-zinc-100 bg-zinc-50 px-1 py-0.5 font-mono text-[11px] text-zinc-700">@</code> for workspace mentions.
      </PanelRow>
      <PanelRow>
        <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-zinc-400">Commands</p>
        <div className="divide-y divide-zinc-100">
          {COMPOSER_COMMANDS.map((command) => (
            <CommandRow command={command} key={command.name} />
          ))}
        </div>
      </PanelRow>
      <PanelRow>
        <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-zinc-400">Mentions</p>
        <div className="divide-y divide-zinc-100">
          {MENTION_TARGETS.map((target) => (
            <div className="flex items-center gap-2 py-2" key={target.token}>
              <span className="text-zinc-500">{target.icon}</span>
              <span className="w-24 shrink-0 font-mono text-[12px] font-semibold text-zinc-800">{target.token}</span>
              <span className="min-w-0 flex-1 text-[12px] leading-5 text-zinc-500">{target.description}</span>
            </div>
          ))}
        </div>
      </PanelRow>
    </PanelSurface>
  );
}

function CommandRow({ command }: { command: ComposerCommand }) {
  return (
    <div className="flex items-start gap-2 py-2">
      <span className="font-mono text-[12px] font-semibold text-zinc-900">{command.label}</span>
      <span className="rounded-full bg-zinc-100 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide text-zinc-500">
        {command.badge}
      </span>
      <span className="min-w-0 flex-1 text-[12px] leading-5 text-zinc-500">{command.description}</span>
    </div>
  );
}

function HistoryPanel({
  onReusePrompt,
  promptHistory,
}: {
  onReusePrompt: (prompt: string) => void;
  promptHistory: string[];
}) {
  return (
    <PanelSurface>
      <PanelRow className="text-[12px] leading-5 text-zinc-500">
        Recent prompts are stored locally. Press <kbd className="rounded border border-zinc-100 bg-zinc-50 px-1 py-0.5 font-mono text-[10px] text-zinc-600">↑</kbd> in an empty composer to reuse the latest.
      </PanelRow>
      {promptHistory.length === 0 ? (
        <div className="px-5 py-8 text-center">
          <Clock3 className="mx-auto mb-2 text-zinc-200" size={22} />
          <p className="text-[12px] text-zinc-400">No prompt history yet.</p>
        </div>
      ) : (
        <div className="divide-y divide-zinc-100">
          {promptHistory.map((prompt) => (
            <button
              className="group flex w-full items-center gap-2.5 px-4 py-3 text-left transition-colors hover:bg-zinc-50"
              key={prompt}
              onClick={() => onReusePrompt(prompt)}
              type="button"
            >
              <CornerDownLeft className="shrink-0 text-zinc-300 transition-colors group-hover:text-zinc-400" size={12} />
              <span className="min-w-0 flex-1 truncate text-[12px] leading-5 text-zinc-600">{prompt}</span>
            </button>
          ))}
        </div>
      )}
    </PanelSurface>
  );
}

function SettingsPanel({
  permissionMode,
  setPermissionMode,
}: {
  permissionMode: PermissionMode;
  setPermissionMode: (mode: PermissionMode) => void;
}) {
  const modes: PermissionMode[] = ["confirm", "auto_read", "auto_safe"];
  return (
    <PanelSurface>
      <PanelRow>
        <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-zinc-400">
          Permission mode
        </p>
        <div className="divide-y divide-zinc-100">
          {modes.map((mode) => (
            <button
              className={[
                "flex w-full items-start gap-2 rounded-lg px-2 py-2.5 text-left transition-colors",
                permissionMode === mode
                  ? "bg-sky-50 text-zinc-950"
                  : "text-zinc-700 hover:bg-zinc-50",
              ].join(" ")}
              key={mode}
              onClick={() => setPermissionMode(mode)}
              type="button"
            >
              <ShieldCheck size={14} className={["mt-0.5 shrink-0", permissionMode === mode ? "text-sky-500" : "text-zinc-400"].join(" ")} />
              <span className="min-w-0 flex-1">
                <span className="block text-[12px] font-semibold">{permissionModeLabel(mode)}</span>
                <span className="mt-0.5 block text-[11px] leading-5 text-zinc-500">
                  {PERMISSION_MODE_CONTEXT[mode]}
                </span>
              </span>
              {permissionMode === mode ? <CheckCircle2 size={14} className="ml-auto mt-0.5 shrink-0 text-sky-500" /> : null}
            </button>
          ))}
        </div>
      </PanelRow>
      <PanelRow>
        <p className="mb-2 text-[10px] font-bold uppercase tracking-widest text-zinc-400">
          Agent harness
        </p>
        <div className="space-y-2 text-[12px] leading-5 text-zinc-600">
          <p>Read tools can run in parallel when they are safe.</p>
          <p>Write tools remain visible in Act mode and should be confirmed when intent is ambiguous.</p>
          <p>Clarifying questions are rendered as inline cards instead of hidden text.</p>
        </div>
      </PanelRow>
    </PanelSurface>
  );
}
