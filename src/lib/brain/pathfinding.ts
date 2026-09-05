import { bidirectional } from "graphology-shortest-path/unweighted";
import type { WorkGraph, WorkGraphEdge, WorkGraphNode } from "../types";
import type { BrainGraph } from "./graphologyAdapter";

export interface PathHop {
  fromNode: WorkGraphNode;
  edge: WorkGraphEdge;
  toNode: WorkGraphNode;
  direction: "forward" | "reverse";
}

export interface PathResult {
  nodeIds: string[];
  edgeIds: string[];
  hops: PathHop[];
}

/**
 * Compute the shortest path between two nodes, treating the graph as
 * undirected (we still know the original edge direction per hop). Returns
 * null if no connection exists.
 */
export function findShortestPath(
  graph: BrainGraph,
  work: WorkGraph,
  fromId: string,
  toId: string,
): PathResult | null {
  if (!graph.hasNode(fromId) || !graph.hasNode(toId)) return null;

  const undirected = graph.copy({ type: "undirected" });
  const ids = bidirectional(undirected, fromId, toId) as string[] | null;
  if (!ids || ids.length < 2) return null;

  const nodeById = new Map(work.nodes.map((n) => [n.id, n]));
  const hops: PathHop[] = [];
  const edgeIds: string[] = [];

  for (let i = 0; i < ids.length - 1; i += 1) {
    const a = ids[i];
    const b = ids[i + 1];
    let edgeKey: string | undefined;
    let direction: "forward" | "reverse" = "forward";
    if (graph.hasEdge(a, b)) {
      edgeKey = graph.edge(a, b);
    } else if (graph.hasEdge(b, a)) {
      edgeKey = graph.edge(b, a);
      direction = "reverse";
    }
    if (!edgeKey) continue;
    const edgeAttrs = graph.getEdgeAttributes(edgeKey);
    const sourceNode = nodeById.get(a);
    const targetNode = nodeById.get(b);
    if (!sourceNode || !targetNode) continue;
    edgeIds.push(edgeKey);
    const workEdge = work.edges.find((e) => e.id === edgeKey) ?? {
      id: edgeKey,
      source: edgeAttrs.source,
      target: edgeAttrs.target,
      kind: edgeAttrs.relation,
      label: edgeAttrs.label,
      properties: {},
    };
    hops.push({ fromNode: sourceNode, edge: workEdge, toNode: targetNode, direction });
  }

  return { nodeIds: ids, edgeIds, hops };
}

export function formatHopRelation(hop: PathHop): string {
  const rel = hop.edge.label || hop.edge.kind;
  return hop.direction === "forward" ? `—[${rel}]→` : `←[${rel}]—`;
}
