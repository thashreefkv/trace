import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowRight,
  Brain,
  CalendarDays,
  FileText,
  Globe,
  Inbox,
  KanbanSquare,
  Layers3,
  Mail,
  MessageSquareText,
  Mic,
  Search,
  Sparkles,
  UsersRound,
} from "lucide-react";
import { searchAll } from "../lib/ipc";
import type { SearchResult, SearchResultKind } from "../lib/types";

type Mode = "find" | "ask";

const kindIcon: Record<SearchResultKind, ReactNode> = {
  deliverable: <KanbanSquare size={13} className="text-sky-500" />,
  initiative: <Layers3 size={13} className="text-violet-500" />,
  stakeholder: <UsersRound size={13} className="text-emerald-500" />,
  meeting: <Mic size={13} className="text-orange-400" />,
  capture: <Inbox size={13} className="text-zinc-400" />,
  email: <Mail size={13} className="text-red-400" />,
  email_thread: <Mail size={13} className="text-red-400" />,
  email_message: <Mail size={13} className="text-red-400" />,
  calendar_event: <CalendarDays size={13} className="text-orange-400" />,
  file: <FileText size={13} className="text-zinc-500" />,
  ask_turn: <Sparkles size={13} className="text-violet-500" />,
  conversation: <MessageSquareText size={13} className="text-indigo-500" />,
  memory: <Brain size={13} className="text-teal-600" />,
  web: <Globe size={13} className="text-sky-400" />,
};

const kindLabel: Record<SearchResultKind, string> = {
  deliverable: "Deliverable",
  initiative: "Initiative",
  stakeholder: "Stakeholder",
  meeting: "Meeting",
  capture: "Capture",
  email: "Email",
  email_thread: "Email thread",
  email_message: "Email message",
  calendar_event: "Calendar event",
  file: "File",
  ask_turn: "Prior answer",
  conversation: "Conversation",
  memory: "Memory",
  web: "Web",
};

const kindOrder: SearchResultKind[] = [
  "deliverable",
  "email",
  "memory",
  "initiative",
  "stakeholder",
  "meeting",
  "capture",
  "conversation",
];

