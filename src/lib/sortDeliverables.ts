import type { Deliverable } from "./types";

// Pre-coerce timestamps once rather than calling Date.parse inside the comparator
// (comparators run O(n log n) times; Date.parse per call is expensive on large lists).
export interface SortableDeliverable extends Deliverable {
  _sortTs: number;
}

export function prepareSortable(deliverables: Deliverable[]): SortableDeliverable[] {
  return deliverables.map((d) => ({
    ...d,
    _sortTs: Date.parse(d.shipped_at ?? d.updated_at ?? d.created_at),
  }));
}

// Sort shipped deliverables most-recent first, then by updated_at.
export function compareDeliverablesForLens(
  a: SortableDeliverable,
  b: SortableDeliverable,
): number {
  return b._sortTs - a._sortTs;
}
