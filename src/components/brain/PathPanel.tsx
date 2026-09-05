import { motion } from "framer-motion";
import { ArrowRight, MapPin, Route, X } from "lucide-react";
import type { PathResult } from "../../lib/brain/pathfinding";
import { colorForKind, labelForKind } from "../../lib/brain/kinds";

interface PathPanelProps {
  endpoints: { from: string | null; to: string | null };
  path: PathResult | null;
  noPathReason: string | null;
  onClear: () => void;
  onSelectNode: (id: string) => void;
}

export function PathPanel({
  endpoints,
  path,
  noPathReason,
  onClear,
  onSelectNode,
}: PathPanelProps) {
  const hasSelection = endpoints.from || endpoints.to;
  if (!hasSelection) return null;

  return (
    <motion.div
      animate={{ opacity: 1, y: 0 }}
      className="pointer-events-auto absolute right-3 top-3 w-[280px] rounded-2xl border border-zinc-100 bg-white p-3 shadow-[0_12px_36px_rgba(0,0,0,0.10)]"
      exit={{ opacity: 0, y: -6 }}
      initial={{ opacity: 0, y: -6 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
    >
      <header className="flex items-start justify-between gap-2 pb-2">
        <div className="flex items-center gap-1.5 text-[11px] uppercase tracking-wider text-zinc-500">
          <Route className="text-violet-500" size={12} />
          Connect path
        </div>
        <button
          aria-label="Clear path selection"
          className="grid h-6 w-6 place-items-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
          onClick={onClear}
          type="button"
        >
          <X size={12} />
        </button>
      </header>

      {!endpoints.to && (
        <p className="rounded-xl border border-dashed border-zinc-200 bg-zinc-50 px-2.5 py-2 text-[12px] text-zinc-500">
          ⌘-click another node to trace the shortest path between them.
        </p>
      )}

      {endpoints.to && noPathReason && (
        <p className="rounded-xl bg-amber-50 px-2.5 py-2 text-[12px] text-amber-800">
          {noPathReason}
        </p>
      )}

      {path && (
        <ol className="mt-1 space-y-1.5 text-[12px]">
          {path.hops.map((hop, i) => {
            const fromPalette = colorForKind(hop.fromNode.kind);
            const toPalette = colorForKind(hop.toNode.kind);
            const rel = hop.edge.label || hop.edge.kind.replace(/_/g, " ");
            const isLast = i === path.hops.length - 1;
            return (
              <li className="rounded-xl border border-zinc-100 bg-zinc-50 px-2.5 py-2" key={hop.edge.id}>
                <div className="flex items-center gap-2">
                  <button
                    className="flex min-w-0 flex-1 items-center gap-1.5 truncate text-left text-zinc-700 hover:text-zinc-900"
                    onClick={() => onSelectNode(hop.fromNode.id)}
                    type="button"
                  >
                    <span
                      aria-hidden
                      className="h-1.5 w-1.5 shrink-0 rounded-full"
                      style={{ background: fromPalette.stroke }}
                    />
                    <span className="truncate font-medium">{hop.fromNode.label}</span>
                  </button>
                </div>
                <div className="ml-3 mt-1 flex items-center gap-1 text-[11px] text-zinc-500">
                  <ArrowRight size={11} />
                  <span className="rounded-md bg-white px-1.5 py-px text-zinc-600">{rel}</span>
                </div>
                {isLast && (
                  <button
                    className="mt-1 flex min-w-0 items-center gap-1.5 truncate text-left text-[12px] text-zinc-700 hover:text-zinc-900"
                    onClick={() => onSelectNode(hop.toNode.id)}
                    type="button"
                  >
                    <span
                      aria-hidden
                      className="h-1.5 w-1.5 shrink-0 rounded-full"
                      style={{ background: toPalette.stroke }}
                    />
                    <span className="truncate font-medium">{hop.toNode.label}</span>
                    <span className="text-[10px] text-zinc-400">
                      {labelForKind(hop.toNode.kind)}
                    </span>
                  </button>
                )}
              </li>
            );
          })}
        </ol>
      )}

      {path && (
        <footer className="mt-2 flex items-center justify-between text-[11px] text-zinc-500">
          <span className="flex items-center gap-1.5">
            <MapPin size={11} className="text-zinc-400" />
            {path.hops.length} hop{path.hops.length === 1 ? "" : "s"}
          </span>
          <span>{path.nodeIds.length} nodes</span>
        </footer>
      )}
    </motion.div>
  );
}
