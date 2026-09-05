import { forwardRef, useEffect, useImperativeHandle, useMemo, useRef } from "react";
import { Graph as CosmosGraph } from "@cosmos.gl/graph";
import type { BrainGraph } from "../../../lib/brain/graphologyAdapter";
import { applyColorOverlay, cosmosFromGraphology } from "./cosmosAdapter";

export interface CosmosCanvasHandle {
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
}

export interface CosmosNodeClickMeta {
  metaKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

interface CosmosCanvasProps {
  graph: BrainGraph | null;
  selectedNodeId: string | null;
  onNodeClick: (id: string | null, meta?: CosmosNodeClickMeta) => void;
  className?: string;
  darkBackground?: boolean;
}

// Cosmos 2D GPU force renderer. Sub-millisecond per frame at 5k nodes.
// Owns its own canvas; refs forward a small handle that maps id-keyed APIs
// (focus, highlight, overlay) to Cosmos' index-keyed buffers.
export const CosmosCanvas = forwardRef<CosmosCanvasHandle, CosmosCanvasProps>(function CosmosCanvas(
  { graph, selectedNodeId, onNodeClick, className, darkBackground },
  ref,
) {
  const containerRef = useRef<HTMLDivElement>(null);
  const cosmosRef = useRef<CosmosGraph | null>(null);
  const baseColorsRef = useRef<Float32Array | null>(null);
  const idToIndexRef = useRef<Map<string, number> | null>(null);
  const indexToIdRef = useRef<string[] | null>(null);
  const overlayRef = useRef<Map<string, string> | null>(null);
  const highlightRef = useRef<Set<string> | null>(null);
  const focusSetRef = useRef<Set<string> | null>(null);
  const bulkRef = useRef<Set<string> | null>(null);
  const onNodeClickRef = useRef(onNodeClick);

  useEffect(() => {
    onNodeClickRef.current = onNodeClick;
  }, [onNodeClick]);

  const buffers = useMemo(() => (graph ? cosmosFromGraphology(graph) : null), [graph]);

  useEffect(() => {
    if (!containerRef.current || !buffers) return;
    let cosmos: CosmosGraph;
    try {
      cosmos = new CosmosGraph(containerRef.current, {
        backgroundColor: darkBackground ? "#0a0a0c" : "#fafafa",
        simulationGravity: 0.25,
        simulationRepulsion: 1.0,
        simulationRepulsionTheta: 1.15,
        simulationLinkSpring: 1.0,
        simulationLinkDistance: 8,
        simulationFriction: 0.85,
        simulationDecay: 1000,
        spaceSize: 4096,
        scalePointsOnZoom: true,
        renderLinks: true,
        curvedLinks: true,
        pointSizeScale: 1,
        linkWidthScale: 1,
        fitViewOnInit: true,
        fitViewDelay: 200,
        fitViewPadding: 0.08,
        rescalePositions: false,
        onClick: (index, _pointPosition, _event) => {
          if (index == null) {
            onNodeClickRef.current?.(null);
            return;
          }
          const id = indexToIdRef.current?.[index];
          if (!id) return;
          const native = (_event as MouseEvent | undefined);
          onNodeClickRef.current?.(id, {
            metaKey: Boolean(native?.metaKey),
            shiftKey: Boolean(native?.shiftKey),
            altKey: Boolean(native?.altKey),
          });
        },
      });
    } catch (err) {
      console.error("[brain] Cosmos init failed", err);
      return;
    }

    cosmosRef.current = cosmos;
    baseColorsRef.current = new Float32Array(buffers.colors);
    idToIndexRef.current = buffers.idToIndex;
    indexToIdRef.current = buffers.indexToId;

    cosmos.setPointPositions(buffers.positions);
    cosmos.setPointColors(buffers.colors);
    cosmos.setPointSizes(buffers.sizes);
    cosmos.setLinks(buffers.links);
    cosmos.setLinkColors(buffers.linkColors);
    cosmos.setLinkWidths(buffers.linkWidths);
    cosmos.render(1);
    cosmos.start();

    // After the initial simulation settles, freeze positions and re-fit.
    const stopTimer = window.setTimeout(() => {
      try {
        cosmos.pause();
        cosmos.fitView(600, 0.08);
      } catch {/* graph may have unmounted */}
    }, 1800);

    return () => {
      window.clearTimeout(stopTimer);
      try {
        cosmos.destroy?.();
      } catch (err) {
        console.warn("[brain] Cosmos destroy failed", err);
      }
      cosmosRef.current = null;
      baseColorsRef.current = null;
      idToIndexRef.current = null;
      indexToIdRef.current = null;
    };
  }, [buffers, darkBackground]);

  // Recompose colours when overlay / focus / highlight changes.
  useEffect(() => {
    const cosmos = cosmosRef.current;
    const base = baseColorsRef.current;
    const idMap = idToIndexRef.current;
    if (!cosmos || !base || !idMap) return;
    const next = new Float32Array(base);
    applyColorOverlay(next, idMap, overlayRef.current);
    const dimAlpha = 0.18;
    const highlight = highlightRef.current;
    const focus = focusSetRef.current;
    if (highlight || focus) {
      for (let i = 0; i < next.length; i += 4) {
        const id = indexToIdRef.current?.[i / 4];
        if (!id) continue;
        const keepFocus = focus ? focus.has(id) : true;
        const keepHighlight = highlight ? highlight.has(id) : true;
        if (!keepFocus || !keepHighlight) {
          next[i + 3] = dimAlpha;
        }
      }
    }
    cosmos.setPointColors(next);
    // Selected / bulk emphasis: bump alpha on selected, render a brighter point.
    if (selectedNodeId) {
      const idx = idMap.get(selectedNodeId);
      if (idx != null) {
        next[idx * 4 + 3] = 1;
      }
    }
  }, [selectedNodeId]);

  useImperativeHandle(
    ref,
    () => ({
      focus(nodeId, opts) {
        const cosmos = cosmosRef.current;
        const idx = idToIndexRef.current?.get(nodeId);
        if (!cosmos || idx == null) return;
        cosmos.zoomToPointByIndex(idx, opts?.duration ?? 480, opts?.ratio != null ? Math.max(1.5, 5 - opts.ratio * 8) : 3);
      },
      highlight(ids) {
        highlightRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        nudgeColors(cosmosRef.current, baseColorsRef.current, idToIndexRef.current, indexToIdRef.current, overlayRef.current, highlightRef.current, focusSetRef.current, bulkRef.current);
      },
      pathHighlight() {
        // Path highlighting on Cosmos requires link-color mutation; deferred.
      },
      setFocusSet(ids) {
        focusSetRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        nudgeColors(cosmosRef.current, baseColorsRef.current, idToIndexRef.current, indexToIdRef.current, overlayRef.current, highlightRef.current, focusSetRef.current, bulkRef.current);
      },
      setColorOverlay(colors) {
        overlayRef.current = colors && colors.size > 0 ? new Map(colors) : null;
        nudgeColors(cosmosRef.current, baseColorsRef.current, idToIndexRef.current, indexToIdRef.current, overlayRef.current, highlightRef.current, focusSetRef.current, bulkRef.current);
      },
      setBulkSelection(ids) {
        bulkRef.current = ids && ids.size > 0 ? new Set(ids) : null;
        nudgeColors(cosmosRef.current, baseColorsRef.current, idToIndexRef.current, indexToIdRef.current, overlayRef.current, highlightRef.current, focusSetRef.current, bulkRef.current);
      },
      pinNode(nodeId, pinned = true) {
        const cosmos = cosmosRef.current;
        const idx = idToIndexRef.current?.get(nodeId);
        if (!cosmos || idx == null) return;
        try {
          cosmos.setPinnedPoints(pinned ? [idx] : []);
        } catch {/* older Cosmos versions */}
      },
      reset() {
        highlightRef.current = null;
        focusSetRef.current = null;
        bulkRef.current = null;
        nudgeColors(cosmosRef.current, baseColorsRef.current, idToIndexRef.current, indexToIdRef.current, null, null, null, null);
      },
      recenter() {
        cosmosRef.current?.fitView(420, 0.08);
      },
      fitToScreen(opts) {
        cosmosRef.current?.fitView(opts?.durationMs ?? 600, opts?.paddingRatio ?? 0.08);
      },
      exportPng() {
        const canvas = containerRef.current?.querySelector("canvas");
        if (!canvas) return null;
        try {
          return (canvas as HTMLCanvasElement).toDataURL("image/png");
        } catch {
          return null;
        }
      },
    }),
    [],
  );

  return (
    <div
      className={className}
      ref={containerRef}
      style={{ position: "relative", width: "100%", height: "100%" }}
    />
  );
});

function nudgeColors(
  cosmos: CosmosGraph | null,
  base: Float32Array | null,
  idToIndex: Map<string, number> | null,
  indexToId: string[] | null,
  overlay: Map<string, string> | null,
  highlight: Set<string> | null,
  focus: Set<string> | null,
  bulk: Set<string> | null,
) {
  if (!cosmos || !base || !idToIndex || !indexToId) return;
  const next = new Float32Array(base);
  applyColorOverlay(next, idToIndex, overlay);
  if (highlight || focus) {
    for (let i = 0; i < next.length; i += 4) {
      const id = indexToId[i / 4];
      if (!id) continue;
      const keepFocus = focus ? focus.has(id) : true;
      const keepHighlight = highlight ? highlight.has(id) : true;
      if (!keepFocus || !keepHighlight) {
        next[i + 3] = 0.18;
      }
    }
  }
  if (bulk) {
    for (const id of bulk) {
      const idx = idToIndex.get(id);
      if (idx == null) continue;
      next[idx * 4] = 0.05;
      next[idx * 4 + 1] = 0.66;
      next[idx * 4 + 2] = 0.92;
      next[idx * 4 + 3] = 1;
    }
  }
  cosmos.setPointColors(next);
}
