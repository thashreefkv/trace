import { useCallback, useEffect, useState } from "react";
import {
  Check,
  ChevronRight,
  FolderOpen,
  Loader2,
  Plus,
  RefreshCw,
  X,
} from "lucide-react";
import {
  driveImport,
  driveImportFolder,
  driveListChildren,
  driveStatus,
  linkFileToEntity,
  linkFolderFilesToEntity,
  type DriveAccount,
  type DriveEntry,
  type FileEntityKind,
} from "../../lib/files";
import { ConnectDriveCard } from "./ConnectDriveCard";
import { mimeIcon } from "./mimeIcon";

interface DriveFilePickerModalProps {
  entityKind: FileEntityKind;
  entityId: string;
  traceFolderId: string | null;
  onClose: () => void;
  onAttached: () => void;
}

export function DriveFilePickerModal({
  entityKind,
  entityId,
  traceFolderId,
  onClose,
  onAttached,
}: DriveFilePickerModalProps) {
  const [account, setAccount] = useState<DriveAccount | null>(null);
  const [loadingStatus, setLoadingStatus] = useState(true);
  const [entries, setEntries] = useState<DriveEntry[] | null>(null);
  const [loadingRoot, setLoadingRoot] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // fileId → "importing" | "done"
  const [importState, setImportState] = useState<Record<string, "importing" | "done">>({});

  const loadRoot = useCallback(async (_acc: DriveAccount) => {
    try {
      setLoadingRoot(true);
      setError(null);
      const listing = await driveListChildren(null, null);
      setEntries(listing.entries);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingRoot(false);
    }
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const status = await driveStatus();
        const acc = status.accounts[0] ?? null;
        setAccount(acc);
        if (acc) await loadRoot(acc);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoadingStatus(false);
      }
    })();
  }, [loadRoot]);

  async function handleAttach(entry: DriveEntry) {
    if (!account || entry.isFolder) return;
    setImportState((s) => ({ ...s, [entry.id]: "importing" }));
    try {
      setError(null);
      const rows = await driveImport(account.id, [entry.id], traceFolderId);
      if (rows[0]) {
        await linkFileToEntity(rows[0].id, entityKind, entityId);
        onAttached();
      }
      setImportState((s) => ({ ...s, [entry.id]: "done" }));
    } catch (e) {
      setError(String(e));
      setImportState((s) => {
        const next = { ...s };
        delete next[entry.id];
        return next;
      });
    }
  }

  async function handleAttachFolder(entry: DriveEntry) {
    if (!account || !entry.isFolder) return;
    setImportState((s) => ({ ...s, [entry.id]: "importing" }));
    try {
      setError(null);
      const folder = await driveImportFolder(account.id, entry.id, entry.name, traceFolderId);
      await linkFolderFilesToEntity(folder.id, entityKind, entityId);
      onAttached();
      setImportState((s) => ({ ...s, [entry.id]: "done" }));
    } catch (e) {
      setError(String(e));
      setImportState((s) => {
        const next = { ...s };
        delete next[entry.id];
        return next;
      });
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-[1px]"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="flex h-[600px] w-[520px] max-w-[calc(100vw-2rem)] flex-col rounded-xl border border-zinc-200 bg-white shadow-2xl">
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-200 px-4 py-3">
          <div>
            <p className="text-[11px] font-bold uppercase tracking-widest text-zinc-500">
              Google Drive
            </p>
            <h2 className="text-[14px] font-semibold text-zinc-900">Attach Drive file</h2>
          </div>
          <button
            aria-label="Close"
            className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-100"
            onClick={onClose}
            type="button"
          >
            <X size={14} />
          </button>
        </div>

        {/* Body */}
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {loadingStatus ? (
            <div className="space-y-2 p-3">
              <div className="skeleton h-8" />
              <div className="skeleton h-8 w-2/3" />
            </div>
          ) : !account ? (
            <div className="p-3">
              <ConnectDriveCard
                onConnected={(acc) => {
                  setAccount(acc);
                  void loadRoot(acc);
                }}
              />
            </div>
          ) : (
            <div className="space-y-2">
              <div className="flex items-center justify-between gap-2 px-1">
                <p className="truncate text-[11px] text-zinc-500">{account.email}</p>
                <button
                  aria-label="Refresh"
                  className="inline-flex h-6 w-6 items-center justify-center rounded text-zinc-400 hover:bg-zinc-100"
                  disabled={loadingRoot}
                  onClick={() => void loadRoot(account)}
                  type="button"
                >
                  <RefreshCw className={loadingRoot ? "animate-spin" : ""} size={12} />
                </button>
              </div>

              {error ? (
                <div className="rounded-md bg-rose-50 px-3 py-2 text-[12px] text-rose-700">
                  {error}
                </div>
              ) : null}

              <div className="rounded-md border border-zinc-200 bg-white">
                {loadingRoot ? (
                  <div className="space-y-1.5 p-2">
                    {[...Array(4)].map((_, i) => (
                      <div key={i} className="skeleton h-8" />
                    ))}
                  </div>
                ) : entries == null || entries.length === 0 ? (
                  <div className="py-8 text-center text-[12px] text-zinc-500">
                    {entries == null ? "Failed to load." : "No files in Drive root."}
                  </div>
                ) : (
                  <ul className="divide-y divide-zinc-100">
                    {entries.map((entry) => (
                      <DrivePickerNode
                        entry={entry}
                        entityId={entityId}
                        entityKind={entityKind}
                        importState={importState}
                        key={entry.id}
                        onAttach={handleAttach}
                        onAttachFolder={handleAttachFolder}
                        onAttached={onAttached}
                        setError={setError}
                        setImportState={setImportState}
                        account={account}
                        traceFolderId={traceFolderId}
                      />
                    ))}
                  </ul>
                )}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end border-t border-zinc-200 px-4 py-3">
          <button className="btn" onClick={onClose} type="button">
            Done
          </button>
        </div>
      </div>
    </div>
  );
}

interface DrivePickerNodeProps {
  account: DriveAccount;
  entry: DriveEntry;
  entityKind: FileEntityKind;
  entityId: string;
  traceFolderId: string | null;
  importState: Record<string, "importing" | "done">;
  onAttach: (entry: DriveEntry) => Promise<void>;
  onAttachFolder: (entry: DriveEntry) => Promise<void>;
  onAttached: () => void;
  setError: (e: string | null) => void;
  setImportState: React.Dispatch<React.SetStateAction<Record<string, "importing" | "done">>>;
}

function DrivePickerNode({
  account,
  entry,
  entityKind,
  entityId,
  traceFolderId,
  importState,
  onAttach,
  onAttachFolder,
  onAttached,
  setError,
  setImportState,
}: DrivePickerNodeProps) {
  const [open, setOpen] = useState(false);
  const [children, setChildren] = useState<DriveEntry[] | null>(null);
  const [loadingChildren, setLoadingChildren] = useState(false);

  const state = importState[entry.id];

  async function toggleOpen() {
    const next = !open;
    setOpen(next);
    if (next && entry.isFolder && !children) {
      try {
        setLoadingChildren(true);
        const listing = await driveListChildren(entry.id, null);
        setChildren(listing.entries);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoadingChildren(false);
      }
    }
  }

  return (
    <li>
      <div className="group flex items-center gap-1.5 px-2 py-1.5 text-[12px] hover:bg-zinc-50">
        {entry.isFolder ? (
          <button
            aria-label={open ? "Collapse" : "Expand"}
            className={[
              "inline-flex h-5 w-5 shrink-0 items-center justify-center rounded text-zinc-400 transition-transform",
              open ? "rotate-90" : "",
            ].join(" ")}
            onClick={() => void toggleOpen()}
            type="button"
          >
            <ChevronRight size={11} />
          </button>
        ) : (
          <span className="inline-block h-5 w-5 shrink-0" />
        )}

        <span className="shrink-0">
          {entry.isFolder ? (
            <FolderOpen className="text-amber-500" size={13} />
          ) : (
            mimeIcon(entry.mimeType, 13)
          )}
        </span>

        <span className="min-w-0 flex-1 truncate text-zinc-800">{entry.name}</span>

        {state === "done" ? (
          <span className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded text-emerald-500">
            <Check size={13} />
          </span>
        ) : (
          <button
            aria-label={entry.isFolder ? "Attach folder to this item" : "Attach to this item"}
            className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded text-zinc-400 opacity-0 hover:bg-zinc-100 hover:text-zinc-700 group-hover:opacity-100"
            disabled={state === "importing"}
            onClick={() => entry.isFolder ? void onAttachFolder(entry) : void onAttach(entry)}
            title={entry.isFolder ? "Attach folder (imports all files)" : "Attach to this item"}
            type="button"
          >
            {state === "importing" ? (
              <Loader2 className="animate-spin" size={12} />
            ) : (
              <Plus size={12} />
            )}
          </button>
        )}
      </div>

      {open && entry.isFolder ? (
        <div className="ml-4 border-l border-zinc-100 pl-1">
          {loadingChildren ? (
            <div className="flex items-center gap-1.5 py-1.5 pl-2 text-[11px] text-zinc-500">
              <Loader2 className="animate-spin" size={11} />
              Loading…
            </div>
          ) : children && children.length > 0 ? (
            <ul className="divide-y divide-zinc-50">
              {children.map((child) => (
                <DrivePickerNode
                  account={account}
                  entry={child}
                  entityId={entityId}
                  entityKind={entityKind}
                  traceFolderId={traceFolderId}
                  importState={importState}
                  key={child.id}
                  onAttach={onAttach}
                  onAttachFolder={onAttachFolder}
                  onAttached={onAttached}
                  setError={setError}
                  setImportState={setImportState}
                />
              ))}
            </ul>
          ) : children ? (
            <div className="py-1 pl-2 text-[11px] text-zinc-400">Empty folder.</div>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}
