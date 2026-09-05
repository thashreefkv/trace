import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  CheckCircle2,
  ChevronRight,
  CloudCog,
  ExternalLink,
  Eye,
  File,
  FolderOpen,
  FolderPlus,
  Info,
  Link as LinkIcon,
  Loader2,
  MoveRight,
  Pencil,
  Plus,
  RefreshCw,
  Search,
  Trash2,
  Upload,
  X,
} from "lucide-react";
import {
  attachLocalDirectory,
  attachLocalFile,
  createTraceFolder,
  deleteFile,
  deleteTraceFolder,
  disconnectGoogleDrive,
  drivePullChanges,
  driveSyncStatus,
  driveStatus,
  getFolderLinks,
  isPreviewableDriveMime,
  listFolderChildren,
  listTraceFolders,
  moveFile,
  moveTraceFolder,
  openDrivePreview,
  openFile,
  renameFile,
  renameTraceFolder,
  searchFiles,
  unlinkFileFromEntity,
  unlinkFolderFromEntity,
  type DriveAccount,
  type DriveSyncStatus,
  type FileLinkRef,
  type FileRow,
  type FolderListing,
  type TraceFolder,
  FILE_ENTITY_KIND_LABELS,
} from "../lib/files";
import { FolderLinkPicker } from "../components/files/FolderLinkPicker";
import { humanSize, mimeIcon } from "../components/files/mimeIcon";
import { ConnectDriveCard } from "../components/files/ConnectDriveCard";
import { DriveTreePane } from "../components/files/DriveTreePane";
import { FileLinkPicker } from "../components/files/FileLinkPicker";
import { EmptyState } from "../components/EmptyState";
import { Dialog, DialogConfirm } from "../components/ui/Dialog";
import { safeExternalUrl } from "../lib/urlSafety";

// ── Name dialog ───────────────────────────────────────────────────────────────

interface NameDialogState {
  title: string;
  defaultValue: string;
  onConfirm: (name: string) => void;
}

function NameDialog({ state, onClose }: { state: NameDialogState; onClose: () => void }) {
  const [value, setValue] = useState(state.defaultValue);
  const inputRef = useRef<HTMLInputElement>(null);

  function submit() {
    const trimmed = value.trim();
    if (!trimmed) return;
    state.onConfirm(trimmed);
    onClose();
  }

  return (
    <Dialog onOpenChange={(o) => { if (!o) onClose(); }} open title={state.title} size="sm">
      <Dialog.Body>
        <input
          ref={inputRef}
          autoFocus
          className="field-control w-full"
          onChange={(e) => setValue(e.currentTarget.value)}
          onKeyDown={(e) => { if (e.key === "Enter") submit(); }}
          value={value}
        />
      </Dialog.Body>
      <Dialog.Footer>
        <Dialog.Cancel onClick={onClose}>Cancel</Dialog.Cancel>
        <Dialog.Action disabled={!value.trim()} onClick={submit} variant="primary">Confirm</Dialog.Action>
      </Dialog.Footer>
    </Dialog>
  );
}

// ── Confirm dialog ────────────────────────────────────────────────────────────

interface ConfirmDialogState {
  message: string;
  onConfirm: () => void;
}

function ConfirmDialog({ state, onClose }: { state: ConfirmDialogState; onClose: () => void }) {
  return (
    <DialogConfirm
      confirmLabel="Delete"
      description={state.message}
      destructive
      onConfirm={() => { state.onConfirm(); onClose(); }}
      onOpenChange={(o) => { if (!o) onClose(); }}
      open
      title="Are you sure?"
    />
  );
}

// ── Context menu ──────────────────────────────────────────────────────────────

interface ContextMenuItem {
  label: string;
  icon: React.ReactNode;
  danger?: boolean;
  onClick: () => void;
}

function ContextMenu({
  x, y, items, onClose,
}: { x: number; y: number; items: ContextMenuItem[]; onClose: () => void }) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  return (
    <div
      ref={ref}
      className="fixed z-50 min-w-[168px] overflow-hidden rounded-xl border border-zinc-100 bg-white py-1 shadow-[0_2px_12px_rgba(0,0,0,0.06)]"
      style={{ left: x, top: y }}
    >
      {items.map((item, i) => (
        <button
          className={[
            "flex w-full items-center gap-2.5 px-3 py-2 text-left text-[13px] transition-colors hover:bg-zinc-50",
            item.danger ? "text-rose-600" : "text-zinc-700",
          ].join(" ")}
          key={i}
          onClick={() => { item.onClick(); onClose(); }}
          type="button"
        >
          <span className={item.danger ? "text-rose-400" : "text-zinc-400"}>{item.icon}</span>
          {item.label}
        </button>
      ))}
    </div>
  );
}

// ── Folder picker modal ───────────────────────────────────────────────────────

