import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { motion, AnimatePresence } from "framer-motion";
import {
  ArrowRight,
  CalendarDays,
  FolderTree,
  Inbox,
  KanbanSquare,
  Layers3,
  Mail,
  Mic,
  Network,
  Search,
  Settings,
  UsersRound,
  Zap,
} from "lucide-react";
import { searchDeliverables, listInitiatives } from "../lib/ipc";
import type { Deliverable, Initiative } from "../lib/types";
import { deliverableStateLabels, deliverableStateColors } from "../lib/types";

interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
}

type ResultKind = "action" | "deliverable" | "initiative";

interface PaletteItem {
  id: string;
  kind: ResultKind;
  label: string;
  subtitle?: string;
  icon: React.ReactNode;
  badge?: { label: string; color: string };
  onSelect: () => void;
}

const NAV_ITEMS: Omit<PaletteItem, "onSelect">[] = [
  { id: "nav-board", kind: "action", label: "Go to Board", subtitle: "Deliverables kanban", icon: <KanbanSquare size={15} /> },
  { id: "nav-week", kind: "action", label: "Go to Week", subtitle: "Weekly planner", icon: <CalendarDays size={15} /> },
  { id: "nav-captures", kind: "action", label: "Go to Captures", subtitle: "Capture inbox", icon: <Inbox size={15} /> },
  { id: "nav-email", kind: "action", label: "Go to Email", subtitle: "Gmail workspace", icon: <Mail size={15} /> },
  { id: "nav-files", kind: "action", label: "Go to Files", subtitle: "Local + Drive files", icon: <FolderTree size={15} /> },
  { id: "nav-meetings", kind: "action", label: "Go to Meetings", subtitle: "Meeting recordings", icon: <Mic size={15} /> },
  { id: "nav-initiatives", kind: "action", label: "Go to Initiatives", subtitle: "Initiatives list", icon: <Layers3 size={15} /> },
  { id: "nav-stakeholders", kind: "action", label: "Go to Stakeholders", subtitle: "Stakeholder lens", icon: <UsersRound size={15} /> },
  { id: "nav-context", kind: "action", label: "Go to Memory", subtitle: "Work graph", icon: <Network size={15} /> },
  { id: "nav-settings", kind: "action", label: "Go to Settings", subtitle: "Connections, AI, memory, diagnostics", icon: <Settings size={15} /> },
];

const NAV_ROUTES: Record<string, string> = {
  "nav-board": "/deliverables",
  "nav-week": "/week",
  "nav-captures": "/captures",
  "nav-email": "/email",
  "nav-files": "/files",
  "nav-meetings": "/meetings",
  "nav-initiatives": "/initiatives",
  "nav-stakeholders": "/stakeholders",
  "nav-context": "/context",
  "nav-settings": "/settings",
};

const stateColorMap: Record<string, string> = {
  blue: "bg-blue-100 text-blue-700",
  amber: "bg-amber-100 text-amber-700",
  green: "bg-emerald-100 text-emerald-700",
  zinc: "bg-zinc-100 text-zinc-500",
};

