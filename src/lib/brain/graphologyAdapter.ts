import { MultiDirectedGraph } from "graphology";
import type Graph from "graphology";
import type { WorkGraph, WorkGraphEdge, WorkGraphNode } from "../types";
import { colorForKind, labelForKind } from "./kinds";

const NODE_MIN_SIZE = 4.5;
const NODE_MAX_SIZE = 22;

export interface BrainNodeAttributes {
  id: string;
  entity_id: string;
  kind: string;
  kindLabel: string;
  label: string;
  subtitle: string | null;
  status: string | null;
  url: string | null;
  updated_at: string | null;
  weight: number;
  degree: number;
  hidden_by_default: boolean;
  color: string;
  borderColor: string;
  textColor: string;
  size: number;
  /**
   * Sigma node program. `"circle"` is the v3 built-in. A custom WebGL
   * `"brain-glyph"` program (kind-iconed halo) is on the Phase 3 roadmap;
   * until then `"circle"` keeps the canvas alive.
   */
  type: "circle";
  x: number;
  y: number;
  pinned?: boolean;
  selected?: boolean;
}

export interface BrainEdgeAttributes {
  id: string;
  source: string;
  target: string;
  relation: string;
  label: string;
  weight: number;
  inferred: boolean;
  inferredStatus?: "pending" | "accepted" | "rejected";
  confidence?: number;
  color: string;
  size: number;
  type: "arrow";
}

export type BrainGraph = Graph<BrainNodeAttributes, BrainEdgeAttributes>;
type BrainGraphCtor = MultiDirectedGraph<BrainNodeAttributes, BrainEdgeAttributes>;

function lerpColor(hexA: string, hexB: string, t: number): string {
  const a = parseHex(hexA);
  const b = parseHex(hexB);
  const r = Math.round(a[0] + (b[0] - a[0]) * t);
  const g = Math.round(a[1] + (b[1] - a[1]) * t);
  const bb = Math.round(a[2] + (b[2] - a[2]) * t);
  return `#${[r, g, bb].map((c) => c.toString(16).padStart(2, "0")).join("")}`;
}

function parseHex(hex: string): [number, number, number] {
  const clean = hex.replace("#", "");
  return [
    parseInt(clean.slice(0, 2), 16),
    parseInt(clean.slice(2, 4), 16),
    parseInt(clean.slice(4, 6), 16),
  ];
}

function degreeFromCounts(map: Map<string, number>, id: string): number {
  return map.get(id) ?? 0;
}

function placeNode(seed: number): { x: number; y: number } {
  const angle = (seed * 137.508 * Math.PI) / 180;
  const radius = Math.sqrt(seed) * 14;
  return { x: Math.cos(angle) * radius, y: Math.sin(angle) * radius };
}

function nodeSize(degree: number): number {
  const size = NODE_MIN_SIZE + Math.sqrt(degree) * 2.4;
  return Math.min(NODE_MAX_SIZE, Math.max(NODE_MIN_SIZE, size));
}

function edgeColor(weight: number, inferred: boolean, pending: boolean): string {
  if (inferred && pending) return "#f59e0b";
  if (inferred) return "#94a3b8";
  return lerpColor("#cbd5e1", "#0ea5e9", Math.max(0, Math.min(1, weight)));
}

function inferEdgeStatus(edge: WorkGraphEdge): "pending" | "accepted" | "rejected" | undefined {
  const raw = (edge.properties?.status ?? edge.properties?.inference_status) as
    | string
    | undefined;
  if (raw === "pending" || raw === "accepted" || raw === "rejected") return raw;
  return undefined;
}

function isInferred(edge: WorkGraphEdge): boolean {
  if (edge.properties?.inferred === true) return true;
  if (typeof edge.properties?.brain_inference_id === "string") return true;
  if (edge.kind === "inferred") return true;
  if (typeof edge.properties?.confidence === "number") return true;
  return false;
}

function edgeConfidence(edge: WorkGraphEdge): number | undefined {
  const raw = edge.properties?.confidence;
  return typeof raw === "number" ? raw : undefined;
}

