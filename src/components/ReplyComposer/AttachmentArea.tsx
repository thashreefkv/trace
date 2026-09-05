import { useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import {
  File as FileIcon,
  FileText,
  Image as ImageIcon,
  Loader2,
  Paperclip,
  X,
} from "lucide-react";
import type { LocalEmailDraftAttachment } from "../../lib/types";
import { toast } from "../../lib/toast";

interface Props {
  attachments: LocalEmailDraftAttachment[];
  onAdd: (sourcePath: string) => Promise<void>;
  onRemove: (attachmentId: string) => Promise<void>;
  disabled?: boolean;
}

export function AttachmentArea({ attachments, onAdd, onRemove, disabled }: Props) {
  const [adding, setAdding] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);

  async function handlePick() {
    if (disabled || adding) return;
    try {
      const selected = await openDialog({
        multiple: true,
        title: "Attach files",
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      setAdding(true);
      for (const p of paths) {
        try {
          await onAdd(p);
        } catch (error) {
          toast.error(`Failed to attach ${p}: ${error}`);
        }
      }
    } catch (error) {
      toast.error(`Could not open file picker: ${error}`);
    } finally {
      setAdding(false);
    }
  }

  async function handleRemove(id: string) {
    setRemovingId(id);
    try {
      await onRemove(id);
    } finally {
      setRemovingId(null);
    }
  }

  if (attachments.length === 0) {
    return (
      <button
        className="flex items-center gap-1.5 rounded-md px-2 py-1 text-[12px] font-medium text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900 disabled:cursor-not-allowed disabled:opacity-40"
        disabled={disabled || adding}
        onClick={() => void handlePick()}
        type="button"
      >
        {adding ? (
          <Loader2 className="animate-spin" size={13} />
        ) : (
          <Paperclip size={13} />
        )}
        Attach files
      </button>
    );
  }

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {attachments.map((att) => (
        <AttachmentChip
          attachment={att}
          key={att.id}
          onRemove={() => void handleRemove(att.id)}
          removing={removingId === att.id}
        />
      ))}
      <button
        className="flex items-center gap-1 rounded-md border border-dashed border-zinc-200 px-2 py-1 text-[11px] font-medium text-zinc-500 hover:border-zinc-300 hover:bg-zinc-50 hover:text-zinc-900 disabled:cursor-not-allowed disabled:opacity-40"
        disabled={disabled || adding}
        onClick={() => void handlePick()}
        type="button"
      >
        {adding ? (
          <Loader2 className="animate-spin" size={11} />
        ) : (
          <Paperclip size={11} />
        )}
        Add more
      </button>
    </div>
  );
}

function AttachmentChip({
  attachment,
  onRemove,
  removing,
}: {
  attachment: LocalEmailDraftAttachment;
  onRemove: () => void;
  removing: boolean;
}) {
  const Icon = iconForMime(attachment.mime_type);
  return (
    <span className="inline-flex max-w-[220px] items-center gap-1.5 rounded-md border border-zinc-200 bg-zinc-50 px-2 py-1 text-[11px] text-zinc-700">
      <Icon className="shrink-0 text-zinc-500" size={12} />
      <span className="truncate" title={attachment.filename}>
        {attachment.filename}
      </span>
      <span className="shrink-0 text-zinc-400">
        {formatBytes(attachment.file_size)}
      </span>
      <button
        aria-label={`Remove ${attachment.filename}`}
        className="shrink-0 rounded p-0.5 text-zinc-400 hover:bg-zinc-200 hover:text-rose-600 disabled:opacity-40"
        disabled={removing}
        onClick={onRemove}
        title="Remove"
        type="button"
      >
        {removing ? <Loader2 className="animate-spin" size={11} /> : <X size={11} />}
      </button>
    </span>
  );
}

function iconForMime(mime: string) {
  if (mime.startsWith("image/")) return ImageIcon;
  if (mime === "application/pdf" || mime.startsWith("text/")) return FileText;
  return FileIcon;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 102.4) / 10} KB`;
  return `${Math.round(bytes / 104857.6) / 10} MB`;
}
