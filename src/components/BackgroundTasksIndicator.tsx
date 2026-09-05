import { useMemo, useState } from "react";
import * as Popover from "@radix-ui/react-dialog";
import {
  Brain,
  CalendarDays,
  FolderTree,
  Mail,
  RefreshCw,
  Sparkles,
  Wand2,
  type LucideIcon,
} from "lucide-react";
import {
  useBackgroundTasks,
  type BgSource,
  type BgTaskState,
} from "../lib/backgroundTasks";
import { gmailSyncNow, gcalSync } from "../lib/ipc";
import { toast } from "../lib/toast";

const SOURCE_LABELS: Record<BgSource, string> = {
  gmail: "Gmail",
  drive: "Drive",
  calendar: "Calendar",
  brain: "Brain",
  embedding: "Embeddings",
  capture_promote: "AI promotion",
};

const SOURCE_ICONS: Record<BgSource, LucideIcon> = {
  gmail: Mail,
  drive: FolderTree,
  calendar: CalendarDays,
  brain: Brain,
  embedding: Sparkles,
  capture_promote: Wand2,
};

function timeAgo(ms?: number): string {
  if (!ms) return "Never";
  const diff = Date.now() - ms;
  if (diff < 1000) return "just now";
  const s = Math.floor(diff / 1000);
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  return `${Math.floor(h / 24)}d ago`;
}

function statusDotClass(status: BgTaskState["status"]): string {
  if (status === "running") return "bg-sky-500 animate-pulse";
  if (status === "ok") return "bg-emerald-500";
  if (status === "error") return "bg-red-500";
  return "bg-zinc-300";
}

async function retrySource(source: BgSource) {
  try {
    if (source === "gmail") {
      await gmailSyncNow();
    } else if (source === "calendar") {
      await gcalSync();
    } else if (source === "drive") {
      toast.info("Drive syncs automatically every 3 minutes.");
      return;
    } else if (source === "brain") {
      toast.info("Brain rebuilds automatically after edits.");
      return;
    }
  } catch (e) {
    // ipc wrapper will toast; nothing else to do
    void e;
  }
}

export function BackgroundTasksIndicator() {
  const tasks = useBackgroundTasks();
  const [open, setOpen] = useState(false);

  const { label, dotClass } = useMemo(() => {
    const values = Object.values(tasks);
    const anyRunning = values.some((t) => t.status === "running");
    const anyError = values.some((t) => t.status === "error");
    if (anyRunning) return { label: "Syncing…", dotClass: "bg-sky-500 animate-pulse" };
    if (anyError) return { label: "Sync issues", dotClass: "bg-red-500" };
    const anyOk = values.some((t) => t.status === "ok");
    if (anyOk) return { label: "Up to date", dotClass: "bg-emerald-500" };
    return { label: "Idle", dotClass: "bg-zinc-300" };
  }, [tasks]);

  return (
    <Popover.Root open={open} onOpenChange={setOpen}>
      <Popover.Trigger asChild>
        <button
          aria-label={`Background tasks: ${label}`}
          className="flex items-center gap-2 rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5 text-[11px] font-medium text-zinc-600 transition-colors hover:border-zinc-300 hover:text-zinc-900"
          type="button"
        >
          <span className={`h-2 w-2 rounded-full ${dotClass}`} />
          <span>{label}</span>
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Overlay className="fixed inset-0 z-40 bg-zinc-950/10 backdrop-blur-[2px]" />
        <Popover.Content
          className="fixed right-4 top-14 z-50 w-80 rounded-2xl border border-zinc-100 bg-white p-4 shadow-2xl"
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          <Popover.Title className="page-kicker mb-1">
            Background tasks
          </Popover.Title>
          <Popover.Description className="mb-3 text-xs text-zinc-400">
            Sync, indexing, and AI jobs running in the background.
          </Popover.Description>
          <div className="space-y-2">
            {(Object.values(tasks) as BgTaskState[]).map((task) => {
              const Icon = SOURCE_ICONS[task.source];
              const subtitle =
                task.status === "running"
                  ? "Syncing now…"
                  : task.status === "error"
                    ? task.lastError ?? "Sync failed"
                    : task.status === "ok"
                      ? `${task.lastSummary ?? "Synced"} · ${timeAgo(task.lastFinishedAt)}`
                      : "Not run yet";
              const subtitleClass =
                task.status === "error" ? "text-red-600" : "text-zinc-500";
              const retriable = task.status === "error" && (task.source === "gmail" || task.source === "calendar");
              return (
                <div
                  key={task.source}
                  className="flex items-center gap-3 rounded-xl border border-zinc-100 bg-zinc-50 p-3"
                >
                  <Icon className="text-zinc-400" size={14} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-zinc-900">
                        {SOURCE_LABELS[task.source]}
                      </span>
                      <span
                        className={`h-1.5 w-1.5 rounded-full ${statusDotClass(task.status)}`}
                      />
                    </div>
                    <p className={`truncate text-xs ${subtitleClass}`}>
                      {subtitle}
                    </p>
                  </div>
                  {retriable && (
                    <button
                      aria-label={`Retry ${SOURCE_LABELS[task.source]} sync`}
                      className="rounded-md border border-zinc-200 bg-white p-1.5 text-zinc-500 transition-colors hover:border-zinc-300 hover:text-zinc-900"
                      onClick={() => void retrySource(task.source)}
                      type="button"
                    >
                      <RefreshCw size={12} />
                    </button>
                  )}
                </div>
              );
            })}
          </div>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