export function toGraphology(work: WorkGraph): BrainGraph {
  // Multi-directed: the backend emits parallel edges between the same node
  // pair (e.g. a stakeholder can have several relations to a deliverable),
  // and the default `multi: false` graphology throws on the second insert.
  const graph: BrainGraphCtor = new MultiDirectedGraph<
    BrainNodeAttributes,
    BrainEdgeAttributes
  >({ allowSelfLoops: true });

  const degree = new Map<string, number>();
  for (const edge of work.edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }

  work.nodes.forEach((node, idx) => {
    const palette = colorForKind(node.kind);
    const deg = degreeFromCounts(degree, node.id);
    const pos = placeNode(idx + 1);
    const attrs: BrainNodeAttributes = {
      id: node.id,
      entity_id: node.entity_id,
      kind: node.kind,
      kindLabel: labelForKind(node.kind),
      label: node.label,
      subtitle: node.subtitle,
      status: node.status,
      url: node.url,
      updated_at: node.updated_at,
      weight: node.weight,
      degree: deg,
      hidden_by_default: node.hidden_by_default,
      color: palette.fill,
      borderColor: palette.stroke,
      textColor: palette.text,
      size: nodeSize(deg) * (0.85 + node.weight * 0.3),
      type: "circle",
      x: pos.x,
      y: pos.y,
    };
    if (!graph.hasNode(node.id)) {
      try {
        graph.addNode(node.id, attrs);
      } catch (error) {
        // Defensive: bad payload should never sink the whole canvas.
        if (typeof console !== "undefined") {
          console.warn("[brain] failed to add node", node.id, error);
        }
      }
    }
  });

  const seenEdgeKeys = new Set<string>();
  for (const edge of work.edges) {
    if (!graph.hasNode(edge.source) || !graph.hasNode(edge.target)) continue;
    const inferred = isInferred(edge);
    const status = inferEdgeStatus(edge);
    const confidence = edgeConfidence(edge);
    const weight =
      typeof confidence === "number"
        ? confidence
        : typeof edge.properties?.weight === "number"
          ? (edge.properties.weight as number)
          : 0.55;
    const attrs: BrainEdgeAttributes = {
      id: edge.id,
      source: edge.source,
      target: edge.target,
      relation: edge.kind,
      label: edge.label,
      weight,
      inferred,
      inferredStatus: status,
      confidence,
      color: edgeColor(weight, inferred, status === "pending"),
      size: inferred ? 1 : Math.max(0.7, weight * 1.6),
      type: "arrow",
    };
    // Some backends emit duplicate edge IDs; namespace them so insert
    // always succeeds even when the upstream payload is messy.
    let key = edge.id;
    if (seenEdgeKeys.has(key)) key = `${edge.id}::${seenEdgeKeys.size}`;
    seenEdgeKeys.add(key);
    try {
      graph.addEdgeWithKey(key, edge.source, edge.target, attrs);
    } catch (error) {
      if (typeof console !== "undefined") {
        console.warn("[brain] failed to add edge", edge.id, error);
      }
    }
  }

  return graph as unknown as BrainGraph;
}

export function collectKinds(work: WorkGraph): Map<string, number> {
  const map = new Map<string, number>();
  for (const node of work.nodes) {
    map.set(node.kind, (map.get(node.kind) ?? 0) + 1);
  }
  return map;
}

export function collectRelations(work: WorkGraph): Map<string, number> {
  const map = new Map<string, number>();
  for (const edge of work.edges) {
    map.set(edge.kind, (map.get(edge.kind) ?? 0) + 1);
  }
  return map;
}

export function neighborhoodIds(
  graph: BrainGraph,
  seedIds: Iterable<string>,
  hops = 1,
): Set<string> {
  const result = new Set<string>();
  let frontier = new Set<string>();
  for (const id of seedIds) {
    if (graph.hasNode(id)) {
      result.add(id);
      frontier.add(id);
    }
  }
  for (let i = 0; i < hops; i += 1) {
    const next = new Set<string>();
    for (const id of frontier) {
      graph.forEachNeighbor(id, (n) => {
        if (!result.has(n)) {
          result.add(n);
          next.add(n);
        }
      });
    }
    frontier = next;
    if (frontier.size === 0) break;
  }
  return result;
}

export function findWorkNode(work: WorkGraph, id: string): WorkGraphNode | null {
  return work.nodes.find((n) => n.id === id) ?? null;
}
