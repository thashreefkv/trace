import { interpolateMagma, interpolateTurbo, interpolateViridis } from "d3-scale-chromatic";
import { interpolateRgb } from "d3-interpolate";
import type { BrainGraph } from "./graphologyAdapter";

export type ColorMode = "kind" | "recency" | "centrality" | "community";

export interface ColorOverlayContext {
  /** Map of nodeId → community color, populated when communities are loaded. */
  communityColors?: Map<string, string>;
  /** Reference "now" timestamp (ms). Defaults to Date.now() at compute time. */
  referenceNow?: number;
}

export interface ColorOverlay {
  mode: ColorMode;
  label: string;
  /** Per-node colour, indexed by node id. */
  nodeColor: Map<string, string>;
  /** Optional gradient stops for the legend. */
  legendStops?: Array<{ offset: number; color: string; label?: string }>;
  /** Optional gradient track label. */
  legendDescription?: string;
}

const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Build a color reducer override for the active overlay mode. Returns null
 * for "kind" — in that case the default per-kind palette already baked into
 * each node's `color` attribute is correct and no override is needed.
 */
export function computeColorOverlay(
  graph: BrainGraph,
  mode: ColorMode,
  ctx: ColorOverlayContext = {},
): ColorOverlay | null {
  if (mode === "kind") return null;
  if (mode === "recency") return buildRecencyOverlay(graph, ctx.referenceNow ?? Date.now());
  if (mode === "centrality") return buildCentralityOverlay(graph);
  if (mode === "community") return buildCommunityOverlay(graph, ctx.communityColors ?? new Map());
  return null;
}

function buildRecencyOverlay(graph: BrainGraph, now: number): ColorOverlay {
  const ages: Array<{ id: string; ageDays: number | null }> = [];
  graph.forEachNode((id, attrs) => {
    const ts = attrs.updated_at ? Date.parse(attrs.updated_at) : NaN;
    const age = Number.isNaN(ts) ? null : (now - ts) / DAY_MS;
    ages.push({ id, ageDays: age });
  });
  const valid = ages.filter((a) => a.ageDays != null) as Array<{ id: string; ageDays: number }>;
  const maxAge = valid.length > 0 ? Math.max(...valid.map((a) => a.ageDays)) : 1;
  const nodeColor = new Map<string, string>();
  for (const { id, ageDays } of ages) {
    if (ageDays == null) {
      nodeColor.set(id, "#e4e4e7");
      continue;
    }
    // Newer = warmer. Reverse the magma scale so fresh wins the warm end.
    const t = 1 - Math.min(1, ageDays / Math.max(maxAge, 1));
    nodeColor.set(id, interpolateMagma(0.15 + t * 0.7));
  }
  return {
    mode: "recency",
    label: "Recency",
    nodeColor,
    legendDescription: `last ${Math.round(maxAge)}d → today`,
    legendStops: [
      { offset: 0, color: interpolateMagma(0.15), label: "stale" },
      { offset: 0.5, color: interpolateMagma(0.5) },
      { offset: 1, color: interpolateMagma(0.85), label: "fresh" },
    ],
  };
}

function buildCentralityOverlay(graph: BrainGraph): ColorOverlay {
  const degrees: Array<{ id: string; degree: number }> = [];
  graph.forEachNode((id) => {
    degrees.push({ id, degree: graph.degree(id) });
  });
  const max = degrees.reduce((m, d) => Math.max(m, d.degree), 1);
  const nodeColor = new Map<string, string>();
  for (const { id, degree } of degrees) {
    const t = degree / max;
    nodeColor.set(id, interpolateViridis(0.1 + t * 0.8));
  }
  return {
    mode: "centrality",
    label: "Connections",
    nodeColor,
    legendDescription: `0 → ${max} edges`,
    legendStops: [
      { offset: 0, color: interpolateViridis(0.1), label: "few" },
      { offset: 0.5, color: interpolateViridis(0.5) },
      { offset: 1, color: interpolateViridis(0.9), label: "many" },
    ],
  };
}

function buildCommunityOverlay(
  graph: BrainGraph,
  communityColors: Map<string, string>,
): ColorOverlay {
  const nodeColor = new Map<string, string>();
  graph.forEachNode((id) => {
    nodeColor.set(id, communityColors.get(id) ?? "#e4e4e7");
  });
  return {
    mode: "community",
    label: "Community",
    nodeColor,
    legendDescription:
      communityColors.size > 0
        ? `${communityColors.size} entities tinted by GraphRAG community`
        : "Enable Community hulls to populate",
  };
}

/**
 * Lighten or darken a colour by mixing with white/black. Used for selection
 * halos in overlay modes that override the base kind color.
 */
export function mixWithWhite(color: string, t: number): string {
  return interpolateRgb(color, "#ffffff")(t);
}

/**
 * Pick a "good" text colour for a given background colour using YIQ luma.
 * Returns one of the two zinc shades that match the design system.
 */
export function readableTextOn(color: string): string {
  const rgb = parseColor(color);
  const yiq = (rgb[0] * 299 + rgb[1] * 587 + rgb[2] * 114) / 1000;
  return yiq >= 160 ? "#1f2937" : "#fafafa";
}

function parseColor(color: string): [number, number, number] {
  if (color.startsWith("#")) {
    const hex = color.slice(1);
    const norm = hex.length === 3 ? hex.split("").map((c) => c + c).join("") : hex;
    return [
      parseInt(norm.slice(0, 2), 16),
      parseInt(norm.slice(2, 4), 16),
      parseInt(norm.slice(4, 6), 16),
    ];
  }
  // rgb(R, G, B) or rgba(...) — defensive parse.
  const match = color.match(/(\d+(?:\.\d+)?)/g);
  if (!match || match.length < 3) return [200, 200, 200];
  return [Number(match[0]), Number(match[1]), Number(match[2])];
}

// Force the gradient import to stay live so tree-shaking doesn't drop it for
// future overlay modes that reference `interpolateTurbo` directly.
void interpolateTurbo;
