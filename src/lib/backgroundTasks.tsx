import {
  createContext,
  useContext,
  useEffect,
  useReducer,
  type ReactNode,
} from "react";
import { listen } from "@tauri-apps/api/event";

export type BgSource =
  | "gmail"
  | "drive"
  | "calendar"
  | "brain"
  | "embedding"
  | "capture_promote";
export type BgStatus = "idle" | "running" | "ok" | "error";

export interface BgTaskState {
  source: BgSource;
  status: BgStatus;
  lastStartedAt?: number;
  lastFinishedAt?: number;
  lastSummary?: string;
  lastError?: string;
  errorStreak: number;
}

interface BgEventPayload {
  source: BgSource;
  kind: "started" | "finished" | "error";
  at: number;
  summary?: string;
  error?: string;
}

export type BgTasksMap = Record<BgSource, BgTaskState>;

const initialState: BgTasksMap = {
  gmail: { source: "gmail", status: "idle", errorStreak: 0 },
  drive: { source: "drive", status: "idle", errorStreak: 0 },
  calendar: { source: "calendar", status: "idle", errorStreak: 0 },
  brain: { source: "brain", status: "idle", errorStreak: 0 },
  embedding: { source: "embedding", status: "idle", errorStreak: 0 },
  capture_promote: { source: "capture_promote", status: "idle", errorStreak: 0 },
};

const BgTasksContext = createContext<BgTasksMap>(initialState);

function reducer(state: BgTasksMap, event: BgEventPayload): BgTasksMap {
  const prev = state[event.source];
  if (!prev) return state;
  switch (event.kind) {
    case "started":
      return {
        ...state,
        [event.source]: { ...prev, status: "running", lastStartedAt: event.at },
      };
    case "finished":
      return {
        ...state,
        [event.source]: {
          ...prev,
          status: "ok",
          lastFinishedAt: event.at,
          lastSummary: event.summary,
          lastError: undefined,
          errorStreak: 0,
        },
      };
    case "error":
      return {
        ...state,
        [event.source]: {
          ...prev,
          status: "error",
          lastFinishedAt: event.at,
          lastError: event.error,
          errorStreak: prev.errorStreak + 1,
        },
      };
    default:
      return state;
  }
}

function isTauriRuntime() {
  return Boolean(
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__,
  );
}

export function BackgroundTasksProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, initialState);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    let unlisten: (() => void) | undefined;
    void listen<BgEventPayload>("bg:event", (event) => {
      dispatch(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  return (
    <BgTasksContext.Provider value={state}>{children}</BgTasksContext.Provider>
  );
}

export function useBackgroundTasks() {
  return useContext(BgTasksContext);
}
