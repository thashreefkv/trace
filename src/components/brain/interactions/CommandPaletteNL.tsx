import { useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Box,
  Clock,
  Crosshair,
  GitBranch,
  Maximize2,
  Moon,
  Search,
  Sparkles,
  Sun,
  Target,
  Wand2,
  Workflow,
  Zap,
} from "lucide-react";
import type { LayoutMode } from "../../../lib/brain/layouts";
import { MOTION } from "../../../lib/motion";

interface CommandPaletteNLProps {
  open: boolean;
  onClose: () => void;
  onSetMode: (mode: LayoutMode) => void;
  onFitToScreen: () => void;
  onToggleTheme: () => void;
  onFocusSearch: () => void;
  onToggleFocus: () => void;
  canvasTheme: "light" | "dark";
}

type Action =
  | { kind: "mode"; mode: LayoutMode; label: string; hint: string; Icon: typeof Zap }
  | { kind: "command"; id: string; label: string; hint: string; Icon: typeof Zap };

const MODE_ACTIONS: Action[] = [
  { kind: "mode", mode: "force", label: "Force 2D", hint: "GPU force simulation", Icon: Zap },
  { kind: "mode", mode: "force3d", label: "3D Force", hint: "Three.js orbit with bloom", Icon: Box },
  { kind: "mode", mode: "hierarchical", label: "Hierarchy", hint: "ELK layered top-down", Icon: GitBranch },
  { kind: "mode", mode: "radial", label: "Radial", hint: "BFS rings", Icon: Target },
  { kind: "mode", mode: "umap", label: "UMAP", hint: "Semantic 2D projection from embeddings", Icon: Wand2 },
  { kind: "mode", mode: "timeline", label: "Timeline", hint: "x = time, y = kind row", Icon: Clock },
  { kind: "mode", mode: "communities", label: "Communities", hint: "GraphRAG clusters", Icon: Workflow },
];

export function CommandPaletteNL({
  open,
  onClose,
  onSetMode,
  onFitToScreen,
  onToggleTheme,
  onFocusSearch,
  onToggleFocus,
  canvasTheme,
}: CommandPaletteNLProps) {
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setQuery("");
      setActiveIndex(0);
      requestAnimationFrame(() => inputRef.current?.focus());
    }
  }, [open]);

  const commandActions: Action[] = useMemo(
    () => [
      { kind: "command", id: "fit", label: "Fit to screen", hint: "Frame all visible nodes", Icon: Maximize2 },
      { kind: "command", id: "recenter", label: "Recenter", hint: "Camera to bbox center", Icon: Crosshair },
      { kind: "command", id: "focus", label: "Toggle focus mode", hint: "Hide rails for full-bleed canvas", Icon: Sparkles },
      { kind: "command", id: "search", label: "Focus search bar", hint: "/", Icon: Search },
      {
        kind: "command",
        id: "theme",
        label: canvasTheme === "dark" ? "Light canvas" : "Dark canvas",
        hint: "Swap canvas backdrop palette",
        Icon: canvasTheme === "dark" ? Sun : Moon,
      },
    ],
    [canvasTheme],
  );

  const actions: Action[] = useMemo(() => [...MODE_ACTIONS, ...commandActions], [commandActions]);

  const filtered: Action[] = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return actions;
    return actions.filter((a) => {
      const haystack = `${a.label} ${a.hint} ${a.kind === "mode" ? a.mode : a.id}`.toLowerCase();
      return haystack.includes(q);
    });
  }, [query, actions]);

  useEffect(() => {
    if (activeIndex >= filtered.length) setActiveIndex(0);
  }, [filtered, activeIndex]);

  function runAction(action: Action) {
    if (action.kind === "mode") {
      onSetMode(action.mode);
    } else if (action.id === "fit" || action.id === "recenter") {
      onFitToScreen();
    } else if (action.id === "focus") {
      onToggleFocus();
    } else if (action.id === "search") {
      onFocusSearch();
    } else if (action.id === "theme") {
      onToggleTheme();
    }
    onClose();
  }

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          animate={{ opacity: 1 }}
          className="fixed inset-0 z-[60] bg-zinc-950/30 backdrop-blur-sm"
          exit={{ opacity: 0 }}
          initial={{ opacity: 0 }}
          onClick={onClose}
        >
          <motion.div
            animate={{ y: 0, opacity: 1, scale: 1 }}
            className="mx-auto mt-[12vh] w-full max-w-xl overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-2xl"
            exit={{ y: 12, opacity: 0, scale: 0.98 }}
            initial={{ y: 12, opacity: 0, scale: 0.98 }}
            onClick={(e) => e.stopPropagation()}
            transition={MOTION.spring}
          >
            <div className="flex items-center gap-2 border-b border-zinc-100 px-4 py-3">
              <Search aria-hidden className="text-zinc-400" size={16} />
              <input
                aria-label="Command palette query"
                className="min-w-0 flex-1 bg-transparent text-[14px] text-zinc-900 placeholder:text-zinc-400 focus:outline-none"
                onChange={(e) => setQuery(e.currentTarget.value)}
                onKeyDown={(e) => {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setActiveIndex((i) => Math.min(filtered.length - 1, i + 1));
                  } else if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setActiveIndex((i) => Math.max(0, i - 1));
                  } else if (e.key === "Enter") {
                    e.preventDefault();
                    const action = filtered[activeIndex];
                    if (action) runAction(action);
                  } else if (e.key === "Escape") {
                    onClose();
                  }
                }}
                placeholder="Switch mode, run command, or search…"
                ref={inputRef}
                value={query}
              />
              <span className="rounded-md border border-zinc-200 bg-zinc-50 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
                ESC
              </span>
            </div>
            <ul className="max-h-96 overflow-y-auto p-1.5">
              {filtered.length === 0 ? (
                <li className="px-3 py-6 text-center text-[12px] text-zinc-400">No matches</li>
              ) : (
                filtered.map((action, idx) => {
                  const isActive = idx === activeIndex;
                  const Icon = action.Icon;
                  return (
                    <li key={`${action.kind}-${action.kind === "mode" ? action.mode : action.id}`}>
                      <button
                        className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left transition-colors ${
                          isActive ? "bg-sky-50 text-zinc-900" : "text-zinc-700 hover:bg-zinc-50"
                        }`}
                        onClick={() => runAction(action)}
                        onMouseEnter={() => setActiveIndex(idx)}
                        type="button"
                      >
                        <Icon
                          aria-hidden
                          className={isActive ? "text-sky-500" : "text-zinc-400"}
                          size={14}
                        />
                        <span className="flex-1 min-w-0">
                          <span className="block text-[13px] font-medium">{action.label}</span>
                          <span className="block text-[11px] text-zinc-400">{action.hint}</span>
                        </span>
                        <span className="text-[10.5px] uppercase tracking-wider text-zinc-300">
                          {action.kind === "mode" ? "Mode" : "Action"}
                        </span>
                      </button>
                    </li>
                  );
                })
              )}
            </ul>
            <div className="flex items-center justify-between border-t border-zinc-100 px-4 py-2 text-[10.5px] text-zinc-400">
              <span>
                <kbd className="rounded border border-zinc-200 px-1.5 py-0.5">↑</kbd>{" "}
                <kbd className="rounded border border-zinc-200 px-1.5 py-0.5">↓</kbd> navigate
              </span>
              <span>
                <kbd className="rounded border border-zinc-200 px-1.5 py-0.5">↵</kbd> run
              </span>
              <span>
                <kbd className="rounded border border-zinc-200 px-1.5 py-0.5">⌘⇧K</kbd> toggle
              </span>
            </div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
