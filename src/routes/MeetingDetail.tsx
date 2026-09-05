import { useCallback, useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  BookOpen,
  Brain,
  CalendarCheck,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Circle,
  ClipboardList,
  Flag,
  History,
  Inbox,
  Layers3,
  Loader2,
  Mic,
  NotebookPen,
  Plus,
  Square,
  StickyNote,
  Trash2,
  Users,
} from "lucide-react";
import {
  applyFlaggedToBacklog,
  applyMeetingAction,
  deleteMeeting,
  dismissMeetingAction,
  getMeeting,
  listDeliverables,
  listInitiatives,
  processMeetingAudio,
  updateMeetingStakeholders,
  updateMeetingTitle,
} from "../lib/ipc";
import { StakeholderPicker } from "../components/StakeholderPicker";
import type {
  AgentMeetingActionKind,
  AudioMeetingActionKind,
  Deliverable,
  Initiative,
  MeetingAction,
  MeetingActionKind,
  MeetingWithActions,
} from "../lib/types";
import { parseKeyDecisions } from "../lib/meeting";

// ── helpers ──────────────────────────────────────────────────────────────────

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", {
    month: "long",
    day: "numeric",
    year: "numeric",
  });
}

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function parseActionPayload(action: MeetingAction): Record<string, unknown> {
  if (!action.payload) return {};
  try {
    const parsed = JSON.parse(action.payload);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

type ActionTargetType = "deliverable" | "initiative" | null;

function actionTargetType(action: MeetingAction): ActionTargetType {
  if (
    action.kind === "deliverable_note" ||
    action.kind === "task_created" ||
    action.kind === "state_updated" ||
    action.kind === "deadline_set" ||
    action.kind === "blocker_set"
  ) {
    return "deliverable";
  }
  if (action.kind === "initiative_note") {
    return "initiative";
  }

  const payload = parseActionPayload(action);
  if (payload.target_kind === "deliverable" || payload.target_kind === "initiative") {
    return payload.target_kind;
  }
  return null;
}

// ── action kind badge — audio recording actions ───────────────────────────────

const audioKindConfig: Record<
  AudioMeetingActionKind,
  { label: string; icon: React.ReactNode; color: string }
> = {
  deliverable_note: {
    label: "Deliverable note",
    icon: <BookOpen size={11} />,
    color: "bg-blue-50 text-blue-700",
  },
  initiative_note: {
    label: "Initiative note",
    icon: <Layers3 size={11} />,
    color: "bg-violet-50 text-violet-700",
  },
  capture: {
    label: "Capture",
    icon: <Inbox size={11} />,
    color: "bg-amber-50 text-amber-700",
  },
};

const agentKindLabels: Record<AgentMeetingActionKind, string> = {
  note_added: "Note added",
  task_created: "Task created",
  state_updated: "State updated",
  deadline_set: "Deadline set",
  blocker_set: "Blocker",
  capture_created: "Capture",
  flagged: "Flagged",
};

function KindBadge({ kind }: { kind: MeetingActionKind }) {
  const cfg = audioKindConfig[kind as AudioMeetingActionKind];
  const agentLabel = agentKindLabels[kind as AgentMeetingActionKind];
  if (agentLabel) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-600">
        {agentLabel}
      </span>
    );
  }
  if (!cfg) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-500">
        {kind}
      </span>
    );
  }
  return (
    <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ${cfg.color}`}>
      {cfg.icon}
      {cfg.label}
    </span>
  );
}

// ── agent activity item ───────────────────────────────────────────────────────

const agentKindConfig: Record<
  AgentMeetingActionKind,
  { label: string; icon: React.ReactNode; dotColor: string; badgeColor: string }
> = {
  note_added: {
    label: "Note added",
    icon: <NotebookPen size={12} />,
    dotColor: "bg-sky-400",
    badgeColor: "bg-sky-50 text-sky-700",
  },
  task_created: {
    label: "Task created",
    icon: <ClipboardList size={12} />,
    dotColor: "bg-violet-400",
    badgeColor: "bg-violet-50 text-violet-700",
  },
  state_updated: {
    label: "State updated",
    icon: <CheckCircle2 size={12} />,
    dotColor: "bg-emerald-400",
    badgeColor: "bg-emerald-50 text-emerald-700",
  },
  deadline_set: {
    label: "Deadline set",
    icon: <CalendarCheck size={12} />,
    dotColor: "bg-red-400",
    badgeColor: "bg-red-50 text-red-700",
  },
  blocker_set: {
    label: "Blocker",
    icon: <AlertTriangle size={12} />,
    dotColor: "bg-orange-400",
    badgeColor: "bg-orange-50 text-orange-700",
  },
  capture_created: {
    label: "Capture saved",
    icon: <StickyNote size={12} />,
    dotColor: "bg-amber-400",
    badgeColor: "bg-amber-50 text-amber-700",
  },
  flagged: {
    label: "Flagged deliverable",
    icon: <Flag size={12} />,
    dotColor: "bg-amber-400",
    badgeColor: "bg-amber-50 text-amber-700",
  },
};

function AgentActivityItem({ action }: { action: MeetingAction }) {
  const cfg = agentKindConfig[action.kind as AgentMeetingActionKind] ?? {
    label: action.kind,
    icon: <Circle size={12} />,
    dotColor: "bg-zinc-300",
    badgeColor: "bg-zinc-50 text-zinc-500",
  };

  return (
    <div className="flex items-start gap-3">
      {/* Timeline dot */}
      <div className="relative flex shrink-0 flex-col items-center">
        <div className={`mt-1 h-2 w-2 rounded-full ${cfg.dotColor}`} />
      </div>
      <div className="min-w-0 flex-1 pb-4">
        <div className="flex flex-wrap items-center gap-2">
          <span className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px] font-medium ${cfg.badgeColor}`}>
            {cfg.icon}
            {cfg.label}
          </span>
          {action.target_title && (
            <span className="text-[11px] text-zinc-400">→ {action.target_title}</span>
          )}
        </div>
        <p className="mt-1 text-[12px] leading-relaxed text-zinc-600">{action.body}</p>
      </div>
    </div>
  );
}

