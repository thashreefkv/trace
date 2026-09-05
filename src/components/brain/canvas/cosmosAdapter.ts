import type { BrainEdgeAttributes, BrainGraph, BrainNodeAttributes } from "../../../lib/brain/graphologyAdapter";

export interface CosmosBuffers {
  positions: Float32Array;       // [x0, y0, x1, y1, ...]
  colors: Float32Array;          // [r, g, b, a] per node, 0..1 floats
  sizes: Float32Array;
  links: Float32Array;           // [src0, tgt0, src1, tgt1, ...] indices
  linkColors: Float32Array;
  linkWidths: Float32Array;
  indexToId: string[];
  idToIndex: Map<string, number>;
}

const RGBA_LEN = 4;

// Cosmos likes ~0..1 RGBA. Our nodes carry CSS hex (#RRGGBB) or "rgba(r,g,b,a)" /
// "rgb(r,g,b)" / "hsl(...)" strings (from d3-scale-chromatic). Parse the common
// cases and fall back to a neutral grey.
function parseColor(input: string | undefined, alpha = 1): [number, number, number, number] {
  if (!input) return [0.6, 0.6, 0.65, alpha];
  const s = input.trim();
  if (s.startsWith("#")) {
    const hex = s.slice(1);
    if (hex.length === 3) {
      const r = parseInt(hex[0] + hex[0], 16) / 255;
      const g = parseInt(hex[1] + hex[1], 16) / 255;
      const b = parseInt(hex[2] + hex[2], 16) / 255;
      return [r, g, b, alpha];
    }
    if (hex.length === 6) {
      const r = parseInt(hex.slice(0, 2), 16) / 255;
      const g = parseInt(hex.slice(2, 4), 16) / 255;
      const b = parseInt(hex.slice(4, 6), 16) / 255;
      return [r, g, b, alpha];
    }
  }
  const rgbMatch = s.match(/rgba?\(([^)]+)\)/i);
  if (rgbMatch) {
    const parts = rgbMatch[1].split(/[,\s]+/).map((p) => p.trim()).filter(Boolean);
    const r = (parseFloat(parts[0]) || 0) / 255;
    const g = (parseFloat(parts[1]) || 0) / 255;
    const b = (parseFloat(parts[2]) || 0) / 255;
    const a = parts[3] != null ? parseFloat(parts[3]) : alpha;
    return [r, g, b, Number.isFinite(a) ? a : alpha];
  }
  const hslMatch = s.match(/hsla?\(\s*([\d.]+)[,\s]+([\d.]+)%[,\s]+([\d.]+)%/i);
  if (hslMatch) {
    return hslToRgb(parseFloat(hslMatch[1]), parseFloat(hslMatch[2]) / 100, parseFloat(hslMatch[3]) / 100, alpha);
  }
  return [0.6, 0.6, 0.65, alpha];
}

function hslToRgb(h: number, s: number, l: number, a: number): [number, number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const hp = ((h % 360) + 360) % 360 / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  let rp = 0, gp = 0, bp = 0;
  if (hp < 1) { rp = c; gp = x; }
  else if (hp < 2) { rp = x; gp = c; }
  else if (hp < 3) { gp = c; bp = x; }
  else if (hp < 4) { gp = x; bp = c; }
  else if (hp < 5) { rp = x; bp = c; }
  else { rp = c; bp = x; }
  const m = l - c / 2;
  return [rp + m, gp + m, bp + m, a];
}

// Convert a graphology MultiDirectedGraph into Cosmos' flat Float32 buffers.
// We snapshot the graph at call time — when the source graph mutates (filters,
// new entities), call this again and pass the new buffers to Cosmos.
export function cosmosFromGraphology(graph: BrainGraph): CosmosBuffers {
  const n = graph.order;
  const positions = new Float32Array(n * 2);
  const colors = new Float32Array(n * RGBA_LEN);
  const sizes = new Float32Array(n);
  const indexToId: string[] = new Array(n);
  const idToIndex = new Map<string, number>();

  let i = 0;
  graph.forEachNode((id, raw) => {
    const attrs = raw as Partial<BrainNodeAttributes>;
    indexToId[i] = id;
    idToIndex.set(id, i);
    positions[i * 2] = (attrs.x as number | undefined) ?? 0;
    positions[i * 2 + 1] = (attrs.y as number | undefined) ?? 0;
    const [r, g, b, a] = parseColor(attrs.color as string | undefined, 0.92);
    colors[i * RGBA_LEN] = r;
    colors[i * RGBA_LEN + 1] = g;
    colors[i * RGBA_LEN + 2] = b;
    colors[i * RGBA_LEN + 3] = a;
    sizes[i] = Math.max(2, Math.min(24, (attrs.size as number | undefined) ?? 6));
    i++;
  });

  const m = graph.size;
  const links = new Float32Array(m * 2);
  const linkColors = new Float32Array(m * RGBA_LEN);
  const linkWidths = new Float32Array(m);

  let j = 0;
  graph.forEachEdge((_edgeId, raw, source, target) => {
    const attrs = raw as Partial<BrainEdgeAttributes>;
    const src = idToIndex.get(source);
    const tgt = idToIndex.get(target);
    if (src == null || tgt == null) {
      // Cosmos can't address an absent node — write a self-loop on 0 as a
      // harmless filler; it won't render visibly.
      links[j * 2] = 0;
      links[j * 2 + 1] = 0;
      linkColors[j * RGBA_LEN + 3] = 0;
      j++;
      return;
    }
    links[j * 2] = src;
    links[j * 2 + 1] = tgt;
    const baseColor = attrs.color as string | undefined;
    const isInferred = (attrs.inferred as boolean | undefined) === true;
    const colorAlpha = isInferred ? 0.55 : 0.42;
    const [r, g, b, a] = parseColor(baseColor ?? "#a1a1aa", colorAlpha);
    linkColors[j * RGBA_LEN] = r;
    linkColors[j * RGBA_LEN + 1] = g;
    linkColors[j * RGBA_LEN + 2] = b;
    linkColors[j * RGBA_LEN + 3] = a;
    linkWidths[j] = Math.max(0.6, Math.min(3, (attrs.size as number | undefined) ?? 1));
    j++;
  });

  return { positions, colors, sizes, links, linkColors, linkWidths, indexToId, idToIndex };
}

// Apply a colour overlay map (id → CSS color) onto an existing colour buffer
// without re-snapshotting the whole graph. Returns the mutated buffer for
// convenience — caller should still pass it to setPointColors.
export function applyColorOverlay(
  baseColors: Float32Array,
  idToIndex: Map<string, number>,
  overlay: Map<string, string> | null,
  defaultAlpha = 0.92,
): Float32Array {
  if (!overlay || overlay.size === 0) return baseColors;
  for (const [id, color] of overlay) {
    const idx = idToIndex.get(id);
    if (idx == null) continue;
    const [r, g, b, a] = parseColor(color, defaultAlpha);
    baseColors[idx * RGBA_LEN] = r;
    baseColors[idx * RGBA_LEN + 1] = g;
    baseColors[idx * RGBA_LEN + 2] = b;
    baseColors[idx * RGBA_LEN + 3] = a;
  }
  return baseColors;
}
