import type { BrainGraph } from "./graphologyAdapter";

export type LayoutMode =
  | "force"
  | "force3d"
  | "hierarchical"
  | "radial"
  | "umap"
  | "timeline"
  | "communities";

export interface LayoutPositions {
  [nodeId: string]: { x: number; y: number };
}

export interface RunLayoutOptions {
  focusNodeId?: string | null;
  /**
   * Hierarchical layout direction. ELK supports DOWN | RIGHT | LEFT | UP.
   * For knowledge graphs DOWN usually looks like an org chart, RIGHT like a
   * gantt-y left-to-right cascade.
   */
  direction?: "DOWN" | "RIGHT";
}

/**
 * Compute target positions for a layout mode. Force is intentionally a no-op
 * here — the FA2 web worker in BrainCanvas owns force layout. Returning null
 * tells the canvas to leave positions alone and let the worker keep simulating.
 */
export async function computeLayout(
  graph: BrainGraph,
  mode: LayoutMode,
  opts: RunLayoutOptions = {},
): Promise<LayoutPositions | null> {
  if (mode === "force" || mode === "force3d") return null;
  if (mode === "radial") return computeRadial(graph, opts.focusNodeId ?? null);
  if (mode === "hierarchical") return computeHierarchical(graph, opts.direction ?? "DOWN");
  if (mode === "umap") return computeUmap(graph);
  if (mode === "timeline") return computeTimeline(graph);
  if (mode === "communities") return computeCommunities(graph);
  return null;
}

/**
 * Radial layout: BFS rings centred on the focus node (or the most-connected
 * node if no focus). Ring radius scales with distance from the centre; angles
 * distribute neighbours evenly around each ring.
 */
function computeRadial(graph: BrainGraph, focusId: string | null): LayoutPositions {
  if (graph.order === 0) return {};
  const root = focusId && graph.hasNode(focusId) ? focusId : pickHubNode(graph);
  const distance = new Map<string, number>();
  distance.set(root, 0);
  const queue: string[] = [root];
  let head = 0;
  while (head < queue.length) {
    const cur = queue[head++];
    const d = distance.get(cur)!;
    graph.forEachNeighbor(cur, (n) => {
      if (!distance.has(n)) {
        distance.set(n, d + 1);
        queue.push(n);
      }
    });
  }
  // Disconnected components → assign them to a ring beyond the BFS reach.
  const maxRing = Math.max(0, ...distance.values()) + 1;
  graph.forEachNode((id) => {
    if (!distance.has(id)) distance.set(id, maxRing);
  });

  const ringMembers = new Map<number, string[]>();
  for (const [id, ring] of distance) {
    if (!ringMembers.has(ring)) ringMembers.set(ring, []);
    ringMembers.get(ring)!.push(id);
  }

  const positions: LayoutPositions = {};
  for (const [ring, members] of ringMembers) {
    if (ring === 0) {
      positions[members[0]] = { x: 0, y: 0 };
      continue;
    }
    // Density fix: ring radius scales with both ring distance AND member count
    // so high-fan-out rings don't overlap their neighbours.
    const minSpacing = 32;
    const byCount = (members.length * minSpacing) / (2 * Math.PI);
    const radius = Math.max(180 * ring, byCount + (ring - 1) * 90);
    const offset = (ring % 2) * 0.5;
    members.forEach((id, i) => {
      const angle = ((i + offset) / members.length) * Math.PI * 2;
      positions[id] = {
        x: Math.cos(angle) * radius,
        y: Math.sin(angle) * radius,
      };
    });
  }
  return positions;
}

// Timeline layout — x = updated_at (linear scale), y = kind row (sorted by
// frequency desc). Gives an at-a-glance event stream view of the graph.
function computeTimeline(graph: BrainGraph): LayoutPositions {
  if (graph.order === 0) return {};
  const kinds = new Map<string, number>();
  const tsByNode = new Map<string, number>();
  let minTs = Infinity;
  let maxTs = -Infinity;
  graph.forEachNode((id, attrs) => {
    const a = attrs as unknown as Record<string, unknown>;
    const kind = (a.kind as string | undefined) ?? "entity";
    kinds.set(kind, (kinds.get(kind) ?? 0) + 1);
    const updated = (a.updated_at as string | undefined) ?? null;
    const t = updated ? Date.parse(updated) : NaN;
    if (Number.isFinite(t)) {
      tsByNode.set(id, t);
      if (t < minTs) minTs = t;
      if (t > maxTs) maxTs = t;
    }
  });
  if (!Number.isFinite(minTs) || !Number.isFinite(maxTs) || maxTs === minTs) {
    // Fall back to alphabetical so nothing collapses to a point.
    minTs = 0;
    maxTs = Math.max(graph.order, 1);
    let i = 0;
    graph.forEachNode((id) => {
      tsByNode.set(id, i++);
    });
  }
  const sortedKinds = Array.from(kinds.entries())
    .sort((a, b) => b[1] - a[1])
    .map(([k]) => k);
  const kindRow = new Map<string, number>(sortedKinds.map((k, idx) => [k, idx]));

  const widthSpan = 3200;
  const rowHeight = 110;
  const positions: LayoutPositions = {};
  graph.forEachNode((id, attrs) => {
    const a = attrs as unknown as Record<string, unknown>;
    const kind = (a.kind as string | undefined) ?? "entity";
    const t = tsByNode.get(id) ?? minTs;
    const nx = ((t - minTs) / (maxTs - minTs)) * widthSpan - widthSpan / 2;
    const row = kindRow.get(kind) ?? 0;
    const ny = (row - (sortedKinds.length - 1) / 2) * rowHeight;
    positions[id] = { x: nx, y: ny };
  });
  return positions;
}

