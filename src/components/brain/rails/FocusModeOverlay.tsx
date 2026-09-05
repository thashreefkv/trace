import { motion, AnimatePresence } from "framer-motion";
import { Crosshair, Minus, Plus, X } from "lucide-react";
import { MOTION } from "../../../lib/motion";

interface FocusModeOverlayProps {
  visible: boolean;
  onExit: () => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onRecenter: () => void;
  nodeCount: number;
  edgeCount: number;
}

export function FocusModeOverlay({
  visible,
  onExit,
  onZoomIn,
  onZoomOut,
  onRecenter,
  nodeCount,
  edgeCount,
}: FocusModeOverlayProps) {
  return (
    <AnimatePresence>
      {visible && (
        <motion.div
          animate={{ opacity: 1, y: 0 }}
          className="pointer-events-none fixed bottom-6 left-1/2 z-50 -translate-x-1/2"
          exit={{ opacity: 0, y: 12 }}
          initial={{ opacity: 0, y: 12 }}
          transition={MOTION.spring}
        >
          <div className="pointer-events-auto flex items-center gap-2 rounded-2xl border border-zinc-100 bg-white/90 px-2 py-1.5 shadow-[0_12px_40px_rgba(0,0,0,0.12)] backdrop-blur">
            <span className="px-2 text-[11px] font-medium tabular-nums text-zinc-500">
              {nodeCount.toLocaleString()} <span className="text-zinc-300">·</span>{" "}
              {edgeCount.toLocaleString()}
            </span>
            <span aria-hidden className="h-5 w-px bg-zinc-200" />
            <IconBtn label="Zoom out" onClick={onZoomOut}>
              <Minus size={14} />
            </IconBtn>
            <IconBtn label="Zoom in" onClick={onZoomIn}>
              <Plus size={14} />
            </IconBtn>
            <IconBtn label="Fit to screen" onClick={onRecenter}>
              <Crosshair size={14} />
            </IconBtn>
            <span aria-hidden className="h-5 w-px bg-zinc-200" />
            <button
              className="grid h-7 w-7 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
              onClick={onExit}
              title="Exit focus mode (Esc)"
              type="button"
            >
              <X size={14} />
            </button>
            <span className="hidden pr-1.5 text-[10.5px] font-medium uppercase tracking-wider text-zinc-300 sm:inline">
              Focus
            </span>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

function IconBtn({
  label,
  onClick,
  children,
}: {
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      className="grid h-7 w-7 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
      onClick={onClick}
      title={label}
      type="button"
    >
      {children}
    </button>
  );
}
