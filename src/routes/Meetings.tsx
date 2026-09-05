import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  AlertCircle,
  CheckCircle,
  ChevronRight,
  FileText,
  History,
  Mic,
  Plus,
  Upload,
  Video,
} from "lucide-react";
import { createMeeting, listMeetings } from "../lib/ipc";
import type { Meeting } from "../lib/types";
import { MinutesUploadPanel } from "../components/MinutesPanel";
import { DriveTranscriptImportPanel } from "../components/files/DriveTranscriptImportPanel";
import { EmptyState } from "../components/EmptyState";

function formatDuration(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

function formatDate(dateStr: string): string {
  try {
    const d = new Date(dateStr + "T00:00:00");
    return d.toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
  } catch {
    return dateStr;
  }
}

function StatusChip({ status }: { status: Meeting["status"] }) {
  if (status === "done") {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-emerald-100 bg-emerald-50 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-emerald-700 shadow-sm shadow-emerald-500/5">
        <CheckCircle size={10} strokeWidth={2.5} />
        Done
      </span>
    );
  }
  if (status === "error") {
    return (
      <span className="inline-flex items-center gap-1 rounded-full border border-red-100 bg-red-50 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-red-700 shadow-sm shadow-red-500/5">
        <AlertCircle size={10} strokeWidth={2.5} />
        Error
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded-full border border-zinc-200 bg-zinc-50 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-zinc-500 shadow-sm">
      <Mic size={10} strokeWidth={2.5} />
      Draft
    </span>
  );
}

type ActivePanel = null | "minutes" | "drive";

export function Meetings() {
  const navigate = useNavigate();
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [loading, setLoading] = useState(true);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activePanel, setActivePanel] = useState<ActivePanel>(null);

  useEffect(() => {
    setLoading(true);
    listMeetings()
      .then(setMeetings)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, []);

  async function handleNew() {
    setCreating(true);
    try {
      const today = new Date().toISOString().slice(0, 10);
      const meeting = await createMeeting({
        title: "Untitled meeting",
        date: today,
        stakeholder_ids: [],
      });
      navigate(`/meetings/${meeting.id}`);
    } catch (e) {
      setError(String(e));
      setCreating(false);
    }
  }

  function togglePanel(panel: ActivePanel) {
    setActivePanel((prev) => (prev === panel ? null : panel));
  }

  return (
    <div className="mx-auto max-w-5xl px-6 py-10">
      <div className="mb-10 flex flex-col gap-6 md:flex-row md:items-end md:justify-between">
        <div className="space-y-1">
          <p className="page-kicker">Workspace Records</p>
          <h1 className="page-title">Meetings</h1>
          <p className="text-[14px] text-zinc-500 max-w-md">
            Transcribe discussions, extract AI action items, and maintain a persistent history of decisions.
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <div className="flex flex-col items-end gap-1">
            <div className="flex items-center gap-2">
              <button
                onClick={() => togglePanel("drive")}
                className={[
                  "btn",
                  activePanel === "drive" ? "bg-zinc-100 border-zinc-300" : ""
                ].join(" ")}
              >
                <Video size={14} className="text-zinc-500" />
                From Drive
              </button>
              <button
                onClick={() => togglePanel("minutes")}
                className={[
                  "btn",
                  activePanel === "minutes" ? "bg-zinc-100 border-zinc-300" : ""
                ].join(" ")}
              >
                <Upload size={14} className="text-zinc-500" />
                Upload
              </button>
              <button
                onClick={handleNew}
                disabled={creating}
                className="btn btn-primary"
              >
                <Plus size={16} />
                New meeting
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* Panels */}
      {activePanel && (
        <div className="mb-8 motion-panel overflow-hidden">
          <div className="bg-zinc-50/50 p-6">
            {activePanel === "drive" ? (
              <DriveTranscriptImportPanel
                onClose={() => setActivePanel(null)}
                onImported={() => {
                  listMeetings().then(setMeetings).catch(() => null);
                }}
              />
            ) : (
              <MinutesUploadPanel onClose={() => setActivePanel(null)} />
            )}
          </div>
        </div>
      )}

      {error && (
        <div className="notice notice-error mb-6 flex items-center gap-3">
          <AlertCircle size={16} />
          {error}
        </div>
      )}

      {loading ? (
        <div className="space-y-2">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="skeleton h-[72px]" />
          ))}
        </div>
      ) : meetings.length === 0 ? (
        <EmptyState
          variant="page"
          icon={Mic}
          title="No meetings indexed"
          description="Record a live discussion or import transcripts to extract action items and decisions automatically."
          cta={{ label: "Initialize a meeting", onClick: handleNew, primary: true }}
        />
      ) : (
        <div className="space-y-3">
          {meetings.map((m) => {
            const isUploaded = m.transcript === null && m.duration_secs === null && m.status === "done";
            return (
              <button
                key={m.id}
                onClick={() => navigate(`/meetings/${m.id}`)}
                className="group relative flex w-full items-center gap-4 rounded-xl border border-zinc-100 bg-white p-4 text-left transition-all hover:border-sky-200 hover:shadow-[0_4px_20px_rgba(0,0,0,0.09)] active:scale-[0.995]"
              >
                <div className={[
                  "flex h-12 w-12 shrink-0 items-center justify-center rounded-xl transition-colors",
                  isUploaded ? "bg-sky-50 text-sky-500 group-hover:bg-sky-100" : "bg-zinc-50 text-zinc-400 group-hover:bg-zinc-100 group-hover:text-zinc-600",
                ].join(" ")}>
                  {isUploaded ? <Upload size={20} /> : <FileText size={22} />}
                </div>
                
                <div className="min-w-0 flex-1 flex flex-col sm:flex-row sm:items-center sm:justify-between gap-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2 mb-0.5">
                      <h3 className="truncate text-[15px] font-semibold text-zinc-900 group-hover:text-sky-600 transition-colors">
                        {m.title || "Untitled Meeting"}
                      </h3>
                      {isUploaded && (
                        <span className="shrink-0 flex items-center gap-1 text-[10px] font-bold uppercase tracking-widest text-zinc-400 bg-zinc-100/50 px-1.5 py-0.5 rounded">
                          Import
                        </span>
                      )}
                    </div>
                    <div className="flex items-center gap-2 text-[12px] text-zinc-500">
                      <span className="font-medium text-zinc-400">{formatDate(m.date)}</span>
                      {m.duration_secs != null && (
                        <>
                          <span className="h-0.5 w-0.5 rounded-full bg-zinc-300" />
                          <span className="flex items-center gap-1">
                            <History size={10} />
                            {formatDuration(m.duration_secs)}
                          </span>
                        </>
                      )}
                    </div>
                  </div>
                  
                  <div className="shrink-0 flex items-center gap-4">
                    {m.stakeholders.length > 0 && (
                      <div className="flex -space-x-2 overflow-hidden">
                        {m.stakeholders.slice(0, 3).map((s) => (
                          <div 
                            key={s.id} 
                            className="inline-flex h-6 w-6 items-center justify-center rounded-full border-2 border-white bg-zinc-100 text-[10px] font-bold text-zinc-600 shadow-sm"
                            title={s.name}
                          >
                            {s.name.charAt(0).toUpperCase()}
                          </div>
                        ))}
                        {m.stakeholders.length > 3 && (
                          <div className="inline-flex h-6 w-6 items-center justify-center rounded-full border-2 border-white bg-zinc-100 text-[10px] font-bold text-zinc-500 shadow-sm">
                            +{m.stakeholders.length - 3}
                          </div>
                        )}
                      </div>
                    )}
                    <StatusChip status={m.status} />
                    <ChevronRight size={14} className="text-zinc-300 opacity-0 group-hover:opacity-100 group-hover:translate-x-1 transition-all" />
                  </div>
                </div>
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
