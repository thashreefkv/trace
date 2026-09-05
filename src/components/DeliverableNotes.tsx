import { useEffect, useRef, useState } from "react";
import { Send, Trash2 } from "lucide-react";
import { listDeliverableNotes, createDeliverableNote, deleteDeliverableNote } from "../lib/ipc";
import type { DeliverableNote } from "../lib/types";

interface Props {
  deliverableId: string;
}

export function DeliverableNotes({ deliverableId }: Props) {
  const [notes, setNotes] = useState<DeliverableNote[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [body, setBody] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    void load();
  }, [deliverableId]);

  async function load() {
    try {
      setError(null);
      setIsLoading(true);
      const result = await listDeliverableNotes(deliverableId);
      setNotes(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleSubmit() {
    const text = body.trim();
    if (!text) return;
    try {
      setIsSubmitting(true);
      const note = await createDeliverableNote({ deliverable_id: deliverableId, body: text });
      setNotes((prev) => [note, ...prev]);
      setBody("");
      textareaRef.current?.focus();
    } catch (e) {
      setError(String(e));
    } finally {
      setIsSubmitting(false);
    }
  }

  async function handleDelete(id: string) {
    try {
      await deleteDeliverableNote(id);
      setNotes((prev) => prev.filter((n) => n.id !== id));
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      {error && <div className="notice notice-error">{error}</div>}

      {/* Compose */}
      <div className="flex gap-2">
        <textarea
          className="field-control min-h-[72px] flex-1 resize-y"
          disabled={isSubmitting}
          onChange={(e) => setBody(e.currentTarget.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void handleSubmit();
          }}
          placeholder="Write a note… (⌘↵ to submit)"
          ref={textareaRef}
          value={body}
        />
        <button
          className="btn self-end"
          disabled={isSubmitting || !body.trim()}
          onClick={() => void handleSubmit()}
          type="button"
        >
          <Send aria-hidden="true" size={16} />
        </button>
      </div>

      {/* Notes feed */}
      {isLoading ? (
        <p className="text-sm text-zinc-500">Loading notes…</p>
      ) : notes.length === 0 ? (
        <p className="text-sm text-zinc-400 dark:text-neutral-600">No notes yet.</p>
      ) : (
        <ul className="space-y-3">
          {notes.map((note) => (
            <NoteEntry key={note.id} note={note} onDelete={handleDelete} />
          ))}
        </ul>
      )}
    </div>
  );
}

interface NoteEntryProps {
  note: DeliverableNote;
  onDelete: (id: string) => void;
}

function NoteEntry({ note, onDelete }: NoteEntryProps) {
  const date = new Date(note.created_at);
  const label = date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <li className="group relative rounded-lg border border-zinc-100 bg-zinc-50 px-4 py-3 dark:border-zinc-800 dark:bg-zinc-900">
      <div className="mb-1.5 flex items-center justify-between gap-2">
        <span className="font-mono text-[11px] text-zinc-400">{label}</span>
        <button
          className="text-zinc-300 opacity-0 transition-opacity hover:text-red-500 group-hover:opacity-100 dark:text-zinc-700"
          onClick={() => onDelete(note.id)}
          type="button"
        >
          <Trash2 size={13} />
        </button>
      </div>
      <p className="whitespace-pre-wrap text-sm leading-relaxed text-zinc-700 dark:text-zinc-300">
        {note.body}
      </p>
    </li>
  );
}
