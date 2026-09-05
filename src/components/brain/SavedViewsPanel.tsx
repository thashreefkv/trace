import { useState } from "react";
import { Bookmark, Loader2, MoreVertical, Plus, Trash2 } from "lucide-react";
import type { SavedBrainView } from "../../lib/types";

interface SavedViewsPanelProps {
  views: SavedBrainView[];
  isLoading: boolean;
  onApply: (view: SavedBrainView) => void;
  onSaveCurrent: (name: string) => Promise<void> | void;
  onDelete: (view: SavedBrainView) => Promise<void> | void;
  activeViewId: string | null;
}

export function SavedViewsPanel({
  views,
  isLoading,
  onApply,
  onSaveCurrent,
  onDelete,
  activeViewId,
}: SavedViewsPanelProps) {
  const [draftName, setDraftName] = useState("");
  const [savingDraft, setSavingDraft] = useState(false);
  const [menuOpenId, setMenuOpenId] = useState<string | null>(null);

  const submitSave = async () => {
    const name = draftName.trim();
    if (!name) return;
    setSavingDraft(true);
    try {
      await onSaveCurrent(name);
      setDraftName("");
    } finally {
      setSavingDraft(false);
    }
  };

  return (
    <section className="mt-4">
      <h4 className="mb-2 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
        <Bookmark className="text-zinc-300" size={13} />
        Saved views
      </h4>

      <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-2">
        <div className="flex items-center gap-1.5">
          <input
            className="min-w-0 flex-1 rounded-lg border border-zinc-200 bg-white px-2 py-1.5 text-[12px] placeholder:text-zinc-400 focus:border-sky-300 focus:outline-none focus:ring-2 focus:ring-sky-100"
            onChange={(event) => setDraftName(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") void submitSave();
            }}
            placeholder="Save current view as…"
            value={draftName}
          />
          <button
            aria-label="Save view"
            className="grid h-7 w-7 place-items-center rounded-lg bg-zinc-900 text-white transition-colors hover:bg-zinc-800 disabled:opacity-50"
            disabled={savingDraft || draftName.trim().length === 0}
            onClick={() => void submitSave()}
            type="button"
          >
            {savingDraft ? <Loader2 className="animate-spin" size={12} /> : <Plus size={13} />}
          </button>
        </div>

        {isLoading ? (
          <div className="mt-2 grid h-6 place-items-center text-[11px] text-zinc-400">
            <Loader2 className="animate-spin" size={11} />
          </div>
        ) : views.length === 0 ? (
          <p className="mt-2 px-1 text-[11px] text-zinc-400">No saved views yet.</p>
        ) : (
          <ul className="mt-2 space-y-1">
            {views.map((view) => {
              const isActive = view.id === activeViewId;
              const isMenuOpen = menuOpenId === view.id;
              return (
                <li className="relative" key={view.id}>
                  <div
                    className={`flex items-center gap-1 rounded-lg px-1 py-1 transition-colors ${
                      isActive ? "bg-white shadow-[0_1px_3px_rgba(0,0,0,0.06)]" : "hover:bg-white"
                    }`}
                  >
                    <button
                      className="flex min-w-0 flex-1 items-center gap-1.5 text-left"
                      onClick={() => onApply(view)}
                      type="button"
                    >
                      <span
                        aria-hidden
                        className={`h-1.5 w-1.5 shrink-0 rounded-full ${
                          isActive ? "bg-sky-500" : "bg-zinc-300"
                        }`}
                      />
                      <span className="min-w-0 flex-1 truncate text-[12px] font-medium text-zinc-700">
                        {view.name}
                      </span>
                      <span className="text-[10px] text-zinc-400">{relative(view.updated_at)}</span>
                    </button>
                    <button
                      aria-label="View actions"
                      className="grid h-6 w-6 place-items-center rounded-md text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600"
                      onClick={() => setMenuOpenId(isMenuOpen ? null : view.id)}
                      type="button"
                    >
                      <MoreVertical size={12} />
                    </button>
                  </div>
                  {isMenuOpen && (
                    <div className="absolute right-0 top-7 z-10 w-32 overflow-hidden rounded-lg border border-zinc-100 bg-white shadow-[0_8px_24px_rgba(0,0,0,0.10)]">
                      <button
                        className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left text-[11px] text-rose-600 hover:bg-rose-50"
                        onClick={async () => {
                          setMenuOpenId(null);
                          await onDelete(view);
                        }}
                        type="button"
                      >
                        <Trash2 size={11} />
                        Delete view
                      </button>
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
}

function relative(ms: number): string {
  if (!ms) return "";
  const diff = Date.now() - ms;
  const mins = Math.round(diff / 60_000);
  if (mins < 1) return "now";
  if (mins < 60) return `${mins}m`;
  const hrs = Math.round(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  const days = Math.round(hrs / 24);
  return `${days}d`;
}
