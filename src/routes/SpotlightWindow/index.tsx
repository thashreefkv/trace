import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { listen } from "@tauri-apps/api/event";
import { AnimatePresence, motion } from "framer-motion";
import { Search, Loader2, Square, Sparkles } from "lucide-react";
import {
  cancelAskRun,
  hideSpotlightWindow,
  showMainWindow,
  startAskRun,
} from "../../lib/ipc";
import { MarkdownAnswer, ReferenceDisclosure } from "../AskWorkspace/Citations";
import { ASK_HISTORY_KEY, type AskRunEventPayload, type AskTurn } from "../AskWorkspace/state";
import { ThinkingTicker } from "../AskWorkspace/ThinkingTicker";
import type { SearchResult } from "../../lib/types";

function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

function generateId() {
  return Math.random().toString(36).slice(2, 10) + Date.now().toString(36);
}

function newTurn(question: string, parentId: string | null): AskTurn {
  return {
    id: generateId(),
    parentId,
    question,
    answer: "",
    reasoning: "",
    refs: [],
    questions: [],
    steps: [],
    status: "running",
    error: null,
  };
}

function readHistory(): string[] {
  try {
    const raw = window.localStorage.getItem(ASK_HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((value): value is string => typeof value === "string") : [];
  } catch {
    return [];
  }
}

function pushHistory(question: string) {
  const trimmed = question.trim();
  if (!trimmed) return;
  try {
    const existing = readHistory().filter((entry) => entry !== trimmed);
    existing.unshift(trimmed);
    const next = existing.slice(0, 50);
    window.localStorage.setItem(ASK_HISTORY_KEY, JSON.stringify(next));
  } catch {}
}

export function SpotlightWindow() {
  const [input, setInput] = useState("");
  const [turns, setTurns] = useState<AskTurn[]>([]);
  const [activeRunId, setActiveRunId] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const runToTurnRef = useRef<Map<string, string>>(new Map());
  const historyIndexRef = useRef<number>(-1);
  const historyDraftRef = useRef<string>("");
  const answerScrollRef = useRef<HTMLDivElement>(null);

  // ── Page chrome: transparent body so only the card shows. ──────────────
  useLayoutEffect(() => {
    const html = document.documentElement;
    const body = document.body;
    const prevHtmlBg = html.style.background;
    const prevBodyBg = body.style.background;
    const prevBodyMargin = body.style.margin;
    const prevBodyOverflow = body.style.overflow;
    const prevBodyHeight = body.style.height;
    html.style.background = "transparent";
    body.style.background = "transparent";
    body.style.margin = "0";
    body.style.overflow = "hidden";
    body.style.height = "100vh";
    return () => {
      html.style.background = prevHtmlBg;
      body.style.background = prevBodyBg;
      body.style.margin = prevBodyMargin;
      body.style.overflow = prevBodyOverflow;
      body.style.height = prevBodyHeight;
    };
  }, []);

  // ── Focus input on mount and whenever the panel is re-shown. ───────────
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlistenOpening: (() => void) | undefined;
    let unlistenDismissed: (() => void) | undefined;

    void listen("spotlight:opening", () => {
      // Re-focus the input every time the user invokes the shortcut.
      inputRef.current?.focus();
      inputRef.current?.select();
    }).then((fn) => {
      unlistenOpening = fn;
    });

    void listen("spotlight:dismissed", () => {
      // Cancel any in-flight run and wipe the conversation — each spotlight
      // session is throwaway.
      void resetSession();
    }).then((fn) => {
      unlistenDismissed = fn;
    });

    return () => {
      unlistenOpening?.();
      unlistenDismissed?.();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const findTurnId = useCallback((runId: string) => {
    return runToTurnRef.current.get(runId) ?? null;
  }, []);

  const patchTurn = useCallback(
    (turnId: string, updater: (turn: AskTurn) => AskTurn) => {
      setTurns((current) =>
        current.map((turn) => (turn.id === turnId ? updater(turn) : turn)),
      );
    },
    [],
  );

  // ── Subscribe to streaming Ask events. ─────────────────────────────────
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;

    void listen<AskRunEventPayload>("ask:event", (event) => {
      const payload = event.payload;
      const turnId = findTurnId(payload.run_id);
      if (!turnId) return;

      switch (payload.kind) {
        case "started":
          patchTurn(turnId, (turn) => ({ ...turn, status: "streaming" }));
          break;
        case "text_delta":
          patchTurn(turnId, (turn) => ({
            ...turn,
            answer: turn.answer + payload.delta,
            status: "streaming",
          }));
          break;
        case "reasoning_delta":
          patchTurn(turnId, (turn) => ({
            ...turn,
            reasoning: turn.reasoning + payload.delta,
          }));
          break;
        case "done":
          patchTurn(turnId, (turn) => ({
            ...turn,
            answer: payload.result.answer,
            refs: payload.result.refs ?? [],
            questions: payload.result.questions ?? [],
            status: "done",
          }));
          runToTurnRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        case "cancelled":
          patchTurn(turnId, (turn) => ({ ...turn, status: "cancelled" }));
          runToTurnRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        case "error":
          patchTurn(turnId, (turn) => ({
            ...turn,
            status: "error",
            error: payload.message,
          }));
          runToTurnRef.current.delete(payload.run_id);
          setActiveRunId((current) => (current === payload.run_id ? null : current));
          break;
        default:
          break;
      }
    })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(() => {});

    return () => unlisten?.();
  }, [findTurnId, patchTurn]);

  // Auto-scroll answer area as content streams.
  useEffect(() => {
    const el = answerScrollRef.current;
    if (!el) return;
    el.scrollTop = el.scrollHeight;
  }, [turns]);

  const resetSession = useCallback(async () => {
    const runId = activeRunId;
    if (runId) {
      try {
        await cancelAskRun(runId);
      } catch {}
    }
    runToTurnRef.current.clear();
    setActiveRunId(null);
    setTurns([]);
    setInput("");
    setErrorMessage(null);
    historyIndexRef.current = -1;
    historyDraftRef.current = "";
  }, [activeRunId]);

  const dismiss = useCallback(async () => {
    if (!isTauriRuntime()) {
      await resetSession();
      return;
    }
    try {
      await hideSpotlightWindow();
    } catch {}
    // The Rust side emits spotlight:dismissed; our handler resets state.
  }, [resetSession]);

  const submitQuestion = useCallback(async () => {
    const question = input.trim();
    if (!question) return;
    if (activeRunId) return; // already streaming

    setErrorMessage(null);
    pushHistory(question);
    historyIndexRef.current = -1;
    historyDraftRef.current = "";

    const parentId = turns.length > 0 ? turns[turns.length - 1].id : null;
    const turn = newTurn(question, parentId);
    setTurns((current) => [...current, turn]);
    setInput("");

    try {
      const context = turns
        .map((t) => `Q: ${t.question}\nA: ${t.answer}`)
        .join("\n\n");
      const runId = await startAskRun(question, context || undefined);
      runToTurnRef.current.set(runId, turn.id);
      setActiveRunId(runId);
      setTurns((current) =>
        current.map((t) => (t.id === turn.id ? { ...t, runId } : t)),
      );
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : String(caught);
      setErrorMessage(message);
      setTurns((current) =>
        current.map((t) =>
          t.id === turn.id ? { ...t, status: "error", error: message } : t,
        ),
      );
    }
  }, [activeRunId, input, turns]);

  const stopActiveRun = useCallback(async () => {
    if (!activeRunId) return;
    try {
      await cancelAskRun(activeRunId);
    } catch {}
  }, [activeRunId]);

  const onCitationNavigate = useCallback(async (route: string) => {
    if (!isTauriRuntime()) return;
    try {
      await showMainWindow(route);
    } catch (caught) {
      setErrorMessage(
        `Couldn't open Trace: ${caught instanceof Error ? caught.message : String(caught)}`,
      );
      return;
    }
    try {
      await hideSpotlightWindow();
    } catch {
      // Best-effort: if the panel can't hide we still navigated, that's the
      // important part.
    }
  }, []);

  // ── Keyboard: Esc, history. ────────────────────────────────────────────
  function handleKey(event: ReactKeyboardEvent<HTMLInputElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (activeRunId) {
        void stopActiveRun();
        return;
      }
      void dismiss();
      return;
    }
    if (event.key === "ArrowUp" && event.currentTarget.selectionStart === 0) {
      const history = readHistory();
      if (history.length === 0) return;
      if (historyIndexRef.current === -1) {
        historyDraftRef.current = input;
      }
      const nextIndex = Math.min(historyIndexRef.current + 1, history.length - 1);
      if (nextIndex !== historyIndexRef.current) {
        historyIndexRef.current = nextIndex;
        event.preventDefault();
        setInput(history[nextIndex]);
      }
      return;
    }
    if (event.key === "ArrowDown" && historyIndexRef.current >= 0) {
      const history = readHistory();
      const nextIndex = historyIndexRef.current - 1;
      historyIndexRef.current = nextIndex;
      event.preventDefault();
      if (nextIndex === -1) {
        setInput(historyDraftRef.current);
      } else {
        setInput(history[nextIndex]);
      }
    }
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    void submitQuestion();
  }

  const hasTurns = turns.length > 0;
  const lastTurn = hasTurns ? turns[turns.length - 1] : null;
  const isStreaming = lastTurn?.status === "running" || lastTurn?.status === "streaming";

  return (
    <div className="flex h-screen w-screen items-start justify-center bg-transparent pt-3">
      <motion.div
        animate={{ opacity: 1, y: 0 }}
        className="overflow-hidden rounded-2xl bg-white/95 backdrop-blur-xl"
        initial={{ opacity: 0, y: -8 }}
        style={{
          width: 700,
          boxShadow:
            "0 24px 60px rgba(0,0,0,0.22), 0 2px 6px rgba(0,0,0,0.08)",
        }}
        transition={{ duration: 0.14, ease: "easeOut" }}
      >
        <SpotlightInput
          activeRunId={activeRunId}
          input={input}
          inputRef={inputRef}
          isStreaming={Boolean(isStreaming)}
          onChange={setInput}
          onKeyDown={handleKey}
          onStop={() => {
            void stopActiveRun();
          }}
          onSubmit={handleSubmit}
        />
        <AnimatePresence initial={false}>
          {hasTurns || errorMessage ? (
            <motion.div
              animate={{ height: "auto", opacity: 1 }}
              className="overflow-hidden"
              exit={{ height: 0, opacity: 0 }}
              initial={{ height: 0, opacity: 0 }}
              transition={{ duration: 0.18, ease: "easeOut" }}
            >
              <div className="border-t border-zinc-100" />
              <div
                className="overflow-y-auto px-4 py-3"
                ref={answerScrollRef}
                style={{ maxHeight: 420 }}
              >
                {errorMessage ? (
                  <div className="mb-3 rounded-xl border border-rose-100 bg-rose-50 px-3 py-2 text-[12px] text-rose-700">
                    {errorMessage}
                  </div>
                ) : null}
                <SpotlightTurns turns={turns} onCitationNavigate={onCitationNavigate} />
              </div>
            </motion.div>
          ) : null}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

interface SpotlightInputProps {
  activeRunId: string | null;
  input: string;
  inputRef: React.RefObject<HTMLInputElement>;
  isStreaming: boolean;
  onChange: (value: string) => void;
  onKeyDown: (event: ReactKeyboardEvent<HTMLInputElement>) => void;
  onStop: () => void;
  onSubmit: (event: FormEvent) => void;
}

function SpotlightInput({
  activeRunId,
  input,
  inputRef,
  isStreaming,
  onChange,
  onKeyDown,
  onStop,
  onSubmit,
}: SpotlightInputProps) {
  return (
    <form
      className="flex items-center gap-3 px-4 py-3"
      onSubmit={onSubmit}
    >
      <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-zinc-100">
        {isStreaming ? (
          <Loader2 size={15} className="animate-spin text-zinc-500" />
        ) : (
          <Search size={15} className="text-zinc-500" />
        )}
      </span>
      <input
        autoFocus
        className="flex-1 bg-transparent text-[20px] font-normal leading-7 text-zinc-950 placeholder:text-zinc-400 focus:outline-none"
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        placeholder="Ask Trace anything…"
        ref={inputRef}
        spellCheck={false}
        type="text"
        value={input}
      />
      {activeRunId ? (
        <button
          aria-label="Stop"
          className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-zinc-500 hover:bg-zinc-100"
          onClick={(event) => {
            event.preventDefault();
            onStop();
          }}
          type="button"
        >
          <Square size={13} className="fill-current" />
        </button>
      ) : (
        <span className="hidden shrink-0 items-center gap-1 rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500 sm:flex">
          <Sparkles size={10} />
          Trace
        </span>
      )}
    </form>
  );
}

interface SpotlightTurnsProps {
  turns: AskTurn[];
  onCitationNavigate: (route: string) => void;
}

function SpotlightTurns({ turns, onCitationNavigate }: SpotlightTurnsProps) {
  return (
    <div className="space-y-4">
      {turns.map((turn, index) => (
        <SpotlightTurn
          key={turn.id}
          turn={turn}
          showQuestion={index > 0}
          onCitationNavigate={onCitationNavigate}
        />
      ))}
    </div>
  );
}

interface SpotlightTurnProps {
  turn: AskTurn;
  showQuestion: boolean;
  onCitationNavigate: (route: string) => void;
}

function SpotlightTurn({ turn, showQuestion, onCitationNavigate }: SpotlightTurnProps) {
  const streaming = turn.status === "running" || turn.status === "streaming";
  const refs = useMemo<SearchResult[]>(() => turn.refs ?? [], [turn.refs]);

  return (
    <div className="space-y-2">
      {showQuestion ? (
        <div className="text-[12px] font-semibold uppercase tracking-wider text-zinc-400">
          {turn.question}
        </div>
      ) : null}
      {turn.status === "error" ? (
        <div className="rounded-xl border border-rose-100 bg-rose-50 px-3 py-2 text-[12px] text-rose-700">
          {turn.error ?? "Something went wrong."}
        </div>
      ) : turn.answer ? (
        <MarkdownAnswer
          content={turn.answer}
          onNavigate={onCitationNavigate}
          refs={refs}
          streaming={streaming}
        />
      ) : (
        <div className="py-1 text-zinc-700">
          <ThinkingTicker />
        </div>
      )}
      {turn.status === "done" && refs.length > 0 ? (
        <ReferenceDisclosure onNavigate={onCitationNavigate} refs={refs} />
      ) : null}
    </div>
  );
}