export function CommandPalette({ open, onClose }: CommandPaletteProps) {
  const navigate = useNavigate();
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [deliverables, setDeliverables] = useState<Deliverable[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);

  // Focus input on open
  useEffect(() => {
    if (open) {
      setQuery("");
      setSelectedIndex(0);
      setDeliverables([]);
      setInitiatives([]);
      setTimeout(() => inputRef.current?.focus(), 30);
    }
  }, [open]);

  // Debounced live search
  useEffect(() => {
    if (!open) return;
    if (!query.trim()) {
      setDeliverables([]);
      setInitiatives([]);
      setSelectedIndex(0);
      return;
    }

    const timeout = window.setTimeout(async () => {
      try {
        setIsSearching(true);
        const [dels, inits] = await Promise.all([
          searchDeliverables(query),
          listInitiatives(),
        ]);
        setDeliverables(dels.slice(0, 5));
        setInitiatives(
          inits.filter((i) =>
            i.title.toLowerCase().includes(query.toLowerCase()),
          ).slice(0, 3),
        );
        setSelectedIndex(0);
      } catch {
        // silently ignore
      } finally {
        setIsSearching(false);
      }
    }, 160);

    return () => window.clearTimeout(timeout);
  }, [query, open]);

  const buildItems = useCallback((): PaletteItem[] => {
    const items: PaletteItem[] = [];

    if (!query.trim()) {
      // Default: show all nav actions
      for (const nav of NAV_ITEMS) {
        items.push({
          ...nav,
          onSelect: () => {
            navigate(NAV_ROUTES[nav.id]);
            onClose();
          },
        });
      }
      return items;
    }

    // Filtered nav
    const filteredNav = NAV_ITEMS.filter(
      (n) =>
        n.label.toLowerCase().includes(query.toLowerCase()) ||
        (n.subtitle ?? "").toLowerCase().includes(query.toLowerCase()),
    );
    for (const nav of filteredNav) {
      items.push({
        ...nav,
        onSelect: () => {
          navigate(NAV_ROUTES[nav.id]);
          onClose();
        },
      });
    }

    // Initiative results
    for (const initiative of initiatives) {
      const color = initiative.status === "live" ? "bg-emerald-100 text-emerald-700" : "bg-zinc-100 text-zinc-500";
      items.push({
        id: `initiative-${initiative.id}`,
        kind: "initiative",
        label: initiative.title,
        subtitle: initiative.framing?.slice(0, 60) || "Initiative",
        icon: <Layers3 size={15} className="text-violet-500" />,
        badge: { label: initiative.status, color },
        onSelect: () => {
          navigate(`/initiatives/${initiative.id}`);
          onClose();
        },
      });
    }

    // Deliverable results
    for (const deliverable of deliverables) {
      const color = deliverableStateColors[deliverable.state];
      items.push({
        id: `deliverable-${deliverable.id}`,
        kind: "deliverable",
        label: deliverable.title,
        subtitle: deliverable.claim?.slice(0, 60),
        icon: <KanbanSquare size={15} className="text-sky-500" />,
        badge: {
          label: deliverableStateLabels[deliverable.state],
          color: stateColorMap[color] ?? "bg-zinc-100 text-zinc-500",
        },
        onSelect: () => {
          navigate(`/deliverables/${deliverable.id}`);
          onClose();
        },
      });
    }

    return items;
  }, [query, deliverables, initiatives, navigate, onClose]);

  const items = buildItems();

  // Keyboard navigation
  useEffect(() => {
    if (!open) return;

    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onClose();
        return;
      }
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((i) => Math.min(i + 1, items.length - 1));
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((i) => Math.max(i - 1, 0));
      }
      if (e.key === "Enter") {
        e.preventDefault();
        items[selectedIndex]?.onSelect();
      }
    }

    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [open, items, selectedIndex, onClose]);

  // Scroll selected item into view
  useEffect(() => {
    const el = listRef.current?.querySelector(`[data-index="${selectedIndex}"]`);
    el?.scrollIntoView({ block: "nearest" });
  }, [selectedIndex]);

  const groupLabel = !query.trim() ? "Quick navigation" : items.length > 0 ? "Results" : null;

  return (
    <AnimatePresence>
      {open && (
        <>
          {/* Backdrop */}
          <motion.div
            className="fixed inset-0 z-50 bg-black/30 backdrop-blur-[2px]"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={onClose}
          />

          {/* Panel */}
          <motion.div
            className="fixed left-1/2 top-[18%] z-50 w-full max-w-[600px] -translate-x-1/2"
            initial={{ opacity: 0, y: -12, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -8, scale: 0.97 }}
            transition={{ type: "spring", stiffness: 500, damping: 36 }}
          >
            <div className="overflow-hidden rounded-2xl border border-zinc-200/80 bg-white shadow-2xl shadow-black/20 dark:border-zinc-700/80 dark:bg-zinc-900">
              {/* Search input */}
              <div className="flex items-center gap-3 border-b border-zinc-100 px-4 py-3.5 dark:border-zinc-800">
                {isSearching ? (
                  <motion.div
                    className="shrink-0 text-zinc-400"
                    animate={{ rotate: 360 }}
                    transition={{ duration: 0.8, repeat: Infinity, ease: "linear" }}
                  >
                    <Zap size={16} />
                  </motion.div>
                ) : (
                  <Search size={16} className="shrink-0 text-zinc-400" />
                )}
                <input
                  ref={inputRef}
                  className="flex-1 bg-transparent text-[14px] text-zinc-900 placeholder:text-zinc-400 outline-none dark:text-zinc-100"
                  placeholder="Search or navigate…"
                  value={query}
                  onChange={(e) => setQuery(e.currentTarget.value)}
                  autoComplete="off"
                  spellCheck={false}
                />
                <kbd className="shrink-0 rounded-md border border-zinc-200 bg-zinc-50 px-1.5 py-0.5 text-[10px] font-semibold text-zinc-400 dark:border-zinc-700 dark:bg-zinc-800">
                  ESC
                </kbd>
              </div>

              {/* Results list */}
              <div
                ref={listRef}
                className="max-h-[360px] overflow-y-auto overscroll-contain py-1"
              >
                {groupLabel && (
                  <p className="px-4 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-widest text-zinc-400">
                    {groupLabel}
                  </p>
                )}

                {items.length === 0 && query.trim() && !isSearching && (
                  <p className="px-4 py-6 text-center text-sm text-zinc-400">
                    No results for "{query}"
                  </p>
                )}

                {items.map((item, index) => (
                  <button
                    key={item.id}
                    data-index={index}
                    className={[
                      "flex w-full items-center gap-3 px-4 py-2.5 text-left transition-colors",
                      selectedIndex === index
                        ? "bg-zinc-50 dark:bg-zinc-800/60"
                        : "hover:bg-zinc-50 dark:hover:bg-zinc-800/40",
                    ].join(" ")}
                    onMouseEnter={() => setSelectedIndex(index)}
                    onClick={item.onSelect}
                    type="button"
                  >
                    <span className="shrink-0 text-zinc-400">{item.icon}</span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-medium text-zinc-900 dark:text-zinc-100">
                        {item.label}
                      </span>
                      {item.subtitle && (
                        <span className="block truncate text-[11px] text-zinc-400">
                          {item.subtitle}
                        </span>
                      )}
                    </span>
                    {item.badge && (
                      <span
                        className={[
                          "shrink-0 rounded-full px-2 py-0.5 text-[10px] font-medium",
                          item.badge.color,
                        ].join(" ")}
                      >
                        {item.badge.label}
                      </span>
                    )}
                    {selectedIndex === index && (
                      <ArrowRight size={13} className="shrink-0 text-zinc-400" />
                    )}
                  </button>
                ))}
              </div>

              {/* Footer hint */}
              <div className="flex items-center gap-4 border-t border-zinc-100 px-4 py-2 dark:border-zinc-800">
                <span className="text-[10px] text-zinc-400">
                  <kbd className="rounded border border-zinc-200 bg-zinc-50 px-1 dark:border-zinc-700 dark:bg-zinc-800">↑↓</kbd>
                  {" "}navigate
                </span>
                <span className="text-[10px] text-zinc-400">
                  <kbd className="rounded border border-zinc-200 bg-zinc-50 px-1 dark:border-zinc-700 dark:bg-zinc-800">↵</kbd>
                  {" "}select
                </span>
                <span className="text-[10px] text-zinc-400">
                  <kbd className="rounded border border-zinc-200 bg-zinc-50 px-1 dark:border-zinc-700 dark:bg-zinc-800">esc</kbd>
                  {" "}close
                </span>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}