export function SearchBar() {
  const navigate = useNavigate();
  const location = useLocation();
  const [mode, setMode] = useState<Mode>("find");
  const [query, setQuery] = useState("");
  const [findResults, setFindResults] = useState<SearchResult[]>([]);
  const [isOpen, setIsOpen] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (!containerRef.current?.contains(e.target as Node)) {
        setIsOpen(false);
      }
    }
    window.addEventListener("mousedown", handleClick);
    return () => window.removeEventListener("mousedown", handleClick);
  }, []);

  useEffect(() => {
    if (location.pathname === "/ask") {
      setMode("ask");
      setFindResults([]);
      setIsOpen(false);
      setError(null);
    }
  }, [location.pathname]);

  useEffect(() => {
    if (mode !== "find") {
      setFindResults([]);
      setIsOpen(false);
      setError(null);
      return;
    }

    const trimmed = query.trim();
    if (!trimmed) {
      setFindResults([]);
      setIsOpen(false);
      setError(null);
      return;
    }

    const timeout = window.setTimeout(() => {
      void runFindSearch(trimmed);
    }, 180);
    return () => window.clearTimeout(timeout);
  }, [query, mode]);

  async function runFindSearch(value: string) {
    setError(null);
    setIsLoading(true);
    setIsOpen(true);

    try {
      setFindResults(await searchAll(value));
    } catch (caught) {
      setError(String(caught));
      setFindResults([]);
    } finally {
      setIsLoading(false);
    }
  }

  function handleModeSwitch(next: Mode) {
    setMode(next);
    setFindResults([]);
    setIsOpen(false);
    setError(null);
    inputRef.current?.focus();
  }

  function openAskPage(value = query) {
    const trimmed = value.trim();
    if (!trimmed) {
      navigate("/ask");
      return;
    }
    setIsOpen(false);
    navigate(`/ask?q=${encodeURIComponent(trimmed)}`);
  }

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter" || mode !== "ask") {
      return;
    }

    event.preventDefault();
    openAskPage();
  }

  const groupedResults = findResults.reduce<Record<SearchResultKind, SearchResult[]>>(
    (acc, result) => {
      acc[result.kind] = [...(acc[result.kind] ?? []), result];
      return acc;
    },
    {} as Record<SearchResultKind, SearchResult[]>,
  );

  return (
    <div className="relative w-full max-w-xl" ref={containerRef}>
      <div className="relative flex items-center gap-0 overflow-hidden rounded-xl border border-zinc-200/80 bg-white shadow-sm transition-shadow focus-within:border-zinc-300 dark:border-zinc-700 dark:bg-zinc-900">
        <div className="flex shrink-0 items-center gap-0 border-r border-zinc-100 px-2 py-1.5 dark:border-zinc-800">
          {(["find", "ask"] as Mode[]).map((m) => (
            <button
              className={[
                "relative inline-flex items-center gap-1 rounded-lg px-2 py-1 text-[11px] font-semibold transition-colors",
                mode === m
                  ? m === "ask"
                    ? "text-violet-700 dark:text-violet-300"
                    : "text-zinc-800 dark:text-zinc-100"
                  : "text-zinc-400 hover:text-zinc-600",
              ].join(" ")}
              key={m}
              onClick={() => handleModeSwitch(m)}
              title={m === "find" ? "Find" : "Ask"}
              type="button"
            >
              {m === "find" ? <Search size={11} /> : <Sparkles size={11} />}
              {m === "find" ? "Find" : "Ask"}
              {mode === m ? (
                <motion.span
                  className={[
                    "absolute -bottom-[7px] left-1 right-1 h-[2px] rounded-full",
                    m === "ask" ? "bg-violet-500" : "bg-zinc-700",
                  ].join(" ")}
                  layoutId="searchbar-mode-indicator"
                  transition={{ type: "spring", stiffness: 500, damping: 35 }}
                />
              ) : null}
            </button>
          ))}
        </div>

        <input
          ref={inputRef}
          autoComplete="off"
          className="h-9 min-w-0 flex-1 bg-transparent px-3 text-[13px] text-zinc-900 outline-none placeholder:text-zinc-400 dark:text-zinc-100"
          onChange={(event) => setQuery(event.currentTarget.value)}
          onFocus={() => {
            if (mode === "find" && query.trim()) {
              setIsOpen(true);
            }
          }}
          onKeyDown={handleKeyDown}
          placeholder={mode === "find" ? "Search everything…" : "Ask anything about your work…"}
          spellCheck={false}
          value={query}
        />

        {mode === "ask" ? (
          <button
            className="mr-1.5 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-lg text-violet-500 transition-colors hover:bg-violet-50 disabled:text-zinc-300 disabled:hover:bg-transparent"
            disabled={!query.trim()}
            onClick={() => openAskPage()}
            title="Open Ask"
            type="button"
          >
            <ArrowRight size={14} />
          </button>
        ) : isLoading ? (
          <motion.div
            animate={{ rotate: 360 }}
            className="mr-3 shrink-0"
            transition={{ duration: 0.9, repeat: Infinity, ease: "linear" }}
          >
            <Sparkles size={13} className="text-violet-400" />
          </motion.div>
        ) : null}
      </div>

      <AnimatePresence>
        {isOpen && mode === "find" && (
          <motion.div
            animate={{ opacity: 1, y: 0, scale: 1 }}
            className="absolute z-20 mt-2 w-full overflow-hidden rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] dark:border-zinc-700 dark:bg-zinc-900"
            exit={{ opacity: 0, y: -4, scale: 0.98 }}
            initial={{ opacity: 0, y: -6, scale: 0.98 }}
            transition={{ type: "spring", stiffness: 500, damping: 36 }}
          >
            {error ? (
              <p className="px-4 py-3 text-sm text-red-500 dark:text-red-400">{error}</p>
            ) : findResults.length === 0 ? (
              <p className="px-4 py-4 text-sm text-zinc-400">No results for "{query}"</p>
            ) : (
              <div className="max-h-[400px] overflow-y-auto">
                {kindOrder.map((kind) => {
                  const group = groupedResults[kind];
                  if (!group?.length) {
                    return null;
                  }
                  return (
                    <div key={kind}>
                      <p className="sticky top-0 bg-zinc-50/90 px-4 py-1.5 text-[10px] font-bold uppercase tracking-widest text-zinc-400 backdrop-blur-sm dark:bg-zinc-800/90">
                        {kindLabel[kind]}
                      </p>
                      {group.map((result) => (
                        <Link
                          className="flex items-center gap-3 border-b border-zinc-50 px-4 py-2.5 last:border-0 hover:bg-zinc-50 dark:border-zinc-800/50 dark:hover:bg-zinc-800/40"
                          key={`${result.kind}-${result.entity_id}`}
                          onClick={() => setIsOpen(false)}
                          to={result.route}
                        >
                          <span className="shrink-0">{kindIcon[result.kind]}</span>
                          <span className="min-w-0 flex-1">
                            <span className="block truncate text-[13px] font-medium text-zinc-900 dark:text-zinc-100">
                              {result.title}
                            </span>
                            {result.subtitle ? (
                              <span className="block truncate text-[11px] text-zinc-400">
                                {result.subtitle}
                              </span>
                            ) : null}
                          </span>
                          {result.state ? (
                            <span className="shrink-0 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] text-zinc-500 dark:bg-zinc-800 dark:text-zinc-400">
                              {result.state}
                            </span>
                          ) : null}
                        </Link>
                      ))}
                    </div>
                  );
                })}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
