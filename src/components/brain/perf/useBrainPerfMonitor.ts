import { useEffect, useState } from "react";

export interface BrainPerfMetrics {
  fps: number;
  frames: number;
  windowSec: number;
}

// Track rolling-1s FPS via requestAnimationFrame deltas. Enable by appending
// `?debug=1` to the URL (BrainExplorer reads this and renders the overlay).
//
// We deliberately don't measure layout/label time here — those numbers are
// renderer-specific and would require hooks deep inside Sigma/Cosmos/Three. FPS
// is the headline metric the user actually feels.
export function useBrainPerfMonitor(enabled: boolean): BrainPerfMetrics {
  const [metrics, setMetrics] = useState<BrainPerfMetrics>({ fps: 0, frames: 0, windowSec: 1 });

  useEffect(() => {
    if (!enabled) return;
    let raf = 0;
    let frames = 0;
    let windowStart = performance.now();
    const tick = () => {
      frames++;
      const now = performance.now();
      const elapsed = now - windowStart;
      if (elapsed >= 1000) {
        const fps = (frames * 1000) / elapsed;
        setMetrics({ fps: Math.round(fps), frames, windowSec: elapsed / 1000 });
        frames = 0;
        windowStart = now;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [enabled]);

  return metrics;
}

export function isPerfDebugEnabled(): boolean {
  if (typeof window === "undefined") return false;
  try {
    return new URLSearchParams(window.location.search).get("debug") === "1";
  } catch {
    return false;
  }
}
