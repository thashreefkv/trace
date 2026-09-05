import { useCallback, useEffect, useState } from "react";
import { Play, RefreshCw, Sparkles } from "lucide-react";
import { embedNow, enqueueAllEmbeddings, getEmbeddingProgress } from "../lib/ipc";
import { toast } from "../lib/toast";
import type { EmbeddingProgress } from "../lib/types";

export function EmbeddingStatusPanel() {
  const [progress, setProgress] = useState<EmbeddingProgress | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setProgress(await getEmbeddingProgress());
    } catch {
      // ipc wrapper toasts
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
    const id = window.setInterval(() => void load(), 15_000);
    return () => window.clearInterval(id);
  }, [load]);

  const handleEnqueueAll = async () => {
    try {
      const count = await enqueueAllEmbeddings();
      toast.info(`Queued ${count} entities for re-check`);
      await load();
    } catch {
      // toasted
    }
  };

  const handleEmbedNow = async () => {
    setRunning(true);
    try {
      const processed = await embedNow();
      toast.success(`Embedded ${processed} item${processed === 1 ? "" : "s"}`);
      await load();
    } catch {
      // toasted
    } finally {
      setRunning(false);
    }
  };

  const completion =
    progress && progress.total > 0
      ? Math.round((progress.embedded / progress.total) * 100)
      : 0;

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <Sparkles size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">
              Semantic embeddings
            </h2>
            <p className="text-[11px] text-zinc-400">
              {progress
                ? `${progress.model} · ${progress.dim} dimensions`
                : "Powers hybrid retrieval for Ask and the brain graph."}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="btn"
            disabled={running}
            onClick={() => void handleEmbedNow()}
            type="button"
          >
            <Play size={14} />
            {running ? "Embedding…" : "Embed now"}
          </button>
          <button
            aria-label="Refresh"
            className="btn h-8 w-8 px-0"
            disabled={loading}
            onClick={() => void load()}
            type="button"
          >
            <RefreshCw size={14} className={loading ? "animate-spin" : ""} />
          </button>
        </div>
      </div>

      {!progress ? (
        <div className="space-y-2 p-5">
          <div className="h-12 animate-pulse rounded-xl bg-zinc-100" />
        </div>
      ) : (
        <div className="px-5 py-4 space-y-3">
          <div className="grid grid-cols-4 gap-2">
            <Stat label="Embedded" value={`${progress.embedded}`} />
            <Stat
              label="Pending"
              value={`${progress.pending}`}
              tone={progress.pending > 0 ? "info" : undefined}
            />
            <Stat label="Stale" value={`${progress.stale}`} />
            <Stat label="Complete" value={`${completion}%`} />
          </div>

          <div className="h-1.5 overflow-hidden rounded-full bg-zinc-100">
            <div
              className="h-full bg-sky-400 transition-all"
              style={{ width: `${completion}%` }}
            />
          </div>

          <div className="flex items-center justify-between">
            <p className="text-[11px] text-zinc-400">
              Runs automatically every minute when the app is open.
            </p>
            <button
              className="text-[11px] text-zinc-500 underline-offset-2 hover:text-zinc-900 hover:underline"
              onClick={() => void handleEnqueueAll()}
              type="button"
            >
              Re-check all entities
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "info";
}) {
  return (
    <div
      className={`rounded-xl border p-3 ${
        tone === "info"
          ? "border-sky-100 bg-sky-50"
          : "border-zinc-100 bg-zinc-50"
      }`}
    >
      <p className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </p>
      <p
        className={`mt-1 text-lg font-bold ${
          tone === "info" ? "text-sky-700" : "text-zinc-950"
        }`}
      >
        {value}
      </p>
    </div>
  );
}
