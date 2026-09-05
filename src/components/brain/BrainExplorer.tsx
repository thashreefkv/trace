import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { AlertTriangle, Brain, Sparkles, ZoomIn, ZoomOut } from "lucide-react";
import { useNavigate } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createMemory,
  deleteBrainView,
  listGraphCommunities,
  listSavedBrainViews,
  reviewInference,
  saveBrainView,
} from "../../lib/ipc";
import type {
  SavedBrainView,
  WorkGraph,
  WorkGraphEdge,
  WorkGraphNode,
} from "../../lib/types";
import {
  collectKinds,
  collectRelations,
  neighborhoodIds,
} from "../../lib/brain/graphologyAdapter";
import { computeColorOverlay, type ColorMode } from "../../lib/brain/colorOverlays";
import { computeLayout, tweenToPositions, type LayoutMode } from "../../lib/brain/layouts";
import { findShortestPath, type PathResult } from "../../lib/brain/pathfinding";
import { useBrainGraph, useBrainStatus } from "../../hooks/useBrainGraph";
import {
  type BrainEdgeClickInfo,
  type BrainNodeClickMeta,
} from "./BrainCanvas";
import { CanvasShell, type BrainRendererHandle } from "./canvas/CanvasShell";
import { BrainSearchBar, type BrainSearchResult } from "./BrainSearchBar";
import { BrainTimeScrubber } from "./BrainTimeScrubber";
import { BrainTopBar } from "./BrainTopBar";
import { BrainLasso } from "./BrainLasso";
import { BulkActionsToolbar } from "./BulkActionsToolbar";
import { CommunitiesOverlay } from "./CommunitiesOverlay";
import { FocusModePill } from "./FocusModePill";
import { InferenceEdgePopover } from "./InferenceEdgePopover";
import { PathPanel } from "./PathPanel";
import { SavedViewsPanel } from "./SavedViewsPanel";
import { LeftRail, LeftRailReopenTab } from "./rails/LeftRail";
import { RightInspector, RightInspectorReopenTab } from "./rails/RightInspector";
import { FocusModeOverlay } from "./rails/FocusModeOverlay";
import { useRailState } from "./rails/useRailState";
import { useBrainKeybinds } from "./interactions/useBrainKeybinds";
import { CommandPaletteNL } from "./interactions/CommandPaletteNL";
import { Minimap } from "./canvas/Minimap";
import { isPerfDebugEnabled, useBrainPerfMonitor } from "./perf/useBrainPerfMonitor";

