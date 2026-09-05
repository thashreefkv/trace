import { useEffect, useRef, useState } from "react";
import {
  ArrowDown,
  ArrowUp,
  Check,
  Circle,
  ExternalLink,
  ListChecks,
  Loader2,
  MessageSquare,
  Paperclip,
  Plus,
  Sparkles,
  Trash2,
  X,
} from "lucide-react";
import { EmptyState } from "./EmptyState";
import {
  applyGeneratedDeliverableTasks,
  createDeliverableTask,
  deleteDeliverableTask,
  generateDeliverableTasks,
  listDeliverableTasks,
  reorderDeliverableTask,
  updateDeliverableTask,
} from "../lib/ipc";
import { recordBrainSignal } from "../lib/brainSignals";
import { safeExternalUrl } from "../lib/urlSafety";
import type { DeliverableTask, GeneratedTaskSuggestion, TaskStatus } from "../lib/types";
import { EntityFilesPanel } from "./files/EntityFilesPanel";

interface Props {
  deliverableId: string;
}

interface SuggestedTask extends GeneratedTaskSuggestion {
  accepted: boolean | null;
}

export function DeliverableTasks({ deliverableId }: Props) {
  const [tasks, setTasks] = useState<DeliverableTask[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [newTitle, setNewTitle] = useState("");
  const [newDueDate, setNewDueDate] = useState("");
  const [isAdding, setIsAdding] = useState(false);
  const [isGenerating, setIsGenerating] = useState(false);
  const [suggestions, setSuggestions] = useState<SuggestedTask[] | null>(null);
  const [suggestedDeadline, setSuggestedDeadline] = useState<string | null>(null);
  const [suggestionRationale, setSuggestionRationale] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    void load();
  }, [deliverableId]);

  async function load() {
    try {
      setError(null);
      setIsLoading(true);
      const result = await listDeliverableTasks(deliverableId);
      setTasks(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleAdd() {
    const title = newTitle.trim();
    if (!title) return;
    try {
      setIsAdding(true);
      const task = await createDeliverableTask({
        deliverable_id: deliverableId,
        title,
        due_date: newDueDate.trim() || null,
      });
      setTasks((prev) => [...prev, task]);
      setNewTitle("");
      setNewDueDate("");
      inputRef.current?.focus();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsAdding(false);
    }
  }

  async function handleToggleStatus(task: DeliverableTask) {
    const next: TaskStatus = task.status === "done" ? "todo" : "done";
    try {
      const updated = await updateDeliverableTask(task.id, {
        title: task.title,
        status: next,
        due_date: task.due_date,
        notes: task.notes,
        url: task.url,
      });
      setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
      if (next === "done") {
        void recordBrainSignal({
          template: "focus_today",
          itemId: `task:${task.id}`,
          itemKind: "task",
          eventType: "completed_after_seen",
          context: {
            deliverable_id: deliverableId,
            source: "task_completion",
          },
        });
      }
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSetDoing(task: DeliverableTask) {
    const next: TaskStatus = task.status === "doing" ? "todo" : "doing";
    try {
      const updated = await updateDeliverableTask(task.id, {
        title: task.title,
        status: next,
        due_date: task.due_date,
        notes: task.notes,
        url: task.url,
      });
      setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSaveTitle(task: DeliverableTask, title: string) {
    const trimmed = title.trim();
    if (!trimmed || trimmed === task.title) return;
    try {
      const updated = await updateDeliverableTask(task.id, {
        title: trimmed,
        status: task.status as TaskStatus,
        due_date: task.due_date,
        notes: task.notes,
        url: task.url,
      });
      setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSaveTaskMeta(
    task: DeliverableTask,
    notes: string | null,
    url: string | null,
  ) {
    try {
      const updated = await updateDeliverableTask(task.id, {
        title: task.title,
        status: task.status as TaskStatus,
        due_date: task.due_date,
        notes: notes?.trim() || null,
        url: url?.trim() || null,
      });
      setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleSaveDueDate(task: DeliverableTask, date: string | null) {
    try {
      const updated = await updateDeliverableTask(task.id, {
        title: task.title,
        status: task.status as TaskStatus,
        due_date: date?.trim() || null,
        notes: task.notes,
        url: task.url,
      });
      setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteDeliverableTask(id);
      setTasks((prev) => prev.filter((t) => t.id !== id));
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleReorder(id: string, direction: "up" | "down") {
    try {
      const updated = await reorderDeliverableTask({ id, direction });
      setTasks(updated);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleGenerate() {
    try {
      setIsGenerating(true);
      setError(null);
      setSuggestions(null);
      const result = await generateDeliverableTasks(deliverableId);
      setSuggestions(result.tasks.map((task) => ({ ...task, accepted: null })));
      setSuggestedDeadline(result.suggested_deliverable_deadline);
      setSuggestionRationale(result.rationale);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsGenerating(false);
    }
  }

  async function handleAcceptSuggestions() {
    if (!suggestions) return;
    const accepted = suggestions.filter((s) => s.accepted !== false);
    if (accepted.length === 0) {
      setSuggestions(null);
      setSuggestedDeadline(null);
      setSuggestionRationale("");
      return;
    }
    try {
      const created = await applyGeneratedDeliverableTasks({
        deliverable_id: deliverableId,
        tasks: accepted.map(({ accepted: _accepted, ...task }) => task),
        suggested_deliverable_deadline: suggestedDeadline,
      });
      setTasks((prev) => [...prev, ...created]);
    } catch (e) {
      setError(String(e));
      return;
    }
    setSuggestions(null);
    setSuggestedDeadline(null);
    setSuggestionRationale("");
  }

  function toggleSuggestion(index: number) {
    setSuggestions((prev) =>
      prev
        ? prev.map((s, i) =>
            i === index ? { ...s, accepted: s.accepted === false ? null : false } : s,
          )
        : prev,
    );
  }

  function updateSuggestionTitle(index: number, title: string) {
    setSuggestions((prev) =>
      prev ? prev.map((s, i) => (i === index ? { ...s, title } : s)) : prev,
    );
  }

  const doing = tasks.filter((t) => t.status === "doing");
  const todo = tasks.filter((t) => t.status === "todo");
  const done = tasks.filter((t) => t.status === "done");
  const ordered = [...doing, ...todo, ...done];

  return (
    <div className="space-y-4">
      {error && <div className="notice notice-error text-sm">{error}</div>}

      {/* AI suggestions panel */}
      {suggestions && (
        <div className="overflow-hidden rounded-xl border border-violet-100 bg-white">
          <div className="flex items-start justify-between gap-3 border-b border-violet-50 bg-violet-50 px-4 py-3">
            <div className="flex items-start gap-2">
              <Sparkles className="mt-0.5 shrink-0 text-violet-500" size={13} />
              <div>
                <p className="text-sm font-semibold text-zinc-900">AI-suggested tasks</p>
                {suggestionRationale ? (
                  <p className="mt-0.5 text-xs leading-5 text-zinc-500">{suggestionRationale}</p>
                ) : null}
                {suggestedDeadline ? (
                  <p className="mt-1 text-[11px] font-semibold text-violet-700">
                    Suggested deadline: {suggestedDeadline}
                  </p>
                ) : null}
              </div>
            </div>
            <button
              className="shrink-0 rounded p-1 text-zinc-300 transition-colors hover:bg-violet-100 hover:text-zinc-600"
              onClick={() => setSuggestions(null)}
              type="button"
            >
              <X size={13} />
            </button>
          </div>

          <ul className="divide-y divide-zinc-50">
            {suggestions.map((s, i) => (
              <li
                key={i}
                className={[
                  "group flex items-start gap-3 px-4 py-3 transition-colors",
                  s.accepted === false ? "opacity-40" : "hover:bg-zinc-50",
                ].join(" ")}
              >
                <button
                  className="mt-0.5 shrink-0"
                  onClick={() => toggleSuggestion(i)}
                  type="button"
                >
                  {s.accepted === false ? (
                    <div className="flex h-4 w-4 items-center justify-center rounded border-2 border-zinc-200" />
                  ) : (
                    <div className="flex h-4 w-4 items-center justify-center rounded border-2 border-violet-400 bg-violet-400">
                      <Check className="text-white" size={9} />
                    </div>
                  )}
                </button>
                <div className="min-w-0 flex-1">
                  <input
                    className="w-full bg-transparent text-sm font-medium text-zinc-900 placeholder:text-zinc-300 focus:outline-none"
                    onChange={(e) => updateSuggestionTitle(i, e.currentTarget.value)}
                    onFocus={(e) => e.stopPropagation()}
                    placeholder="Task title…"
                    value={s.title}
                  />
                  {(s.due_date || s.reason) && (
                    <p className="mt-0.5 text-[11px] leading-4 text-zinc-400">
                      {s.due_date ? `Due ${s.due_date}` : "No date"}
                      {s.reason ? ` · ${s.reason}` : ""}
                    </p>
                  )}
                </div>
              </li>
            ))}
          </ul>

          <div className="flex items-center gap-2 border-t border-zinc-50 px-4 py-3">
            <button
              className="btn btn-primary text-xs"
              onClick={() => void handleAcceptSuggestions()}
              type="button"
            >
              <Check size={12} />
              Add {suggestions.filter((s) => s.accepted !== false).length} tasks
            </button>
            <button
              className="btn text-xs"
              onClick={() => setSuggestions(null)}
              type="button"
            >
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Header bar */}
      <div className="flex items-center justify-between">
        <p className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
          {tasks.length} task{tasks.length !== 1 ? "s" : ""} · {done.length} done
        </p>
        <button
          className="btn text-xs"
          disabled={isGenerating}
          onClick={() => void handleGenerate()}
          type="button"
        >
          {isGenerating ? (
            <Loader2 className="animate-spin" size={13} />
          ) : (
            <Sparkles size={13} />
          )}
          {isGenerating ? "Thinking…" : "Generate with AI"}
        </button>
      </div>

      {/* Task list */}
      {isLoading ? (
        <div className="space-y-2">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="h-10 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : tasks.length === 0 && !suggestions ? (
        <EmptyState
          variant="inline"
          icon={ListChecks}
          title="No tasks yet"
          description="Add one below or let AI generate a plan."
          cta={{ label: "Generate with AI", onClick: () => void handleGenerate(), primary: true }}
        />
      ) : (
        <div className="space-y-1">
          {ordered.map((task, idx) => (
            <TaskRow
              key={task.id}
              isFirst={idx === 0}
              isLast={idx === ordered.length - 1}
              onDelete={handleDelete}
              onReorder={handleReorder}
              onSaveDueDate={handleSaveDueDate}
              onSaveMeta={handleSaveTaskMeta}
              onSaveTitle={handleSaveTitle}
              onSetDoing={handleSetDoing}
              onToggle={handleToggleStatus}
              task={task}
            />
          ))}
        </div>
      )}

      {/* Add task row */}
      <div className="flex gap-2">
        <input
          className="field-control flex-1 text-sm"
          disabled={isAdding}
          onChange={(e) => setNewTitle(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void handleAdd();
          }}
          placeholder="Add a task…"
          ref={inputRef}
          type="text"
          value={newTitle}
        />
        <input
          className="field-control w-36 text-sm"
          disabled={isAdding}
          onChange={(e) => setNewDueDate(e.currentTarget.value)}
          title="Due date (optional)"
          type="date"
          value={newDueDate}
        />
        <button
          className="btn"
          disabled={isAdding || !newTitle.trim()}
          onClick={() => void handleAdd()}
          type="button"
        >
          <Plus size={14} />
        </button>
      </div>
    </div>
  );
}

interface TaskRowProps {
  task: DeliverableTask;
  isFirst: boolean;
  isLast: boolean;
  onToggle: (task: DeliverableTask) => void;
  onSetDoing: (task: DeliverableTask) => void;
  onDelete: (id: string) => void;
  onReorder: (id: string, direction: "up" | "down") => void;
  onSaveTitle: (task: DeliverableTask, title: string) => void;
  onSaveMeta: (task: DeliverableTask, notes: string | null, url: string | null) => void;
  onSaveDueDate: (task: DeliverableTask, date: string | null) => void;
}

function TaskRow({
  task,
  isFirst,
  isLast,
  onToggle,
  onSetDoing,
  onDelete,
  onReorder,
  onSaveTitle,
  onSaveMeta,
  onSaveDueDate,
}: TaskRowProps) {
  const [expanded, setExpanded] = useState(false);
  const [filesOpen, setFilesOpen] = useState(false);
  const [fileCount, setFileCount] = useState(0);
  const [editNotes, setEditNotes] = useState(task.notes ?? "");
  const [editUrl, setEditUrl] = useState(task.url ?? "");
  const [isSavingMeta, setIsSavingMeta] = useState(false);
  const [editingDate, setEditingDate] = useState(false);
  const [editingTitle, setEditingTitle] = useState(false);
  const [editTitle, setEditTitle] = useState(task.title);
  const dateInputRef = useRef<HTMLInputElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);

  const isDone = task.status === "done";
  const isDoing = task.status === "doing";
  const hasExtra = !!(task.notes || task.url);
  const safeTaskUrl = safeExternalUrl(task.url);

  function startDateEdit() {
    setEditingDate(true);
    setTimeout(() => dateInputRef.current?.showPicker?.(), 0);
  }

  function commitDate(value: string) {
    setEditingDate(false);
    onSaveDueDate(task, value || null);
  }

  function startTitleEdit() {
    if (isDone) return;
    setEditTitle(task.title);
    setEditingTitle(true);
    setTimeout(() => titleInputRef.current?.select(), 0);
  }

  function commitTitle() {
    setEditingTitle(false);
    onSaveTitle(task, editTitle);
  }

  async function handleSaveMeta() {
    setIsSavingMeta(true);
    try {
      await onSaveMeta(task, editNotes || null, editUrl || null);
      setExpanded(false);
    } finally {
      setIsSavingMeta(false);
    }
  }

  function handleExpand() {
    setEditNotes(task.notes ?? "");
    setEditUrl(task.url ?? "");
    setExpanded((v) => !v);
  }

  return (
    <div
      className={[
        "rounded-lg border transition-colors",
        isDone ? "border-zinc-100" : "border-zinc-200",
      ].join(" ")}
    >
      {/* Main row */}
      <div
        className={[
          "group flex items-center gap-2 px-2 py-2",
          isDone ? "opacity-60" : "",
        ].join(" ")}
      >
        {/* Reorder arrows */}
        <div className="flex shrink-0 flex-col opacity-0 transition-opacity group-hover:opacity-100">
          <button
            className="text-zinc-300 hover:text-zinc-600 disabled:opacity-30"
            disabled={isFirst}
            onClick={() => onReorder(task.id, "up")}
            title="Move up"
            type="button"
          >
            <ArrowUp size={11} />
          </button>
          <button
            className="text-zinc-300 hover:text-zinc-600 disabled:opacity-30"
            disabled={isLast}
            onClick={() => onReorder(task.id, "down")}
            title="Move down"
            type="button"
          >
            <ArrowDown size={11} />
          </button>
        </div>

        {/* Status toggle */}
        <button
          className="shrink-0 text-zinc-300 hover:text-zinc-600"
          onClick={() => onToggle(task)}
          title={isDone ? "Mark as to-do" : "Mark as done"}
          type="button"
        >
          {isDone ? (
            <Check className="text-emerald-500" size={16} />
          ) : isDoing ? (
            <Loader2 className="animate-spin text-amber-500" size={16} />
          ) : (
            <Circle size={16} />
          )}
        </button>

        {/* Title — click to edit */}
        {editingTitle ? (
          <input
            ref={titleInputRef}
            autoFocus
            className="flex-1 bg-transparent text-sm text-zinc-900 focus:outline-none"
            onBlur={commitTitle}
            onChange={(e) => setEditTitle(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitTitle();
              if (e.key === "Escape") {
                setEditTitle(task.title);
                setEditingTitle(false);
              }
            }}
            value={editTitle}
          />
        ) : (
          <span
            className={[
              "flex-1 cursor-text text-sm",
              isDone
                ? "text-zinc-400 line-through"
                : "text-zinc-900 hover:text-zinc-700",
            ].join(" ")}
            onClick={startTitleEdit}
            title={isDone ? undefined : "Click to edit"}
          >
            {task.title}
          </span>
        )}

        {/* Due date */}
        {editingDate ? (
          <input
            autoFocus
            className="field-control w-32 shrink-0 text-xs"
            defaultValue={task.due_date ?? ""}
            onBlur={(e) => commitDate(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitDate(e.currentTarget.value);
              if (e.key === "Escape") setEditingDate(false);
            }}
            ref={dateInputRef}
            type="date"
          />
        ) : task.due_date ? (
          <button
            className="shrink-0 text-xs text-zinc-400 hover:text-zinc-700"
            onClick={startDateEdit}
            title="Edit due date"
            type="button"
          >
            {task.due_date}
          </button>
        ) : (
          <button
            className="shrink-0 text-xs text-zinc-300 opacity-0 hover:text-zinc-500 group-hover:opacity-100"
            onClick={startDateEdit}
            title="Set due date"
            type="button"
          >
            + date
          </button>
        )}

        {/* URL quick-link */}
        {safeTaskUrl && (
          <a
            className="shrink-0 text-zinc-300 hover:text-violet-500"
            href={safeTaskUrl}
            onClick={(e) => e.stopPropagation()}
            rel="noopener noreferrer"
            target="_blank"
            title={safeTaskUrl}
          >
            <ExternalLink size={13} />
          </a>
        )}

        {/* Notes expand */}
        <button
          className={[
            "shrink-0 transition-opacity",
            hasExtra || expanded
              ? "text-violet-500 opacity-100"
              : "text-zinc-300 opacity-0 group-hover:opacity-100",
          ].join(" ")}
          onClick={handleExpand}
          title={expanded ? "Collapse" : "Notes & link"}
          type="button"
        >
          <MessageSquare size={13} />
        </button>

        {/* Files */}
        <button
          className={[
            "shrink-0 transition-opacity",
            fileCount > 0 || filesOpen
              ? "text-sky-500 opacity-100"
              : "text-zinc-300 opacity-0 group-hover:opacity-100",
          ].join(" ")}
          onClick={() => setFilesOpen((v) => !v)}
          title={filesOpen ? "Hide files" : `Files${fileCount > 0 ? ` (${fileCount})` : ""}`}
          type="button"
        >
          <Paperclip size={13} />
        </button>

        {/* In-progress toggle */}
        {!isDone && (
          <button
            className={[
              "shrink-0 rounded px-1.5 py-0.5 text-[10px] font-medium transition-all",
              isDoing
                ? "bg-amber-100 text-amber-700 opacity-100"
                : "bg-zinc-100 text-zinc-400 opacity-0 group-hover:opacity-100",
            ].join(" ")}
            onClick={() => onSetDoing(task)}
            title={isDoing ? "Remove in-progress" : "Mark as in-progress"}
            type="button"
          >
            {isDoing ? "in progress" : "start"}
          </button>
        )}

        {/* Delete */}
        <button
          className="shrink-0 text-zinc-200 opacity-0 transition-opacity hover:text-red-500 group-hover:opacity-100"
          onClick={() => onDelete(task.id)}
          type="button"
        >
          <Trash2 size={13} />
        </button>
      </div>

      {/* Expanded notes / URL panel */}
      {expanded && (
        <div className="border-t border-zinc-100 px-3 pb-3 pt-2">
          <div className="space-y-2">
            <div>
              <label className="mb-1 block text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                Notes
              </label>
              <textarea
                className="field-control w-full resize-none text-sm"
                onChange={(e) => setEditNotes(e.currentTarget.value)}
                placeholder="Add context, blockers, or details…"
                rows={2}
                value={editNotes}
              />
            </div>
            <div>
              <label className="mb-1 block text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
                Link
              </label>
              <input
                className="field-control w-full text-sm"
                onChange={(e) => setEditUrl(e.currentTarget.value)}
                placeholder="https://…"
                type="url"
                value={editUrl}
              />
            </div>
            <div className="flex gap-2">
              <button
                className="btn btn-primary text-xs"
                disabled={isSavingMeta}
                onClick={() => void handleSaveMeta()}
                type="button"
              >
                {isSavingMeta ? "Saving…" : "Save"}
              </button>
              <button className="btn text-xs" onClick={() => setExpanded(false)} type="button">
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Files panel */}
      {filesOpen && (
        <div className="border-t border-zinc-100 px-3 pb-3 pt-2">
          <EntityFilesPanel
            entityKind="deliverable_task"
            entityId={task.id}
            onCountChange={setFileCount}
          />
        </div>
      )}
    </div>
  );
}
