import { motion } from "framer-motion";
import { EyeOff, MessageSquarePlus, Pin, Sparkles, X } from "lucide-react";

interface BulkActionsToolbarProps {
  count: number;
  onClear: () => void;
  onPinAll: () => void;
  onHideAll: () => void;
  onMakeMemory: () => void;
  onOpenInAsk: () => void;
}

export function BulkActionsToolbar({
  count,
  onClear,
  onPinAll,
  onHideAll,
  onMakeMemory,
  onOpenInAsk,
}: BulkActionsToolbarProps) {
  if (count === 0) return null;
  return (
    <motion.div
      animate={{ opacity: 1, y: 0 }}
      className="pointer-events-auto absolute bottom-4 left-1/2 z-20 flex -translate-x-1/2 items-center gap-1.5 rounded-2xl border border-zinc-100 bg-white p-1.5 shadow-[0_12px_36px_rgba(0,0,0,0.14)]"
      exit={{ opacity: 0, y: 8 }}
      initial={{ opacity: 0, y: 8 }}
      transition={{ duration: 0.18, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="rounded-xl bg-zinc-900 px-2.5 py-1.5 text-[11px] font-semibold text-white">
        {count} selected
      </div>
      <ToolbarButton icon={<Pin size={12} />} label="Pin" onClick={onPinAll} />
      <ToolbarButton icon={<EyeOff size={12} />} label="Hide" onClick={onHideAll} />
      <ToolbarButton
        icon={<Sparkles size={12} />}
        label="Make memory"
        onClick={onMakeMemory}
      />
      <ToolbarButton
        icon={<MessageSquarePlus size={12} />}
        label="Open in Ask"
        onClick={onOpenInAsk}
      />
      <button
        aria-label="Clear selection"
        className="ml-1 grid h-7 w-7 place-items-center rounded-lg text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
        onClick={onClear}
        type="button"
      >
        <X size={12} />
      </button>
    </motion.div>
  );
}

function ToolbarButton({
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
      className="flex items-center gap-1.5 rounded-xl border border-transparent px-2.5 py-1.5 text-[11px] font-medium text-zinc-700 transition-colors hover:border-zinc-200 hover:bg-zinc-50 hover:text-zinc-900"
      onClick={onClick}
      type="button"
    >
      <span className="text-zinc-500">{icon}</span>
      {label}
    </button>
  );
}