function AgentActivityLog({ actions }: { actions: MeetingAction[] }) {
  const [open, setOpen] = useState(true);
  if (actions.length === 0) return null;

  return (
    <section className="mt-8">
      <button
        onClick={() => setOpen((v) => !v)}
        className="mb-4 flex w-full items-center justify-between group"
      >
        <div className="flex items-center gap-2">
          <h2 className="text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-400 group-hover:text-zinc-500 transition-colors">
            Agent activity log
          </h2>
          <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-bold text-zinc-500">
            {actions.length}
          </span>
        </div>
        {open ? <ChevronDown size={14} className="text-zinc-400" /> : <ChevronRight size={14} className="text-zinc-400" />}
      </button>
      {open && (
        <div className="relative pl-1">
          {/* Vertical timeline line */}
          <div className="absolute left-[3px] top-3 bottom-4 w-px bg-zinc-100" />
          <div className="space-y-1">
            {actions.map((action) => (
              <AgentActivityItem key={action.id} action={action} />
            ))}
          </div>
        </div>
      )}
    </section>
  );
}

// ── single action card ────────────────────────────────────────────────────────

interface ActionCardProps {
  action: MeetingAction;
  deliverables: Deliverable[];
  initiatives: Initiative[];
  onApply: (actionId: string, targetId: string | null) => Promise<void>;
  onDismiss: (actionId: string) => Promise<void>;
  onAddToBacklog: (actionId: string) => Promise<void>;
}

