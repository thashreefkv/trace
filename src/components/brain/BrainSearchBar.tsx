import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  AlertTriangle,
  CornerDownLeft,
  Loader2,
  Search,
  Sparkles,
  Terminal,
  X,
} from "lucide-react";
import { queryBrainCypher, retrieveBrainContext, retrieveMemories } from "../../lib/ipc";
import type {
  BrainContextResult,
  BrainCypherResult,
  MemoryRetrievalResult,
  WorkGraphNode,
} from "../../lib/types";
import { colorForKind, labelForKind } from "../../lib/brain/kinds";
import { BrainCypherEditor } from "./BrainCypherEditor";

type SearchMode = "nl" | "cypher";

export interface BrainSearchResult {
  hitIds: Set<string>;
  rankedNodes: WorkGraphNode[];
  summary: string | null;
  memoryHits: MemoryRetrievalResult | null;
  cypherResult: BrainCypherResult | null;
  query: string;
  mode: SearchMode;
}

interface BrainSearchBarProps {
  onResults: (result: BrainSearchResult | null) => void;
  onSelectNode?: (id: string) => void;
  totalNodes: number;
}

const NL_DEBOUNCE_MS = 220;
const ID_PATTERN = /^[a-z][a-z0-9_]*:[a-zA-Z0-9_.-]+$/;

