import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Check, Loader2, X } from "lucide-react";
import type { WorkGraphEdge } from "../../lib/types";

interface InferenceEdgePopoverProps {
  edge: WorkGraphEdge | null;
  anchor: { x: number; y: number } | null;
  onAccept: (edge: WorkGraphEdge) => Promise<void> | void;
  onReject: (edge: WorkGraphEdge) => Promise<void> | void;
  onClose: () => void;
}

export function InferenceEdgePopover({
  edge,
  anchor,
  onAccept,
  onReject,
  onClose,
}: InferenceEdgePopoverProps) {
  const [busy, setBusy] = useState<"accept" | "reject" | null>(null);

  useEffect(() => {
    setBusy(null);
  }, [edge?.id]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  if (!edge || !anchor) return null;
  const rationale =
    (edge.properties?.rationale as string | undefined) ??
    (edge.properties?.reason as string | undefined) ??
    "Trace inferred this connection from cross-source signals.";
  const confidence =
    typeof edge.properties?.confidence === "number"
      ? Math.round((edge.properties.confidence as number) * 100)
      : null;
  const template = (edge.properties?.template as string | undefined) ?? null;

  return (
    <motion.div
      animate={{ opacity: 1, y: 0, scale: 1 }}
      className="absolute z-30 w-[300px] rounded-2xl border border-zinc-100 bg-white p-3 shadow-[0_16px_42px_rgba(0,0,0,0.12)]"
      exit={{ opacity: 0, y: -6, scale: 0.96 }}
      initial={{ opacity: 0, y: -6, scale: 0.96 }}
      style={{ left: anchor.x + 12, top: anchor.y + 12 }}
      transition={{ duration: 0.16, ease: [0.16, 1, 0.3, 1] }}
    >
      <header className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <p className="page-kicker">Inferred · pending review</p>
          <p className="mt-1 text-[12px] font-semibold text-zinc-900">
            {edge.label || edge.kind.replace(/_/g, " ")}
          </p>
          <div className="mt-1 flex items-center gap-2 text-[11px] text-zinc-500">
            {confidence != null && (
              <span className="rounded-md bg-amber-50 px-1.5 py-0.5 text-amber-800">
                {confidence}% confidence
              </span>
            )}
            {template && (
              <span className="rounded-md bg-zinc-100 px-1.5 py-0.5">{template}</span>
            )}
          </div>
        </div>
        <button
          aria-label="Close"
          className="grid h-6 w-6 shrink-0 place-items-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
          onClick={onClose}
          type="button"
        >
          <X size={12} />
        </button>
      </header>

      <p className="mt-2 break-words rounded-xl border border-zinc-100 bg-zinc-50 p-2 text-[12px] leading-relaxed text-zinc-700">
        {rationale}
      </p>

      <footer className="mt-3 flex items-center gap-2">
        <button
          className="flex flex-1 items-center justify-center gap-1.5 rounded-xl bg-emerald-600 px-2.5 py-1.5 text-[12px] font-medium text-white transition-colors hover:bg-emerald-700 disabled:opacity-60"
          disabled={busy !== null}
          onClick={async () => {
            setBusy("accept");
            try {
              await onAccept(edge);
              onClose();
            } finally {
              setBusy(null);
            }
          }}
          type="button"
        >
          {busy === "accept" ? <Loader2 className="animate-spin" size={12} /> : <Check size={12} />}
          Accept
        </button>
        <button
          className="flex flex-1 items-center justify-center gap-1.5 rounded-xl border border-zinc-200 bg-white px-2.5 py-1.5 text-[12px] font-medium text-zinc-600 transition-colors hover:bg-zinc-50 hover:text-zinc-900 disabled:opacity-60"
          disabled={busy !== null}
          onClick={async () => {
            setBusy("reject");
            try {
              await onReject(edge);
              onClose();
            } finally {
              setBusy(null);
            }
          }}
          type="button"
        >
          {busy === "reject" ? <Loader2 className="animate-spin" size={12} /> : <X size={12} />}
          Reject
        </button>
      </footer>
    </motion.div>
  );
}