function ActionCard({ action, deliverables, initiatives, onApply, onDismiss, onAddToBacklog }: ActionCardProps) {
  const [selectedTarget, setSelectedTarget] = useState<string>(
    action.target_id ?? "",
  );
  const [busy, setBusy] = useState(false);

  const isFlagged = action.kind === "flagged";
  const targetType = actionTargetType(action);
  const needsTarget = !isFlagged && targetType !== null;
  const options =
    targetType === "deliverable"
      ? deliverables.map((d) => ({ id: d.id, label: d.title }))
      : targetType === "initiative"
        ? initiatives.map((i) => ({ id: i.id, label: i.title }))
        : [];

  const canApply = !needsTarget || selectedTarget !== "";

  async function handleApply() {
    setBusy(true);
    try {
      await onApply(action.id, needsTarget && selectedTarget ? selectedTarget : null);
    } finally {
      setBusy(false);
    }
  }

  async function handleDismiss() {
    setBusy(true);
    try {
      await onDismiss(action.id);
    } finally {
      setBusy(false);
    }
  }

  async function handleAddToBacklog() {
    setBusy(true);
    try {
      await onAddToBacklog(action.id);
    } finally {
      setBusy(false);
    }
  }

  if (action.applied) {
    return (
      <div className="rounded-xl border border-emerald-100 bg-emerald-50/30 px-4 py-3.5 shadow-sm shadow-emerald-500/5">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            <KindBadge kind={action.kind} />
            {action.target_title && (
              <span className="ml-2 text-[11px] font-medium text-zinc-400">re: {action.target_title}</span>
            )}
            <p className="mt-2 text-[13px] leading-relaxed text-zinc-600 italic">{action.body}</p>
          </div>
          <span className="inline-flex shrink-0 items-center gap-1 text-[11px] font-bold text-emerald-600 uppercase tracking-wider">
            <CheckCircle2 size={12} />
            Applied
          </span>
        </div>
      </div>
    );
  }

  return (
    <div className={`rounded-xl border px-4 py-4 shadow-sm transition-all ${isFlagged ? "border-amber-200 bg-amber-50/40" : "border-zinc-200 bg-white"}`}>
      <div className="flex items-start gap-4">
        {isFlagged ? (
          <div className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-amber-100 text-amber-600">
            <Flag size={14} fill="currentColor" />
          </div>
        ) : (
          <div className="mt-1 flex h-6 w-6 shrink-0 items-center justify-center rounded-lg bg-zinc-100 text-zinc-400">
            <StickyNote size={14} />
          </div>
        )}
        <div className="min-w-0 flex-1 space-y-3">
          <div className="flex flex-wrap items-center gap-2">
            <KindBadge kind={action.kind} />
            {action.target_title && (
              <span className="text-[11px] font-medium text-zinc-400">Target: {action.target_title}</span>
            )}
          </div>
          <p className="text-[14px] leading-relaxed text-zinc-800">{action.body}</p>

          {needsTarget && (
            <div className="space-y-1.5">
              <label className="field-label">Target Entity</label>
              <select
                value={selectedTarget}
                onChange={(e) => setSelectedTarget(e.target.value)}
                className="field-control text-[12px] bg-zinc-50/50"
              >
                <option value="">
                  {targetType === "deliverable"
                    ? "— Choose a deliverable —"
                    : "— Choose an initiative —"}
                </option>
                {options.map((opt) => (
                  <option key={opt.id} value={opt.id}>
                    {opt.label}
                  </option>
                ))}
              </select>
            </div>
          )}

          <div className="flex flex-wrap gap-2 pt-1">
            {isFlagged ? (
              <>
                <button
                  onClick={handleAddToBacklog}
                  disabled={busy}
                  className="btn btn-primary h-7 px-3 text-[11px]"
                >
                  <Plus size={12} />
                  Add to Backlog
                </button>
                <button
                  onClick={handleApply}
                  disabled={busy}
                  className="btn h-7 px-3 text-[11px]"
                >
                  Save as Thought
                </button>
              </>
            ) : (
              <button
                onClick={handleApply}
                disabled={busy || !canApply}
                className="btn btn-primary h-7 px-3 text-[11px]"
              >
                <CheckCircle2 size={12} />
                Approve & Sync
              </button>
            )}
            <button
              onClick={handleDismiss}
              disabled={busy}
              className="btn btn-danger h-7 px-3 text-[11px] bg-transparent"
            >
              Dismiss
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── recorder ─────────────────────────────────────────────────────────────────

interface RecorderProps {
  meetingId: string;
  onProcessed: (result: MeetingWithActions) => void;
  onError: (msg: string) => void;
}

function Recorder({ meetingId, onProcessed, onError }: RecorderProps) {
  const [isRecording, setIsRecording] = useState(false);
  const [hasRecording, setHasRecording] = useState(false);
  const [processing, setProcessing] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const [micError, setMicError] = useState<string | null>(null);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const audioBase64Ref = useRef<string | null>(null);
  const mimeTypeRef = useRef<string>("audio/webm");
  const durationRef = useRef<number>(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  function stopTimer() {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }

  async function startRecording() {
    setMicError(null);
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const mimeType = MediaRecorder.isTypeSupported("audio/webm;codecs=opus")
        ? "audio/webm;codecs=opus"
        : "audio/webm";
      mimeTypeRef.current = mimeType.split(";")[0]; // strip codec suffix for Gemini
      const recorder = new MediaRecorder(stream, { mimeType });
      mediaRecorderRef.current = recorder;
      chunksRef.current = [];
      audioBase64Ref.current = null;

      recorder.ondataavailable = (e) => {
        if (e.data.size > 0) chunksRef.current.push(e.data);
      };

      recorder.start(1000);
      setIsRecording(true);
      setElapsed(0);

      timerRef.current = setInterval(() => {
        setElapsed((prev) => prev + 1);
      }, 1000);
    } catch (e) {
      setMicError(
        "Could not access microphone. Please grant microphone permission in System Settings → Privacy → Microphone.",
      );
    }
  }

  function stopRecording() {
    const recorder = mediaRecorderRef.current;
    if (!recorder) return;
    stopTimer();
    durationRef.current = elapsed;

    recorder.onstop = () => {
      const blob = new Blob(chunksRef.current, { type: mimeTypeRef.current });
      const reader = new FileReader();
      reader.onloadend = () => {
        const dataUrl = reader.result as string;
        // dataUrl = "data:<mime>;base64,<data>"
        audioBase64Ref.current = dataUrl.split(",")[1] ?? "";
        setHasRecording(true);
      };
      reader.readAsDataURL(blob);
      recorder.stream.getTracks().forEach((t) => t.stop());
    };

    recorder.stop();
    setIsRecording(false);
  }

  async function handleProcess() {
    if (!audioBase64Ref.current) return;
    setProcessing(true);
    try {
      const result = await processMeetingAudio({
        meeting_id: meetingId,
        audio_base64: audioBase64Ref.current,
        mime_type: mimeTypeRef.current,
        duration_secs: durationRef.current,
      });
      onProcessed(result);
    } catch (e) {
      onError(String(e));
    } finally {
      setProcessing(false);
    }
  }

  function handleReRecord() {
    audioBase64Ref.current = null;
    chunksRef.current = [];
    setHasRecording(false);
    setElapsed(0);
    durationRef.current = 0;
  }

  useEffect(() => () => stopTimer(), []);

  if (processing) {
    return (
      <div className="flex flex-col items-center gap-4 rounded-2xl border border-zinc-100 bg-white py-16 shadow-[0_2px_12px_rgba(0,0,0,0.06)] overflow-hidden relative">
        <div className="absolute inset-0 bg-gradient-to-b from-sky-50/20 to-transparent pointer-events-none" />
        <div className="relative flex h-10 w-10 items-center justify-center">
          <Loader2 className="absolute animate-spin text-sky-200" size={40} />
          <Brain size={16} className="relative text-sky-600" />
        </div>
        <div className="text-center space-y-1">
          <p className="text-[15px] font-semibold text-zinc-900">Intelligence Synthesis</p>
          <p className="text-[13px] text-zinc-500">Trace is extracting action items and decisions…</p>
        </div>
        <div className="mt-4 flex gap-1">
          <div className="h-1.5 w-1.5 rounded-full bg-sky-500 animate-bounce [animation-delay:-0.3s]" />
          <div className="h-1.5 w-1.5 rounded-full bg-sky-500 animate-bounce [animation-delay:-0.15s]" />
          <div className="h-1.5 w-1.5 rounded-full bg-sky-500 animate-bounce" />
        </div>
      </div>
    );
  }

  return (
    <div className="rounded-2xl border border-zinc-200 bg-white px-8 py-10 shadow-sm relative overflow-hidden">
      <div className="absolute top-0 right-0 p-4 opacity-5 pointer-events-none">
        <Mic size={120} />
      </div>
      
      {micError && (
        <div className="notice notice-error mb-6 flex items-start gap-2">
          <AlertCircle size={16} className="mt-0.5 shrink-0" />
          <span className="text-[12px]">{micError}</span>
        </div>
      )}

      <div className="flex flex-col items-center gap-8">
        {/* Timer Display */}
        <div className="flex flex-col items-center gap-1">
          <span className="text-[11px] font-bold uppercase tracking-widest text-zinc-400">
            {isRecording ? "Live Recording" : hasRecording ? "Review Duration" : "Ready to Record"}
          </span>
          <div className={`font-mono text-[48px] font-light tabular-nums tracking-tight ${isRecording ? "text-red-600" : "text-zinc-900"}`}>
            {formatDuration(elapsed)}
          </div>
        </div>

        {/* Dynamic Visualizer */}
        <div className="flex items-center justify-center gap-1 h-12 w-full max-w-xs">
          {isRecording ? (
            Array.from({ length: 24 }).map((_, i) => (
              <div
                key={i}
                className="w-1 rounded-full bg-red-400/60 animate-pulse"
                style={{
                  height: `${15 + Math.random() * 35}px`,
                  animationDelay: `${i * 40}ms`,
                  animationDuration: `${500 + Math.random() * 500}ms`,
                }}
              />
            ))
          ) : (
            <div className="h-[2px] w-full bg-zinc-100 rounded-full" />
          )}
        </div>

        {!isRecording && !hasRecording && (
          <p className="text-[14px] text-zinc-500 text-center max-w-[280px] leading-relaxed">
            Initialize the session and speak naturally about the meeting's outcomes.
          </p>
        )}

        {hasRecording && !isRecording && (
          <div className="flex items-center gap-2 rounded-full bg-zinc-50 px-3 py-1.5 text-[12px] font-medium text-zinc-500">
            <CheckCircle2 size={14} className="text-emerald-500" />
            Session captured successfully
          </div>
        )}

        {/* Action Controls */}
        <div className="flex items-center gap-4 pt-2">
          {!isRecording && !hasRecording && (
            <button
              onClick={startRecording}
              className="group relative flex h-16 w-16 items-center justify-center rounded-full bg-red-600 text-white shadow-sm shadow-red-500/15 transition-all hover:bg-red-500 hover:scale-110 active:scale-95"
            >
              <div className="absolute inset-0 rounded-full bg-red-500 animate-ping opacity-20 group-hover:hidden" />
              <Mic size={28} />
            </button>
          )}

          {isRecording && (
            <button
              onClick={stopRecording}
              className="flex h-16 w-16 items-center justify-center rounded-full bg-zinc-900 text-white shadow-sm shadow-zinc-900/15 transition-all hover:bg-zinc-800 hover:scale-110 active:scale-95"
            >
              <Square size={24} className="fill-current" />
            </button>
          )}

          {hasRecording && !isRecording && (
            <div className="flex gap-3">
              <button
                onClick={handleProcess}
                className="btn btn-primary btn-lg rounded-2xl"
              >
                <Brain size={18} />
                Synthesize Meeting Data
              </button>
              <button
                onClick={handleReRecord}
                className="btn h-12 px-6 rounded-2xl border-zinc-200 hover:bg-red-50 hover:text-red-600 hover:border-red-100 transition-all"
              >
                <History size={18} />
                Discard & Retry
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── main view ─────────────────────────────────────────────────────────────────

export function MeetingDetail() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const [data, setData] = useState<MeetingWithActions | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [processError, setProcessError] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [transcriptOpen, setTranscriptOpen] = useState(false);
  const [deliverables, setDeliverables] = useState<Deliverable[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    Promise.all([
      getMeeting(id),
      listDeliverables(),
      listInitiatives(),
    ])
      .then(([meetingData, dels, inits]) => {
        setData(meetingData);
        setTitleDraft(meetingData.meeting.title);
        setDeliverables(dels);
        setInitiatives(inits);
      })
      .catch((e) => setLoadError(String(e)))
      .finally(() => setLoading(false));
  }, [id]);

  const handleProcessed = useCallback((result: MeetingWithActions) => {
    setData(result);
    setTitleDraft(result.meeting.title);
    setProcessError(null);
  }, []);

  const handleProcessError = useCallback((msg: string) => {
    setProcessError(msg);
    if (id) {
      getMeeting(id).then(setData).catch(() => {});
    }
  }, [id]);

  async function saveTitle() {
    if (!id || !data) return;
    const trimmed = titleDraft.trim();
    if (!trimmed || trimmed === data.meeting.title) {
      setEditingTitle(false);
      return;
    }
    const updated = await updateMeetingTitle(id, trimmed);
    setData((prev) => prev ? { ...prev, meeting: updated } : prev);
    setEditingTitle(false);
  }

  async function handleApply(actionId: string, targetId: string | null) {
    const updated = await applyMeetingAction({ action_id: actionId, target_id: targetId });
    setData((prev) =>
      prev
        ? {
            ...prev,
            actions: prev.actions.map((a) => (a.id === updated.id ? updated : a)),
          }
        : prev,
    );
    window.dispatchEvent(new CustomEvent("board-data-changed"));
  }

  async function handleDismiss(actionId: string) {
    await dismissMeetingAction(actionId);
    setData((prev) =>
      prev ? { ...prev, actions: prev.actions.filter((a) => a.id !== actionId) } : prev,
    );
  }

  async function handleAddToBacklog(actionId: string) {
    const deliverable = await applyFlaggedToBacklog({ action_id: actionId, initiative_ids: [] });
    setData((prev) =>
      prev
        ? {
            ...prev,
            actions: prev.actions.map((a) =>
              a.id === actionId ? { ...a, applied: true, target_id: deliverable.id, target_title: deliverable.title } : a,
            ),
          }
        : prev,
    );
    window.dispatchEvent(new CustomEvent("board-data-changed"));
  }
  
  async function handleUpdateStakeholders(stakeholderIds: string[]) {
    if (!id) return;
    await updateMeetingStakeholders(id, stakeholderIds);
    // Refresh meeting data to get updated stakeholders list
    const updated = await getMeeting(id);
    setData(updated);
  }

  async function handleDelete() {
    if (!id) return;
    if (!confirm("Delete this meeting? This cannot be undone.")) return;
    await deleteMeeting(id);
    navigate("/meetings");
  }

  if (loading) {
    return (
      <div className="space-y-3 p-5">
        {[...Array(4)].map((_, i) => (
          <div key={i} className="skeleton h-16" />
        ))}
      </div>
    );
  }

  if (loadError || !data) {
    return (
      <div className="mx-auto max-w-2xl px-6 py-8">
        <div className="rounded-md border border-red-200 bg-red-50 px-4 py-3 text-[13px] text-red-700">
          {loadError ?? "Meeting not found."}
        </div>
      </div>
    );
  }

  const { meeting, actions } = data;
  const keyDecisions = parseKeyDecisions(meeting.key_decisions);
  const pendingActions = actions.filter((a) => !a.applied);
  const appliedActions = actions.filter((a) => a.applied);

  return (
    <div className="mx-auto max-w-2xl px-6 py-10">
      {/* Navigation */}
      <div className="mb-10 flex items-center justify-between">
        <button
          onClick={() => navigate("/meetings")}
          className="group inline-flex items-center gap-2 text-[13px] font-medium text-zinc-400 hover:text-zinc-900 transition-colors"
        >
          <div className="flex h-7 w-7 items-center justify-center rounded-full border border-zinc-100 bg-white shadow-sm group-hover:border-zinc-200 transition-colors">
            <ArrowLeft size={14} />
          </div>
          Back to Meetings
        </button>
        <button
          onClick={handleDelete}
          className="btn btn-danger h-8 px-3 text-[12px] opacity-60 hover:opacity-100 transition-all"
        >
          <Trash2 size={13} />
          Delete Meeting
        </button>
      </div>

      <div className="space-y-12">
        {/* Header Section */}
        <div className="space-y-2">
          <div className="flex items-center gap-3">
            <p className="page-kicker">Session Intelligence</p>
            <div className="h-1 w-1 rounded-full bg-zinc-200" />
            <span className="text-[11px] font-bold text-zinc-400 uppercase tracking-widest">
              {formatDate(meeting.date)}
            </span>
          </div>
          
          {editingTitle ? (
            <div className="flex items-center gap-2">
              <input
                autoFocus
                value={titleDraft}
                onChange={(e) => setTitleDraft(e.target.value)}
                onBlur={saveTitle}
                onKeyDown={(e) => {
                  if (e.key === "Enter") saveTitle();
                  if (e.key === "Escape") {
                    setTitleDraft(meeting.title);
                    setEditingTitle(false);
                  }
                }}
                className="page-title w-full bg-transparent border-b border-sky-500 focus:outline-none pb-1"
              />
            </div>
          ) : (
            <h1
              onClick={() => setEditingTitle(true)}
              className="page-title cursor-text group flex items-center gap-2 hover:text-sky-600 transition-colors"
              title="Click to edit title"
            >
              {meeting.title || "Untitled Meeting"}
              <Plus size={16} className="text-zinc-200 opacity-0 group-hover:opacity-100 transition-all rotate-45" />
            </h1>
          )}

          {/* Stakeholders Section */}
          <div className="flex flex-col gap-3 pt-4">
            <label className="text-[10px] font-bold uppercase tracking-widest text-zinc-400 flex items-center gap-1.5">
              <Users size={12} />
              Session Participants
            </label>
            <div className="flex flex-wrap items-center gap-2">
              {meeting.stakeholders.map((s) => (
                <Link
                  key={s.id}
                  to={`/stakeholders/${s.id}`}
                  className="inline-flex items-center gap-1.5 rounded-full border border-sky-100 bg-sky-50 px-3 py-1 text-[12px] font-semibold text-sky-700 transition-all hover:bg-sky-100 hover:border-sky-200"
                >
                  <div className="flex h-5 w-5 items-center justify-center rounded-full bg-sky-200/50 text-[10px] font-bold">
                    {s.name.charAt(0).toUpperCase()}
                  </div>
                  {s.name}
                </Link>
              ))}
              <StakeholderPicker
                selectedIds={meeting.stakeholders.map((s) => s.id)}
                onChange={handleUpdateStakeholders}
                trigger={
                  <button className="flex h-8 w-8 items-center justify-center rounded-full border border-dashed border-zinc-200 text-zinc-400 hover:border-sky-300 hover:bg-sky-50 hover:text-sky-600 transition-all">
                    <Plus size={16} />
                  </button>
                }
              />
            </div>
          </div>
        </div>

        {/* Summary section */}
        <div className="grid gap-8 sm:grid-cols-2">
          <div className="space-y-4">
            <div className="flex items-center gap-2 border-b border-zinc-100 pb-2">
              <StickyNote size={14} className="text-zinc-400" />
              <h2 className="text-[11px] font-bold uppercase tracking-wider text-zinc-900">Summary</h2>
            </div>
            <p className="text-[14px] leading-relaxed text-zinc-600">
              {meeting.summary || "No summary available."}
            </p>
          </div>

          <div className="space-y-4">
            <div className="flex items-center gap-2 border-b border-zinc-100 pb-2">
              <CheckCircle2 size={14} className="text-zinc-400" />
              <h2 className="text-[11px] font-bold uppercase tracking-wider text-zinc-900">Key Decisions</h2>
            </div>
            {keyDecisions.length > 0 ? (
              <ul className="space-y-3">
                {keyDecisions.map((d, i) => (
                  <li key={i} className="flex items-start gap-2 text-[13px] text-zinc-600">
                    <div className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-sky-500" />
                    {d}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-[13px] text-zinc-400 italic">No key decisions identified.</p>
            )}
          </div>
        </div>

        {/* Recorder — only when draft or error */}
        {(meeting.status === "draft" || meeting.status === "error") && (
          <div className="space-y-6">
            {meeting.status === "error" && (
              <div className="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-4 py-3 text-[13px] text-red-700">
                <AlertCircle size={14} className="mt-0.5 shrink-0" />
                <div>
                  <p className="font-medium">Processing failed</p>
                  <p className="mt-0.5 text-[12px]">
                    {processError ?? meeting.error_message ?? "Unknown error"}
                  </p>
                  <p className="mt-1 text-[12px]">You can re-record and try again.</p>
                </div>
              </div>
            )}
            {processError && meeting.status !== "error" && (
               <div className="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-4 py-3 text-[13px] text-red-700">
                <AlertCircle size={14} className="mt-0.5 shrink-0" />
                <p>{processError}</p>
              </div>
            )}
            <Recorder
              meetingId={meeting.id}
              onProcessed={handleProcessed}
              onError={handleProcessError}
            />
          </div>
        )}

        {/* Action intelligence */}
        {meeting.status === "done" && (
          <section className="space-y-8">
            <div className="space-y-6">
              <div className="flex items-center justify-between border-b border-zinc-100 pb-3">
                <div className="flex items-center gap-2">
                  <h2 className="text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-400">
                    Action Items & Proposals
                  </h2>
                  {pendingActions.length > 0 && (
                    <span className="rounded-md bg-amber-50 px-1.5 py-0.5 text-[10px] font-bold text-amber-600 border border-amber-100">
                      {pendingActions.length} pending
                    </span>
                  )}
                </div>
                {actions.length > 0 && (
                  <span className="text-[11px] font-medium text-zinc-400">
                    {actions.length} Total
                  </span>
                )}
              </div>

              {actions.length === 0 ? (
                <div className="flex flex-col items-center justify-center py-12 text-center rounded-2xl border border-dashed border-zinc-100 bg-zinc-50/30">
                  <ClipboardList size={24} className="text-zinc-200 mb-2" />
                  <p className="text-[13px] text-zinc-400">No action items were identified in this session.</p>
                </div>
              ) : (
                <div className="space-y-3">
                  {pendingActions.map((action) => (
                    <ActionCard
                      key={action.id}
                      action={action}
                      deliverables={deliverables}
                      initiatives={initiatives}
                      onApply={handleApply}
                      onDismiss={handleDismiss}
                      onAddToBacklog={handleAddToBacklog}
                    />
                  ))}
                  
                  {appliedActions.length > 0 && (
                    <div className="space-y-3 pt-4">
                      <h3 className="text-[10px] font-bold uppercase tracking-widest text-zinc-400">
                        Applied & Logged
                      </h3>
                      <div className="space-y-3">
                        {appliedActions.map((action) => (
                          <ActionCard
                            key={action.id}
                            action={action}
                            deliverables={deliverables}
                            initiatives={initiatives}
                            onApply={handleApply}
                            onDismiss={handleDismiss}
                            onAddToBacklog={handleAddToBacklog}
                          />
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}
            </div>

            <AgentActivityLog actions={actions} />

            {/* Transcript intelligence */}
            {meeting.transcript && (
              <div className="pt-4 border-t border-zinc-100">
                <button
                  onClick={() => setTranscriptOpen((v) => !v)}
                  className="flex w-full items-center justify-between group"
                >
                  <div className="flex items-center gap-2">
                    <h2 className="text-[11px] font-bold uppercase tracking-[0.15em] text-zinc-400 group-hover:text-zinc-500 transition-colors">
                      Full Session Transcript
                    </h2>
                  </div>
                  {transcriptOpen ? (
                    <ChevronDown size={14} className="text-zinc-400" />
                  ) : (
                    <ChevronRight size={14} className="text-zinc-400" />
                  )}
                </button>
                {transcriptOpen && (
                  <div className="mt-4 motion-panel p-6 overflow-hidden bg-zinc-50/50">
                    <div className="prose prose-sm max-w-none">
                      <pre className="whitespace-pre-wrap font-sans text-[12px] leading-relaxed text-zinc-600 selection:bg-sky-100">
                        {meeting.transcript}
                      </pre>
                    </div>
                  </div>
                )}
              </div>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