export function BrainSearchBar({ onResults, onSelectNode, totalNodes }: BrainSearchBarProps) {
  const [mode, setMode] = useState<SearchMode>("nl");
  const [query, setQuery] = useState("");
  const [cypher, setCypher] = useState("MATCH (n)-[r]->(m)\nRETURN n, r, m\nLIMIT 50");
  const [isSearching, setIsSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeResults, setActiveResults] = useState<BrainSearchResult | null>(null);
  const [resultsOpen, setResultsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  // Toggle chip via "/cypher" command.
  useEffect(() => {
    if (mode === "nl" && query.trim().toLowerCase().startsWith("/cypher")) {
      setMode("cypher");
      setQuery("");
    }
  }, [mode, query]);

  const clearResults = useCallback(() => {
    setActiveResults(null);
    setResultsOpen(false);
    setError(null);
    onResults(null);
  }, [onResults]);

  // NL search — debounced.
  useEffect(() => {
    if (mode !== "nl") return;
    const trimmed = query.trim();
    if (!trimmed) {
      clearResults();
      return;
    }
    const handle = window.setTimeout(async () => {
      setIsSearching(true);
      setError(null);
      try {
        const [ctx, mems] = await Promise.all([
          retrieveBrainContext({ query: trimmed, limit: 32, max_hops: 1 }).catch(() => null),
          retrieveMemories({ query: trimmed, limit: 10 }).catch(() => null),
        ]);
        const result = synthesizeNlResult(trimmed, ctx, mems);
        setActiveResults(result);
        setResultsOpen(true);
        onResults(result);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
        clearResults();
      } finally {
        setIsSearching(false);
      }
    }, NL_DEBOUNCE_MS);
    return () => window.clearTimeout(handle);
  }, [mode, query, onResults, clearResults]);

  // Cypher run — only on Cmd+Enter, not debounced.
  const runCypher = useCallback(async () => {
    const trimmed = cypher.trim();
    if (!trimmed) return;
    setIsSearching(true);
    setError(null);
    try {
      const res = await queryBrainCypher({ query: trimmed, limit: 200 });
      const result = synthesizeCypherResult(trimmed, res);
      setActiveResults(result);
      setResultsOpen(true);
      onResults(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      clearResults();
    } finally {
      setIsSearching(false);
    }
  }, [cypher, onResults, clearResults]);

  // Close panel on outside click.
  useEffect(() => {
    const onPointerDown = (event: MouseEvent) => {
      if (!containerRef.current) return;
      if (containerRef.current.contains(event.target as Node)) return;
      setResultsOpen(false);
    };
    window.addEventListener("mousedown", onPointerDown);
    return () => window.removeEventListener("mousedown", onPointerDown);
  }, []);

  const placeholder = useMemo(() => {
    if (mode === "cypher") return "MATCH (n) RETURN n LIMIT 20  ·  Cmd+Enter to run";
    return `Search ${totalNodes.toLocaleString()} entities · try "/cypher" for raw queries`;
  }, [mode, totalNodes]);

  return (
    <div className="relative" ref={containerRef}>
      <div className="flex items-center gap-2 rounded-xl border border-zinc-200 bg-white px-3 py-2 transition-colors focus-within:border-sky-300 focus-within:ring-2 focus-within:ring-sky-100">
        <div className="text-zinc-400">
          {isSearching ? (
            <Loader2 className="animate-spin" size={15} />
          ) : mode === "nl" ? (
            <Search size={15} />
          ) : (
            <Terminal size={15} />
          )}
        </div>

        {mode === "nl" ? (
          <input
            className="min-w-0 flex-1 bg-transparent text-[13px] text-zinc-900 placeholder:text-zinc-400 focus:outline-none"
            data-brain-search-input="true"
            onChange={(event) => setQuery(event.currentTarget.value)}
            onFocus={() => activeResults && setResultsOpen(true)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setQuery("");
                clearResults();
              }
            }}
            placeholder={placeholder}
            value={query}
          />
        ) : (
          <div className="min-w-0 flex-1">
            <BrainCypherEditor
              onChange={setCypher}
              onRun={() => void runCypher()}
              value={cypher}
            />
            {!cypher && (
              <p className="-mt-4 text-[10.5px] text-zinc-400">{placeholder}</p>
            )}
          </div>
        )}

        <div className="flex shrink-0 items-center gap-1.5">
          {mode === "cypher" && (
            <button
              className="hidden items-center gap-1 rounded-lg border border-zinc-200 bg-zinc-50 px-2 py-1 text-[10px] font-medium text-zinc-500 transition-colors hover:border-sky-300 hover:bg-sky-50 hover:text-sky-700 md:flex"
              onClick={() => void runCypher()}
              type="button"
            >
              <CornerDownLeft size={11} /> Run
            </button>
          )}
          {(query || cypher) && (
            <button
              aria-label="Clear"
              className="grid h-6 w-6 place-items-center rounded-md text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
              onClick={() => {
                setQuery("");
                clearResults();
              }}
              type="button"
            >
              <X size={12} />
            </button>
          )}
          <button
            aria-label={mode === "nl" ? "Switch to Cypher" : "Switch to natural language"}
            className={`flex items-center gap-1 rounded-lg border px-2 py-1 text-[10px] font-medium uppercase tracking-wider transition-colors ${
              mode === "cypher"
                ? "border-violet-200 bg-violet-50 text-violet-700"
                : "border-zinc-200 bg-zinc-50 text-zinc-500 hover:border-sky-200 hover:text-sky-700"
            }`}
            onClick={() => {
              setMode((m) => (m === "nl" ? "cypher" : "nl"));
              clearResults();
            }}
            type="button"
          >
            {mode === "cypher" ? <Terminal size={11} /> : <Sparkles size={11} />}
            <span>/{mode === "cypher" ? "cypher" : "nl"}</span>
          </button>
        </div>
      </div>

      <AnimatePresence mode="wait">
        {error && (
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            className="absolute left-0 right-0 top-[calc(100%+6px)] z-30 flex items-start gap-2 rounded-xl border border-rose-100 bg-rose-50 px-3 py-2 text-[12px] text-rose-700"
            exit={{ opacity: 0, y: -4 }}
            initial={{ opacity: 0, y: -4 }}
            key="error"
          >
            <AlertTriangle className="mt-0.5 shrink-0" size={13} />
            <span className="min-w-0 flex-1 break-words">{error}</span>
          </motion.div>
        )}
        {!error && resultsOpen && activeResults && (
          <motion.div
            animate={{ opacity: 1, y: 0 }}
            className="absolute left-0 right-0 top-[calc(100%+6px)] z-30 max-h-[420px] overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_12px_40px_rgba(0,0,0,0.08)]"
            exit={{ opacity: 0, y: -4 }}
            initial={{ opacity: 0, y: -4 }}
            key="results"
          >
            <SearchResultsPanel
              onSelectNode={(id) => {
                onSelectNode?.(id);
                setResultsOpen(false);
              }}
              result={activeResults}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function SearchResultsPanel({
  result,
  onSelectNode,
}: {
  result: BrainSearchResult;
  onSelectNode: (id: string) => void;
}) {
  const showCypher = result.mode === "cypher" && result.cypherResult;
  return (
    <div className="flex max-h-[420px] flex-col">
      <header className="flex items-center justify-between border-b border-zinc-100 px-4 py-2.5">
        <div className="min-w-0">
          <p className="text-[11px] uppercase tracking-wider text-zinc-400">
            {result.mode === "nl" ? "Natural-language match" : "Cypher result"}
          </p>
          <p className="truncate text-[12px] text-zinc-600">
            {result.hitIds.size} node{result.hitIds.size === 1 ? "" : "s"} highlighted
            {result.summary ? ` · ${result.summary.slice(0, 80)}` : ""}
          </p>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto p-2">
        {result.rankedNodes.slice(0, 24).map((node) => {
          const palette = colorForKind(node.kind);
          return (
            <button
              className="flex w-full items-start gap-2 rounded-xl px-2.5 py-2 text-left transition-colors hover:bg-zinc-50"
              key={node.id}
              onClick={() => onSelectNode(node.id)}
              type="button"
            >
              <span
                aria-hidden
                className="mt-1.5 h-2 w-2 shrink-0 rounded-full"
                style={{ background: palette.stroke }}
              />
              <span className="min-w-0 flex-1">
                <span className="flex items-center gap-1.5">
                  <span className="text-[10px] uppercase tracking-wider text-zinc-400">
                    {labelForKind(node.kind)}
                  </span>
                  {node.status && (
                    <span className="rounded-md bg-zinc-100 px-1.5 py-px text-[10px] text-zinc-500">
                      {node.status}
                    </span>
                  )}
                </span>
                <span className="block truncate text-[13px] font-medium text-zinc-900">
                  {node.label}
                </span>
                {node.subtitle && (
                  <span className="block truncate text-[12px] text-zinc-500">{node.subtitle}</span>
                )}
              </span>
            </button>
          );
        })}
        {result.rankedNodes.length === 0 && !showCypher && (
          <div className="grid place-items-center py-10 text-[12px] text-zinc-400">
            No entity matches — refine the query.
          </div>
        )}

        {showCypher && result.cypherResult && (
          <div className="mt-2 rounded-xl border border-zinc-100 bg-zinc-50 p-3">
            <p className="text-[10px] uppercase tracking-wider text-zinc-400">Rows</p>
            <p className="text-[12px] text-zinc-700">
              {result.cypherResult.rows.length}
              {result.cypherResult.truncated ? " (truncated)" : ""} · columns:{" "}
              {result.cypherResult.columns.join(", ")}
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

function synthesizeNlResult(
  query: string,
  ctx: BrainContextResult | null,
  mems: MemoryRetrievalResult | null,
): BrainSearchResult {
  const hits = new Set<string>();
  const ranked: WorkGraphNode[] = [];
  if (ctx) {
    for (const node of ctx.ranked_nodes) {
      if (!hits.has(node.id)) {
        hits.add(node.id);
        ranked.push(node);
      }
    }
    for (const node of ctx.graph.nodes) {
      if (!hits.has(node.id)) hits.add(node.id);
    }
  }
  if (mems) {
    for (const memory of mems.memories) {
      hits.add(`memory:${memory.id}`);
      if (memory.source_kind && memory.source_id) {
        hits.add(`${memory.source_kind}:${memory.source_id}`);
      }
    }
  }
  return {
    hitIds: hits,
    rankedNodes: ranked,
    summary: ctx?.summary ?? null,
    memoryHits: mems,
    cypherResult: null,
    query,
    mode: "nl",
  };
}

function synthesizeCypherResult(query: string, res: BrainCypherResult): BrainSearchResult {
  const hits = new Set<string>();
  for (const row of res.rows) {
    for (const value of Object.values(row)) {
      collectIdsFromValue(value, hits);
    }
  }
  return {
    hitIds: hits,
    rankedNodes: [],
    summary: `${res.rows.length} row${res.rows.length === 1 ? "" : "s"}`,
    memoryHits: null,
    cypherResult: res,
    query,
    mode: "cypher",
  };
}

function collectIdsFromValue(value: unknown, into: Set<string>) {
  if (value == null) return;
  if (typeof value === "string") {
    if (ID_PATTERN.test(value)) into.add(value);
    return;
  }
  if (Array.isArray(value)) {
    for (const item of value) collectIdsFromValue(item, into);
    return;
  }
  if (typeof value === "object") {
    for (const item of Object.values(value as Record<string, unknown>)) {
      collectIdsFromValue(item, into);
    }
  }
}