function FolderPickerModal({
  title, excludeId, onSelect, onClose,
}: { title: string; excludeId?: string; onSelect: (folderId: string | null) => void; onClose: () => void }) {
  const [folders, setFolders] = useState<TraceFolder[]>([]);
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    listTraceFolders(null).then(setFolders).catch(() => {});
  }, []);

  function renderFolders(list: TraceFolder[], depth = 0): React.ReactNode {
    return list
      .filter((f) => !excludeId || f.id !== excludeId)
      .map((f) => (
        <button
          className={[
            "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm",
            selected === f.id ? "bg-zinc-900 text-white" : "text-zinc-700 hover:bg-zinc-100",
          ].join(" ")}
          key={f.id}
          onClick={() => setSelected(f.id)}
          style={{ paddingLeft: `${8 + depth * 16}px` }}
          type="button"
        >
          <FolderOpen className={selected === f.id ? "text-amber-300" : "text-amber-500"} size={13} />
          {f.name}
        </button>
      ));
  }

  return (
    <Dialog onOpenChange={(o) => { if (!o) onClose(); }} open title={title} size="sm">
      <Dialog.Body>
        <div className="max-h-60 overflow-y-auto rounded-xl border border-zinc-100 bg-zinc-50 p-1">
          <button
            className={[
              "flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-sm",
              selected === null ? "bg-zinc-900 text-white" : "text-zinc-700 hover:bg-zinc-100",
            ].join(" ")}
            onClick={() => setSelected(null)}
            type="button"
          >
            <FolderOpen className={selected === null ? "text-amber-300" : "text-amber-500"} size={13} />
            Root
          </button>
          {renderFolders(folders)}
        </div>
      </Dialog.Body>
      <Dialog.Footer>
        <Dialog.Cancel onClick={onClose}>Cancel</Dialog.Cancel>
        <Dialog.Action onClick={() => { onSelect(selected); onClose(); }} variant="primary">Move here</Dialog.Action>
      </Dialog.Footer>
    </Dialog>
  );
}

// ── Drive sync pill ───────────────────────────────────────────────────────────

function DriveSyncPill({ account, onSynced }: { account: DriveAccount; onSynced: () => void }) {
  const [status, setStatus] = useState<DriveSyncStatus | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try { setStatus(await driveSyncStatus(account.id)); } catch { /* ignore */ }
  }, [account.id]);

  useEffect(() => { void load(); }, [load]);

  async function handleSync() {
    try {
      setSyncing(true);
      setError(null);
      await drivePullChanges(account.id);
      await load();
      onSynced();
    } catch (e) {
      setError(String(e));
    } finally {
      setSyncing(false);
    }
  }

  const lastSyncLabel = status?.lastSyncAt
    ? new Date(status.lastSyncAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
    : status?.initialized ? "Never synced" : "Not initialized";

  return (
    <div className="flex items-center gap-1">
      {error ? (
        <span className="text-[11px] text-rose-500" title={error}>Sync error</span>
      ) : status?.lastSyncAt ? (
        <span className="flex items-center gap-1 text-[11px] text-zinc-500">
          <CheckCircle2 className="text-emerald-500" size={11} />
          {lastSyncLabel}
        </span>
      ) : (
        <span className="text-[11px] text-zinc-400">{lastSyncLabel}</span>
      )}
      <button
        aria-label="Sync Drive now"
        className="inline-flex h-6 w-6 items-center justify-center rounded-lg text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700"
        disabled={syncing}
        onClick={() => void handleSync()}
        title="Sync now"
        type="button"
      >
        <RefreshCw className={syncing ? "animate-spin" : ""} size={11} />
      </button>
    </div>
  );
}

// ── Main workspace ────────────────────────────────────────────────────────────

