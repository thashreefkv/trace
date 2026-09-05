import { useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getBrainGraph, getBrainStatus, rebuildBrain } from "../lib/ipc";
import type { BrainGraphFilters, BrainStatus, WorkGraph } from "../lib/types";
import {
  collectKinds,
  collectRelations,
  toGraphology,
  type BrainGraph,
} from "../lib/brain/graphologyAdapter";

const DEFAULT_FILTERS: BrainGraphFilters = {
  include_dismissed_captures: false,
  include_killed_deliverables: false,
};

export const BRAIN_GRAPH_KEY = "brain-graph";
export const BRAIN_STATUS_KEY = "brain-status";

export interface UseBrainGraphResult {
  isLoading: boolean;
  isFetching: boolean;
  error: unknown;
  raw: WorkGraph | null;
  graph: BrainGraph | null;
  kindCounts: Map<string, number>;
  relationCounts: Map<string, number>;
  refetch: () => Promise<unknown>;
}

export function useBrainGraph(filters: BrainGraphFilters = DEFAULT_FILTERS): UseBrainGraphResult {
  const query = useQuery({
    queryKey: [BRAIN_GRAPH_KEY, filters],
    queryFn: () => getBrainGraph(filters),
    staleTime: 30_000,
    refetchOnWindowFocus: false,
  });

  const graph = useMemo(() => (query.data ? toGraphology(query.data) : null), [query.data]);
  const kindCounts = useMemo(() => (query.data ? collectKinds(query.data) : new Map()), [query.data]);
  const relationCounts = useMemo(
    () => (query.data ? collectRelations(query.data) : new Map()),
    [query.data],
  );

  return {
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    error: query.error,
    raw: query.data ?? null,
    graph,
    kindCounts,
    relationCounts,
    refetch: query.refetch,
  };
}

export function useBrainStatus() {
  const queryClient = useQueryClient();
  const query = useQuery<BrainStatus>({
    queryKey: [BRAIN_STATUS_KEY],
    queryFn: () => getBrainStatus(),
    staleTime: 60_000,
    refetchOnWindowFocus: false,
  });

  const triggerRebuild = async () => {
    const next = await rebuildBrain();
    queryClient.setQueryData([BRAIN_STATUS_KEY], next);
    await queryClient.invalidateQueries({ queryKey: [BRAIN_GRAPH_KEY] });
    return next;
  };

  return { ...query, triggerRebuild };
}
