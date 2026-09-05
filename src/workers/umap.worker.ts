// UMAP web worker. Receives a flat embedding matrix and returns 2D positions.
// Keeps the main thread at 60fps even while UMAP iterates (5k × 1024 dims ≈ 3-4s
// the first time; second visits hit the SQLite layout cache and skip this).

import { UMAP } from "umap-js";

interface UmapJob {
  ids: string[];
  vectors: number[][]; // length = ids.length, each inner array same dim
  options?: { nNeighbors?: number; minDist?: number; nEpochs?: number };
}

interface UmapResult {
  ids: string[];
  positions: number[]; // [x0, y0, x1, y1, ...]
}

type WorkerCtx = {
  addEventListener(type: "message", listener: (event: MessageEvent<UmapJob>) => void): void;
  postMessage(message: UmapResult): void;
};

const ctx = self as unknown as WorkerCtx;

ctx.addEventListener("message", (event: MessageEvent<UmapJob>) => {
  const { ids, vectors, options } = event.data;
  if (!Array.isArray(vectors) || vectors.length === 0) {
    ctx.postMessage({ ids, positions: [] } satisfies UmapResult);
    return;
  }
  try {
    const umap = new UMAP({
      nNeighbors: options?.nNeighbors ?? 15,
      minDist: options?.minDist ?? 0.1,
      nEpochs: options?.nEpochs ?? 200,
      nComponents: 2,
    });
    const embedded = umap.fit(vectors);
    // Normalize to [-1500, 1500].
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const [x, y] of embedded) {
      if (x < minX) minX = x;
      if (y < minY) minY = y;
      if (x > maxX) maxX = x;
      if (y > maxY) maxY = y;
    }
    const w = maxX - minX || 1;
    const h = maxY - minY || 1;
    const scale = 3000 / Math.max(w, h);
    const positions = new Array(embedded.length * 2);
    for (let i = 0; i < embedded.length; i++) {
      const [x, y] = embedded[i];
      positions[i * 2] = (x - (minX + maxX) / 2) * scale;
      positions[i * 2 + 1] = (y - (minY + maxY) / 2) * scale;
    }
    ctx.postMessage({ ids, positions } satisfies UmapResult);
  } catch (err) {
    ctx.postMessage({ ids, positions: [], error: String(err) } as unknown as UmapResult);
  }
});

export {}; // Marks this as a module — required for TS worker compilation.
