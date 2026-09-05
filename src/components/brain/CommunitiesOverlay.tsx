import { useEffect, useRef } from "react";
import type Sigma from "sigma";
import type { GraphCommunitySummary } from "../../lib/types";

interface CommunitiesOverlayProps {
  sigma: Sigma | null;
  communities: GraphCommunitySummary[];
  visible: boolean;
}

const HUE_SEED = 31;

/**
 * Paints concave hulls behind nodes for each active GraphRAG community.
 * Sits in its own absolutely-positioned canvas so Sigma's WebGL pipeline
 * stays untouched. We re-paint on Sigma's `afterRender` event so hulls
 * track pan/zoom/layout changes in lockstep.
 */
export function CommunitiesOverlay({ sigma, communities, visible }: CommunitiesOverlayProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!sigma || !canvasRef.current) return;
    if (!visible || communities.length === 0) {
      clear(canvasRef.current);
      return;
    }
    const canvas = canvasRef.current;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const repaint = () => {
      const container = sigma.getContainer();
      const dpr = window.devicePixelRatio || 1;
      const { clientWidth: w, clientHeight: h } = container;
      if (canvas.width !== w * dpr || canvas.height !== h * dpr) {
        canvas.width = w * dpr;
        canvas.height = h * dpr;
        canvas.style.width = `${w}px`;
        canvas.style.height = `${h}px`;
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);

      for (const community of communities) {
        const points: Array<[number, number]> = [];
        for (const member of community.members) {
          const nodeId = `${member.entity_kind}:${member.entity_id}`;
          if (!sigma.getGraph().hasNode(nodeId)) continue;
          const display = sigma.getNodeDisplayData(nodeId);
          if (!display) continue;
          const viewport = sigma.graphToViewport({ x: display.x, y: display.y });
          points.push([viewport.x, viewport.y]);
        }
        if (points.length < 3) continue;
        const hue = hashHue(community.id);
        const hullPoints = paddedConvexHull(points, 18);
        if (hullPoints.length < 3) continue;

        ctx.beginPath();
        ctx.moveTo(hullPoints[0][0], hullPoints[0][1]);
        for (let i = 1; i < hullPoints.length; i += 1) {
          ctx.lineTo(hullPoints[i][0], hullPoints[i][1]);
        }
        ctx.closePath();
        ctx.fillStyle = `hsla(${hue}, 70%, 60%, 0.08)`;
        ctx.fill();
        ctx.strokeStyle = `hsla(${hue}, 70%, 45%, 0.32)`;
        ctx.lineWidth = 1.4;
        ctx.stroke();

        // Centroid label
        const centroidX = hullPoints.reduce((acc, p) => acc + p[0], 0) / hullPoints.length;
        const centroidY = hullPoints.reduce((acc, p) => acc + p[1], 0) / hullPoints.length;
        ctx.font = "600 11px ui-sans-serif, system-ui, sans-serif";
        ctx.textAlign = "center";
        ctx.fillStyle = `hsla(${hue}, 70%, 30%, 0.9)`;
        ctx.fillText(community.title, centroidX, centroidY);
      }
    };

    repaint();
    sigma.on("afterRender", repaint);
    const ro = new ResizeObserver(repaint);
    ro.observe(sigma.getContainer());
    return () => {
      sigma.removeListener("afterRender", repaint);
      ro.disconnect();
    };
  }, [sigma, communities, visible]);

  return (
    <canvas
      aria-hidden
      className="pointer-events-none absolute inset-0"
      ref={canvasRef}
      style={{ zIndex: 1 }}
    />
  );
}

function clear(canvas: HTMLCanvasElement) {
  const ctx = canvas.getContext("2d");
  ctx?.clearRect(0, 0, canvas.width, canvas.height);
}

function hashHue(id: string): number {
  let h = 0;
  for (let i = 0; i < id.length; i += 1) {
    h = (h * HUE_SEED + id.charCodeAt(i)) >>> 0;
  }
  return h % 360;
}

type Point = [number, number];

/**
 * Andrew's monotone-chain hull with a small outward pad. Keeping this tiny
 * routine local avoids pulling an abandoned geometry package into the app.
 */
function paddedConvexHull(points: Point[], padding: number): Point[] {
  const unique = Array.from(new Map(points.map((point) => [`${point[0]}:${point[1]}`, point])).values());
  if (unique.length < 3) return unique;

  const sorted = [...unique].sort(([ax, ay], [bx, by]) => ax - bx || ay - by);
  const cross = (origin: Point, a: Point, b: Point) =>
    (a[0] - origin[0]) * (b[1] - origin[1]) - (a[1] - origin[1]) * (b[0] - origin[0]);

  const lower: Point[] = [];
  for (const point of sorted) {
    while (lower.length >= 2 && cross(lower[lower.length - 2], lower[lower.length - 1], point) <= 0) {
      lower.pop();
    }
    lower.push(point);
  }

  const upper: Point[] = [];
  for (let index = sorted.length - 1; index >= 0; index -= 1) {
    const point = sorted[index];
    while (upper.length >= 2 && cross(upper[upper.length - 2], upper[upper.length - 1], point) <= 0) {
      upper.pop();
    }
    upper.push(point);
  }

  const outline = lower.slice(0, -1).concat(upper.slice(0, -1));
  const center = outline.reduce<Point>(
    ([x, y], point) => [x + point[0] / outline.length, y + point[1] / outline.length],
    [0, 0],
  );

  return outline.map(([x, y]) => {
    const dx = x - center[0];
    const dy = y - center[1];
    const distance = Math.hypot(dx, dy) || 1;
    return [x + (dx / distance) * padding, y + (dy / distance) * padding];
  });
}
