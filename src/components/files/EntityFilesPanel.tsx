import { useCallback, useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { Cloud, ExternalLink, Eye, FolderOpen, Paperclip, Plus, Trash2 } from "lucide-react";
import { useNavigate } from "react-router-dom";
import {
  attachLocalDirectory,
  attachLocalFile,
  filesForEntity,
  foldersForEntity,
  isPreviewableDriveMime,
  linkFileToEntity,
  linkFolderFilesToEntity,
  openDrivePreview,
  openFile,
  resolveEntityFolder,
  unlinkFileFromEntity,
  unlinkFolderFromEntity,
  type FileEntityKind,
  type FileRow,
  type TraceFolderWithLinks,
} from "../../lib/files";
import { humanSize, mimeIcon } from "./mimeIcon";
import { DriveFilePickerModal } from "./DriveFilePickerModal";

interface EntityFilesPanelProps {
  entityKind: FileEntityKind;
  entityId: string;
  onCountChange?: (count: number) => void;
}

export function EntityFilesPanel({ entityKind, entityId, onCountChange }: EntityFilesPanelProps) {
  const navigate = useNavigate();
  const [files, setFiles] = useState<FileRow[]>([]);
  const [folders, setFolders] = useState<TraceFolderWithLinks[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [drivePickerOpen, setDrivePickerOpen] = useState(false);
  const [resolvedFolderId, setResolvedFolderId] = useState<string | null | undefined>(undefined);

  const refresh = useCallback(async () => {
    try {
      setLoading(true);
      const [nextFiles, nextFolders] = await Promise.all([
        filesForEntity(entityKind, entityId),
        foldersForEntity(entityKind, entityId),
      ]);
      setFiles(nextFiles);
      setFolders(nextFolders);
      onCountChange?.(nextFiles.length + nextFolders.length);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, [entityKind, entityId, onCountChange]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function getOrResolveFolderId(): Promise<string | null> {
    if (resolvedFolderId !== undefined) return resolvedFolderId;
    try {
      const folder = await resolveEntityFolder(entityKind, entityId);
      setResolvedFolderId(folder.id);
      return folder.id;
    } catch {
      setResolvedFolderId(null);
      return null;
    }
  }

  async function handleAttachLocal() {
    try {
      const picked = await openDialog({ multiple: false, directory: false });
      if (!picked || typeof picked !== "string") return;
      setBusy(true);
      setError(null);
      const folderId = await getOrResolveFolderId();
      const file = await attachLocalFile(picked, folderId);
      await linkFileToEntity(file.id, entityKind, entityId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleAttachLocalFolder() {
    try {
      const picked = await openDialog({ multiple: false, directory: true });
      if (!picked || typeof picked !== "string") return;
      setBusy(true);
      setError(null);
      const folderId = await getOrResolveFolderId();
      const folder = await attachLocalDirectory(picked, folderId);
      await linkFolderFilesToEntity(folder.id, entityKind, entityId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpenDrivePicker() {
    const folderId = await getOrResolveFolderId();
    setResolvedFolderId(folderId);
    setDrivePickerOpen(true);
  }

  async function handleUnlink(fileId: string) {
    try {
      setBusy(true);
      setError(null);
      await unlinkFileFromEntity(fileId, entityKind, entityId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleUnlinkFolder(folderId: string) {
    try {
      setBusy(true);
      setError(null);
      await unlinkFolderFromEntity(folderId, entityKind, entityId);
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function handleOpen(fileId: string) {
    try {
      await openFile(fileId);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handlePreview(file: FileRow) {
    try {
      await openDrivePreview(
        file.id,
        file.driveFileId!,
        file.driveMime!,
        file.name,
      );
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <>
    {drivePickerOpen ? (
      <DriveFilePickerModal
        entityKind={entityKind}
        entityId={entityId}
        traceFolderId={resolvedFolderId ?? null}
        onClose={() => setDrivePickerOpen(false)}
        onAttached={() => void refresh()}
      />
    ) : null}
    <section className="space-y-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Paperclip className="text-zinc-500" size={14} />
          <h3 className="text-[13px] font-semibold text-zinc-900">
            Files <span className="text-zinc-400">({files.length})</span>
          </h3>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            className="btn"
            disabled={busy}
            onClick={() => void handleOpenDrivePicker()}
            title="Attach a Google Drive file or folder"
            type="button"
          >
            <Cloud size={13} />
            Drive
          </button>
          <button className="btn" disabled={busy} onClick={() => void handleAttachLocal()} type="button">
            <Plus size={13} />
            File
          </button>
          <button
            className="btn"
            disabled={busy}
            onClick={() => void handleAttachLocalFolder()}
            title="Attach a local folder"
            type="button"
          >
            <FolderOpen size={13} />
            Folder
          </button>
        </div>
      </div>

      {error ? <div className="notice notice-error">{error}</div> : null}

      {loading ? (
        <div className="space-y-1.5">
          {[...Array(3)].map((_, i) => (
            <div key={i} className="skeleton h-10" />
          ))}
        </div>
      ) : folders.length > 0 ? (
        <ul className="mb-2 space-y-1.5">
          {folders.map((folder) => (
            <li
              className="flex items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 py-2"
              key={folder.id}
            >
              <FolderOpen className="shrink-0 text-amber-500" size={14} />
              <button
                className="min-w-0 flex-1 text-left"
                onClick={() => navigate(`/files?folder=${folder.id}`)}
                type="button"
              >
                <span className="block truncate text-[13px] font-medium text-zinc-900 hover:underline">
                  {folder.name}
                </span>
                <span className="block truncate text-[11px] text-zinc-500">
                  Folder · {folder.fileCount} file{folder.fileCount !== 1 ? "s" : ""}
                </span>
              </button>
              <button
                aria-label="Unlink folder"
                className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-rose-50 hover:text-rose-600"
                disabled={busy}
                onClick={() => void handleUnlinkFolder(folder.id)}
                title="Unlink from this item"
                type="button"
              >
                <Trash2 size={13} />
              </button>
            </li>
          ))}
        </ul>
      ) : null}

      {!loading && files.length === 0 && folders.length === 0 ? (
        <div className="rounded-md border border-dashed border-zinc-300 px-3 py-6 text-center text-[12px] text-zinc-500">
          No files linked yet. Attach a local file to get started.
        </div>
      ) : (
        <ul className="space-y-1.5">
          {files.map((file) => (
            <li
              className="flex items-center gap-2 rounded-md border border-zinc-200 bg-white px-3 py-2"
              key={file.id}
            >
              <span className="shrink-0">{mimeIcon(file.mimeType ?? file.driveMime)}</span>
              <button
                className="min-w-0 flex-1 text-left"
                onClick={() => void handleOpen(file.id)}
                title={file.localPath ?? file.driveWebViewLink ?? ""}
                type="button"
              >
                <span className="block truncate text-[13px] font-medium text-zinc-900 hover:underline">
                  {file.name}
                </span>
                <span className="block truncate text-[11px] text-zinc-500">
                  <span className="inline-flex items-center gap-1">
                    {file.kind === "drive" ? "Drive" : "Local"}
                    {file.isMissing ? " · missing" : ""}
                    {file.sizeBytes ? ` · ${humanSize(file.sizeBytes)}` : ""}
                  </span>
                </span>
              </button>
              <button
                aria-label="Open"
                className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-100 hover:text-zinc-700"
                onClick={() => void handleOpen(file.id)}
                title="Open"
                type="button"
              >
                <ExternalLink size={13} />
              </button>
              {isPreviewableDriveMime(file.driveMime) && file.driveFileId ? (
                <button
                  aria-label="Preview"
                  className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-sky-50 hover:text-sky-700"
                  onClick={() => void handlePreview(file)}
                  title="Preview in Trace"
                  type="button"
                >
                  <Eye size={13} />
                </button>
              ) : null}
              <button
                aria-label="Unlink"
                className="inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-rose-50 hover:text-rose-600"
                disabled={busy}
                onClick={() => void handleUnlink(file.id)}
                title="Unlink from this item"
                type="button"
              >
                <Trash2 size={13} />
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
    </>
  );
}
