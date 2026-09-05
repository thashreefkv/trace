import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { useNavigate } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  CheckCircle2,
  ChevronRight,
  FileText,
  Folder,
  FolderOpen,
  Loader2,
  Plus,
  Users,
  Video,
  X,
} from "lucide-react";
import { StakeholderPicker } from "../StakeholderPicker";
import {
  clearGmeetFolder,
  driveListChildren,
  driveStatus,
  getGmeetFolder,
  importDriveTranscript,
  setGmeetFolder,
  type DriveAccount,
  type DriveEntry,
  type GmeetFolder,
} from "../../lib/files";
import type { AskProgressEvent, MeetingWithActions } from "../../lib/types";
import { ConnectDriveCard } from "./ConnectDriveCard";

const GDOC_MIME = "application/vnd.google-apps.document";

interface DriveTranscriptImportPanelProps {
  onClose: () => void;
  onImported?: () => void;
}

type Phase = "loading" | "no-drive" | "pick-folder" | "browse" | "importing" | "done" | "error";

interface ProgressEntry {
  id: number;
  label: string;
}

export function DriveTranscriptImportPanel({
  onClose,
  onImported,
}: DriveTranscriptImportPanelProps) {
  const navigate = useNavigate();
  const [phase, setPhase] = useState<Phase>("loading");
  const [account, setAccount] = useState<DriveAccount | null>(null);
  const [gmeetFolder, setGmeetFolderState] = useState<GmeetFolder | null>(null);
  const [transcripts, setTranscripts] = useState<DriveEntry[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<ProgressEntry[]>([]);
  const [result, setResult] = useState<MeetingWithActions | null>(null);
  const [importingId, setImportingId] = useState<string | null>(null);
  const [selectedStakeholderIds, setSelectedStakeholderIds] = useState<string[]>([]);
  const counterRef = useRef(0);
  const progressEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    progressEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [progress]);

  useEffect(() => {
    void (async () => {
      try {
        const status = await driveStatus();
        if (!status.connected || status.accounts.length === 0) {
          setPhase("no-drive");
          return;
        }
        const acc = status.accounts[0];
        setAccount(acc);
        const folder = await getGmeetFolder(acc.id);
        if (folder) {
          setGmeetFolderState(folder);
          await loadTranscripts(folder.folderId);
        } else {
          setPhase("pick-folder");
        }
      } catch (e) {
        setError(String(e));
        setPhase("error");
      }
    })();
  }, []);

  async function loadTranscripts(folderId: string) {
    try {
      setPhase("loading");
      const listing = await driveListChildren(folderId, null);
      const docs = listing.entries.filter((e) => e.mimeType === GDOC_MIME);
      setTranscripts(docs);
      setPhase("browse");
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }

  async function handleFolderSelected(folderId: string, folderName: string) {
    if (!account) return;
    try {
      await setGmeetFolder(account.id, folderId, folderName);
      const folder: GmeetFolder = { accountId: account.id, folderId, folderName };
      setGmeetFolderState(folder);
      await loadTranscripts(folderId);
    } catch (e) {
      setError(String(e));
      setPhase("error");
    }
  }

  async function handleChangeFolder() {
    if (!account || !gmeetFolder) return;
    await clearGmeetFolder(account.id);
    setGmeetFolderState(null);
    setTranscripts([]);
    setPhase("pick-folder");
  }

  async function handleImport(entry: DriveEntry) {
    setImportingId(entry.id);
    setPhase("importing");
    setProgress([]);
    setResult(null);
    setError(null);

    const unlisten = await listen<AskProgressEvent>("minutes:progress", (event) => {
      setProgress((prev) => [
        ...prev,
        { id: counterRef.current++, label: event.payload.label },
      ]);
    });

    try {
      const res = await importDriveTranscript(entry.id, entry.name, selectedStakeholderIds);
      setResult(res);
      setPhase("done");
      onImported?.();
      window.dispatchEvent(new CustomEvent("board-data-changed"));
    } catch (e) {
      setError(String(e));
      setPhase("error");
    } finally {
      unlisten();
      setImportingId(null);
    }
  }

  return (
    <div className="flex flex-col overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-xl bg-blue-50 border border-blue-100 shadow-sm">
            <Video size={16} className="text-blue-600" />
          </div>
          <div>
            <span className="block text-[14px] font-bold text-zinc-900 leading-none">Drive Integration</span>
            {gmeetFolder && phase !== "pick-folder" && (
              <span className="mt-1 flex items-center gap-1 text-[11px] font-medium text-zinc-400">
                <Folder size={10} />
                {gmeetFolder.folderName}
              </span>
            )}
          </div>
        </div>
        <div className="flex items-center gap-3">
          {gmeetFolder && phase === "browse" && (
            <button
              onClick={() => void handleChangeFolder()}
              className="text-[11px] font-bold uppercase tracking-wider text-zinc-400 hover:text-zinc-900 transition-colors"
            >
              Change Root
            </button>
          )}
          <button
            onClick={onClose}
            className="rounded-full p-1.5 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600 transition-colors"
          >
            <X size={16} />
          </button>
        </div>
      </div>

      {/* Body */}
      <div className="min-h-[120px]">
        {phase === "loading" && (
          <div className="flex items-center justify-center gap-2 py-10 text-[12px] text-zinc-500">
            <Loader2 size={14} className="animate-spin" />
            Loading…
          </div>
        )}

        {phase === "no-drive" && (
          <div className="p-4">
            <ConnectDriveCard
              onConnected={(acc) => {
                setAccount(acc);
                setPhase("pick-folder");
              }}
            />
          </div>
        )}

        {phase === "pick-folder" && (
          <DriveFolderPicker onSelected={handleFolderSelected} />
        )}

        {phase === "browse" && (
          <div className="flex flex-col">
            <div className="px-5 py-3 bg-zinc-50/50 border-b border-zinc-100">
              <label className="text-[10px] font-bold uppercase tracking-widest text-zinc-400 flex items-center gap-1.5 mb-2">
                <Users size={12} />
                Associate Stakeholders
              </label>
              <StakeholderPicker 
                selectedIds={selectedStakeholderIds} 
                onChange={setSelectedStakeholderIds}
                placeholder="Assign stakeholders to transcripts..."
              />
            </div>
            <TranscriptList
              transcripts={transcripts}
              onImport={handleImport}
              onRefresh={() => gmeetFolder && loadTranscripts(gmeetFolder.folderId)}
            />
          </div>
        )}

        {(phase === "importing" || phase === "done") && (
          <div className="flex flex-col gap-3 p-4">
            {progress.length > 0 && (
              <div className="max-h-40 overflow-y-auto rounded-lg bg-zinc-50 p-3 text-[11px] font-mono">
                {progress.map((p) => (
                  <div key={p.id} className="flex items-center gap-1.5 py-0.5 text-zinc-600">
                    <CheckCircle2 size={10} className="shrink-0 text-emerald-500" />
                    <span>{p.label}</span>
                  </div>
                ))}
                {phase === "importing" && (
                  <div className="flex items-center gap-1.5 py-0.5 text-sky-600">
                    <Loader2 size={10} className="shrink-0 animate-spin" />
                    <span>Processing…</span>
                  </div>
                )}
                <div ref={progressEndRef} />
              </div>
            )}
            {phase === "importing" && progress.length === 0 && (
              <div className="flex items-center justify-center gap-2 py-4 text-[12px] text-zinc-500">
                <Loader2 size={14} className="animate-spin" />
                Starting agent…
              </div>
            )}
            {result && (
              <div className="rounded-2xl border border-amber-200 bg-amber-50/40 p-5 shadow-sm">
                <div className="flex items-center gap-3">
                  <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-amber-100 text-amber-600 shadow-sm border border-amber-200">
                    <CheckCircle2 size={16} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="text-[14px] font-bold text-amber-950 truncate">{result.meeting.title}</p>
                    <p className="text-[11px] font-medium text-amber-700/70">
                      {result.meeting.date} · {result.actions.length} Proposals Identified
                    </p>
                  </div>
                </div>
                <button
                  onClick={() => {
                    onClose();
                    navigate(`/meetings/${result.meeting.id}`);
                  }}
                  className="mt-4 btn w-full bg-white border-amber-200 text-amber-900 hover:bg-amber-50"
                >
                  Enter Session Workspace
                  <ArrowRight size={14} />
                </button>
              </div>
            )}
            {phase === "done" && (
              <button
                onClick={() => {
                  setPhase("browse");
                  setProgress([]);
                  setResult(null);
                }}
                className="btn w-full mt-2 border-zinc-200 hover:bg-zinc-50"
              >
                <Plus size={16} />
                Import Another Transcript
              </button>
            )}
          </div>
        )}

        {phase === "error" && (
          <div className="m-4 rounded-lg border border-red-200 bg-red-50 p-3 text-[12px] text-red-700">
            <div className="mb-1 flex items-center gap-1.5 font-semibold">
              <AlertTriangle size={12} />
              {importingId ? "Import failed" : "Error"}
            </div>
            <p>{error}</p>
            <button
              onClick={() => {
                setPhase(gmeetFolder ? "browse" : "pick-folder");
                setError(null);
              }}
              className="mt-2 rounded bg-red-100 px-2 py-1 text-[11px] font-medium text-red-700 hover:bg-red-200 transition-colors"
            >
              Go back
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

// ── Folder picker ─────────────────────────────────────────────────────────────

function DriveFolderPicker({
  onSelected,
}: {
  onSelected: (folderId: string, folderName: string) => void;
}) {
  const [stack, setStack] = useState<Array<{ id: string; name: string }>>([]);
  const [entries, setEntries] = useState<DriveEntry[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const currentId = stack.length > 0 ? stack[stack.length - 1].id : null;
  const currentName = stack.length > 0 ? stack[stack.length - 1].name : "My Drive";

  useEffect(() => {
    void loadFolder(currentId);
  }, [currentId]);

  async function loadFolder(parentId: string | null) {
    setLoading(true);
    setEntries(null);
    setError(null);
    try {
      const listing = await driveListChildren(parentId, null);
      setEntries(listing.entries.filter((e) => e.isFolder));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  function handleNavigate(entry: DriveEntry) {
    setStack((prev) => [...prev, { id: entry.id, name: entry.name }]);
  }

  function handleBack() {
    setStack((prev) => prev.slice(0, -1));
  }

  return (
    <div className="flex flex-col">
      {/* Breadcrumb */}
      <div className="flex items-center gap-1 border-b border-zinc-100 px-4 py-2 text-[12px]">
        {stack.length > 0 && (
          <button
            onClick={handleBack}
            className="text-zinc-400 hover:text-zinc-700 transition-colors"
          >
            ←
          </button>
        )}
        <FolderOpen size={13} className="text-zinc-400" />
        <span className="font-medium text-zinc-700">{currentName}</span>
      </div>

      {/* Instruction */}
      <p className="px-4 pt-3 text-[12px] text-zinc-500">
        Navigate to the folder where Google Meet saves your transcripts, then click{" "}
        <strong>Select this folder</strong>.
      </p>

      {/* Folder list */}
      <div className="max-h-56 overflow-y-auto p-2">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-8 text-[12px] text-zinc-500">
            <Loader2 size={14} className="animate-spin" />
            Loading…
          </div>
        ) : error ? (
          <p className="px-3 py-4 text-[12px] text-red-600">{error}</p>
        ) : entries && entries.length === 0 ? (
          <p className="px-3 py-4 text-center text-[12px] text-zinc-400">No subfolders here.</p>
        ) : (
          <ul className="space-y-0.5">
            {entries?.map((entry) => (
              <li key={entry.id}>
                <button
                  onClick={() => handleNavigate(entry)}
                  className="flex w-full items-center gap-2.5 rounded-md px-3 py-2 text-left text-[13px] text-zinc-700 hover:bg-zinc-50 transition-colors"
                >
                  <Folder size={14} className="shrink-0 text-blue-400" />
                  <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                  <ChevronRight size={13} className="shrink-0 text-zinc-300" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {/* Select button */}
      {stack.length > 0 && (
        <div className="border-t border-zinc-100 px-5 py-4 bg-zinc-50/50">
          <button
            onClick={() => {
              const top = stack[stack.length - 1];
              onSelected(top.id, top.name);
            }}
            className="btn btn-primary btn-lg w-full"
          >
            <FolderOpen size={16} />
            Set "{stack[stack.length - 1].name}" as Root
          </button>
        </div>
      )}
    </div>
  );
}

// ── Transcript list ───────────────────────────────────────────────────────────

function TranscriptList({
  transcripts,
  onImport,
  onRefresh,
}: {
  transcripts: DriveEntry[];
  onImport: (entry: DriveEntry) => void;
  onRefresh: () => void;
}) {
  if (transcripts.length === 0) {
    return (
      <div className="flex flex-col items-center gap-2 py-10 text-center">
        <FileText size={22} className="text-zinc-300" />
        <p className="text-[13px] text-zinc-500">No Google Docs found in this folder.</p>
        <p className="max-w-xs text-[12px] text-zinc-400">
          Make sure Google Meet is configured to save transcripts to this folder.
        </p>
        <button
          onClick={onRefresh}
          className="mt-1 text-[12px] text-zinc-500 underline hover:text-zinc-700 transition-colors"
        >
          Refresh
        </button>
      </div>
    );
  }

  return (
    <div className="max-h-80 overflow-y-auto divide-y divide-zinc-100">
      {transcripts.map((entry) => (
        <div
          key={entry.id}
          className="group flex items-center gap-4 px-5 py-4 hover:bg-zinc-50/80 transition-all active:bg-zinc-100"
        >
          <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-50 text-blue-500 group-hover:bg-blue-100 transition-colors shadow-sm border border-blue-100">
            <FileText size={20} />
          </div>
          <div className="min-w-0 flex-1 space-y-0.5">
            <p className="truncate text-[14px] font-bold text-zinc-900 group-hover:text-blue-600 transition-colors">{entry.name}</p>
            {entry.modifiedTime && (
              <p className="text-[11px] font-medium text-zinc-400 uppercase tracking-wider">
                Modified {new Date(entry.modifiedTime).toLocaleDateString("en-US", {
                  month: "short",
                  day: "numeric",
                })}
              </p>
            )}
          </div>
          <button
            onClick={() => onImport(entry)}
            className="btn h-8 px-4 text-[12px] shadow-sm border-zinc-200 hover:border-blue-200 group-hover:bg-white"
          >
            Sync Session
          </button>
        </div>
      ))}
    </div>
  );
}
