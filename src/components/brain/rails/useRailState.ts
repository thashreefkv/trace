import { useCallback, useEffect, useRef, useState } from "react";

export type RailMode = "expanded" | "iconStrip" | "hidden";

export interface BrainRailState {
  leftRail: RailMode;
  rightRail: RailMode;
  isFocus: boolean;
  canvasTheme: "light" | "dark";
  setLeftRail: (m: RailMode) => void;
  setRightRail: (m: RailMode) => void;
  cycleLeftRail: () => void;
  cycleRightRail: () => void;
  setFocus: (v: boolean) => void;
  toggleFocus: () => void;
  setCanvasTheme: (t: "light" | "dark") => void;
  toggleCanvasTheme: () => void;
}

const STORAGE_KEY = "brain.rails.v2";

interface Persisted {
  leftRail: RailMode;
  rightRail: RailMode;
  canvasTheme: "light" | "dark";
}

const DEFAULTS: Persisted = {
  leftRail: "expanded",
  rightRail: "expanded",
  canvasTheme: "light",
};

function isRailMode(v: unknown): v is RailMode {
  return v === "expanded" || v === "iconStrip" || v === "hidden";
}

function readPersisted(): Persisted {
  if (typeof window === "undefined") return DEFAULTS;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULTS;
    const parsed = JSON.parse(raw) as Partial<Persisted>;
    return {
      leftRail: isRailMode(parsed.leftRail) ? parsed.leftRail : DEFAULTS.leftRail,
      rightRail: isRailMode(parsed.rightRail) ? parsed.rightRail : DEFAULTS.rightRail,
      canvasTheme: parsed.canvasTheme === "dark" ? "dark" : "light",
    };
  } catch {
    return DEFAULTS;
  }
}

function cycle(cur: RailMode): RailMode {
  if (cur === "expanded") return "iconStrip";
  if (cur === "iconStrip") return "hidden";
  return "expanded";
}

export function useRailState(): BrainRailState {
  const initial = readPersisted();
  const [leftRail, setLeftRailRaw] = useState<RailMode>(initial.leftRail);
  const [rightRail, setRightRailRaw] = useState<RailMode>(initial.rightRail);
  const [isFocus, setIsFocus] = useState(false);
  const [canvasTheme, setCanvasTheme] = useState<"light" | "dark">(initial.canvasTheme);

  // When the user enters focus mode, stash the current rail states so they can
  // be restored on exit.
  const preFocusRef = useRef<{ left: RailMode; right: RailMode } | null>(null);

  useEffect(() => {
    if (isFocus) return; // don't persist transient focus-mode rail states
    try {
      window.localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ leftRail, rightRail, canvasTheme }),
      );
    } catch {
      /* ignore storage errors (private mode, quota) */
    }
  }, [leftRail, rightRail, canvasTheme, isFocus]);

  const setLeftRail = useCallback((m: RailMode) => setLeftRailRaw(m), []);
  const setRightRail = useCallback((m: RailMode) => setRightRailRaw(m), []);
  const cycleLeftRail = useCallback(() => setLeftRailRaw((cur) => cycle(cur)), []);
  const cycleRightRail = useCallback(() => setRightRailRaw((cur) => cycle(cur)), []);

  const setFocus = useCallback((next: boolean) => {
    setIsFocus((cur) => {
      if (cur === next) return cur;
      if (next) {
        // Entering focus — stash current rail states then hide rails.
        preFocusRef.current = { left: leftRail, right: rightRail };
        setLeftRailRaw("hidden");
        setRightRailRaw("hidden");
      } else if (preFocusRef.current) {
        setLeftRailRaw(preFocusRef.current.left);
        setRightRailRaw(preFocusRef.current.right);
        preFocusRef.current = null;
      }
      return next;
    });
  }, [leftRail, rightRail]);

  const toggleFocus = useCallback(() => setFocus(!isFocus), [isFocus, setFocus]);

  const toggleCanvasTheme = useCallback(
    () => setCanvasTheme((t) => (t === "light" ? "dark" : "light")),
    [],
  );

  return {
    leftRail,
    rightRail,
    isFocus,
    canvasTheme,
    setLeftRail,
    setRightRail,
    cycleLeftRail,
    cycleRightRail,
    setFocus,
    toggleFocus,
    setCanvasTheme,
    toggleCanvasTheme,
  };
}

export const RAIL_WIDTH: Record<RailMode, number> = {
  expanded: 280,
  iconStrip: 48,
  hidden: 0,
};

export const INSPECTOR_WIDTH: Record<RailMode, number> = {
  expanded: 360,
  iconStrip: 56,
  hidden: 0,
};
