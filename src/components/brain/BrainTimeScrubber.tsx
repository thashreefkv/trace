import { useEffect, useMemo, useRef, useState } from "react";
import { Clock, X } from "lucide-react";
import type { WorkGraph } from "../../lib/types";

interface BrainTimeScrubberProps {
  work: WorkGraph | null;
  onRangeChange: (range: { from: number; to: number } | null) => void;
  onClose: () => void;
}

interface Bucket {
  ts: number;
  count: number;
}

export function BrainTimeScrubber({ work, onRangeChange, onClose }: BrainTimeScrubberProps) {
  const buckets = useMemo(() => buildBuckets(work), [work]);
  const minTs = buckets[0]?.ts ?? 0;
  const maxTs = buckets[buckets.length - 1]?.ts ?? 0;
  const maxCount = useMemo(() => buckets.reduce((m, b) => Math.max(m, b.count), 1), [buckets]);

  // Thumbs as fractions [0..1].
  const [from, setFrom] = useState(0);
  const [to, setTo] = useState(1);
  const containerRef = useRef<HTMLDivElement>(null);
  const dragging = useRef<"from" | "to" | null>(null);

  // Commit only on mouseup; drag updates state via rAF for smoothness.
  useEffect(() => {
    if (minTs === 0 || maxTs === 0) {
      onRangeChange(null);
      return;
    }
    if (from <= 0.001 && to >= 0.999) {
      onRangeChange(null);
      return;
    }
    const span = maxTs - minTs;
    onRangeChange({ from: minTs + span * from, to: minTs + span * to });
  }, [from, to, minTs, maxTs, onRangeChange]);

  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      if (!dragging.current || !containerRef.current) return;
      const rect = containerRef.current.getBoundingClientRect();
      const ratio = clamp((event.clientX - rect.left) / rect.width, 0, 1);
      if (dragging.current === "from") {
        setFrom(Math.min(to - 0.02, ratio));
      } else {
        setTo(Math.max(from + 0.02, ratio));
      }
    };
    const onUp = () => {
      dragging.current = null;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
  }, [from, to]);

  const fromLabel = formatTs(minTs + (maxTs - minTs) * from);
  const toLabel = formatTs(minTs + (maxTs - minTs) * to);

  return (
    <section className="flex items-center gap-3 rounded-2xl border border-zinc-100 bg-white px-3 py-2.5 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
      <div className="flex shrink-0 items-center gap-1.5 text-[11px] text-zinc-500">
        <Clock aria-hidden className="text-zinc-400" size={13} />
        <span>{fromLabel}</span>
        <span className="text-zinc-300">→</span>
        <span>{toLabel}</span>
      </div>

      <div
        className="relative flex h-9 min-w-0 flex-1 items-end gap-px overflow-hidden rounded-lg bg-zinc-50 px-1"
        ref={containerRef}
      >
        {buckets.length === 0 ? (
          <div className="grid h-full w-full place-items-center text-[11px] text-zinc-400">
            No timestamped data
          </div>
        ) : (
          <>
            {buckets.map((b, idx) => {
              const ratio = idx / Math.max(1, buckets.length - 1);
              const active = ratio >= from && ratio <= to;
              return (
                <div
                  className="flex-1 transition-colors"
                  key={b.ts}
                  style={{
                    background: active ? "#0ea5e9" : "#e4e4e7",
                    height: `${Math.max(6, (b.count / maxCount) * 100)}%`,
                    opacity: active ? 0.85 : 0.45,
                  }}
                  title={`${formatTs(b.ts)} · ${b.count}`}
                />
              );
            })}
            <div
              className="pointer-events-none absolute inset-y-0"
              style={{
                left: `${from * 100}%`,
                width: `${(to - from) * 100}%`,
                background: "rgba(14,165,233,0.12)",
              }}
            />
            <button
              aria-label="Range start"
              className="absolute top-0 h-full w-2 -translate-x-1/2 cursor-ew-resize rounded-full bg-sky-500 shadow-[0_0_0_3px_rgba(14,165,233,0.18)]"
              onPointerDown={(event) => {
                event.preventDefault();
                dragging.current = "from";
              }}
              style={{ left: `${from * 100}%` }}
              type="button"
            />
            <button
              aria-label="Range end"
              className="absolute top-0 h-full w-2 -translate-x-1/2 cursor-ew-resize rounded-full bg-sky-500 shadow-[0_0_0_3px_rgba(14,165,233,0.18)]"
              onPointerDown={(event) => {
                event.preventDefault();
                dragging.current = "to";
              }}
              style={{ left: `${to * 100}%` }}
              type="button"
            />
          </>
        )}
      </div>

      <button
        className="rounded-lg border border-zinc-200 bg-white px-2 py-1 text-[11px] font-medium text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
        onClick={() => {
          setFrom(0);
          setTo(1);
        }}
        type="button"
      >
        Reset
      </button>
      <button
        aria-label="Close time scrubber"
        className="grid h-7 w-7 place-items-center rounded-lg text-zinc-400 hover:bg-zinc-50 hover:text-zinc-600"
        onClick={onClose}
        type="button"
      >
        <X size={13} />
      </button>
    </section>
  );
}

const DAY_MS = 24 * 60 * 60 * 1000;

function buildBuckets(work: WorkGraph | null): Bucket[] {
  if (!work) return [];
  const counts = new Map<number, number>();
  for (const node of work.nodes) {
    const ts = parseTs(node.updated_at);
    if (ts == null) continue;
    const day = Math.floor(ts / DAY_MS) * DAY_MS;
    counts.set(day, (counts.get(day) ?? 0) + 1);
  }
  if (counts.size === 0) return [];
  const minDay = Math.min(...counts.keys());
  const maxDay = Math.max(...counts.keys());
  const buckets: Bucket[] = [];
  for (let day = minDay; day <= maxDay; day += DAY_MS) {
    buckets.push({ ts: day, count: counts.get(day) ?? 0 });
  }
  return buckets;
}

function parseTs(raw: string | null): number | null {
  if (!raw) return null;
  const ts = Date.parse(raw);
  if (Number.isNaN(ts)) return null;
  return ts;
}

function formatTs(ts: number): string {
  if (!Number.isFinite(ts) || ts <= 0) return "—";
  const d = new Date(ts);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "2-digit" });
}

function clamp(v: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, v));
}
