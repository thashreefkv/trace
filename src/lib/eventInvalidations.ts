import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { queryClient } from "./queryClient";
import { qk } from "./queries";
import type { BgSource } from "./backgroundTasks";

function isTauriRuntime() {
  return Boolean((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__);
}

type BgEventPayload = {
  source: BgSource;
  kind: "started" | "finished" | "error";
  at: number;
  summary?: string;
  error?: string;
};

// Debounce invalidations per namespace — initial Gmail sync fires bg:event
// dozens of times per second; 200ms trailing window collapses bursts.
const timers = new Map<string, ReturnType<typeof setTimeout>>();
function debouncedInvalidate(namespace: string, key: readonly unknown[]) {
  const existing = timers.get(namespace);
  if (existing !== undefined) clearTimeout(existing);
  timers.set(
    namespace,
    setTimeout(() => {
      void queryClient.invalidateQueries({ queryKey: key as unknown[] });
      timers.delete(namespace);
    }, 200),
  );
}

// Called once inside AppRoutes. Maps Tauri + window events → query invalidations.
// Components keep their own listeners for toasts/notifications — this hook
// is purely the cache invalidation bridge.
export function useEventInvalidations() {
  useEffect(() => {
    if (!isTauriRuntime()) return;

    const cleanups: Array<() => void> = [];

    void listen<BgEventPayload>("bg:event", (event) => {
      const { source, kind } = event.payload;
      if (kind !== "finished") return;
      switch (source) {
        case "gmail":
          debouncedInvalidate("gmail", qk.gmail.all);
          break;
        case "drive":
          debouncedInvalidate("deliverables", qk.deliverables.all);
          break;
        case "calendar":
          debouncedInvalidate("gcal", qk.gcal.all);
          break;
        case "capture_promote":
          debouncedInvalidate("deliverables", qk.deliverables.all);
          debouncedInvalidate("stakeholders", qk.stakeholders.all);
          debouncedInvalidate("initiatives", qk.initiatives.all);
          debouncedInvalidate("captures", qk.captures.all);
          break;
      }
    }).then((fn) => cleanups.push(fn));

    void listen("gmail:new-mail", () => {
      debouncedInvalidate("gmail", qk.gmail.all);
    }).then((fn) => cleanups.push(fn));

    void listen<{ capture_id: string }>("capture:promotion_ready", () => {
      debouncedInvalidate("captures", qk.captures.all);
    }).then((fn) => cleanups.push(fn));

    function handleBoardChanged() {
      debouncedInvalidate("deliverables", qk.deliverables.all);
    }
    window.addEventListener("board-data-changed", handleBoardChanged);
    cleanups.push(() => window.removeEventListener("board-data-changed", handleBoardChanged));

    return () => cleanups.forEach((fn) => fn());
  }, []);
}