export function FilesWorkspace() {
  const [searchParams, setSearchParams] = useSearchParams();
  const folderParam = searchParams.get("folder");
  const fileParam = searchParams.get("file");
  const folderDetailParam = searchParams.get("folderDetail");
  const queryParam = searchParams.get("q") ?? "";
  const paneParam = (searchParams.get("pane") as "trace" | "drive" | null) ?? "trace";

  const [listing, setListing] = useState<FolderListing | null>(null);
  const [rootFolders, setRootFolders] = useState<TraceFolder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchHits, setSearchHits] = useState<FileRow[] | null>(null);
  const [searchBusy, setSearchBusy] = useState(false);
  const [linkPickerFor, setLinkPickerFor] = useState<FileRow | null>(null);
  const [folderLinkPickerFor, setFolderLinkPickerFor] = useState<TraceFolder | null>(null);
  const [folderDetailLinks, setFolderDetailLinks] = useState<FileLinkRef[] | null>(null);
  const [driveAccount, setDriveAccount] = useState<DriveAccount | null>(null);
  const [driveLoaded, setDriveLoaded] = useState(false);

  const [nameDialog, setNameDialog] = useState<NameDialogState | null>(null);
  const [confirmDialog, setConfirmDialog] = useState<ConfirmDialogState | null>(null);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(null);
  const [folderPicker, setFolderPicker] = useState<{ title: string; excludeId?: string; onSelect: (id: string | null) => void } | null>(null);

  const currentFolderId = folderParam || null;

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const [folders, content] = await Promise.all([
        listTraceFolders(null),
        listFolderChildren(currentFolderId),
      ]);
      setRootFolders(folders);
      setListing(content);
      if (fileParam) {
        const refreshed = content.files.find((f) => f.id === fileParam);
        if (refreshed && linkPickerFor && linkPickerFor.id === refreshed.id) {
          setLinkPickerFor(refreshed);
        }
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [currentFolderId, fileParam, linkPickerFor]);

  useEffect(() => { void refresh(); }, [currentFolderId]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    void (async () => {
      try {
        const status = await driveStatus();
        setDriveAccount(status.accounts[0] ?? null);
      } catch { /* ignore */ } finally {
        setDriveLoaded(true);
      }
    })();
  }, []);

  useEffect(() => {
    if (!folderDetailParam) { setFolderDetailLinks(null); return; }
    let cancelled = false;
    getFolderLinks(folderDetailParam)
      .then((links) => { if (!cancelled) setFolderDetailLinks(links); })
      .catch(() => { if (!cancelled) setFolderDetailLinks([]); });
    return () => { cancelled = true; };
  }, [folderDetailParam]);

  useEffect(() => {
    const q = queryParam.trim();
    if (!q) { setSearchHits(null); return; }
    let cancelled = false;
    setSearchBusy(true);
    (async () => {
      try {
        const hits = await searchFiles(q, 80);
        if (!cancelled) setSearchHits(hits);
      } catch (e) {
        if (!cancelled) setError(String(e));
      } finally {
        if (!cancelled) setSearchBusy(false);
      }
    })();
    return () => { cancelled = true; };
  }, [queryParam]);

  function setParam(name: string, value: string | null) {
    const next = new URLSearchParams(searchParams);
    if (value == null || value === "") next.delete(name); else next.set(name, value);
    setSearchParams(next, { replace: true });
  }

  function selectFolderDetail(folderId: string | null) {
    const next = new URLSearchParams(searchParams);
    if (folderId) { next.set("folderDetail", folderId); next.delete("file"); }
    else next.delete("folderDetail");
    setSearchParams(next, { replace: true });
  }

  function selectFile(fileId: string | null) {
    const next = new URLSearchParams(searchParams);
    if (fileId) { next.set("file", fileId); next.delete("folderDetail"); }
    else next.delete("file");
    setSearchParams(next, { replace: true });
  }

  function promptName(title: string, defaultValue: string, onConfirm: (v: string) => void) {
    setNameDialog({ title, defaultValue, onConfirm });
  }

  function promptConfirm(message: string, onConfirm: () => void) {
    setConfirmDialog({ message, onConfirm });
  }

  function handleNewFolder() {
    promptName("New folder name", "", async (name) => {
      try { await createTraceFolder(currentFolderId, name); await refresh(); }
      catch (e) { setError(String(e)); }
    });
  }

  async function handleAttachLocal() {
    const picked = await openDialog({ multiple: false, directory: false });
    if (!picked || typeof picked !== "string") return;
    try { await attachLocalFile(picked, currentFolderId); await refresh(); }
    catch (e) { setError(String(e)); }
  }

  async function handleAttachFolder() {
    const picked = await openDialog({ multiple: false, directory: true });
    if (!picked || typeof picked !== "string") return;
    try { await attachLocalDirectory(picked, currentFolderId); await refresh(); }
    catch (e) { setError(String(e)); }
  }

  function handleRenameFolder(folder: TraceFolder) {
    promptName("Rename folder", folder.name, async (name) => {
      try { await renameTraceFolder(folder.id, name); await refresh(); }
      catch (e) { setError(String(e)); }
    });
  }

  function handleDeleteFolder(folder: TraceFolder) {
    promptConfirm(`Delete folder "${folder.name}"? It must be empty.`, async () => {
      try { await deleteTraceFolder(folder.id, false); await refresh(); }
      catch (e) { setError(String(e)); }
    });
  }

  function handleRenameFile(file: FileRow) {
    promptName("Rename file", file.name, async (name) => {
      try { await renameFile(file.id, name); await refresh(); }
      catch (e) { setError(String(e)); }
    });
  }

  function handleDeleteFile(file: FileRow) {
    promptConfirm(
      `Remove "${file.name}" from Trace? (The original file is not deleted.)`,
      async () => {
        try {
          await deleteFile(file.id);
          if (fileParam === file.id) setParam("file", null);
          await refresh();
        } catch (e) { setError(String(e)); }
      },
    );
  }

  async function handleOpen(file: FileRow) {
    try { await openFile(file.id); } catch (e) { setError(String(e)); }
  }

  async function handleUnlink(file: FileRow, idx: number) {
    const link = file.links[idx];
    if (!link) return;
    try { await unlinkFileFromEntity(file.id, link.entityKind, link.entityId); await refresh(); }
    catch (e) { setError(String(e)); }
  }

  async function handleMoveToRoot(file: FileRow) {
    try { await moveFile(file.id, null); await refresh(); }
    catch (e) { setError(String(e)); }
  }

  function openFolderContextMenu(e: React.MouseEvent, folder: TraceFolder) {
    e.preventDefault();
    setContextMenu({
      x: e.clientX, y: e.clientY,
      items: [
        { label: "Details & links", icon: <Info size={13} />, onClick: () => selectFolderDetail(folder.id) },
        { label: "Rename", icon: <Pencil size={13} />, onClick: () => handleRenameFolder(folder) },
        {
          label: "Move to…", icon: <MoveRight size={13} />,
          onClick: () => setFolderPicker({
            title: `Move "${folder.name}" into…`, excludeId: folder.id,
            onSelect: async (newParentId) => {
              try { await moveTraceFolder(folder.id, newParentId); await refresh(); }
              catch (ex) { setError(String(ex)); }
            },
          }),
        },
        { label: "Delete", icon: <Trash2 size={13} />, danger: true, onClick: () => handleDeleteFolder(folder) },
      ],
    });
  }

  function openFileContextMenu(e: React.MouseEvent, file: FileRow) {
    e.preventDefault();
    setContextMenu({
      x: e.clientX, y: e.clientY,
      items: [
        { label: "Open", icon: <ExternalLink size={13} />, onClick: () => void handleOpen(file) },
        { label: "Link to…", icon: <LinkIcon size={13} />, onClick: () => setLinkPickerFor(file) },
        { label: "Rename", icon: <Pencil size={13} />, onClick: () => handleRenameFile(file) },
        {
          label: "Move to folder…", icon: <MoveRight size={13} />,
          onClick: () => setFolderPicker({
            title: `Move "${file.name}" into…`,
            onSelect: async (newFolderId) => {
              try { await moveFile(file.id, newFolderId); await refresh(); }
              catch (ex) { setError(String(ex)); }
            },
          }),
        },
        ...(file.traceFolderId ? [{ label: "Move to root", icon: <MoveRight size={13} />, onClick: () => void handleMoveToRoot(file) }] : []),
        { label: "Remove from Trace", icon: <Trash2 size={13} />, danger: true, onClick: () => handleDeleteFile(file) },
      ],
    });
  }

  const selectedFile = useMemo(() => {
    if (!fileParam) return null;
    if (searchHits) return searchHits.find((f) => f.id === fileParam) ?? null;
    return listing?.files.find((f) => f.id === fileParam) ?? null;
  }, [fileParam, searchHits, listing]);

  const selectedFolderForDetail = useMemo(() => {
    if (!folderDetailParam) return null;
    return listing?.folders.find((f) => f.id === folderDetailParam) ?? null;
  }, [folderDetailParam, listing]);

  return (
    <>
      <div className="grid h-full min-h-0 grid-cols-[220px_minmax(0,1fr)_300px]">

        {/* ── Left: folder tree ── */}
        <aside className="flex min-h-0 flex-col border-r border-zinc-100 bg-zinc-50">
          {/* Trace / Drive toggle */}
          <div className="shrink-0 px-3 pt-3 pb-2">
            <div className="flex gap-0.5 rounded-lg border border-zinc-200 bg-white p-0.5">
              <button
                className={`flex-1 rounded-md py-1.5 text-xs font-semibold transition-colors ${paneParam === "trace" ? "bg-zinc-900 text-white" : "text-zinc-500 hover:bg-zinc-100"}`}
                onClick={() => setParam("pane", null)}
                type="button"
              >
                Trace
              </button>
              <button
                className={`flex-1 rounded-md py-1.5 text-xs font-semibold transition-colors ${paneParam === "drive" ? "bg-zinc-900 text-white" : "text-zinc-500 hover:bg-zinc-100"}`}
                onClick={() => setParam("pane", "drive")}
                type="button"
              >
                Drive
              </button>
            </div>
          </div>

          {/* Scrollable tree */}
          <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
            {paneParam === "trace" ? (
              <>
                <div className="mb-1.5 flex items-center justify-between px-2 pt-1">
                  <span className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">Folders</span>
                  <button
                    className="rounded-md p-1 text-zinc-400 hover:bg-zinc-200 hover:text-zinc-700 transition-colors"
                    onClick={handleNewFolder}
                    title="New folder"
                    type="button"
                  >
                    <FolderPlus size={12} />
                  </button>
                </div>
                <button
                  className={`mb-0.5 flex w-full items-center gap-2 rounded-lg px-2.5 py-2 text-left text-[13px] transition-colors ${!currentFolderId ? "bg-zinc-900 text-white" : "text-zinc-600 hover:bg-zinc-200/70"}`}
                  onClick={() => setParam("folder", null)}
                  type="button"
                >
                  <FolderOpen className={!currentFolderId ? "text-amber-300" : "text-amber-500"} size={13} />
                  All files
                </button>
                <TraceFolderTreeNode
                  activeId={currentFolderId}
                  folders={rootFolders}
                  onContextMenu={openFolderContextMenu}
                  onSelect={(id) => setParam("folder", id)}
                />
              </>
            ) : !driveLoaded ? (
              <div className="flex items-center gap-2 px-2 py-3 text-[12px] text-zinc-500">
                <Loader2 className="animate-spin" size={13} />
                Loading Drive…
              </div>
            ) : !driveAccount ? (
              <ConnectDriveCard onConnected={(acc) => setDriveAccount(acc)} />
            ) : (
              <DriveTreePane
                account={driveAccount}
                onImported={() => void refresh()}
                targetTraceFolderId={currentFolderId}
              />
            )}
          </div>
        </aside>

        {/* ── Centre: folder contents ── */}
        <main className="flex min-h-0 min-w-0 flex-col bg-white">
          <header className="shrink-0 border-b border-zinc-100 px-5 py-4">
            {/* Title row */}
            <div className="flex items-center justify-between gap-4">
              <div className="min-w-0">
                <p className="page-kicker">Workspace</p>
                <h1 className="truncate text-lg font-semibold text-zinc-950">
                  {listing?.folder?.name ?? "All files"}
                </h1>
                {listing && listing.breadcrumbs.length > 1 && (
                  <p className="mt-0.5 truncate text-[11px] text-zinc-400">
                    {listing.breadcrumbs.map((b) => b.name).join(" / ")}
                  </p>
                )}
              </div>

              {/* Search */}
              <label className="flex shrink-0 items-center gap-2 rounded-xl border border-zinc-200 bg-zinc-50 px-3 py-1.5 focus-within:border-zinc-400 transition-colors">
                <Search className="text-zinc-400" size={13} />
                <input
                  className="w-36 bg-transparent text-sm text-zinc-900 outline-none placeholder:text-zinc-400"
                  onChange={(e) => setParam("q", e.currentTarget.value)}
                  placeholder="Search files…"
                  value={queryParam}
                />
                {searchBusy && <Loader2 className="animate-spin text-zinc-400" size={12} />}
              </label>
            </div>

            {/* Action row */}
            <div className="mt-3 flex items-center justify-between gap-3">
              {/* Drive status */}
              {driveAccount ? (
                <div className="flex items-center gap-2 rounded-xl border border-zinc-100 bg-zinc-50 px-2.5 py-1.5">
                  <CloudCog className="shrink-0 text-sky-500" size={12} />
                  <span className="max-w-[120px] truncate text-[11px] font-medium text-zinc-600">
                    {driveAccount.email}
                  </span>
                  <DriveSyncPill account={driveAccount} onSynced={() => void refresh()} />
                  <button
                    className="ml-1 flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium text-rose-500 hover:bg-rose-50 transition-colors"
                    onClick={() =>
                      promptConfirm(
                        `Disconnect ${driveAccount.email} from Trace? Drive files imported into Trace will be removed.`,
                        async () => {
                          try { await disconnectGoogleDrive(driveAccount.id); setDriveAccount(null); }
                          catch (e) { setError(String(e)); }
                        },
                      )
                    }
                    type="button"
                  >
                    <X size={9} />
                    Disconnect
                  </button>
                </div>
              ) : driveLoaded ? (
                <button className="btn" onClick={() => setParam("pane", "drive")} type="button">
                  <CloudCog size={13} />
                  Connect Drive
                </button>
              ) : <div />}

              {/* Action buttons */}
              <div className="flex items-center gap-1.5">
                <button className="btn" onClick={handleNewFolder} type="button">
                  <FolderPlus size={13} />
                  New folder
                </button>
                <button className="btn" onClick={() => void handleAttachFolder()} type="button">
                  <FolderOpen size={13} />
                  Attach folder
                </button>
                <button className="btn btn-primary" onClick={() => void handleAttachLocal()} type="button">
                  <Upload size={13} />
                  Attach file
                </button>
              </div>
            </div>
          </header>

          {error && <div className="notice notice-error mx-4 mt-3 text-sm">{error}</div>}

          <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">
            {loading && !listing ? (
              <div className="flex items-center gap-2 text-sm text-zinc-400">
                <Loader2 className="animate-spin" size={14} />
                Loading…
              </div>
            ) : searchHits ? (
              <FileListView
                files={searchHits}
                onContextMenu={openFileContextMenu}
                onDelete={handleDeleteFile}
                onLink={(file) => setLinkPickerFor(file)}
                onOpen={handleOpen}
                onRename={handleRenameFile}
                onSelect={(id) => selectFile(id)}
                selectedFileId={fileParam}
                showFolderHint
              />
            ) : (
              <>
                {listing && listing.folders.length > 0 && (
                  <section className="mb-5">
                    <h2 className="mb-2.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
                      Folders
                    </h2>
                    <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
                      {listing.folders.map((folder) => (
                        <li
                          className={[
                            "group overflow-hidden rounded-xl border bg-white transition-all duration-150",
                            folderDetailParam === folder.id
                              ? "border-zinc-900 shadow-sm"
                              : "border-zinc-100 shadow-[0_1px_4px_rgba(0,0,0,0.05)] hover:shadow-[0_3px_12px_rgba(0,0,0,0.08)] hover:-translate-y-px",
                          ].join(" ")}
                          key={folder.id}
                          onContextMenu={(e) => openFolderContextMenu(e, folder)}
                        >
                          <button
                            className="flex w-full items-center gap-3 p-3 text-left"
                            onClick={() => setParam("folder", folder.id)}
                            type="button"
                          >
                            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-amber-50">
                              <FolderOpen className="text-amber-500" size={17} />
                            </div>
                            <div className="min-w-0 flex-1">
                              <p className="truncate text-sm font-semibold text-zinc-900">{folder.name}</p>
                              <p className="text-[11px] text-zinc-400">
                                {new Date(folder.created_at).toLocaleDateString([], { month: "short", day: "numeric", year: "numeric" })}
                              </p>
                            </div>
                          </button>
                          <div className="flex items-center gap-0.5 border-t border-zinc-50 px-2 py-1 opacity-0 transition-opacity group-hover:opacity-100">
                            <IconBtn title="Details" onClick={() => selectFolderDetail(folder.id)}>
                              <Info size={11} />
                            </IconBtn>
                            <IconBtn title="Rename" onClick={() => handleRenameFolder(folder)}>
                              <Pencil size={11} />
                            </IconBtn>
                            <IconBtn
                              title="Move to…"
                              onClick={() => setFolderPicker({
                                title: `Move "${folder.name}" into…`, excludeId: folder.id,
                                onSelect: async (newParentId) => {
                                  try { await moveTraceFolder(folder.id, newParentId); await refresh(); }
                                  catch (ex) { setError(String(ex)); }
                                },
                              })}
                            >
                              <MoveRight size={11} />
                            </IconBtn>
                            <IconBtn danger title="Delete" onClick={() => handleDeleteFolder(folder)}>
                              <Trash2 size={11} />
                            </IconBtn>
                          </div>
                        </li>
                      ))}
                    </ul>
                  </section>
                )}

                <section>
                  <h2 className="mb-2.5 text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
                    Files
                  </h2>
                  <FileListView
                    files={listing?.files ?? []}
                    onContextMenu={openFileContextMenu}
                    onDelete={handleDeleteFile}
                    onLink={(file) => setLinkPickerFor(file)}
                    onOpen={handleOpen}
                    onRename={handleRenameFile}
                    onSelect={(id) => selectFile(id)}
                    selectedFileId={fileParam}
                  />
                </section>
              </>
            )}
          </div>
        </main>

        {/* ── Right: detail panel ── */}
        <aside className="flex min-h-0 flex-col border-l border-zinc-100 bg-white">
          {selectedFolderForDetail && folderDetailLinks !== null ? (
            <FolderDetail
              folder={selectedFolderForDetail}
              links={folderDetailLinks}
              onClose={() => selectFolderDetail(null)}
              onLink={() => setFolderLinkPickerFor(selectedFolderForDetail)}
              onNavigate={() => { selectFolderDetail(null); setParam("folder", selectedFolderForDetail.id); }}
              onUnlink={async (idx) => {
                const link = folderDetailLinks[idx];
                if (!link) return;
                try {
                  await unlinkFolderFromEntity(
                    selectedFolderForDetail.id,
                    link.entityKind as Parameters<typeof unlinkFolderFromEntity>[1],
                    link.entityId,
                  );
                  setFolderDetailLinks(await getFolderLinks(selectedFolderForDetail.id));
                } catch (e) { setError(String(e)); }
              }}
            />
          ) : selectedFile ? (
            <FileDetail
              file={selectedFile}
              onClose={() => selectFile(null)}
              onDelete={handleDeleteFile}
              onLink={() => setLinkPickerFor(selectedFile)}
              onMoveToFolder={(file) =>
                setFolderPicker({
                  title: `Move "${file.name}" into…`,
                  onSelect: async (newFolderId) => {
                    try { await moveFile(file.id, newFolderId); await refresh(); }
                    catch (ex) { setError(String(ex)); }
                  },
                })
              }
              onMoveToRoot={handleMoveToRoot}
              onOpen={handleOpen}
              onRename={handleRenameFile}
              onUnlink={handleUnlink}
            />
          ) : (
            <div className="flex h-full items-center justify-center p-6">
              <EmptyState
                variant="inline"
                icon={File}
                title="Nothing selected"
                description="Click a file or folder to see details."
              />
            </div>
          )}
        </aside>
      </div>

      {/* Modals */}
      {nameDialog && <NameDialog onClose={() => setNameDialog(null)} state={nameDialog} />}
      {confirmDialog && <ConfirmDialog onClose={() => setConfirmDialog(null)} state={confirmDialog} />}
      {linkPickerFor && (
        <FileLinkPicker
          file={linkPickerFor}
          onClose={() => setLinkPickerFor(null)}
          onLinked={() => void refresh()}
        />
      )}
      {folderLinkPickerFor && folderDetailLinks !== null && (
        <FolderLinkPicker
          existingLinks={folderDetailLinks}
          folderId={folderLinkPickerFor.id}
          folderName={folderLinkPickerFor.name}
          onClose={() => setFolderLinkPickerFor(null)}
          onLinked={async () => {
            setFolderLinkPickerFor(null);
            setFolderDetailLinks(await getFolderLinks(folderLinkPickerFor.id));
          }}
        />
      )}
      {contextMenu && (
        <ContextMenu
          items={contextMenu.items}
          onClose={() => setContextMenu(null)}
          x={contextMenu.x}
          y={contextMenu.y}
        />
      )}
      {folderPicker && (
        <FolderPickerModal
          excludeId={folderPicker.excludeId}
          onClose={() => setFolderPicker(null)}
          onSelect={folderPicker.onSelect}
          title={folderPicker.title}
        />
      )}
    </>
  );
}