// Communities layout — group nodes by their `community_id` (or fallback to a
// degree-based bucketing if no community attr is set), arrange community
// centroids on a circle, and pack members around each centroid with a small
// internal force-like spread.
function computeCommunities(graph: BrainGraph): LayoutPositions {
  if (graph.order === 0) return {};
  const groups = new Map<string, string[]>();
  graph.forEachNode((id, attrs) => {
    const a = attrs as unknown as Record<string, unknown>;
    const community =
      (a.community_id as string | undefined) ||
      (a.community as string | undefined) ||
      (a.kind as string | undefined) ||
      "default";
    const arr = groups.get(community) ?? [];
    arr.push(id);
    groups.set(community, arr);
  });
  const entries = Array.from(groups.entries()).sort((a, b) => b[1].length - a[1].length);
  const positions: LayoutPositions = {};
  const ringRadius = 400 + Math.sqrt(entries.length) * 80;
  entries.forEach(([_, members], idx) => {
    const angle = (idx / entries.length) * Math.PI * 2;
    const cx = Math.cos(angle) * ringRadius;
    const cy = Math.sin(angle) * ringRadius;
    const localRadius = Math.max(40, Math.sqrt(members.length) * 22);
    members.forEach((id, i) => {
      // Spiral packing — golden-angle keeps things even at all member counts.
      const a = i * 2.39996323; // golden angle radians
      const r = Math.sqrt(i) * (localRadius / Math.sqrt(Math.max(1, members.length)));
      positions[id] = { x: cx + Math.cos(a) * r, y: cy + Math.sin(a) * r };
    });
  });
  return positions;
}

// UMAP layout — uses pre-computed semantic positions if available on node
// attributes (umap_x / umap_y). The web worker computes these once and writes
// them back; until then we return null and the caller keeps current positions.
function computeUmap(graph: BrainGraph): LayoutPositions | null {
  if (graph.order === 0) return {};
  const positions: LayoutPositions = {};
  let found = 0;
  graph.forEachNode((id, attrs) => {
    const a = attrs as unknown as Record<string, unknown>;
    const x = a.umap_x as number | undefined;
    const y = a.umap_y as number | undefined;
    if (typeof x === "number" && typeof y === "number") {
      positions[id] = { x, y };
      found++;
    }
  });
  // If none of the nodes have a cached UMAP position, return null — the caller
  // (UMAP web worker) will populate later.
  if (found === 0) return null;
  return positions;
}

function pickHubNode(graph: BrainGraph): string {
  let bestId = "";
  let bestDeg = -1;
  graph.forEachNode((id) => {
    const deg = graph.degree(id);
    if (deg > bestDeg) {
      bestDeg = deg;
      bestId = id;
    }
  });
  return bestId;
}

/**
 * Hierarchical layout via ELKjs (layered). Imported dynamically so the ~250KB
 * elkjs bundle only loads when a user actually picks this layout — most stay
 * on the default force mode.
 */
async function computeHierarchical(
  graph: BrainGraph,
  direction: "DOWN" | "RIGHT",
): Promise<LayoutPositions> {
  const { default: ELK } = await import("elkjs/lib/elk.bundled.js");
  const elk = new ELK();
  const elkGraph = {
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": direction,
      "elk.spacing.nodeNode": "44",
      "elk.layered.spacing.nodeNodeBetweenLayers": "78",
      "elk.layered.crossingMinimization.strategy": "LAYER_SWEEP",
      "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
    },
    children: graph.mapNodes((id, attrs) => ({
      id,
      width: Math.max(48, (attrs.size ?? 6) * 2.4),
      height: Math.max(48, (attrs.size ?? 6) * 2.4),
    })),
    edges: graph.mapEdges((edgeId, _attrs, source, target) => ({
      id: edgeId,
      sources: [source],
      targets: [target],
    })),
  };
  const result = (await elk.layout(elkGraph)) as {
    children?: Array<{ id: string; x?: number; y?: number; width?: number; height?: number }>;
  };
  const positions: LayoutPositions = {};
  for (const child of result.children ?? []) {
    if (child.x == null || child.y == null) continue;
    positions[child.id] = {
      x: child.x + (child.width ?? 0) / 2,
      y: child.y + (child.height ?? 0) / 2,
    };
  }
  return positions;
}

/**
 * Animate node positions from their current values to a target layout. Uses
 * requestAnimationFrame so React never re-renders mid-tween — graphology
 * mutations trigger Sigma's internal repaint via its attribute listener.
 */
export function tweenToPositions(
  graph: BrainGraph,
  targets: LayoutPositions,
  durationMs = 520,
  easing: (t: number) => number = easeInOutCubic,
  onDone?: () => void,
): () => void {
  const start: LayoutPositions = {};
  for (const id of Object.keys(targets)) {
    if (!graph.hasNode(id)) continue;
    const attrs = graph.getNodeAttributes(id);
    start[id] = { x: attrs.x ?? 0, y: attrs.y ?? 0 };
  }
  const t0 = performance.now();
  let frame = 0;
  const tick = () => {
    const elapsed = performance.now() - t0;
    const t = Math.min(1, elapsed / durationMs);
    const e = easing(t);
    for (const [id, dest] of Object.entries(targets)) {
      const from = start[id];
      if (!from) continue;
      graph.mergeNodeAttributes(id, {
        x: from.x + (dest.x - from.x) * e,
        y: from.y + (dest.y - from.y) * e,
      });
    }
    if (t < 1) {
      frame = requestAnimationFrame(tick);
    } else {
      onDone?.();
    }
  };
  frame = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(frame);
}

function easeInOutCubic(t: number): number {
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}
