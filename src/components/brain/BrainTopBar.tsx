import {
  Box,
  ChevronDown,
  Clock,
  Crosshair,
  GitBranch,
  Maximize2,
  Minus,
  Palette,
  Plus,
  RefreshCw,
  Sparkles,
  Target,
  Wand2,
  Workflow,
  Zap,
} from "lucide-react";
import { motion } from "framer-motion";
import { useEffect, useRef, useState } from "react";
import type { ColorMode } from "../../lib/brain/colorOverlays";

export type BrainLayoutMode =
  | "force"
  | "force3d"
  | "hierarchical"
  | "radial"
  | "umap"
  | "timeline"
  | "communities";

interface BrainTopBarProps {
  layout: BrainLayoutMode;
  onLayoutChange: (next: BrainLayoutMode) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onRecenter: () => void;
  onRefresh: () => void;
  isRefreshing: boolean;
  nodeCount: number;
  edgeCount: number;
  generatedAt: string | null;
  searchSlot: React.ReactNode;
  timeScrubberOpen: boolean;
  onToggleTimeScrubber: () => void;
  isLayouting: boolean;
  colorMode: ColorMode;
  onColorModeChange: (mode: ColorMode) => void;
}

const COLOR_MODE_OPTIONS: Array<{ value: ColorMode; label: string; description: string }> = [
  { value: "kind", label: "Entity kind", description: "Default palette by node kind" },
  { value: "recency", label: "Recency", description: "Heat by last-updated timestamp" },
  { value: "centrality", label: "Connections", description: "Heat by edge degree" },
  { value: "community", label: "Community", description: "Tint by GraphRAG cluster" },
];

interface LayoutOption {
  value: BrainLayoutMode;
  label: string;
  Icon: typeof Zap;
  hint: string;
}

const LAYOUT_OPTIONS: LayoutOption[] = [
  { value: "force", label: "Force", Icon: Zap, hint: "GPU force simulation" },
  { value: "force3d", label: "3D", Icon: Box, hint: "Three.js orbit" },
  { value: "hierarchical", label: "Hierarchy", Icon: GitBranch, hint: "ELK layered" },
  { value: "radial", label: "Radial", Icon: Target, hint: "BFS rings" },
  { value: "umap", label: "UMAP", Icon: Wand2, hint: "Semantic 2D projection" },
  { value: "timeline", label: "Timeline", Icon: Clock, hint: "x = time, y = kind row" },
  { value: "communities", label: "Communities", Icon: Workflow, hint: "Grouped clusters" },
];

