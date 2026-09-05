import { forwardRef, useImperativeHandle, useRef, Suspense, lazy } from "react";
import type Sigma from "sigma";
import type { BrainGraph } from "../../../lib/brain/graphologyAdapter";
import {
  BrainCanvas,
  type BrainCanvasHandle,
  type BrainEdgeClickInfo,
  type BrainNodeClickMeta,
} from "../BrainCanvas";
import { CosmosCanvas, type CosmosCanvasHandle } from "./CosmosCanvas";

// Lazy-load three.js — only paid when the user enters 3D.
const ThreeCanvas = lazy(() =>
  import("./ThreeCanvas").then((m) => ({ default: m.ThreeCanvas })),
);

export type RenderMode =
  | "force"
  | "force3d"
  | "hierarchical"
  | "radial"
  | "umap"
  | "timeline"
  | "communities";

export interface BrainRendererHandle {
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
  /** Sigma instance for legacy overlays (lasso, communities). Null when not on Sigma. */
  getSigma: () => Sigma | null;
}

interface CanvasShellProps {
  mode: RenderMode;
  graph: BrainGraph | null;
  selectedNodeId: string | null;
  onNodeClick: (id: string | null, meta?: BrainNodeClickMeta) => void;
  onEdgeClick?: (info: BrainEdgeClickInfo | null) => void;
  className?: string;
  darkCanvas?: boolean;
}

// Routes between the three renderers (Sigma, Cosmos, Three) based on `mode`
// and forwards a unified handle. Each renderer manages its own physics / camera.
export const CanvasShell = forwardRef<BrainRendererHandle, CanvasShellProps>(function CanvasShell(
  { mode, graph, selectedNodeId, onNodeClick, onEdgeClick, className, darkCanvas },
  ref,
) {
  const sigmaRef = useRef<BrainCanvasHandle>(null);
  const cosmosRef = useRef<CosmosCanvasHandle>(null);
  const threeRef = useRef<BrainRendererHandle>(null);

  useImperativeHandle(
    ref,
    () => makeUnifiedHandle({ mode, sigmaRef, cosmosRef, threeRef }),
    [mode],
  );

  if (!graph) return <div className={className} />;

  if (mode === "force") {
    return (
      <CosmosCanvas
        className={className}
        darkBackground={darkCanvas}
        graph={graph}
        onNodeClick={onNodeClick}
        ref={cosmosRef}
        selectedNodeId={selectedNodeId}
      />
    );
  }

  if (mode === "force3d") {
    return (
      <Suspense fallback={<div className={className} />}>
        <ThreeCanvas
          className={className}
          graph={graph}
          onNodeClick={onNodeClick}
          ref={threeRef}
          selectedNodeId={selectedNodeId}
        />
      </Suspense>
    );
  }

  // hierarchical / radial / umap / timeline / communities all use Sigma with
  // pre-computed positions (no physics → no jitter).
  return (
    <BrainCanvas
      className={className}
      graph={graph}
      onEdgeClick={onEdgeClick}
      onNodeClick={onNodeClick}
      ref={sigmaRef}
      runPhysics={false}
      selectedNodeId={selectedNodeId}
    />
  );
});

interface UnifiedHandleArgs {
  mode: RenderMode;
  sigmaRef: React.RefObject<BrainCanvasHandle | null>;
  cosmosRef: React.RefObject<CosmosCanvasHandle | null>;
  threeRef: React.RefObject<BrainRendererHandle | null>;
}

function makeUnifiedHandle({ mode, sigmaRef, cosmosRef, threeRef }: UnifiedHandleArgs): BrainRendererHandle {
  const callActive = <K extends keyof BrainRendererHandle>(
    method: K,
    args: Parameters<NonNullable<BrainRendererHandle[K]>>,
  ) => {
    if (mode === "force") {
      const target = cosmosRef.current as unknown as Record<string, (...a: unknown[]) => unknown> | null;
      target?.[method as string]?.(...(args as unknown[]));
      return;
    }
    if (mode === "force3d") {
      const target = threeRef.current as unknown as Record<string, (...a: unknown[]) => unknown> | null;
      target?.[method as string]?.(...(args as unknown[]));
      return;
    }
    const target = sigmaRef.current as unknown as Record<string, (...a: unknown[]) => unknown> | null;
    target?.[method as string]?.(...(args as unknown[]));
  };

  return {
    focus: (id, opts) => callActive("focus", [id, opts]),
    highlight: (ids) => callActive("highlight", [ids]),
    pathHighlight: (n, e) => callActive("pathHighlight", [n, e]),
    setFocusSet: (ids) => callActive("setFocusSet", [ids]),
    setColorOverlay: (colors) => callActive("setColorOverlay", [colors]),
    setBulkSelection: (ids) => callActive("setBulkSelection", [ids]),
    pinNode: (id, pinned) => callActive("pinNode", [id, pinned]),
    reset: () => callActive("reset", []),
    recenter: () => callActive("recenter", []),
    fitToScreen: (opts) => callActive("fitToScreen", [opts]),
    exportPng: () => {
      if (mode === "force") return cosmosRef.current?.exportPng() ?? null;
      if (mode === "force3d") return threeRef.current?.exportPng() ?? null;
      return sigmaRef.current?.exportPng() ?? null;
    },
    getSigma: () => {
      if (mode === "force" || mode === "force3d") return null;
      return sigmaRef.current?.getSigma() ?? null;
    },
  };
}
