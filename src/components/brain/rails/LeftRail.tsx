import { motion } from "framer-motion";
import { ChevronRight, Filter } from "lucide-react";
import type { ReactNode } from "react";
import { BrainLeftRail } from "../BrainLeftRail";
import { MOTION } from "../../../lib/motion";
import { RAIL_WIDTH, type RailMode } from "./useRailState";

interface LeftRailProps {
  mode: RailMode;
  onModeChange: (m: RailMode) => void;
  // Pass-through to existing BrainLeftRail.
  kindCounts: Map<string, number>;
  relationCounts: Map<string, number>;
  hiddenKinds: Set<string>;
  hiddenRelations: Set<string>;
  onToggleKind: (kind: string) => void;
  onToggleRelation: (relation: string) => void;
  onResetFilters: () => void;
  confidenceThreshold: number;
  onConfidenceChange: (value: number) => void;
  includeDismissedCaptures: boolean;
  includeKilledDeliverables: boolean;
  onToggleIncludeDismissed: () => void;
  onToggleIncludeKilled: () => void;
  communitiesVisible: boolean;
  onToggleCommunities: () => void;
  savedViewsSlot?: ReactNode;
}

export function LeftRail({ mode, onModeChange, ...rest }: LeftRailProps) {
  const width = RAIL_WIDTH[mode];
  return (
    <motion.aside
      animate={{ width }}
      className="relative h-full shrink-0 overflow-hidden"
      initial={false}
      transition={MOTION.spring}
    >
      {mode === "hidden" ? (
        <button
          aria-label="Show filters"
          className="absolute inset-y-0 left-0 grid w-8 place-items-center rounded-r-xl border border-l-0 border-zinc-100 bg-white text-zinc-400 shadow-[0_2px_12px_rgba(0,0,0,0.04)] transition-colors hover:bg-zinc-50 hover:text-zinc-900"
          onClick={() => onModeChange("expanded")}
          type="button"
        >
          <Filter size={13} />
        </button>
      ) : (
        <BrainLeftRail
          collapsed={mode === "iconStrip"}
          onToggleCollapsed={() =>
            onModeChange(mode === "expanded" ? "iconStrip" : "expanded")
          }
          {...rest}
        />
      )}
    </motion.aside>
  );
}

// Tiny floating "open" tab shown when the rail is fully hidden, so we don't
// orphan the user. Lives in the canvas area as an overlay (rendered by
// BrainExplorer next to the canvas, not inside the aside).
export function LeftRailReopenTab({ onOpen }: { onOpen: () => void }) {
  return (
    <button
      aria-label="Show filters"
      className="absolute left-2 top-2 z-20 grid h-8 w-8 place-items-center rounded-lg border border-zinc-100 bg-white text-zinc-500 shadow-[0_2px_12px_rgba(0,0,0,0.06)] transition-colors hover:bg-zinc-50 hover:text-zinc-900"
      onClick={onOpen}
      title="Show filters (⌘\\)"
      type="button"
    >
      <ChevronRight size={14} />
    </button>
  );
}
