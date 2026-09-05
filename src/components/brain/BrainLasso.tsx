import { useEffect, useRef } from "react";
import pointInPolygon from "point-in-polygon";
import type Sigma from "sigma";

interface BrainLassoProps {
  sigma: Sigma | null;
  enabled: boolean;
  onSelect: (nodeIds: string[]) => void;
}

/**
 * Shift-drag a polygonal lasso across the Sigma canvas to multi-select
 * nodes. Renders as an SVG overlay so it never participates in WebGL repaint
 * cycles, and clears itself on mouse-up. The actual nodes-inside-polygon
 * test runs once on release (point-in-polygon over Sigma's viewport coords).
 */
export function BrainLasso({ sigma, enabled, onSelect }: BrainLassoProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const pointsRef = useRef<Array<[number, number]>>([]);
  const draggingRef = useRef(false);
  const polyRef = useRef<SVGPolylineElement | null>(null);

  useEffect(() => {
    if (!enabled || !sigma) return;
    const container = sigma.getContainer();
    const svg = svgRef.current;
    if (!svg) return;

    const onPointerDown = (event: PointerEvent) => {
      if (!event.shiftKey) return;
      event.preventDefault();
      draggingRef.current = true;
      const rect = container.getBoundingClientRect();
      pointsRef.current = [[event.clientX - rect.left, event.clientY - rect.top]];
      updatePolyline();
    };

    const onPointerMove = (event: PointerEvent) => {
      if (!draggingRef.current) return;
      const rect = container.getBoundingClientRect();
      const last = pointsRef.current[pointsRef.current.length - 1];
      const next: [number, number] = [event.clientX - rect.left, event.clientY - rect.top];
      // Skip near-duplicate points to keep the polyline cheap.
      if (last && Math.hypot(next[0] - last[0], next[1] - last[1]) < 4) return;
      pointsRef.current.push(next);
      updatePolyline();
    };

    const onPointerUp = () => {
      if (!draggingRef.current) return;
      draggingRef.current = false;
      const poly = pointsRef.current;
      pointsRef.current = [];
      updatePolyline();
      if (poly.length < 3) return;
      // Resolve nodes whose on-screen position lies inside the polygon.
      const hits: string[] = [];
      sigma.getGraph().forEachNode((id, attrs) => {
        const pos = sigma.graphToViewport({ x: attrs.x as number, y: attrs.y as number });
        if (pointInPolygon([pos.x, pos.y], poly)) hits.push(id);
      });
      onSelect(hits);
    };

    const updatePolyline = () => {
      if (!polyRef.current) return;
      polyRef.current.setAttribute(
        "points",
        pointsRef.current.map((p) => `${p[0]},${p[1]}`).join(" "),
      );
    };

    container.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    return () => {
      container.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
    };
  }, [enabled, sigma, onSelect]);

  if (!enabled) return null;
  return (
    <svg
      aria-hidden
      className="pointer-events-none absolute inset-0"
      ref={svgRef}
      style={{ zIndex: 4 }}
    >
      <polyline
        fill="rgba(14, 165, 233, 0.10)"
        ref={polyRef}
        stroke="#0ea5e9"
        strokeDasharray="6 4"
        strokeWidth={1.5}
      />
    </svg>
  );
}
