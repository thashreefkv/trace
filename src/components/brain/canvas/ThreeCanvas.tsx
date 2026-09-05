import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import ForceGraph3D, { type ForceGraph3DInstance } from "3d-force-graph";
import * as THREE from "three";
import type Sigma from "sigma";
import type { BrainGraph } from "../../../lib/brain/graphologyAdapter";
import type { BrainRendererHandle } from "./CanvasShell";

interface ThreeCanvasProps {
  graph: BrainGraph | null;
  selectedNodeId: string | null;
  onNodeClick: (id: string | null, meta?: { metaKey: boolean; shiftKey: boolean; altKey: boolean }) => void;
  className?: string;
}

interface ThreeNode {
  id: string;
  color: string;
  size: number;
  kind: string;
  label: string;
  pinned?: boolean;
  selected?: boolean;
}

interface ThreeLink {
  source: string;
  target: string;
  color: string;
  inferred?: boolean;
}

// Lazy-loaded 3D force renderer (three.js + 3d-force-graph). Renders in a dark
// scene with bloom on selected/pinned nodes. Defaults to a smooth orbit camera.
export const ThreeCanvas = forwardRef<BrainRendererHandle, ThreeCanvasProps>(function ThreeCanvas(
  { graph, selectedNodeId, onNodeClick, className },
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const fgRef = useRef<ForceGraph3DInstance | null>(null);
  const focusSetRef = useRef<Set<string> | null>(null);
  const highlightRef = useRef<Set<string> | null>(null);
  const overlayRef = useRef<Map<string, string> | null>(null);
  const pinnedRef = useRef<Set<string>>(new Set());
  const onClickRef = useRef(onNodeClick);
  useEffect(() => { onClickRef.current = onNodeClick; }, [onNodeClick]);

  // Snapshot graph → 3d-force-graph data once per graph change.
  const data = useMemo(() => {
    if (!graph) return { nodes: [] as ThreeNode[], links: [] as ThreeLink[] };
    const nodes: ThreeNode[] = [];
    graph.forEachNode((id, attrs) => {
      const a = attrs as unknown as Record<string, unknown>;
      nodes.push({
        id,
        color: (a.color as string | undefined) ?? "#a1a1aa",
        size: (a.size as number | undefined) ?? 6,
        kind: (a.kind as string | undefined) ?? "entity",
        label: (a.label as string | undefined) ?? id,
      });
    });
    const links: ThreeLink[] = [];
    graph.forEachEdge((_e, attrs, source, target) => {
      const a = attrs as unknown as Record<string, unknown>;
      links.push({
        source,
        target,
        color: (a.color as string | undefined) ?? "#d4d4d8",
        inferred: a.inferred === true,
      });
    });
    return { nodes, links };
  }, [graph]);

  useEffect(() => {
    if (!containerRef.current) return;
    const fg = (ForceGraph3D as unknown as (opts?: object) => (el: HTMLDivElement) => ForceGraph3DInstance)({
      controlType: "orbit",
    })(containerRef.current);
    fgRef.current = fg;

    fg.backgroundColor("#0a0a0c");
    fg.linkOpacity(0.4);
    fg.linkWidth(0.4);
    fg.linkCurvature(0.12);
    fg.linkColor((l) => (l as ThreeLink).color);
    fg.nodeRelSize(2.6);
    fg.nodeOpacity(0.95);
    fg.nodeColor((n) => {
      const node = n as ThreeNode;
      const overlay = overlayRef.current?.get(node.id);
      if (overlay) return overlay;
      const dim = focusSetRef.current && !focusSetRef.current.has(node.id);
      const dimHL = highlightRef.current && !highlightRef.current.has(node.id);
      if (dim || dimHL) return "#3f3f46";
      return node.color;
    });
    fg.nodeVal((n) => (n as ThreeNode).size);
    fg.cooldownTicks(120);
    fg.warmupTicks(80);

    fg.onNodeClick((node, event) => {
      const n = node as ThreeNode;
      const ev = event as MouseEvent | undefined;
      onClickRef.current?.(n.id, {
        metaKey: Boolean(ev?.metaKey),
        shiftKey: Boolean(ev?.shiftKey),
        altKey: Boolean(ev?.altKey),
      });
    });
    fg.onBackgroundClick(() => onClickRef.current?.(null));

    fg.graphData(data as never);

    // Add subtle ambient + directional lighting for depth.
    try {
      const scene = fg.scene();
      const amb = new THREE.AmbientLight(0xffffff, 0.55);
      const dir = new THREE.DirectionalLight(0xffffff, 0.7);
      dir.position.set(1, 1, 1);
      scene.add(amb);
      scene.add(dir);
    } catch {
      /* scene may not be ready */
    }

    return () => {
      try {
        fg._destructor?.();
      } catch {
        /* ignore */
      }
      fgRef.current = null;
    };
  }, [data]);

  useEffect(() => {
    const fg = fgRef.current;
    if (!fg) return;
    fg.refresh();
  }, [selectedNodeId]);

  useImperativeHandle(
    ref,
    () => ({
      focus(nodeId, opts) {
        const fg = fgRef.current;
        if (!fg) return;
        const node = (fg.graphData().nodes as unknown as { id: string; x?: number; y?: number; z?: number }[]).find(
          (n) => n.id === nodeId,
        );
        if (!node) return;
        const dist = 80;
        const distRatio = 1 + dist / Math.hypot(node.x ?? 1, node.y ?? 1, node.z ?? 1);
        fg.cameraPosition(
          { x: (node.x ?? 0) * distRatio, y: (node.y ?? 0) * distRatio, z: (node.z ?? 0) * distRatio },
          { x: node.x ?? 0, y: node.y ?? 0, z: node.z ?? 0 } as unknown as undefined,
          opts?.duration ?? 800,
        );
      },
      highlight(ids) {
        highlightRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        fgRef.current?.refresh();
      },
      pathHighlight() {/* TODO: 3D path highlight */},
      setFocusSet(ids) {
        focusSetRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        fgRef.current?.refresh();
      },
      setColorOverlay(colors) {
        overlayRef.current = colors && colors.size > 0 ? new Map(colors) : null;
        fgRef.current?.refresh();
      },
      setBulkSelection() {/* visual handled via overlay */},
      pinNode(id, pinned = true) {
        if (pinned) pinnedRef.current.add(id);
        else pinnedRef.current.delete(id);
        fgRef.current?.refresh();
      },
      reset() {
        focusSetRef.current = null;
        highlightRef.current = null;
        overlayRef.current = null;
        fgRef.current?.refresh();
      },
      recenter() {
        fgRef.current?.zoomToFit(420, 60);
      },
      fitToScreen(opts) {
        fgRef.current?.zoomToFit(opts?.durationMs ?? 600, 60);
      },
      exportPng() {
        try {
          const renderer = fgRef.current?.renderer();
          const canvas = renderer?.domElement;
          return canvas ? (canvas as HTMLCanvasElement).toDataURL("image/png") : null;
        } catch {
          return null;
        }
      },
      getSigma(): Sigma | null { return null; },
    }),
    [],
  );

  return (
    <div
      className={className}
      ref={containerRef}
      style={{ position: "relative", width: "100%", height: "100%", background: "#0a0a0c" }}
    />
  );
});