export function BrainExplorer() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  // ── Filters ──────────────────────────────────────────────────────────
  const [includeDismissedCaptures, setIncludeDismissedCaptures] = useState(false);
  const [includeKilledDeliverables, setIncludeKilledDeliverables] = useState(false);
  const filters = useMemo(
    () => ({
      include_dismissed_captures: includeDismissedCaptures,
      include_killed_deliverables: includeKilledDeliverables,
    }),
    [includeDismissedCaptures, includeKilledDeliverables],
  );

  const { raw, graph, isLoading, isFetching, error, refetch } = useBrainGraph(filters);
  const { data: status } = useBrainStatus();

  // ── UI state ─────────────────────────────────────────────────────────
  const rails = useRailState();
  const {
    leftRail,
    rightRail,
    isFocus,
    setLeftRail,
    setRightRail,
    cycleLeftRail,
    cycleRightRail,
    toggleFocus,
    setFocus,
  } = rails;
  const [hiddenKinds, setHiddenKinds] = useState<Set<string>>(() => new Set());
  const [hiddenRelations, setHiddenRelations] = useState<Set<string>>(() => new Set());
  const [confidence, setConfidence] = useState(0);
  const [layoutMode, setLayoutMode] = useState<LayoutMode>("force");
  const [isLayouting, setIsLayouting] = useState(false);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [pinnedIds, setPinnedIds] = useState<Set<string>>(() => new Set());
  const [searchHits, setSearchHits] = useState<BrainSearchResult | null>(null);
  const [communitiesVisible, setCommunitiesVisible] = useState(false);
  const [timeScrubberOpen, setTimeScrubberOpen] = useState(false);
  const [timeRange, setTimeRange] = useState<{ from: number; to: number } | null>(null);
  const [pathEndpoints, setPathEndpoints] = useState<{ from: string | null; to: string | null }>({
    from: null,
    to: null,
  });
  const [path, setPath] = useState<PathResult | null>(null);
  const [noPathReason, setNoPathReason] = useState<string | null>(null);
  const [inferenceEdge, setInferenceEdge] = useState<{ edge: WorkGraphEdge; x: number; y: number } | null>(null);
  const [activeViewId, setActiveViewId] = useState<string | null>(null);
  const [colorMode, setColorMode] = useState<ColorMode>("kind");
  const [focusedId, setFocusedId] = useState<string | null>(null);
  const [hopRadius, setHopRadius] = useState(2);
  const [bulkSelected, setBulkSelected] = useState<Set<string>>(() => new Set());
  const [paletteOpen, setPaletteOpen] = useState(false);
  const perfDebug = useMemo(() => isPerfDebugEnabled(), []);
  const perf = useBrainPerfMonitor(perfDebug);
  const canvasRef = useRef<BrainRendererHandle>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // ── Saved views + communities (background queries) ───────────────────
  const savedViewsQuery = useQuery<SavedBrainView[]>({
    queryKey: ["brain-saved-views"],
    queryFn: () => listSavedBrainViews(),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  });
  const communitiesQuery = useQuery({
    queryKey: ["brain-communities"],
    queryFn: () => listGraphCommunities(null),
    staleTime: 60_000,
    enabled: communitiesVisible,
    refetchOnWindowFocus: false,
  });

  // ── Filter raw → filteredWork ────────────────────────────────────────
  const filteredWork = useMemo<WorkGraph | null>(() => {
    if (!raw) return null;
    const passKind = (kind: string) => !hiddenKinds.has(kind);
    const passTime = (n: WorkGraphNode) => {
      if (!timeRange) return true;
      const ts = n.updated_at ? Date.parse(n.updated_at) : NaN;
      if (Number.isNaN(ts)) return true;
      return ts >= timeRange.from && ts <= timeRange.to;
    };
    const nodes = raw.nodes.filter((n) => passKind(n.kind) && passTime(n));
    const allowedIds = new Set(nodes.map((n) => n.id));
    const edges = raw.edges.filter((e) => {
      if (hiddenRelations.has(e.kind)) return false;
      if (!allowedIds.has(e.source) || !allowedIds.has(e.target)) return false;
      if (confidence > 0) {
        const conf =
          typeof e.properties?.confidence === "number"
            ? (e.properties.confidence as number)
            : 1;
        if (conf < confidence) return false;
      }
      return true;
    });
    return { ...raw, nodes, edges };
  }, [raw, hiddenKinds, hiddenRelations, confidence, timeRange]);

  const { graphologyGraph, kindCounts, relationCounts } = useMemo(() => {
    if (!filteredWork) {
      return {
        graphologyGraph: null,
        kindCounts: new Map<string, number>(),
        relationCounts: new Map<string, number>(),
      };
    }
    return {
      graphologyGraph: graph,
      kindCounts: collectKinds(filteredWork),
      relationCounts: collectRelations(filteredWork),
    };
  }, [filteredWork, graph]);

  const selectedNode = useMemo<WorkGraphNode | null>(() => {
    if (!filteredWork || !selectedId) return null;
    return filteredWork.nodes.find((n) => n.id === selectedId) ?? null;
  }, [filteredWork, selectedId]);

  // ── Search highlights → canvas reducer ───────────────────────────────
  useEffect(() => {
    if (!canvasRef.current) return;
    canvasRef.current.highlight(searchHits?.hitIds ?? null);
  }, [searchHits]);

  // ── Color overlay computation + push to canvas ───────────────────────
  const communityColorMap = useMemo(() => {
    const map = new Map<string, string>();
    const list = communitiesQuery.data ?? [];
    list.forEach((community, idx) => {
      const hue = (idx * 47) % 360;
      const color = `hsl(${hue}, 70%, 60%)`;
      for (const member of community.members) {
        map.set(`${member.entity_kind}:${member.entity_id}`, color);
      }
    });
    return map;
  }, [communitiesQuery.data]);

  useEffect(() => {
    if (!canvasRef.current) return;
    if (!graphologyGraph) {
      canvasRef.current.setColorOverlay(null);
      return;
    }
    const overlay = computeColorOverlay(graphologyGraph, colorMode, {
      communityColors: communityColorMap,
    });
    canvasRef.current.setColorOverlay(overlay?.nodeColor ?? null);
  }, [colorMode, graphologyGraph, communityColorMap]);

  // ── Focus mode set → canvas reducer ──────────────────────────────────
  useEffect(() => {
    if (!canvasRef.current) return;
    if (!focusedId || !graphologyGraph) {
      canvasRef.current.setFocusSet(null);
      return;
    }
    const focusSet = neighborhoodIds(graphologyGraph, [focusedId], hopRadius);
    canvasRef.current.setFocusSet(focusSet);
  }, [focusedId, hopRadius, graphologyGraph]);

  // ── Bulk selection → canvas reducer ──────────────────────────────────
  useEffect(() => {
    if (!canvasRef.current) return;
    canvasRef.current.setBulkSelection(bulkSelected.size > 0 ? bulkSelected : null);
  }, [bulkSelected]);

  // ── Path finding ─────────────────────────────────────────────────────
  useEffect(() => {
    if (!graphologyGraph || !filteredWork) return;
    const { from, to } = pathEndpoints;
    if (!from || !to) {
      setPath(null);
      setNoPathReason(null);
      canvasRef.current?.pathHighlight(null, null);
      return;
    }
    const result = findShortestPath(graphologyGraph, filteredWork, from, to);
    if (!result) {
      setPath(null);
      setNoPathReason("No connection found within the current filters.");
      canvasRef.current?.pathHighlight(null, null);
      return;
    }
    setPath(result);
    setNoPathReason(null);
    canvasRef.current?.pathHighlight(
      new Set(result.nodeIds),
      new Set(result.edgeIds),
    );
  }, [pathEndpoints, graphologyGraph, filteredWork]);

  // ── Layout transitions (force is owned by FA2 worker; others tween) ──
  useEffect(() => {
    if (!graphologyGraph) return;
    if (layoutMode === "force") return;
    let cancelled = false;
    setIsLayouting(true);
    (async () => {
      const positions = await computeLayout(graphologyGraph, layoutMode, {
        focusNodeId: selectedId,
        direction: "DOWN",
      });
      if (cancelled || !positions) return;
      tweenToPositions(graphologyGraph, positions, 520, undefined, () => {
        setIsLayouting(false);
        // Frame the new layout — without this, hierarchy / radial often land
        // far outside the camera viewport.
        canvasRef.current?.fitToScreen({ durationMs: 480 });
      });
    })().catch(() => setIsLayouting(false));
    return () => {
      cancelled = true;
    };
  }, [layoutMode, graphologyGraph, selectedId]);

  // ── Handlers ─────────────────────────────────────────────────────────
  const onToggleKind = useCallback((kind: string) => {
    setHiddenKinds((prev) => {
      const next = new Set(prev);
      if (next.has(kind)) next.delete(kind);
      else next.add(kind);
      return next;
    });
  }, []);
  const onToggleRelation = useCallback((relation: string) => {
    setHiddenRelations((prev) => {
      const next = new Set(prev);
      if (next.has(relation)) next.delete(relation);
      else next.add(relation);
      return next;
    });
  }, []);
  const onResetFilters = useCallback(() => {
    setHiddenKinds(new Set());
    setHiddenRelations(new Set());
    setConfidence(0);
    setIncludeDismissedCaptures(false);
    setIncludeKilledDeliverables(false);
    setTimeRange(null);
    setActiveViewId(null);
  }, []);

  const onNodeClick = useCallback(
    (id: string | null, meta?: BrainNodeClickMeta) => {
      if (id && meta?.metaKey) {
        setPathEndpoints((prev) => {
          if (!prev.from || (prev.from && prev.to)) {
            return { from: id, to: null };
          }
          if (prev.from === id) {
            return { from: null, to: null };
          }
          return { from: prev.from, to: id };
        });
        return;
      }
      setSelectedId(id);
    },
    [],
  );

  const onSelectFromList = useCallback((id: string) => {
    setSelectedId(id);
    canvasRef.current?.focus(id);
  }, []);

  const onEdgeClick = useCallback(
    (info: BrainEdgeClickInfo | null) => {
      if (!info || !filteredWork) {
        setInferenceEdge(null);
        return;
      }
      const edge = filteredWork.edges.find((e) => e.id === info.edgeId);
      if (!edge) return;
      const inferred =
        edge.properties?.inferred === true ||
        typeof edge.properties?.brain_inference_id === "string";
      const status = edge.properties?.status as string | undefined;
      if (!inferred || status !== "pending") return;
      const rect = containerRef.current?.getBoundingClientRect();
      setInferenceEdge({
        edge,
        x: info.clientX - (rect?.left ?? 0),
        y: info.clientY - (rect?.top ?? 0),
      });
    },
    [filteredWork],
  );

  const onAcceptInference = useCallback(
    async (edge: WorkGraphEdge) => {
      const inferenceId = edge.properties?.brain_inference_id as string | undefined;
      if (!inferenceId) return;
      await reviewInference(inferenceId, "accepted");
      await refetch();
    },
    [refetch],
  );

  const onRejectInference = useCallback(
    async (edge: WorkGraphEdge) => {
      const inferenceId = edge.properties?.brain_inference_id as string | undefined;
      if (!inferenceId) return;
      await reviewInference(inferenceId, "rejected");
      await refetch();
    },
    [refetch],
  );

  const onTogglePin = useCallback((id: string) => {
    setPinnedIds((prev) => {
      const next = new Set(prev);
      const nowPinned = !next.has(id);
      if (nowPinned) next.add(id);
      else next.delete(id);
      canvasRef.current?.pinNode(id, nowPinned);
      return next;
    });
  }, []);

  const onHide = useCallback(
    (id: string) => {
      const work = filteredWork;
      if (!work) return;
      const node = work.nodes.find((n) => n.id === id);
      if (!node) return;
      setHiddenKinds((prev) => {
        const next = new Set(prev);
        next.add(node.kind);
        return next;
      });
      setSelectedId(null);
    },
    [filteredWork],
  );

  const onFindSimilar = useCallback(
    (node: WorkGraphNode) => {
      if (!graphologyGraph) return;
      const neighbors = neighborhoodIds(graphologyGraph, [node.id], 2);
      setSearchHits({
        hitIds: neighbors,
        rankedNodes: [],
        summary: `Neighborhood of ${node.label}`,
        memoryHits: null,
        cypherResult: null,
        query: node.label,
        mode: "nl",
      });
    },
    [graphologyGraph],
  );

  const onMakeMemory = useCallback(
    async (node: WorkGraphNode) => {
      try {
        await createMemory({
          kind: "semantic",
          title: node.label.slice(0, 120),
          body: `From ${node.kind} · ${node.subtitle ?? node.label}`,
          scope: "project",
          tags: [node.kind, "brain-graph"],
        });
        await refetch();
      } catch (err) {
        console.error("createMemory failed", err);
      }
    },
    [refetch],
  );

  const onZoomIn = useCallback(() => {
    const sigma = canvasRef.current?.getSigma();
    sigma?.getCamera().animatedZoom({ duration: 220 });
  }, []);
  const onZoomOut = useCallback(() => {
    const sigma = canvasRef.current?.getSigma();
    sigma?.getCamera().animatedUnzoom({ duration: 220 });
  }, []);
  const onRecenter = useCallback(() => canvasRef.current?.recenter(), []);

  const onLayoutChange = useCallback((next: LayoutMode) => setLayoutMode(next), []);

  // ── Bulk handlers ────────────────────────────────────────────────────
  const onBulkPin = useCallback(() => {
    setPinnedIds((prev) => {
      const next = new Set(prev);
      for (const id of bulkSelected) {
        next.add(id);
        canvasRef.current?.pinNode(id, true);
      }
      return next;
    });
  }, [bulkSelected]);

  const onBulkHide = useCallback(() => {
    if (!filteredWork) return;
    const kinds = new Set<string>();
    for (const id of bulkSelected) {
      const node = filteredWork.nodes.find((n) => n.id === id);
      if (node) kinds.add(node.kind);
    }
    setHiddenKinds((prev) => {
      const next = new Set(prev);
      for (const k of kinds) next.add(k);
      return next;
    });
    setBulkSelected(new Set());
  }, [bulkSelected, filteredWork]);

  const onBulkMakeMemory = useCallback(async () => {
    if (!filteredWork || bulkSelected.size === 0) return;
    const labels = Array.from(bulkSelected)
      .slice(0, 20)
      .map((id) => filteredWork.nodes.find((n) => n.id === id)?.label ?? id);
    try {
      await createMemory({
        kind: "semantic",
        title: `Brain selection of ${bulkSelected.size} entities`.slice(0, 120),
        body: labels.map((l, i) => `${i + 1}. ${l}`).join("\n"),
        scope: "project",
        tags: ["brain-graph", "bulk-selection"],
      });
      setBulkSelected(new Set());
      await refetch();
    } catch (err) {
      console.error("bulk make memory failed", err);
    }
  }, [bulkSelected, filteredWork, refetch]);

  const onBulkOpenInAsk = useCallback(() => {
    const ids = Array.from(bulkSelected).slice(0, 12).join(",");
    if (!ids) return;
    navigate(`/ask?seed=${encodeURIComponent(ids)}`);
  }, [bulkSelected, navigate]);

  // ── Focus mode helpers ───────────────────────────────────────────────
  const focusedNode = useMemo(
    () => (focusedId ? filteredWork?.nodes.find((n) => n.id === focusedId) ?? null : null),
    [focusedId, filteredWork],
  );
  const onEnterFocus = useCallback((id: string) => {
    setFocusedId(id);
    canvasRef.current?.focus(id);
  }, []);

  // ── Saved views ──────────────────────────────────────────────────────
  const onSaveCurrentView = useCallback(
    async (name: string) => {
      const sigma = canvasRef.current?.getSigma();
      const cameraState = sigma?.getCamera().getState() ?? { x: 0.5, y: 0.5, ratio: 1, angle: 0 };
      const created = await saveBrainView({
        name,
        filters: {
          hiddenKinds: Array.from(hiddenKinds),
          hiddenRelations: Array.from(hiddenRelations),
          confidence,
          includeDismissedCaptures,
          includeKilledDeliverables,
          timeRange,
        },
        layout: { mode: layoutMode },
        viewport: cameraState as unknown as Record<string, unknown>,
        pinned: Array.from(pinnedIds),
      });
      setActiveViewId(created.id);
      await queryClient.invalidateQueries({ queryKey: ["brain-saved-views"] });
    },
    [
      confidence,
      hiddenKinds,
      hiddenRelations,
      includeDismissedCaptures,
      includeKilledDeliverables,
      layoutMode,
      pinnedIds,
      queryClient,
      timeRange,
    ],
  );

  const onApplyView = useCallback(
    (view: SavedBrainView) => {
      const f = view.filters as Record<string, unknown>;
      if (Array.isArray(f.hiddenKinds)) setHiddenKinds(new Set(f.hiddenKinds as string[]));
      if (Array.isArray(f.hiddenRelations))
        setHiddenRelations(new Set(f.hiddenRelations as string[]));
      if (typeof f.confidence === "number") setConfidence(f.confidence);
      if (typeof f.includeDismissedCaptures === "boolean")
        setIncludeDismissedCaptures(f.includeDismissedCaptures);
      if (typeof f.includeKilledDeliverables === "boolean")
        setIncludeKilledDeliverables(f.includeKilledDeliverables);
      if (f.timeRange && typeof f.timeRange === "object") {
        setTimeRange(f.timeRange as { from: number; to: number });
        setTimeScrubberOpen(true);
      } else {
        setTimeRange(null);
      }
      const layout = view.layout as { mode?: LayoutMode } | undefined;
      if (layout?.mode) setLayoutMode(layout.mode);
      if (Array.isArray(view.pinned)) {
        setPinnedIds(new Set(view.pinned as string[]));
      }
      const viewport = view.viewport as
        | { x: number; y: number; ratio: number; angle: number }
        | undefined;
      if (viewport) {
        const sigma = canvasRef.current?.getSigma();
        sigma?.getCamera().animate(viewport, { duration: 380 });
      }
      setActiveViewId(view.id);
    },
    [],
  );

  const onDeleteView = useCallback(
    async (view: SavedBrainView) => {
      await deleteBrainView(view.id);
      if (activeViewId === view.id) setActiveViewId(null);
      await queryClient.invalidateQueries({ queryKey: ["brain-saved-views"] });
    },
    [activeViewId, queryClient],
  );

  // ── Render ───────────────────────────────────────────────────────────
  const savedViewsSlot = (
    <SavedViewsPanel
      activeViewId={activeViewId}
      isLoading={savedViewsQuery.isLoading}
      onApply={onApplyView}
      onDelete={onDeleteView}
      onSaveCurrent={onSaveCurrentView}
      views={savedViewsQuery.data ?? []}
    />
  );

  // ── Refit camera when the canvas section resizes ─────────────────────
  // Rails collapse / expand changes the canvas width via framer-motion. The
  // renderers don't auto-refit on container resize, so the camera ends up
  // looking at empty space outside the bbox. Fire a fit after each rail
  // transition (spring is ~320/36 so ~420ms settles).
  useEffect(() => {
    const handle = window.setTimeout(() => {
      canvasRef.current?.fitToScreen({ durationMs: 320 });
    }, 460);
    return () => window.clearTimeout(handle);
  }, [leftRail, rightRail, isFocus]);

  // ResizeObserver — covers window resize, drag-resize, etc. Debounced so
  // mid-animation frames don't trigger refits.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    let timer = 0;
    const obs = new ResizeObserver(() => {
      window.clearTimeout(timer);
      timer = window.setTimeout(() => {
        canvasRef.current?.fitToScreen({ durationMs: 240 });
      }, 220);
    });
    obs.observe(el);
    return () => {
      obs.disconnect();
      window.clearTimeout(timer);
    };
  }, []);

  // ── Keybinds ─────────────────────────────────────────────────────────
  useBrainKeybinds({
    cycleLeftRail,
    cycleRightRail,
    toggleFocus,
    exitFocusOrClear: () => {
      if (isFocus) {
        setFocus(false);
        return;
      }
      if (focusedId) {
        setFocusedId(null);
        return;
      }
      setSelectedId(null);
    },
    openPalette: () => setPaletteOpen((v) => !v),
    setMode: (idx: number) => {
      const modes: LayoutMode[] = [
        "force",
        "force3d",
        "hierarchical",
        "radial",
        "umap",
        "timeline",
        "communities",
      ];
      const m = modes[idx];
      if (m) setLayoutMode(m);
    },
    focusSearch: () => {
      const input = document.querySelector<HTMLInputElement>(
        '[data-brain-search-input="true"]',
      );
      input?.focus();
    },
    recenter: () => canvasRef.current?.fitToScreen({ durationMs: 420 }),
    zoomIn: onZoomIn,
    zoomOut: onZoomOut,
  });

  return (
    <div className="relative flex h-full min-h-[640px] flex-1 flex-col gap-3">
      <AnimatePresence initial={false}>
        {!isFocus && (
          <motion.div
            animate={{ opacity: 1, height: "auto", y: 0 }}
            className="overflow-hidden"
            exit={{ opacity: 0, height: 0, y: -12 }}
            initial={{ opacity: 0, height: 0, y: -12 }}
            transition={{ type: "spring", stiffness: 320, damping: 36 }}
          >
            <BrainTopBar
              colorMode={colorMode}
              edgeCount={filteredWork?.edges.length ?? 0}
              generatedAt={status?.generated_at ?? filteredWork?.generated_at ?? null}
              isLayouting={isLayouting}
              isRefreshing={isFetching}
              layout={layoutMode}
              nodeCount={filteredWork?.nodes.length ?? 0}
              onColorModeChange={setColorMode}
              onLayoutChange={onLayoutChange}
              onRecenter={onRecenter}
              onRefresh={() => void refetch()}
              onToggleTimeScrubber={() => setTimeScrubberOpen((v) => !v)}
              onZoomIn={onZoomIn}
              onZoomOut={onZoomOut}
              searchSlot={
                <BrainSearchBar
                  onResults={setSearchHits}
                  onSelectNode={onSelectFromList}
                  totalNodes={raw?.nodes.length ?? 0}
                />
              }
              timeScrubberOpen={timeScrubberOpen}
            />
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex min-h-0 flex-1 gap-3">
        <LeftRail
          communitiesVisible={communitiesVisible}
          confidenceThreshold={confidence}
          hiddenKinds={hiddenKinds}
          hiddenRelations={hiddenRelations}
          includeDismissedCaptures={includeDismissedCaptures}
          includeKilledDeliverables={includeKilledDeliverables}
          kindCounts={kindCounts}
          mode={leftRail}
          onConfidenceChange={setConfidence}
          onModeChange={setLeftRail}
          onResetFilters={onResetFilters}
          onToggleCommunities={() => setCommunitiesVisible((v) => !v)}
          onToggleIncludeDismissed={() => setIncludeDismissedCaptures((v) => !v)}
          onToggleIncludeKilled={() => setIncludeKilledDeliverables((v) => !v)}
          onToggleKind={onToggleKind}
          onToggleRelation={onToggleRelation}
          relationCounts={relationCounts}
          savedViewsSlot={savedViewsSlot}
        />

        <section
          className={`relative min-h-0 flex-1 overflow-hidden rounded-2xl border shadow-[0_2px_12px_rgba(0,0,0,0.04)] ${
            rails.canvasTheme === "dark"
              ? "border-zinc-800 bg-zinc-950"
              : "border-zinc-100 bg-white"
          }`}
          ref={containerRef}
        >
          {/* Subtle radial gradient backdrop adds depth without competing with the graph. */}
          <div
            aria-hidden
            className={`pointer-events-none absolute inset-0 ${
              rails.canvasTheme === "dark"
                ? "bg-[radial-gradient(ellipse_at_center,rgba(24,24,27,0)_0%,rgba(9,9,11,0.5)_70%,rgba(9,9,11,0.7)_100%)]"
                : "bg-[radial-gradient(ellipse_at_center,rgba(244,244,245,0)_0%,rgba(228,228,231,0.35)_70%,rgba(212,212,216,0.5)_100%)]"
            }`}
          />
          {leftRail === "hidden" && <LeftRailReopenTab onOpen={() => setLeftRail("expanded")} />}
          {rightRail === "hidden" && (
            <RightInspectorReopenTab onOpen={() => setRightRail("expanded")} />
          )}
          {isLoading && <CanvasSkeleton />}
          {!isLoading && Boolean(error) && (
            <CanvasError message={String(error)} onRetry={() => void refetch()} />
          )}
          {!isLoading && !Boolean(error) && filteredWork && filteredWork.nodes.length === 0 && (
            <CanvasEmpty onReset={onResetFilters} />
          )}
          {!isLoading && !Boolean(error) && graphologyGraph && (
            <>
              <CanvasShell
                className="absolute inset-0"
                darkCanvas={rails.canvasTheme === "dark"}
                graph={graphologyGraph}
                mode={layoutMode}
                onEdgeClick={onEdgeClick}
                onNodeClick={onNodeClick}
                ref={canvasRef}
                selectedNodeId={selectedId}
              />
              <CommunitiesOverlay
                communities={communitiesQuery.data ?? []}
                sigma={canvasRef.current?.getSigma() ?? null}
                visible={communitiesVisible}
              />
              <Minimap
                darkBackground={rails.canvasTheme === "dark"}
                graph={graphologyGraph}
                sigma={canvasRef.current?.getSigma() ?? null}
              />
              <BrainLasso
                enabled
                onSelect={(ids) => setBulkSelected(new Set(ids))}
                sigma={canvasRef.current?.getSigma() ?? null}
              />
              <CanvasOverlay
                navigate={navigate}
                nodeCount={filteredWork?.nodes.length ?? 0}
                onZoomIn={onZoomIn}
                onZoomOut={onZoomOut}
              />
              <AnimatePresence mode="wait">
                <FocusModePill
                  focusedLabel={focusedNode?.label ?? null}
                  hopRadius={hopRadius}
                  key={focusedNode?.id ?? "no-focus"}
                  onExit={() => setFocusedId(null)}
                  onHopChange={setHopRadius}
                />
              </AnimatePresence>
              <AnimatePresence mode="wait">
                <BulkActionsToolbar
                  count={bulkSelected.size}
                  key={bulkSelected.size > 0 ? "bulk" : "no-bulk"}
                  onClear={() => setBulkSelected(new Set())}
                  onHideAll={onBulkHide}
                  onMakeMemory={onBulkMakeMemory}
                  onOpenInAsk={onBulkOpenInAsk}
                  onPinAll={onBulkPin}
                />
              </AnimatePresence>
              <AnimatePresence mode="wait">
                <PathPanel
                  endpoints={pathEndpoints}
                  key="path"
                  noPathReason={noPathReason}
                  onClear={() => setPathEndpoints({ from: null, to: null })}
                  onSelectNode={onSelectFromList}
                  path={path}
                />
              </AnimatePresence>
              <AnimatePresence mode="wait">
                <InferenceEdgePopover
                  anchor={inferenceEdge ? { x: inferenceEdge.x, y: inferenceEdge.y } : null}
                  edge={inferenceEdge?.edge ?? null}
                  key={inferenceEdge?.edge.id ?? "no-inf"}
                  onAccept={onAcceptInference}
                  onClose={() => setInferenceEdge(null)}
                  onReject={onRejectInference}
                />
              </AnimatePresence>
            </>
          )}
        </section>

        <RightInspector
          graph={filteredWork}
          mode={rightRail}
          onClose={() => setSelectedId(null)}
          onEnterFocus={onEnterFocus}
          onFindSimilar={onFindSimilar}
          onHide={onHide}
          onMakeMemory={onMakeMemory}
          onModeChange={setRightRail}
          onSelectNode={onSelectFromList}
          onTogglePin={onTogglePin}
          pinned={selectedNode ? pinnedIds.has(selectedNode.id) : false}
          selectedNode={selectedNode}
        />
      </div>

      <FocusModeOverlay
        edgeCount={filteredWork?.edges.length ?? 0}
        nodeCount={filteredWork?.nodes.length ?? 0}
        onExit={() => setFocus(false)}
        onRecenter={onRecenter}
        onZoomIn={onZoomIn}
        onZoomOut={onZoomOut}
        visible={isFocus}
      />

      {perfDebug && (
        <div className="pointer-events-none fixed bottom-3 left-3 z-[60] rounded-xl border border-zinc-100 bg-white/90 px-2.5 py-1.5 text-[10.5px] font-medium text-zinc-600 shadow-[0_2px_12px_rgba(0,0,0,0.06)] backdrop-blur">
          <span className="tabular-nums">
            {perf.fps} fps · {filteredWork?.nodes.length ?? 0} nodes · {filteredWork?.edges.length ?? 0} edges · {layoutMode}
          </span>
        </div>
      )}

      <CommandPaletteNL
        canvasTheme={rails.canvasTheme}
        onClose={() => setPaletteOpen(false)}
        onFitToScreen={() => canvasRef.current?.fitToScreen({ durationMs: 480 })}
        onFocusSearch={() => {
          const input = document.querySelector<HTMLInputElement>(
            '[data-brain-search-input="true"]',
          );
          input?.focus();
        }}
        onSetMode={setLayoutMode}
        onToggleFocus={() => toggleFocus()}
        onToggleTheme={() => rails.toggleCanvasTheme()}
        open={paletteOpen}
      />

      <AnimatePresence>
        {timeScrubberOpen && (
          <BrainTimeScrubber
            onClose={() => {
              setTimeScrubberOpen(false);
              setTimeRange(null);
            }}
            onRangeChange={setTimeRange}
            work={filteredWork}
          />
        )}
      </AnimatePresence>
    </div>
  );
}

function CanvasSkeleton() {
  return (
    <div className="absolute inset-0 grid grid-cols-3 gap-3 p-6">
      <div className="col-span-3 h-12 animate-pulse rounded-xl bg-zinc-100" />
      <div className="col-span-1 h-full animate-pulse rounded-2xl bg-zinc-100" />
      <div className="col-span-2 h-full animate-pulse rounded-2xl bg-zinc-100" />
    </div>
  );
}

function CanvasError({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className="absolute inset-0 grid place-items-center px-6">
      <div className="max-w-md rounded-xl border border-rose-100 bg-rose-50 p-5 text-center">
        <AlertTriangle aria-hidden className="mx-auto text-rose-400" size={24} />
        <p className="mt-2 text-[13px] font-medium text-rose-900">Brain graph failed to load</p>
        <p className="mt-1 text-[12px] text-rose-700">{message}</p>
        <button
          className="mt-3 rounded-xl border border-rose-200 bg-white px-3 py-1.5 text-[12px] font-medium text-rose-700 hover:bg-rose-50"
          onClick={onRetry}
          type="button"
        >
          Retry
        </button>
      </div>
    </div>
  );
}

function CanvasEmpty({ onReset }: { onReset: () => void }) {
  return (
    <div className="absolute inset-0 grid place-items-center">
      <div className="text-center">
        <Sparkles aria-hidden className="mx-auto text-zinc-200" size={32} />
        <p className="mt-2 text-[13px] font-medium text-zinc-500">Nothing matches these filters</p>
        <button
          className="mt-3 rounded-xl border border-zinc-200 bg-white px-3 py-1.5 text-[12px] font-medium text-zinc-700 hover:bg-zinc-50"
          onClick={onReset}
          type="button"
        >
          Reset filters
        </button>
      </div>
    </div>
  );
}

function CanvasOverlay({
  navigate,
  nodeCount,
  onZoomIn,
  onZoomOut,
}: {
  navigate: ReturnType<typeof useNavigate>;
  nodeCount: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
}) {
  return (
    <>
      <div className="pointer-events-none absolute bottom-3 left-3 flex items-center gap-1.5 rounded-xl bg-white/80 px-2.5 py-1.5 text-[11px] text-zinc-500 shadow-[0_2px_12px_rgba(0,0,0,0.06)] backdrop-blur">
        <Brain aria-hidden className="text-zinc-400" size={11} />
        <span className="tabular-nums">{nodeCount.toLocaleString()} nodes in view</span>
      </div>
      <div className="absolute bottom-3 right-3 flex flex-col gap-1 rounded-xl border border-zinc-100 bg-white p-1 shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
        <button
          aria-label="Zoom in"
          className="grid h-7 w-7 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
          onClick={onZoomIn}
          type="button"
        >
          <ZoomIn size={13} />
        </button>
        <button
          aria-label="Zoom out"
          className="grid h-7 w-7 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
          onClick={onZoomOut}
          type="button"
        >
          <ZoomOut size={13} />
        </button>
        <button
          className="grid h-7 w-7 place-items-center rounded-lg text-zinc-400 hover:bg-zinc-50 hover:text-zinc-900"
          onClick={() => navigate("/context")}
          title="Open legacy graph"
          type="button"
        >
          <Brain size={13} />
        </button>
      </div>
    </>
  );
}
