import { motion } from "framer-motion";
import { Focus, X } from "lucide-react";

interface FocusModePillProps {
  focusedLabel: string | null;
  hopRadius: number;
  onHopChange: (hops: number) => void;
  onExit: () => void;
}

export function FocusModePill({
  focusedLabel,
  hopRadius,
  onHopChange,
  onExit,
}: FocusModePillProps) {
  if (!focusedLabel) return null;
  return (
    <motion.div
      animate={{ opacity: 1, y: 0 }}
      className="pointer-events-auto absolute left-1/2 top-3 z-20 flex -translate-x-1/2 items-center gap-2 rounded-2xl border border-zinc-100 bg-white px-2.5 py-1.5 text-[12px] shadow-[0_12px_36px_rgba(0,0,0,0.12)]"
      exit={{ opacity: 0, y: -6 }}
      initial={{ opacity: 0, y: -6 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
    >
      <span className="flex items-center gap-1.5 rounded-lg bg-zinc-900 px-2 py-1 text-[11px] font-medium text-white">
        <Focus size={11} />
        Focus
      </span>
      <span className="max-w-[220px] truncate text-zinc-700">{focusedLabel}</span>
      <span className="text-zinc-200">·</span>
      <div className="flex items-center gap-1.5 rounded-lg bg-zinc-50 px-2 py-1">
        {[1, 2, 3, 4].map((n) => (
          <button
            aria-pressed={hopRadius === n}
            className={`grid h-5 w-5 place-items-center rounded-md text-[10.5px] font-semibold transition-colors ${
              hopRadius === n
                ? "bg-zinc-900 text-white"
                : "text-zinc-500 hover:bg-white hover:text-zinc-900"
            }`}
            key={n}
            onClick={() => onHopChange(n)}
            type="button"
          >
            {n}
          </button>
        ))}
        <span className="text-[10px] text-zinc-400">hops</span>
      </div>
      <button
        aria-label="Exit focus mode"
        className="grid h-6 w-6 place-items-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
        onClick={onExit}
        type="button"
      >
        <X size={12} />
      </button>
    </motion.div>
  );
}
