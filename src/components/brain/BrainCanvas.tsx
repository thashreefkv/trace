import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import Sigma from "sigma";
import FA2Layout from "graphology-layout-forceatlas2/worker";
import forceAtlas2 from "graphology-layout-forceatlas2";
import type Graph from "graphology";
import type { BrainEdgeAttributes, BrainGraph, BrainNodeAttributes } from "../../lib/brain/graphologyAdapter";
import { fitToBBox } from "./canvas/useFitToBBox";

export interface BrainCanvasHandle {
  focus: (nodeId: string, opts?: { ratio?: number; duration?: number }) => void;
  highlight: (ids: Set<string> | null) => void;
  pathHighlight: (nodeIds: Set<string> | null, edgeIds: Set<string> | null) => void;
  setFocusSet: (ids: Set<string> | null) => void;
  setColorOverlay: (colors: Map<string, string> | null) => void;
  setBulkSelection: (ids: Set<string> | null) => void;
  pinNode: (nodeId: string, pinned?: boolean) => void;
  reset: () => void;
  recenter: () => void;
  fitToScreen: (opts?: { paddingRatio?: number; durationMs?: number }) => void;
  exportPng: () => string | null;
  getSigma: () => Sigma | null;
}

export interface BrainNodeClickMeta {
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export interface BrainEdgeClickInfo {
  edgeId: string;
  clientX: number;
  clientY: number;
}

interface BrainCanvasProps {
  graph: BrainGraph | null;
  selectedNodeId: string | null;
  onNodeClick: (id: string | null, meta?: BrainNodeClickMeta) => void;
  onEdgeClick?: (info: BrainEdgeClickInfo | null) => void;
  className?: string;
  /**
   * When false, skip the FA2 worker entirely. Hierarchy/radial layouts already
   * pass through `tweenToPositions` from `lib/brain/layouts.ts`, so we don't
   * want physics fighting the tween. Defaults to true.
   */
  runPhysics?: boolean;
}

const DIM_COLOR = "rgba(228, 228, 231, 0.55)";
const DIM_LABEL_COLOR = "rgba(161, 161, 170, 0.6)";
const EDGE_DIM_COLOR = "rgba(228, 228, 231, 0.35)";

export const BrainCanvas = forwardRef<BrainCanvasHandle, BrainCanvasProps>(function BrainCanvas(
  { graph, selectedNodeId, onNodeClick, onEdgeClick, className, runPhysics = true },
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const layoutRef = useRef<FA2Layout | null>(null);
  const highlightRef = useRef<Set<string> | null>(null);
  const pathNodeRef = useRef<Set<string> | null>(null);
  const pathEdgeRef = useRef<Set<string> | null>(null);
  const focusSetRef = useRef<Set<string> | null>(null);
  const colorOverlayRef = useRef<Map<string, string> | null>(null);
  const bulkSelectionRef = useRef<Set<string> | null>(null);
  const selectedRef = useRef<string | null>(null);
  const graphRef = useRef<BrainGraph | null>(null);
  const onNodeClickRef = useRef(onNodeClick);
  const onEdgeClickRef = useRef(onEdgeClick);

  // Keep callback refs current without remounting Sigma.
  useEffect(() => {
    onNodeClickRef.current = onNodeClick;
  }, [onNodeClick]);
  useEffect(() => {
    onEdgeClickRef.current = onEdgeClick;
  }, [onEdgeClick]);

  // Compute reasonable FA2 settings once per graph. Guarded against empty
  // graphs (some FA2 builds divide by `graph.order` and blow up at 0).
  const fa2Settings = useMemo(() => {
    if (!graph || graph.order === 0) return null;
    try {
      return forceAtlas2.inferSettings(graph as unknown as Graph);
    } catch (error) {
      if (typeof console !== "undefined") {
        console.warn("[brain] inferSettings failed; using FA2 defaults", error);
      }
      return { slowDown: 6, gravity: 1, scalingRatio: 8, barnesHutOptimize: true };
    }
  }, [graph]);

  useEffect(() => {
    if (!containerRef.current) return;
    if (!graph || graph.order === 0) return;

    graphRef.current = graph;
    // Tear down any previous instance before mounting a new one.
    try {
      layoutRef.current?.kill();
    } catch (error) {
      console.warn("[brain] FA2 kill failed", error);
    }
    layoutRef.current = null;
    try {
      sigmaRef.current?.kill();
    } catch (error) {
      console.warn("[brain] sigma kill failed", error);
    }
    sigmaRef.current = null;

    let sigma: Sigma;
    try {
      sigma = new Sigma(graph as unknown as Graph, containerRef.current, {
        renderEdgeLabels: false,
        defaultNodeType: "circle",
        defaultEdgeType: "arrow",
        minCameraRatio: 0.05,
        maxCameraRatio: 12,
        labelDensity: 0.7,
        labelGridCellSize: 80,
        labelRenderedSizeThreshold: 6,
        labelColor: { color: "#27272a" },
        labelWeight: "500",
        labelFont: "ui-sans-serif, system-ui, sans-serif",
        zIndex: true,
      });
    } catch (error) {
      console.error("[brain] Sigma init failed", error);
      return;
    }

    // Node visual reducer — applies overlays / focus / dim / path / select.
    const nodeReducer = (nodeId: string, data: Record<string, unknown>) => {
      const attrs = data as Partial<BrainNodeAttributes> & Record<string, unknown>;
      const isSelected = nodeId === selectedRef.current;
      const highlight = highlightRef.current;
      const focusSet = focusSetRef.current;
      const overlay = colorOverlayRef.current;
      const bulk = bulkSelectionRef.current;
      const pathNodes = pathNodeRef.current;
      const onPath = pathNodes ? pathNodes.has(nodeId) : false;
      const inFocus = focusSet ? focusSet.has(nodeId) : true;
      const dimmedByHighlight = highlight ? !highlight.has(nodeId) : false;
      const next: Record<string, unknown> = { ...attrs };

      // Color overlay takes precedence over the baked-in kind color.
      if (overlay && overlay.has(nodeId)) {
        next.color = overlay.get(nodeId);
      }

      if (!inFocus) {
        next.color = DIM_COLOR;
        next.borderColor = DIM_COLOR;
        next.label = "";
        next.zIndex = 0;
      } else if (dimmedByHighlight && !onPath) {
        next.color = DIM_COLOR;
        next.borderColor = DIM_COLOR;
        next.label = "";
        next.zIndex = 0;
      } else if (onPath) {
        next.color = "#7c3aed";
        next.zIndex = 4;
      } else {
        next.zIndex = isSelected ? 3 : 1;
      }
      if (isSelected) {
        next.highlighted = true;
        next.size = Math.max((attrs.size as number) ?? 6, 12) * 1.4;
      }
      if (bulk && bulk.has(nodeId)) {
        next.borderColor = "#0ea5e9";
        next.size = Math.max((attrs.size as number) ?? 6, 8) * 1.2;
        next.zIndex = 5;
      }
      if (attrs.pinned) {
        next.type = "circle";
      }
      return next;
    };

    const edgeReducer = (edgeId: string, data: Record<string, unknown>) => {
      const attrs = data as Partial<BrainEdgeAttributes> & Record<string, unknown>;
      const highlight = highlightRef.current;
      const focusSet = focusSetRef.current;
      const pathEdges = pathEdgeRef.current;
      const onPath = pathEdges ? pathEdges.has(edgeId) : false;
      const next: Record<string, unknown> = { ...attrs };
      if (onPath) {
        next.color = "#7c3aed";
        next.size = Math.max((attrs.size as number) ?? 1, 2.4);
        next.zIndex = 5;
        return next;
      }
      if (focusSet) {
        const g = graphRef.current;
        const source = g?.source(edgeId);
        const target = g?.target(edgeId);
        if (!source || !target || !focusSet.has(source) || !focusSet.has(target)) {
          next.color = EDGE_DIM_COLOR;
          next.size = 0.3;
          return next;
        }
      }
      if (highlight) {
        const g = graphRef.current;
        const source = g?.source(edgeId);
        const target = g?.target(edgeId);
        const sourceIn = source ? highlight.has(source) : false;
        const targetIn = target ? highlight.has(target) : false;
        if (!sourceIn && !targetIn) {
          next.color = EDGE_DIM_COLOR;
          next.size = 0.4;
          next.hidden = false;
          return next;
        }
      }
      if (attrs.inferred && attrs.inferredStatus === "pending") {
        next.color = "#f59e0b";
        next.size = Math.max((attrs.size as number) ?? 1, 1.4);
      }
      return next;
    };

    // Sigma's typings are heavily generic; cast both reducers to keep call signatures simple.
    sigma.setSetting("nodeReducer", nodeReducer as never);
    sigma.setSetting("edgeReducer", edgeReducer as never);

    sigma.setSetting("labelColor", { color: "#27272a" });

    // Click handlers — capture native keyboard modifiers via the underlying
    // DOM event so the explorer can drive Cmd-click path-finding.
    sigma.on("clickNode", ({ node, event }) => {
      const original = (event?.original ?? event) as MouseEvent | undefined;
      onNodeClickRef.current?.(node, {
        metaKey: Boolean(original?.metaKey),
        shiftKey: Boolean(original?.shiftKey),
        altKey: Boolean(original?.altKey),
      });
    });
    sigma.on("clickStage", () => {
      onNodeClickRef.current?.(null);
    });
    sigma.on("clickEdge", ({ edge, event }) => {
      const original = (event?.original ?? event) as MouseEvent | undefined;
      onEdgeClickRef.current?.({
        edgeId: edge,
        clientX: original?.clientX ?? 0,
        clientY: original?.clientY ?? 0,
      });
    });

    sigmaRef.current = sigma;

    // Frame the graph as soon as Sigma mounts (covers cached/static-position
    // cases) and again whenever physics settles. Without these calls the camera
    // sits at {0.5,0.5,1} of the spawn-time bbox, so once FA2 expands the
    // graph nearly everything ends up off-screen — the bottom-right corner
    // label cluster the user saw.
    const initialFitTimer = window.setTimeout(() => {
      fitToBBox(sigma, graph as unknown as Graph, { durationMs: 0 });
    }, 0);

    // Worker-based layout: run for a few seconds on mount, then stop.
    // Skipped when the caller has its own positions (hierarchy / radial / cache).
    if (runPhysics && fa2Settings) {
      const layout = new FA2Layout(graph as unknown as Graph, {
        settings: { ...fa2Settings, slowDown: 5, gravity: 1.1 },
      });
      layoutRef.current = layout;
      layout.start();
      const timer = window.setTimeout(() => {
        layout.stop();
        fitToBBox(sigma, graph as unknown as Graph, { durationMs: 600 });
      }, 2800);
      return () => {
        window.clearTimeout(timer);
        window.clearTimeout(initialFitTimer);
        layout.kill();
        layoutRef.current = null;
        sigma.kill();
        sigmaRef.current = null;
      };
    }

    return () => {
      window.clearTimeout(initialFitTimer);
      sigma.kill();
      sigmaRef.current = null;
    };
  }, [graph, fa2Settings, runPhysics]);

  // Reflect external selection changes into the reducer via a refresh.
  useEffect(() => {
    selectedRef.current = selectedNodeId;
    sigmaRef.current?.refresh();
  }, [selectedNodeId]);

  useImperativeHandle(
    ref,
    () => ({
      focus(nodeId, opts) {
        const sigma = sigmaRef.current;
        const g = graphRef.current;
        if (!sigma || !g || !g.hasNode(nodeId)) return;
        const attrs = g.getNodeAttributes(nodeId);
        sigma.getCamera().animate(
          { x: attrs.x as number, y: attrs.y as number, ratio: opts?.ratio ?? 0.45 },
          { duration: opts?.duration ?? 480 },
        );
      },
      highlight(ids) {
        highlightRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        sigmaRef.current?.refresh();
      },
      pathHighlight(nodeIds, edgeIds) {
        pathNodeRef.current = nodeIds && nodeIds.size > 0 ? new Set(nodeIds) : null;
        pathEdgeRef.current = edgeIds && edgeIds.size > 0 ? new Set(edgeIds) : null;
        sigmaRef.current?.refresh();
      },
      setFocusSet(ids) {
        focusSetRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        sigmaRef.current?.refresh();
      },
      setColorOverlay(colors) {
        colorOverlayRef.current = colors && colors.size > 0 ? new Map(colors) : null;
        sigmaRef.current?.refresh();
      },
      setBulkSelection(ids) {
        bulkSelectionRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        sigmaRef.current?.refresh();
      },
      pinNode(nodeId, pinned = true) {
        const g = graphRef.current;
        if (!g || !g.hasNode(nodeId)) return;
        g.setNodeAttribute(nodeId, "pinned", pinned);
        sigmaRef.current?.refresh();
      },
      reset() {
        highlightRef.current = null;
        selectedRef.current = null;
        sigmaRef.current?.refresh();
      },
      recenter() {
        const sigma = sigmaRef.current;
        const g = graphRef.current;
        if (!sigma || !g) return;
        fitToBBox(sigma, g as unknown as Graph, { durationMs: 420 });
      },
      fitToScreen(opts) {
        const sigma = sigmaRef.current;
        const g = graphRef.current;
        if (!sigma || !g) return;
        fitToBBox(sigma, g as unknown as Graph, {
          paddingRatio: opts?.paddingRatio,
          durationMs: opts?.durationMs ?? 600,
        });
      },
      exportPng() {
        const sigma = sigmaRef.current;
        if (!sigma) return null;
        const canvas = sigma.getCanvases().nodes;
        return canvas ? canvas.toDataURL("image/png") : null;
      },
      getSigma() {
        return sigmaRef.current;
      },
    }),
    [],
  );

  // Suppress dim label color via inline style so the label color setting wins.
  void DIM_LABEL_COLOR;

  return <div ref={containerRef} className={className} />;
});
