import { useEffect, useRef } from "react";
import type Sigma from "sigma";
import type { BrainGraph } from "../../../lib/brain/graphologyAdapter";

interface MinimapProps {
  graph: BrainGraph | null;
  sigma: Sigma | null;
  // Set to true to hide the minimap (e.g. in focus mode if you want).
  hidden?: boolean;
  darkBackground?: boolean;
}

// Tiny overview map in the bottom-right corner. Draws all node positions as
// dots and overlays the current viewport rectangle. Click to recenter the
// camera at that location. Works only when a Sigma instance is provided; in
// Cosmos / Three modes we render nothing (each renderer has its own overview).
export function Minimap({ graph, sigma, hidden, darkBackground }: MinimapProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas || !graph || !sigma) return;
    if (graph.order === 0) return;
    const W = 160;
    const H = 100;
    canvas.width = W * window.devicePixelRatio;
    canvas.height = H * window.devicePixelRatio;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(window.devicePixelRatio, window.devicePixelRatio);

    let frame = 0;
    const draw = () => {
      ctx.clearRect(0, 0, W, H);
      ctx.fillStyle = darkBackground ? "#0a0a0c" : "rgba(244,244,245,0.9)";
      ctx.fillRect(0, 0, W, H);

      // Compute graph bbox each frame (cheap at 5k nodes).
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      graph.forEachNode((_, attrs) => {
        const x = (attrs as unknown as Record<string, unknown>).x as number;
        const y = (attrs as unknown as Record<string, unknown>).y as number;
        if (!Number.isFinite(x) || !Number.isFinite(y)) return;
        if (x < minX) minX = x;
        if (y < minY) minY = y;
        if (x > maxX) maxX = x;
        if (y > maxY) maxY = y;
      });
      if (!Number.isFinite(minX)) {
        frame = requestAnimationFrame(draw);
        return;
      }
      const w = Math.max(1, maxX - minX);
      const h = Math.max(1, maxY - minY);
      const sx = W / w;
      const sy = H / h;
      const s = Math.min(sx, sy) * 0.85;
      const ox = (W - w * s) / 2;
      const oy = (H - h * s) / 2;

      ctx.fillStyle = darkBackground ? "rgba(228,228,231,0.4)" : "rgba(82,82,91,0.4)";
      graph.forEachNode((_, attrs) => {
        const x = (attrs as unknown as Record<string, unknown>).x as number;
        const y = (attrs as unknown as Record<string, unknown>).y as number;
        if (!Number.isFinite(x) || !Number.isFinite(y)) return;
        const px = ox + (x - minX) * s;
        const py = oy + (y - minY) * s;
        ctx.fillRect(px, py, 1.2, 1.2);
      });

      // Viewport rect. Sigma camera state is in normalized framed coords; we
      // approximate by mapping camera (x,y,ratio) to the bbox.
      try {
        const cam = sigma.getCamera().getState();
        // The bbox we just measured is what Sigma also normalizes to (because
        // we set it as custom bbox in `fitToBBox`). camera.x/y in [0,1] of that
        // bbox; camera.ratio = 1 means full bbox visible.
        const vw = w * cam.ratio;
        const vh = h * cam.ratio * (H / W);
        const cx = minX + cam.x * w;
        const cy = minY + cam.y * h;
        const rx = ox + (cx - minX - vw / 2) * s;
        const ry = oy + (cy - minY - vh / 2) * s;
        const rw = vw * s;
        const rh = vh * s;
        ctx.strokeStyle = "#0ea5e9";
        ctx.lineWidth = 1.2;
        ctx.strokeRect(rx, ry, rw, rh);
      } catch {/* camera not ready */}

      frame = requestAnimationFrame(draw);
    };
    frame = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(frame);
  }, [graph, sigma, darkBackground]);

  if (hidden || !sigma) return null;
  return (
    <div className="pointer-events-none absolute bottom-3 right-16 z-10 rounded-xl border border-zinc-100 bg-white/80 p-1 shadow-[0_2px_12px_rgba(0,0,0,0.06)] backdrop-blur">
      <canvas
        className="block rounded-lg"
        ref={canvasRef}
        style={{ width: 160, height: 100 }}
      />
    </div>
  );
}
