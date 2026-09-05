import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Link, useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  BrainCircuit,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  FileText,
  Loader2,
  Plus,
  Sparkles,
  Upload,
  Users,
  X,
} from "lucide-react";
import { StakeholderPicker } from "./StakeholderPicker";
import {
  applyFlaggedToBacklog,
  generateWeeklyDigest,
  updateDeliverableStateFriction,
  uploadMeetingMinutes,
} from "../lib/ipc";
import type {
  AskProgressEvent,
  DigestItem,
  MeetingAction,
  MeetingWithActions,
  WeeklyDigest,
} from "../lib/types";

// ── MinutesUploadPanel ─────────────────────────────────────────────────────────

interface MinutesUploadPanelProps {
  onClose: () => void;
  onBoardRefresh?: () => void;
}

type UploadPhase = "idle" | "processing" | "done" | "error";

interface ProgressEntry {
  id: number;
  label: string;
  tool: string;
}

export function MinutesUploadPanel({ onClose, onBoardRefresh }: MinutesUploadPanelProps) {
  const navigate = useNavigate();
  const [phase, setPhase] = useState<UploadPhase>("idle");
  const [fileName, setFileName] = useState<string | null>(null);
  const [progress, setProgress] = useState<ProgressEntry[]>([]);
  const [result, setResult] = useState<MeetingWithActions | null>(null);
  const [selectedStakeholderIds, setSelectedStakeholderIds] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const progressEndRef = useRef<HTMLDivElement>(null);
  const counterRef = useRef(0);

  // Scroll progress feed to bottom as new items arrive
  useEffect(() => {
    progressEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [progress]);

  async function handlePickAndProcess() {
    const selected = await openDialog({
      title: "Select meeting minutes",
      filters: [
        { name: "Documents", extensions: ["pdf", "docx", "txt", "md"] },
      ],
      multiple: false,
    });

    if (!selected || typeof selected !== "string") return;
    const filePath = selected;
    const name = filePath.split("/").pop() ?? filePath;
    setFileName(name);
    setPhase("processing");
    setProgress([]);
    setResult(null);
    setError(null);

    // Listen for streaming progress events
    const unlisten = await listen<AskProgressEvent>("minutes:progress", (event) => {
      setProgress((prev) => [
        ...prev,
        { id: counterRef.current++, label: event.payload.label, tool: event.payload.tool },
      ]);
    });

    try {
      const res = await uploadMeetingMinutes(filePath, selectedStakeholderIds);
      setResult(res);
      setPhase("done");
      onBoardRefresh?.();
      window.dispatchEvent(new CustomEvent("board-data-changed"));
    } catch (e) {
      setError(String(e));
      setPhase("error");
    } finally {
      unlisten();
    }
  }

  return (
    <div className="flex flex-col gap-0 overflow-hidden rounded-2xl border border-zinc-200 bg-white shadow-sm">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-sky-50 shadow-sm border border-sky-100">
            <BrainCircuit size={16} className="text-sky-600" />
          </div>
          <div>
            <span className="block text-[14px] font-bold text-zinc-900 leading-none">Import Session</span>
            {fileName && (
              <span className="mt-1 block max-w-[200px] truncate text-[11px] font-medium text-zinc-400">
                {fileName}
              </span>
            )}
          </div>
        </div>
        <button onClick={onClose} className="rounded-full p-1.5 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600 transition-colors">
          <X size={16} />
        </button>
      </div>

      {/* Body */}
      <div className="flex flex-col gap-4 p-6">
        {phase === "idle" && (
          <div className="flex flex-col items-center gap-4 py-8 text-center">
            <div className="flex h-16 w-16 items-center justify-center rounded-2xl border border-zinc-100 bg-zinc-50 shadow-inner">
              <Upload size={24} className="text-zinc-300" />
            </div>
            <div className="space-y-1">
              <p className="text-[15px] font-semibold text-zinc-900">Upload Meeting Minutes</p>
              <p className="max-w-[300px] text-[12px] text-zinc-500 leading-relaxed">
                Import PDF, DOCX, or Markdown files. Trace will automatically synthesize action items and key decisions.
              </p>
            </div>
            <div className="w-full max-w-sm space-y-2 text-left mb-4">
              <label className="text-[11px] font-bold uppercase tracking-wider text-zinc-400 flex items-center gap-2">
                <Users size={12} />
                Associate Stakeholders
              </label>
              <StakeholderPicker 
                selectedIds={selectedStakeholderIds} 
                onChange={setSelectedStakeholderIds}
                placeholder="Assign stakeholders to this meeting..."
              />
            </div>

            <button
              onClick={handlePickAndProcess}
              className="btn btn-primary h-10 px-6 mt-2"
            >
              <FileText size={14} />
              Select Workspace Document
            </button>
          </div>
        )}

        {(phase === "processing" || phase === "done") && (
          <>
            {/* Progress feed */}
            {progress.length > 0 && (
              <div className="max-h-40 overflow-y-auto rounded-lg bg-zinc-50 p-3 text-[11px] font-mono">
                {progress.map((p) => (
                  <div key={p.id} className="flex items-center gap-1.5 py-0.5 text-zinc-600">
                    <CheckCircle2 size={10} className="shrink-0 text-emerald-500" />
                    <span>{p.label}</span>
                  </div>
                ))}
                {phase === "processing" && (
                  <div className="flex items-center gap-1.5 py-0.5 text-sky-600">
                    <Loader2 size={10} className="shrink-0 animate-spin" />
                    <span>Processing…</span>
                  </div>
                )}
                <div ref={progressEndRef} />
              </div>
            )}

            {phase === "processing" && progress.length === 0 && (
              <div className="flex items-center justify-center gap-2 py-4 text-[12px] text-zinc-500">
                <Loader2 size={14} className="animate-spin" />
                Starting agent…
              </div>
            )}

            {/* Results */}
            {result && (
              <MinutesSessionCard
                meeting={result}
                onOpen={() => { onClose(); navigate(`/meetings/${result.meeting.id}`); }}
                onBoardRefresh={onBoardRefresh}
              />
            )}
          </>
        )}

        {phase === "error" && (
          <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-[12px] text-red-700">
            <div className="mb-1 flex items-center gap-1.5 font-semibold">
              <AlertTriangle size={12} />
              Processing failed
            </div>
            <p className="text-red-600">{error}</p>
            <button
              onClick={() => {
                setPhase("idle");
                setFileName(null);
              }}
              className="mt-2 rounded bg-red-100 px-2 py-1 text-[11px] font-medium text-red-700 hover:bg-red-200 transition-colors"
            >
              Try again
            </button>
          </div>
        )}

        {phase === "done" && (
          <button
            onClick={() => {
              setPhase("idle");
              setFileName(null);
              setProgress([]);
              setResult(null);
            }}
            className="btn w-full mt-2 h-10 border-zinc-200 hover:bg-zinc-50"
          >
            <Plus size={16} />
            Import Another Document
          </button>
        )}
      </div>
    </div>
  );
}

// ── MinutesSessionCard ────────────────────────────────────────────────────────

function MinutesSessionCard({
  meeting,
  onOpen,
  onBoardRefresh,
}: {
  meeting: MeetingWithActions;
  onOpen: () => void;
  onBoardRefresh?: () => void;
}) {
  const m = meeting.meeting;
  const [addingToBacklog, setAddingToBacklog] = useState(false);
  const [backlogDone, setBacklogDone] = useState(false);

  const flaggedActions: MeetingAction[] = meeting.actions.filter(
    (a) => a.kind === "flagged" && !a.applied,
  );

  async function handleAddFlaggedToBacklog() {
    if (flaggedActions.length === 0) return;
    setAddingToBacklog(true);
    try {
      for (const action of flaggedActions) {
        await applyFlaggedToBacklog({ action_id: action.id, initiative_ids: [] });
      }
      setBacklogDone(true);
      onBoardRefresh?.();
      window.dispatchEvent(new CustomEvent("board-data-changed"));
    } finally {
      setAddingToBacklog(false);
    }
  }

  return (
    <div className="rounded-2xl border border-amber-200 bg-amber-50/40 overflow-hidden shadow-sm">
      <div className="flex items-start gap-4 p-5">
        <div className="mt-0.5 flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-100 shadow-sm border border-amber-200">
          <CheckCircle2 size={18} className="text-amber-600" />
        </div>
        <div className="min-w-0 flex-1 space-y-1">
          <p className="text-[14px] font-bold text-amber-950 leading-tight">{m.title}</p>
          <div className="flex items-center gap-2 text-[11px] font-medium text-amber-700/70">
            <span>{m.date}</span>
            <span className="h-1 w-1 rounded-full bg-amber-200" />
            <span>{meeting.actions.length} Intelligence Proposals</span>
          </div>
          {m.summary && (
            <p className="mt-3 text-[12px] leading-relaxed text-amber-900 opacity-80 line-clamp-2 italic">{m.summary}</p>
          )}
        </div>
      </div>
      <div className="flex flex-wrap items-center gap-4 border-t border-amber-200/50 bg-amber-100/30 px-5 py-3">
        <button
          onClick={onOpen}
          className="inline-flex items-center gap-2 text-[12px] font-bold text-amber-700 hover:text-amber-950 transition-colors"
        >
          Review Extraction
          <ArrowRight size={14} />
        </button>
        {flaggedActions.length > 0 && !backlogDone && (
          <button
            onClick={() => void handleAddFlaggedToBacklog()}
            disabled={addingToBacklog}
            className="inline-flex items-center gap-2 text-[12px] font-bold text-zinc-500 hover:text-zinc-900 transition-colors disabled:opacity-50"
          >
            {addingToBacklog ? <Loader2 size={14} className="animate-spin" /> : <Plus size={14} />}
            Commit {flaggedActions.length} items to Backlog
          </button>
        )}
        {backlogDone && (
          <span className="inline-flex items-center gap-1 text-[12px] text-emerald-600 font-bold">
            <CheckCircle2 size={14} />
            Persisted to Backlog
          </span>
        )}
      </div>
    </div>
  );
}

// ── WeeklyDigestPanel ─────────────────────────────────────────────────────────

interface WeeklyDigestPanelProps {
  onClose: () => void;
}

type DigestPhase = "idle" | "loading" | "done" | "error";

export function WeeklyDigestPanel({ onClose }: WeeklyDigestPanelProps) {
  const [phase, setPhase] = useState<DigestPhase>("idle");
  const [digest, setDigest] = useState<WeeklyDigest | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function runDigest() {
    setPhase("loading");
    setError(null);
    try {
      const d = await generateWeeklyDigest();
      setDigest(d);
      setPhase("done");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }

  return (
    <div className="flex flex-col gap-0 overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-100 px-4 py-3">
        <div className="flex items-center gap-2">
          <div className="flex h-6 w-6 items-center justify-center rounded-md bg-violet-50">
            <Sparkles size={13} className="text-violet-600" />
          </div>
          <span className="text-[13px] font-semibold text-zinc-900">Weekly digest</span>
        </div>
        <button onClick={onClose} className="rounded p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600">
          <X size={14} />
        </button>
      </div>

      <div className="flex flex-col gap-3 p-4">
        {phase === "idle" && (
          <div className="flex flex-col items-center gap-3 py-4 text-center">
            <p className="text-[12px] text-zinc-500">
              The AI will scan your entire workspace — deliverables, initiatives, stakeholders — and
              return a health report with at-risk items, stale work, and a focus recommendation.
            </p>
            <button
              onClick={runDigest}
              className="inline-flex items-center gap-1.5 rounded-md bg-violet-600 px-3 py-1.5 text-[12px] font-medium text-white hover:bg-violet-700 transition-colors"
            >
              <Sparkles size={13} />
              Run digest
            </button>
          </div>
        )}

        {phase === "loading" && (
          <div className="flex flex-col items-center gap-2 py-8">
            <Loader2 size={24} className="animate-spin text-violet-400" />
            <p className="text-[12px] text-zinc-500">Scanning workspace…</p>
          </div>
        )}

        {phase === "error" && (
          <div className="rounded-lg border border-red-200 bg-red-50 p-3 text-[12px] text-red-700">
            <div className="mb-1 flex items-center gap-1.5 font-semibold">
              <AlertTriangle size={12} />
              Digest failed
            </div>
            <p>{error}</p>
            <button
              onClick={() => setPhase("idle")}
              className="mt-2 rounded bg-red-100 px-2 py-1 text-[11px] font-medium text-red-700 hover:bg-red-200 transition-colors"
            >
              Try again
            </button>
          </div>
        )}

        {phase === "done" && digest && <DigestResultPanel digest={digest} onRerun={() => { setPhase("idle"); setDigest(null); }} />}
      </div>
    </div>
  );
}

function DigestResultPanel({ digest, onRerun }: { digest: WeeklyDigest; onRerun: () => void }) {
  return (
    <div className="flex flex-col gap-3">
      {/* Summary */}
      <div className="rounded-lg border border-violet-200 bg-violet-50 p-3">
        <div className="mb-1 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-violet-700">
          <Sparkles size={10} />
          Workspace summary
        </div>
        <p className="text-[12px] leading-relaxed text-violet-900">{digest.summary}</p>
      </div>

      {/* Focus recommendation */}
      <div className="rounded-lg border border-sky-200 bg-sky-50 p-3">
        <div className="mb-1 text-[11px] font-semibold uppercase tracking-wider text-sky-700">
          Focus recommendation
        </div>
        <p className="text-[12px] leading-relaxed text-sky-900">{digest.focus_recommendation}</p>
      </div>

      {/* Sections */}
      {digest.at_risk.length > 0 && (
        <DigestSection title="At risk" items={digest.at_risk} color="red" showBacklogAction />
      )}
      {digest.stale.length > 0 && (
        <DigestSection title="Stale" items={digest.stale} color="amber" showBacklogAction />
      )}
      {digest.conflicts.length > 0 && (
        <DigestSection title="Conflicts" items={digest.conflicts} color="orange" />
      )}
      {digest.wins.length > 0 && (
        <DigestSection title="Wins" items={digest.wins} color="emerald" />
      )}

      <button
        onClick={onRerun}
        className="inline-flex items-center justify-center gap-1.5 rounded-md border border-zinc-200 px-3 py-1.5 text-[12px] font-medium text-zinc-600 hover:bg-zinc-50 transition-colors"
      >
        <Sparkles size={13} />
        Run again
      </button>
    </div>
  );
}

type DigestColor = "red" | "amber" | "orange" | "emerald";

const digestColorMap: Record<DigestColor, { section: string; item: string; dot: string }> = {
  red: {
    section: "border-red-200 bg-red-50 text-red-700",
    item: "border-red-100",
    dot: "bg-red-400",
  },
  amber: {
    section: "border-amber-200 bg-amber-50 text-amber-700",
    item: "border-amber-100",
    dot: "bg-amber-400",
  },
  orange: {
    section: "border-orange-200 bg-orange-50 text-orange-700",
    item: "border-orange-100",
    dot: "bg-orange-400",
  },
  emerald: {
    section: "border-emerald-200 bg-emerald-50 text-emerald-700",
    item: "border-emerald-100",
    dot: "bg-emerald-400",
  },
};

function DigestSection({
  title,
  items,
  color,
  showBacklogAction,
}: {
  title: string;
  items: DigestItem[];
  color: DigestColor;
  showBacklogAction?: boolean;
}) {
  const [open, setOpen] = useState(true);
  const [movingId, setMovingId] = useState<string | null>(null);
  const [movedIds, setMovedIds] = useState<Set<string>>(new Set());
  const cls = digestColorMap[color];

  function deliverableIdFromRoute(route: string | null): string | null {
    if (!route) return null;
    const m = route.match(/\/deliverables\/([^/]+)/);
    return m ? m[1] : null;
  }

  async function handleMoveToBacklog(item: DigestItem) {
    const id = deliverableIdFromRoute(item.route);
    if (!id) return;
    setMovingId(id);
    try {
      await updateDeliverableStateFriction(id, "backlog", null);
      setMovedIds((prev) => new Set(prev).add(id));
      window.dispatchEvent(new CustomEvent("board-data-changed"));
    } finally {
      setMovingId(null);
    }
  }

  return (
    <div className={`rounded-lg border overflow-hidden ${cls.section}`}>
      <button
        onClick={() => setOpen((v) => !v)}
        className={`flex w-full items-center justify-between px-3 py-2 text-left hover:opacity-90 transition-opacity`}
      >
        <span className="text-[11px] font-semibold">
          {title} ({items.length})
        </span>
        {open ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
      </button>
      {open && (
        <div className="bg-white">
          {items.map((item, i) => {
            const deliverableId = deliverableIdFromRoute(item.route);
            const moved = deliverableId ? movedIds.has(deliverableId) : false;
            return (
              <div key={i} className={`flex items-start gap-2.5 border-t px-3 py-2 ${cls.item}`}>
                <div className={`mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full ${cls.dot}`} />
                <div className="min-w-0 flex-1">
                  {item.route ? (
                    <Link
                      to={item.route}
                      className="text-[12px] font-semibold text-zinc-800 hover:text-sky-600 transition-colors"
                    >
                      {item.title}
                    </Link>
                  ) : (
                    <p className="text-[12px] font-semibold text-zinc-800">{item.title}</p>
                  )}
                  <p className="text-[11px] text-zinc-500">{item.reason}</p>
                  {showBacklogAction && deliverableId && (
                    <div className="mt-1.5">
                      {moved ? (
                        <span className="text-[11px] text-emerald-600 font-medium">Moved to Backlog</span>
                      ) : (
                        <button
                          onClick={() => void handleMoveToBacklog(item)}
                          disabled={movingId === deliverableId}
                          className="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[11px] font-medium bg-zinc-100 text-zinc-600 hover:bg-zinc-200 transition-colors disabled:opacity-50"
                        >
                          {movingId === deliverableId ? (
                            <Loader2 size={10} className="animate-spin" />
                          ) : (
                            <ArrowRight size={10} />
                          )}
                          Move to Backlog
                        </button>
                      )}
                    </div>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
