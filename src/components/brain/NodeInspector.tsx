import { useMemo } from "react";
import { motion } from "framer-motion";
import {
  ExternalLink,
  EyeOff,
  Focus,
  MousePointerClick,
  Pin,
  PinOff,
  Plus,
  Sparkles,
  X,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import type { WorkGraph, WorkGraphEdge, WorkGraphNode } from "../../lib/types";
import { colorForKind, KNOWN_RELATION_LABELS, labelForKind } from "../../lib/brain/kinds";
import {
  deepLinkForNode,
  fieldsForNode,
  kindHeadingLabel,
} from "../../lib/brain/inspectorFields";

interface NodeInspectorProps {
  node: WorkGraphNode | null;
  graph: WorkGraph | null;
  pinned: boolean;
  onClose: () => void;
  onSelectNode: (id: string) => void;
  onTogglePin: (id: string) => void;
  onHide: (id: string) => void;
  onFindSimilar: (node: WorkGraphNode) => void;
  onMakeMemory: (node: WorkGraphNode) => void;
  onEnterFocus?: (id: string) => void;
}

interface NeighborSummary {
  edge: WorkGraphEdge;
  node: WorkGraphNode;
  direction: "out" | "in";
}

export function NodeInspector({
  node,
  graph,
  pinned,
  onClose,
  onSelectNode,
  onTogglePin,
  onHide,
  onFindSimilar,
  onMakeMemory,
  onEnterFocus,
}: NodeInspectorProps) {
  const navigate = useNavigate();

  const neighbors = useMemo(() => {
    if (!node || !graph) return [];
    const byId = new Map(graph.nodes.map((n) => [n.id, n]));
    const out: NeighborSummary[] = [];
    for (const edge of graph.edges) {
      if (edge.source === node.id) {
        const other = byId.get(edge.target);
        if (other) out.push({ edge, node: other, direction: "out" });
      } else if (edge.target === node.id) {
        const other = byId.get(edge.source);
        if (other) out.push({ edge, node: other, direction: "in" });
      }
    }
    return out.slice(0, 60);
  }, [node, graph]);

  const grouped = useMemo(() => {
    const map = new Map<string, NeighborSummary[]>();
    for (const n of neighbors) {
      const arr = map.get(n.edge.kind) ?? [];
      arr.push(n);
      map.set(n.edge.kind, arr);
    }
    return Array.from(map.entries()).sort((a, b) => b[1].length - a[1].length);
  }, [neighbors]);

  if (!node) {
    return (
      <aside className="flex h-full items-center justify-center rounded-2xl border border-zinc-100 bg-white p-6 text-center shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
        <div className="space-y-2">
          <MousePointerClick aria-hidden className="mx-auto text-zinc-200" size={32} />
          <p className="text-[12px] font-medium text-zinc-500">Click a node to inspect</p>
          <p className="text-[11px] text-zinc-400">
            Or run a Cypher query to project rows onto the canvas.
          </p>
        </div>
      </aside>
    );
  }

  const palette = colorForKind(node.kind);
  const facts = fieldsForNode(node);
  const link = deepLinkForNode(node);

  return (
    <motion.aside
      animate={{ opacity: 1, x: 0 }}
      className="flex h-full flex-col overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.04)]"
      exit={{ opacity: 0, x: 12 }}
      initial={{ opacity: 0, x: 12 }}
      key={node.id}
      transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
    >
      <header className="flex items-start justify-between gap-3 border-b border-zinc-100 px-4 pb-3 pt-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-1.5">
            <span
              className="inline-flex items-center gap-1 rounded-md border px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wider"
              style={{ background: palette.fill, borderColor: palette.stroke, color: palette.text }}
            >
              {kindHeadingLabel(node)}
            </span>
            {node.status && (
              <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                {node.status}
              </span>
            )}
            {pinned && (
              <span className="inline-flex items-center gap-1 rounded-md bg-amber-50 px-1.5 py-0.5 text-[10px] font-medium text-amber-700">
                <Pin size={9} /> Pinned
              </span>
            )}
          </div>
          <h3 className="mt-1.5 break-words text-[14px] font-semibold text-zinc-950">
            {node.label}
          </h3>
          {node.subtitle && (
            <p className="mt-0.5 text-[12px] text-zinc-500">{node.subtitle}</p>
          )}
        </div>
        <button
          aria-label="Close"
          className="grid h-7 w-7 shrink-0 place-items-center rounded-lg text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
          onClick={onClose}
          type="button"
        >
          <X size={14} />
        </button>
      </header>

      <div className="flex-1 space-y-3 overflow-y-auto p-4">
        {facts.length > 0 && (
          <section className="rounded-xl border border-zinc-100 bg-zinc-50 p-3.5">
            <h4 className="page-kicker mb-2">Key facts</h4>
            <dl className="grid grid-cols-[110px_minmax(0,1fr)] gap-x-3 gap-y-1.5 text-[12px]">
              {facts.map((f) => (
                <div className="contents" key={f.label}>
                  <dt className="text-zinc-500">{f.label}</dt>
                  <dd className="break-words font-medium text-zinc-800">{f.value}</dd>
                </div>
              ))}
            </dl>
          </section>
        )}

        <section className="rounded-xl border border-zinc-100 bg-zinc-50 p-3.5">
          <div className="mb-2 flex items-center justify-between">
            <h4 className="page-kicker">Connections</h4>
            <span className="text-[11px] tabular-nums text-zinc-400">
              {neighbors.length}
            </span>
          </div>
          {grouped.length === 0 ? (
            <p className="text-[12px] text-zinc-400">No edges in the current view.</p>
          ) : (
            <ul className="space-y-2.5">
              {grouped.map(([relation, items]) => (
                <li key={relation}>
                  <p className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
                    {KNOWN_RELATION_LABELS[relation] ?? relation.replace(/_/g, " ")} ·{" "}
                    {items.length}
                  </p>
                  <ul className="space-y-1">
                    {items.slice(0, 12).map((n) => {
                      const otherPalette = colorForKind(n.node.kind);
                      return (
                        <li key={n.edge.id}>
                          <button
                            className="flex w-full items-center gap-2 rounded-lg px-1.5 py-1 text-left text-[12px] text-zinc-700 transition-colors hover:bg-white"
                            onClick={() => onSelectNode(n.node.id)}
                            type="button"
                          >
                            <span
                              aria-hidden
                              className="h-2 w-2 shrink-0 rounded-full"
                              style={{ background: otherPalette.stroke }}
                            />
                            <span className="min-w-0 flex-1 truncate">{n.node.label}</span>
                            <span className="text-[10px] text-zinc-400">
                              {labelForKind(n.node.kind)}
                            </span>
                          </button>
                        </li>
                      );
                    })}
                    {items.length > 12 && (
                      <li className="px-1.5 text-[11px] text-zinc-400">
                        + {items.length - 12} more
                      </li>
                    )}
                  </ul>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <footer className="grid grid-cols-2 gap-2 border-t border-zinc-100 p-3">
        <ActionButton
          icon={pinned ? <PinOff size={12} /> : <Pin size={12} />}
          label={pinned ? "Unpin" : "Pin"}
          onClick={() => onTogglePin(node.id)}
        />
        <ActionButton icon={<EyeOff size={12} />} label="Hide" onClick={() => onHide(node.id)} />
        <ActionButton
          icon={<Sparkles size={12} />}
          label="Find similar"
          onClick={() => onFindSimilar(node)}
        />
        <ActionButton
          icon={<Plus size={12} />}
          label="Add to memory"
          onClick={() => onMakeMemory(node)}
        />
        {onEnterFocus && (
          <button
            className="col-span-2 flex items-center justify-center gap-1.5 rounded-xl border border-violet-200 bg-violet-50 px-2.5 py-1.5 text-[11px] font-medium text-violet-700 transition-colors hover:bg-violet-100"
            onClick={() => onEnterFocus(node.id)}
            type="button"
          >
            <Focus size={12} />
            Enter focus mode
          </button>
        )}
        {link && (
          <button
            className="col-span-2 mt-1 flex items-center justify-center gap-1.5 rounded-xl bg-zinc-900 px-3 py-2 text-[12px] font-medium text-white transition-colors hover:bg-zinc-800"
            onClick={() => navigate(link)}
            type="button"
          >
            Open original
            <ExternalLink size={11} />
          </button>
        )}
      </footer>
    </motion.aside>
  );
}

function ActionButton({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      className="flex items-center justify-center gap-1.5 rounded-xl border border-zinc-200 bg-white px-2.5 py-1.5 text-[11px] font-medium text-zinc-600 transition-colors hover:border-zinc-300 hover:bg-zinc-50 hover:text-zinc-900"
      onClick={onClick}
      type="button"
    >
      <span className="text-zinc-500">{icon}</span>
      {label}
    </button>
  );
}