// ── Small icon button helper ──────────────────────────────────────────────────

function IconBtn({
  children, title, onClick, danger,
}: { children: React.ReactNode; title: string; onClick: () => void; danger?: boolean }) {
  return (
    <button
      className={`inline-flex h-6 w-6 items-center justify-center rounded-md transition-colors ${danger ? "text-zinc-300 hover:bg-rose-50 hover:text-rose-500" : "text-zinc-300 hover:bg-zinc-100 hover:text-zinc-600"}`}
      onClick={onClick}
      title={title}
      type="button"
    >
      {children}
    </button>
  );
}

// ── Folder tree ───────────────────────────────────────────────────────────────

interface TreeNodeProps {
  activeId: string | null;
  folders: TraceFolder[];
  onSelect: (id: string) => void;
  onContextMenu?: (e: React.MouseEvent, folder: TraceFolder) => void;
}

function TraceFolderTreeNode({ activeId, folders, onSelect, onContextMenu }: TreeNodeProps) {
  return (
    <ul className="space-y-0.5">
      {folders.map((folder) => (
        <TraceFolderRow
          activeId={activeId}
          folder={folder}
          key={folder.id}
          onContextMenu={onContextMenu}
          onSelect={onSelect}
        />
      ))}
    </ul>
  );
}

