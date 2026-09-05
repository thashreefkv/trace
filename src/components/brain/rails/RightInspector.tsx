import { motion, AnimatePresence } from "framer-motion";
import { ChevronLeft, Eye, PanelRightClose } from "lucide-react";
import type { WorkGraph, WorkGraphNode } from "../../../lib/types";
import { NodeInspector } from "../NodeInspector";
import { MOTION } from "../../../lib/motion";
import { INSPECTOR_WIDTH, type RailMode } from "./useRailState";

interface RightInspectorProps {
  mode: RailMode;
  onModeChange: (m: RailMode) => void;
  selectedNode: WorkGraphNode | null;
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

export function RightInspector({ mode, onModeChange, selectedNode, ...inspectorProps }: RightInspectorProps) {
  const width = INSPECTOR_WIDTH[mode];

  return (
    <motion.aside
      animate={{ width }}
      className="relative h-full shrink-0 overflow-hidden"
      initial={false}
      transition={MOTION.spring}
    >
      {mode === "iconStrip" && (
        <IconStrip
          hasSelection={Boolean(selectedNode)}
          onExpand={() => onModeChange("expanded")}
          onHide={() => onModeChange("hidden")}
        />
      )}
      {mode === "expanded" && (
        <div className="h-full">
          <CollapseHeader onCollapse={() => onModeChange("iconStrip")} />
          <div className="h-[calc(100%-32px)]">
            <AnimatePresence mode="wait">
              <NodeInspector
                key={selectedNode?.id ?? "empty"}
                node={selectedNode}
                {...inspectorProps}
              />
            </AnimatePresence>
          </div>
        </div>
      )}
      {mode === "hidden" && null}
    </motion.aside>
  );
}

function IconStrip({
  hasSelection,
  onExpand,
  onHide,
}: {
  hasSelection: boolean;
  onExpand: () => void;
  onHide: () => void;
}) {
  return (
    <div className="flex h-full w-14 flex-col items-center gap-2 rounded-2xl border border-zinc-100 bg-white py-3 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
      <button
        aria-label="Expand inspector"
        className="grid h-8 w-8 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
        onClick={onExpand}
        title="Expand inspector"
        type="button"
      >
        <ChevronLeft size={15} />
      </button>
      <div
        aria-hidden
        className={`grid h-8 w-8 place-items-center rounded-lg ${
          hasSelection ? "text-sky-500" : "text-zinc-300"
        }`}
        title={hasSelection ? "Selection ready" : "No selection"}
      >
        <Eye size={14} />
      </div>
      <button
        aria-label="Hide inspector"
        className="mt-auto grid h-8 w-8 place-items-center rounded-lg text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
        onClick={onHide}
        title="Hide inspector"
        type="button"
      >
        <PanelRightClose size={14} />
      </button>
    </div>
  );
}

function CollapseHeader({ onCollapse }: { onCollapse: () => void }) {
  return (
    <div className="pointer-events-none absolute right-2 top-2 z-10">
      <button
        aria-label="Collapse inspector"
        className="pointer-events-auto grid h-7 w-7 place-items-center rounded-lg bg-white/80 text-zinc-500 shadow-[0_2px_8px_rgba(0,0,0,0.06)] backdrop-blur transition-colors hover:bg-white hover:text-zinc-900"
        onClick={onCollapse}
        title="Collapse to strip (⌘.)"
        type="button"
      >
        <PanelRightClose size={13} />
      </button>
    </div>
  );
}

export function RightInspectorReopenTab({ onOpen }: { onOpen: () => void }) {
  return (
    <button
      aria-label="Show inspector"
      className="absolute right-2 top-2 z-20 grid h-8 w-8 place-items-center rounded-lg border border-zinc-100 bg-white text-zinc-500 shadow-[0_2px_12px_rgba(0,0,0,0.06)] transition-colors hover:bg-zinc-50 hover:text-zinc-900"
      onClick={onOpen}
      title="Show inspector (⌘.)"
      type="button"
    >
      <ChevronLeft size={14} />
    </button>
  );
}