export function BrainTopBar({
  layout,
  onLayoutChange,
  onZoomIn,
  onZoomOut,
  onRecenter,
  onRefresh,
  isRefreshing,
  nodeCount,
  edgeCount,
  generatedAt,
  searchSlot,
  timeScrubberOpen,
  onToggleTimeScrubber,
  isLayouting,
  colorMode,
  onColorModeChange,
}: BrainTopBarProps) {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const paletteRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onClick = (event: MouseEvent) => {
      if (!paletteRef.current) return;
      if (paletteRef.current.contains(event.target as Node)) return;
      setPaletteOpen(false);
    };
    window.addEventListener("mousedown", onClick);
    return () => window.removeEventListener("mousedown", onClick);
  }, []);

  const activeColor = COLOR_MODE_OPTIONS.find((opt) => opt.value === colorMode) ?? COLOR_MODE_OPTIONS[0];
  return (
    <header className="flex items-center gap-3 rounded-2xl border border-zinc-100 bg-white px-3 py-2 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
      <div className="flex min-w-0 flex-1 items-center gap-3">
        <div className="hidden shrink-0 items-center gap-2 rounded-xl border border-zinc-100 bg-zinc-50 px-2.5 py-1.5 text-[11px] text-zinc-500 md:flex">
          <Sparkles aria-hidden className="text-violet-500" size={13} />
          <span className="font-semibold tracking-tight text-zinc-700">Brain</span>
          <span className="text-zinc-300">·</span>
          <span className="tabular-nums">{nodeCount.toLocaleString()} nodes</span>
          <span className="text-zinc-300">·</span>
          <span className="tabular-nums">{edgeCount.toLocaleString()} edges</span>
        </div>
        <div className="min-w-0 flex-1">{searchSlot}</div>
      </div>

      <div className="hidden h-7 w-px bg-zinc-100 lg:block" />

      <div
        className="hidden items-center gap-0.5 rounded-xl border border-zinc-100 bg-zinc-50 p-0.5 md:flex"
        role="tablist"
        aria-label="Layout mode"
      >
        {LAYOUT_OPTIONS.map((opt) => {
          const isActive = opt.value === layout;
          const Icon = opt.Icon;
          return (
            <button
              aria-pressed={isActive}
              className="relative grid h-7 w-7 place-items-center rounded-lg text-zinc-500 transition-colors hover:text-zinc-900 disabled:cursor-not-allowed disabled:opacity-60"
              disabled={isLayouting && !isActive}
              key={opt.value}
              onClick={() => onLayoutChange(opt.value)}
              title={`${opt.label} — ${opt.hint}`}
              type="button"
            >
              {isActive && (
                <motion.span
                  className="absolute inset-0 -z-10 rounded-lg bg-white shadow-[0_1px_4px_rgba(0,0,0,0.08)]"
                  layoutId="brain-layout-active"
                  transition={{ type: "spring", stiffness: 380, damping: 32 }}
                />
              )}
              <Icon
                aria-hidden
                className={isActive ? "text-sky-600" : "text-zinc-400"}
                size={13}
              />
              <span className="sr-only">{opt.label}</span>
            </button>
          );
        })}
      </div>

      <div className="hidden h-7 w-px bg-zinc-100 lg:block" />

      <div className="relative hidden lg:block" ref={paletteRef}>
        <button
          aria-haspopup="true"
          aria-expanded={paletteOpen}
          className="flex items-center gap-1.5 rounded-lg px-2 py-1.5 text-[11px] font-medium text-zinc-600 transition-colors hover:bg-zinc-50 hover:text-zinc-900"
          onClick={() => setPaletteOpen((v) => !v)}
          type="button"
        >
          <Palette aria-hidden size={13} />
          <span>{activeColor.label}</span>
          <ChevronDown aria-hidden size={11} />
        </button>
        {paletteOpen && (
          <div className="absolute right-0 top-9 z-30 w-56 rounded-xl border border-zinc-100 bg-white p-1 shadow-[0_12px_36px_rgba(0,0,0,0.12)]">
            {COLOR_MODE_OPTIONS.map((opt) => (
              <button
                className={`flex w-full items-start gap-2 rounded-lg px-2 py-1.5 text-left ${
                  opt.value === colorMode
                    ? "bg-zinc-50 text-zinc-900"
                    : "text-zinc-700 hover:bg-zinc-50"
                }`}
                key={opt.value}
                onClick={() => {
                  onColorModeChange(opt.value);
                  setPaletteOpen(false);
                }}
                type="button"
              >
                <span
                  aria-hidden
                  className={`mt-0.5 h-2 w-2 shrink-0 rounded-full ${
                    opt.value === colorMode ? "bg-sky-500" : "bg-zinc-200"
                  }`}
                />
                <span className="min-w-0 flex-1">
                  <span className="block text-[12px] font-medium">{opt.label}</span>
                  <span className="block text-[10.5px] text-zinc-400">{opt.description}</span>
                </span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className="hidden h-7 w-px bg-zinc-100 lg:block" />

      <div className="flex items-center gap-1">
        <IconButton
          active={timeScrubberOpen}
          label="Time scrubber"
          onClick={onToggleTimeScrubber}
        >
          <Clock size={14} />
        </IconButton>
        <IconButton label="Zoom out" onClick={onZoomOut}>
          <Minus size={14} />
        </IconButton>
        <IconButton label="Zoom in" onClick={onZoomIn}>
          <Plus size={14} />
        </IconButton>
        <IconButton label="Recenter" onClick={onRecenter}>
          <Crosshair size={14} />
        </IconButton>
        <IconButton label="Fit to screen" onClick={onRecenter}>
          <Maximize2 size={14} />
        </IconButton>
        <IconButton label="Refresh" onClick={onRefresh} spinning={isRefreshing}>
          <RefreshCw size={14} />
        </IconButton>
      </div>

      {generatedAt && (
        <div className="hidden text-[11px] text-zinc-400 xl:block">
          Built {timeAgo(generatedAt)}
        </div>
      )}
    </header>
  );
}

function IconButton({
  label,
  onClick,
  spinning,
  active,
  children,
}: {
  label: string;
  onClick: () => void;
  spinning?: boolean;
  active?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      aria-label={label}
      className={`grid h-8 w-8 place-items-center rounded-lg transition-colors ${
        active
          ? "bg-sky-50 text-sky-700"
          : "text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
      } ${spinning ? "animate-spin" : ""}`}
      onClick={onClick}
      title={label}
      type="button"
    >
      {children}
    </button>
  );
}

function timeAgo(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  const diff = Date.now() - date.getTime();
  const mins = Math.round(diff / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.round(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.round(hrs / 24);
  return `${days}d ago`;
}