function TraceFolderRow({
  activeId, folder, onSelect, onContextMenu,
}: { activeId: string | null; folder: TraceFolder; onSelect: (id: string) => void; onContextMenu?: (e: React.MouseEvent, folder: TraceFolder) => void }) {
  const [open, setOpen] = useState(true);
  const [children, setChildren] = useState<TraceFolder[] | null>(null);
  const isActive = activeId === folder.id;

  useEffect(() => {
    if (!open || children) return;
    void (async () => {
      try { setChildren(await listTraceFolders(folder.id)); }
      catch { setChildren([]); }
    })();
  }, [open, children, folder.id]);

  return (
    <li>
      <div
        className={`group flex items-center gap-1 rounded-lg px-1 py-1.5 text-[13px] transition-colors ${isActive ? "bg-zinc-900 text-white" : "text-zinc-600 hover:bg-zinc-200/60"}`}
        onContextMenu={onContextMenu ? (e) => onContextMenu(e, folder) : undefined}
      >
        <button
          aria-label={open ? "Collapse" : "Expand"}
          className={`inline-flex h-5 w-5 shrink-0 items-center justify-center rounded transition-transform ${open ? "rotate-90" : ""} ${isActive ? "text-zinc-300" : "text-zinc-400"}`}
          onClick={() => setOpen((v) => !v)}
          type="button"
        >
          <ChevronRight size={11} />
        </button>
        <button
          className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
          onClick={() => onSelect(folder.id)}
          type="button"
        >
          <FolderOpen className={isActive ? "text-amber-300" : "text-amber-500"} size={12} />
          <span className="truncate">{folder.name}</span>
        </button>
      </div>
      {open && children && children.length > 0 && (
        <div className="pl-4">
          <TraceFolderTreeNode
            activeId={activeId}
            folders={children}
            onContextMenu={onContextMenu}
            onSelect={onSelect}
          />
        </div>
      )}
    </li>
  );
}

