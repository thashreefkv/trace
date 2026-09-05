import { useCallback, useEffect, useState } from "react";
import { ChevronRight, FolderOpen, FolderPlus, Loader2, Plus, RefreshCw } from "lucide-react";
import {
  driveImport,
  driveImportFolder,
  driveListChildren,
  type DriveAccount,
  type DriveEntry,
} from "../../lib/files";
import { mimeIcon } from "./mimeIcon";

interface DriveTreePaneProps {
  account: DriveAccount;
  targetTraceFolderId: string | null;
  onImported: () => void;
}

interface DriveNodeProps {
  account: DriveAccount;
  entry: DriveEntry;
  targetTraceFolderId: string | null;
  onImported: () => void;
}

export function DriveTreePane({
  account,
  targetTraceFolderId,
  onImported,
}: DriveTreePaneProps) {
  const [entries, setEntries] = useState<DriveEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setBusy(true);
      setError(null);
      const listing = await driveListChildren(null, null);
      setEntries(listing.entries);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0">
          <p className="text-[11px] font-bold uppercase tracking-widest text-zinc-500">Drive</p>
          <p className="truncate text-[11px] text-zinc-500">{account.email}</p>
        </div>
        <button
          aria-label="Refresh"
          className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-500 hover:bg-zinc-200/60"
          disabled={busy}
          onClick={() => void load()}
          title="Refresh"
          type="button"
        >
          <RefreshCw className={busy ? "animate-spin" : ""} size={13} />
        </button>
      </div>

      {error ? <div className="notice notice-error">{error}</div> : null}

      <div className="rounded-md border border-zinc-200 bg-white p-1.5">
        {entries == null ? (
          <div className="space-y-1 p-1.5">
            {Array.from({ length: 5 }).map((_, i) => (
              <div key={i} className="skeleton h-7" />
            ))}
          </div>
        ) : entries.length === 0 ? (
          <div className="px-3 py-4 text-center text-[12px] text-zinc-500">
            No files in Drive root.
          </div>
        ) : (
          <ul className="space-y-0.5">
            {entries.map((entry) => (
              <DriveNode
                account={account}
                entry={entry}
                key={entry.id}
                onImported={onImported}
                targetTraceFolderId={targetTraceFolderId}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function DriveNode({ account, entry, targetTraceFolderId, onImported }: DriveNodeProps) {
  const [open, setOpen] = useState(false);
  const [children, setChildren] = useState<DriveEntry[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isFolder = entry.isFolder;

  async function toggleOpen() {
    const next = !open;
    setOpen(next);
    if (next && isFolder && !children) {
      try {
        setLoading(true);
        setError(null);
        const listing = await driveListChildren(entry.id, null);
        setChildren(listing.entries);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    }
  }

  async function handleImport() {
    try {
      setImporting(true);
      setError(null);
      if (isFolder) {
        await driveImportFolder(account.id, entry.id, entry.name, targetTraceFolderId);
      } else {
        await driveImport(account.id, [entry.id], targetTraceFolderId);
      }
      onImported();
    } catch (e) {
      setError(String(e));
    } finally {
      setImporting(false);
    }
  }

  return (
    <li>
      <div className="group flex items-center gap-1 rounded-md px-1 py-1 text-[12px] hover:bg-zinc-50">
        {isFolder ? (
          <button
            aria-label={open ? "Collapse" : "Expand"}
            className={[
              "inline-flex h-5 w-5 items-center justify-center rounded text-zinc-400 transition-transform",
              open ? "rotate-90" : "",
            ].join(" ")}
            onClick={() => void toggleOpen()}
            type="button"
          >
            <ChevronRight size={11} />
          </button>
        ) : (
          <span className="inline-block h-5 w-5" />
        )}
        <span className="shrink-0">
          {isFolder ? (
            <FolderOpen className="text-amber-500" size={12} />
          ) : (
            mimeIcon(entry.mimeType, 12)
          )}
        </span>
        <span className="min-w-0 flex-1 truncate text-zinc-800">{entry.name}</span>

        <button
          aria-label={isFolder ? "Import folder to Trace" : "Import file to Trace"}
          className="inline-flex h-6 w-6 shrink-0 items-center justify-center rounded text-zinc-400 opacity-0 transition-opacity hover:bg-zinc-100 hover:text-zinc-700 group-hover:opacity-100"
          disabled={importing}
          onClick={() => void handleImport()}
          title={isFolder ? "Import folder + its files into Trace" : "Import file to Trace"}
          type="button"
        >
          {importing ? (
            <Loader2 className="animate-spin" size={11} />
          ) : isFolder ? (
            <FolderPlus size={12} />
          ) : (
            <Plus size={12} />
          )}
        </button>
      </div>
      {error ? <div className="ml-6 text-[11px] text-rose-600">{error}</div> : null}
      {open && isFolder ? (
        <div className="ml-3 border-l border-zinc-100 pl-2">
          {loading ? (
            <div className="flex items-center gap-1.5 py-1.5 text-[11px] text-zinc-500">
              <Loader2 className="animate-spin" size={11} />
              Loading…
            </div>
          ) : children && children.length > 0 ? (
            <ul className="space-y-0.5">
              {children.map((child) => (
                <DriveNode
                  account={account}
                  entry={child}
                  key={child.id}
                  onImported={onImported}
                  targetTraceFolderId={targetTraceFolderId}
                />
              ))}
            </ul>
          ) : children ? (
            <div className="py-1 text-[11px] text-zinc-400">Empty.</div>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}
