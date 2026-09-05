import type Sigma from "sigma";
import type Graph from "graphology";

export type FitOptions = {
  paddingRatio?: number;
  durationMs?: number;
  easing?: "linear" | "quadraticIn" | "quadraticOut" | "quadraticInOut" | "cubicIn" | "cubicOut" | "cubicInOut";
};

// Compute the bbox of nodes with finite x/y, set it as Sigma's custom bbox, and
// animate the camera to {x:0.5, y:0.5, ratio:1} so the whole graph fits with
// `paddingRatio` of empty space on each edge.
//
// Why setCustomBBox + ratio:1 instead of just ratio>1: Sigma's auto-bbox is only
// recomputed on graph mutation, not on attribute mutation (FA2 mutates attrs
// directly). After FA2 settles, Sigma's internal bbox is still the spawn-time
// box, which is why the un-fitted graph lands off-screen.
export function fitToBBox(
  sigma: Sigma,
  graph: Graph,
  options: FitOptions = {},
): boolean {
  const padding = options.paddingRatio ?? 0.08;
  const duration = options.durationMs ?? 600;
  const easing = options.easing ?? "quadraticInOut";

  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  let count = 0;
  graph.forEachNode((_, attrs) => {
    const x = (attrs as Record<string, unknown>).x;
    const y = (attrs as Record<string, unknown>).y;
    if (typeof x !== "number" || typeof y !== "number") return;
    if (!Number.isFinite(x) || !Number.isFinite(y)) return;
    if (x < minX) minX = x;
    if (y < minY) minY = y;
    if (x > maxX) maxX = x;
    if (y > maxY) maxY = y;
    count++;
  });
  if (count === 0 || !Number.isFinite(minX)) return false;

  // Guard against degenerate single-point bbox.
  const rawW = maxX - minX;
  const rawH = maxY - minY;
  const w = rawW > 1e-6 ? rawW : 1;
  const h = rawH > 1e-6 ? rawH : 1;
  const padX = w * padding;
  const padY = h * padding;

  // Sigma 3 accepts {x:[min,max], y:[min,max]} for a custom framing box.
  // The renderer normalizes node coords into this box so camera {0.5,0.5,1}
  // shows it fully.
  try {
    (sigma as unknown as { setCustomBBox: (b: { x: [number, number]; y: [number, number] }) => void }).setCustomBBox(
      { x: [minX - padX, maxX + padX], y: [minY - padY, maxY + padY] },
    );
  } catch {
    // Older Sigma versions might not expose setCustomBBox — fall back to
    // ratio-based zoom-out so we still avoid corner-stuck labels.
    const r = 1 + padding * 2;
    sigma.getCamera().animate({ x: 0.5, y: 0.5, ratio: r, angle: 0 }, { duration, easing });
    return true;
  }
  sigma.getCamera().animate(
    { x: 0.5, y: 0.5, ratio: 1, angle: 0 },
    { duration, easing },
  );
  return true;
}