// ── File list ─────────────────────────────────────────────────────────────────

interface FileListProps {
  files: FileRow[];
  selectedFileId: string | null;
  onSelect: (id: string) => void;
  onOpen: (file: FileRow) => void;
  onLink: (file: FileRow) => void;
  onRename: (file: FileRow) => void;
  onDelete: (file: FileRow) => void;
  onContextMenu?: (e: React.MouseEvent, file: FileRow) => void;
  showFolderHint?: boolean;
}

function FileListView({
  files, selectedFileId, onSelect, onOpen, onLink, onRename, onDelete, onContextMenu, showFolderHint,
}: FileListProps) {
  if (files.length === 0) {
    return (
      <div className="flex min-h-[160px] items-center justify-center rounded-xl border border-dashed border-zinc-200">
        <div className="text-center">
          <File className="mx-auto mb-2 text-zinc-200" size={22} />
          <p className="text-xs text-zinc-400">No files here yet.</p>
          <p className="mt-0.5 text-[11px] text-zinc-300">Use "Attach file" or "Attach folder" above.</p>
        </div>
      </div>
    );
  }
  return (
    <ul className="space-y-1.5">
      {files.map((file) => {
        const active = selectedFileId === file.id;
        return (
          <li
            className={[
              "group flex items-center gap-3 rounded-xl border bg-white px-3 py-2.5 transition-all duration-150",
              active
                ? "border-zinc-900 shadow-sm"
                : "border-zinc-100 shadow-[0_1px_4px_rgba(0,0,0,0.04)] hover:shadow-[0_2px_10px_rgba(0,0,0,0.07)] hover:-translate-y-px",
              file.driveTrashed ? "opacity-50" : "",
            ].join(" ")}
            key={file.id}
            onContextMenu={onContextMenu ? (e) => onContextMenu(e, file) : undefined}
          >
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-zinc-50">
              {mimeIcon(file.mimeType ?? file.driveMime)}
            </div>
            <button className="min-w-0 flex-1 text-left" onClick={() => onSelect(file.id)} type="button">
              <span className="block truncate text-sm font-semibold text-zinc-900">{file.name}</span>
              <div className="mt-0.5 flex flex-wrap items-center gap-1.5">
                <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                  {file.kind === "drive" ? "Drive" : "Local"}
                </span>
                {file.sizeBytes ? <span className="text-[11px] text-zinc-400">{humanSize(file.sizeBytes)}</span> : null}
                {file.driveTrashed ? <span className="text-[10px] text-rose-400">Trashed in Drive</span> : null}
                {file.isMissing ? <span className="text-[10px] text-amber-500">Missing on disk</span> : null}
                {showFolderHint && file.traceFolderId ? <span className="text-[11px] text-zinc-400">In folder</span> : null}
                {file.links.length > 0 ? (
                  <span className="text-[11px] text-zinc-400">{file.links.length} link{file.links.length !== 1 ? "s" : ""}</span>
                ) : null}
              </div>
            </button>
            <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
              <IconBtn title="Open" onClick={() => onOpen(file)}>
                <ExternalLink size={12} />
              </IconBtn>
              {isPreviewableDriveMime(file.driveMime) && file.driveFileId ? (
                <IconBtn
                  title="Preview in Trace"
                  onClick={() => void openDrivePreview(file.id, file.driveFileId!, file.driveMime!, file.name)}
                >
                  <Eye size={12} />
                </IconBtn>
              ) : null}
              <IconBtn title="Link" onClick={() => onLink(file)}>
                <LinkIcon size={12} />
              </IconBtn>
              <IconBtn title="Rename" onClick={() => onRename(file)}>
                <Pencil size={12} />
              </IconBtn>
              <IconBtn danger title="Remove" onClick={() => onDelete(file)}>
                <Trash2 size={12} />
              </IconBtn>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

// ── File detail ───────────────────────────────────────────────────────────────

function FileDetail({
  file, onClose, onOpen, onLink, onRename, onDelete, onUnlink, onMoveToRoot, onMoveToFolder,
}: {
  file: FileRow; onClose: () => void; onOpen: (file: FileRow) => void; onLink: () => void;
  onRename: (file: FileRow) => void; onDelete: (file: FileRow) => void;
  onUnlink: (file: FileRow, idx: number) => void; onMoveToRoot: (file: FileRow) => void;
  onMoveToFolder: (file: FileRow) => void;
}) {
  const safeDriveUrl = safeExternalUrl(file.driveWebViewLink);
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      {/* Header */}
      <div className="flex items-start gap-3 border-b border-zinc-100 p-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-zinc-100">
          {mimeIcon(file.mimeType ?? file.driveMime)}
        </div>
        <div className="min-w-0 flex-1">
          <p className="page-kicker">{file.kind === "drive" ? "Drive file" : "Local file"}</p>
          <h2 className="break-words text-sm font-semibold text-zinc-950 leading-tight">{file.name}</h2>
        </div>
        <button
          className="shrink-0 rounded-lg p-1.5 text-zinc-300 hover:bg-zinc-100 hover:text-zinc-600 transition-colors"
          onClick={onClose}
          type="button"
        >
          <X size={13} />
        </button>
      </div>

      <div className="p-4 space-y-4">
        {/* Primary action */}
        <button className="btn btn-primary w-full justify-center" onClick={() => onOpen(file)} type="button">
          <ExternalLink size={13} />
          Open file
        </button>
        {isPreviewableDriveMime(file.driveMime) && file.driveFileId ? (
          <button
            className="btn w-full justify-center"
            onClick={() => void openDrivePreview(file.id, file.driveFileId!, file.driveMime!, file.name)}
            type="button"
          >
            <Eye size={13} />
            Preview in Trace
          </button>
        ) : null}

        {/* Warnings */}
        {file.driveTrashed && <div className="notice notice-error text-xs">Trashed in Google Drive.</div>}
        {file.isMissing && <div className="notice text-xs">File not found on disk. It may have been moved or deleted.</div>}

        {/* Metadata */}
        <div className="space-y-2 rounded-xl border border-zinc-100 bg-zinc-50 p-3">
          {file.mimeType && <DetailRow label="Type"><span className="font-mono text-[11px]">{file.mimeType}</span></DetailRow>}
          {file.sizeBytes ? <DetailRow label="Size">{humanSize(file.sizeBytes)}</DetailRow> : null}
          {file.localPath && <DetailRow label="Path"><span className="break-all font-mono text-[10px] text-zinc-500">{file.localPath}</span></DetailRow>}
          {safeDriveUrl && (
            <DetailRow label="Drive">
              <a className="text-sky-600 hover:underline text-xs" href={safeDriveUrl} rel="noopener noreferrer" target="_blank">
                Open in browser
              </a>
            </DetailRow>
          )}
        </div>

        {/* Entity links */}
        <section>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">Linked to</span>
            <button className="btn h-7 px-2 text-xs" onClick={onLink} type="button">
              <Plus size={11} />
              Link
            </button>
          </div>
          {file.links.length === 0 ? (
            <div className="rounded-xl border border-dashed border-zinc-200 px-3 py-4 text-center text-[11px] text-zinc-400">
              Not linked to any deliverable or stakeholder.
            </div>
          ) : (
            <ul className="space-y-1">
              {file.links.map((link, idx) => (
                <li
                  className="flex items-center justify-between gap-2 rounded-lg border border-zinc-100 bg-zinc-50 px-2.5 py-2"
                  key={`${link.entityKind}:${link.entityId}`}
                >
                  <div className="min-w-0">
                    <span className="rounded-md bg-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-600">
                      {FILE_ENTITY_KIND_LABELS[link.entityKind]}
                    </span>
                    <span className="ml-2 font-mono text-[10px] text-zinc-400">{link.entityId.slice(0, 8)}…</span>
                  </div>
                  <button
                    className="shrink-0 rounded p-1 text-zinc-300 hover:bg-rose-50 hover:text-rose-500 transition-colors"
                    onClick={() => onUnlink(file, idx)}
                    type="button"
                  >
                    <X size={11} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        {/* File actions */}
        <section className="space-y-1 border-t border-zinc-100 pt-3">
          <button className="btn w-full justify-start" onClick={() => onRename(file)} type="button">
            <Pencil size={12} />
            Rename
          </button>
          <button className="btn w-full justify-start" onClick={() => onMoveToFolder(file)} type="button">
            <MoveRight size={12} />
            Move to folder…
          </button>
          {file.traceFolderId && (
            <button className="btn w-full justify-start" onClick={() => onMoveToRoot(file)} type="button">
              <FolderOpen size={12} />
              Move to root
            </button>
          )}
          <button
            className="btn w-full justify-start text-rose-500 hover:bg-rose-50"
            onClick={() => onDelete(file)}
            type="button"
          >
            <Trash2 size={12} />
            Remove from Trace
          </button>
        </section>
      </div>
    </div>
  );
}

function DetailRow({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex flex-col gap-0.5">
      <dt className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">{label}</dt>
      <dd className="text-xs text-zinc-700">{children}</dd>
    </div>
  );
}

// ── Folder detail ─────────────────────────────────────────────────────────────

function FolderDetail({
  folder, links, onClose, onNavigate, onLink, onUnlink,
}: { folder: TraceFolder; links: FileLinkRef[]; onClose: () => void; onNavigate: () => void; onLink: () => void; onUnlink: (idx: number) => void }) {
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      {/* Header */}
      <div className="flex items-start gap-3 border-b border-zinc-100 p-4">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-amber-50">
          <FolderOpen className="text-amber-500" size={18} />
        </div>
        <div className="min-w-0 flex-1">
          <p className="page-kicker">Folder</p>
          <h2 className="break-words text-sm font-semibold text-zinc-950 leading-tight">{folder.name}</h2>
        </div>
        <button
          className="shrink-0 rounded-lg p-1.5 text-zinc-300 hover:bg-zinc-100 hover:text-zinc-600 transition-colors"
          onClick={onClose}
          type="button"
        >
          <X size={13} />
        </button>
      </div>

      <div className="p-4 space-y-4">
        <button className="btn btn-primary w-full justify-center" onClick={onNavigate} type="button">
          <FolderOpen size={13} />
          Open folder
        </button>

        <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
          <DetailRow label="Created">
            {new Date(folder.created_at).toLocaleDateString([], { dateStyle: "medium" })}
          </DetailRow>
        </div>

        <section>
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[10px] font-semibold uppercase tracking-widest text-zinc-400">Linked to</span>
            <button className="btn h-7 px-2 text-xs" onClick={onLink} type="button">
              <Plus size={11} />
              Link
            </button>
          </div>
          {links.length === 0 ? (
            <div className="rounded-xl border border-dashed border-zinc-200 px-3 py-4 text-center text-[11px] text-zinc-400">
              Not linked to any entity yet.
            </div>
          ) : (
            <ul className="space-y-1">
              {links.map((link, idx) => (
                <li
                  className="flex items-center justify-between gap-2 rounded-lg border border-zinc-100 bg-zinc-50 px-2.5 py-2"
                  key={`${link.entityKind}:${link.entityId}`}
                >
                  <div className="min-w-0">
                    <span className="rounded-md bg-zinc-200 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-600">
                      {FILE_ENTITY_KIND_LABELS[link.entityKind as keyof typeof FILE_ENTITY_KIND_LABELS]}
                    </span>
                    <span className="ml-2 font-mono text-[10px] text-zinc-400">{link.entityId.slice(0, 8)}…</span>
                  </div>
                  <button
                    className="shrink-0 rounded p-1 text-zinc-300 hover:bg-rose-50 hover:text-rose-500 transition-colors"
                    onClick={() => onUnlink(idx)}
                    type="button"
                  >
                    <X size={11} />
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>
    </div>
  );
}
