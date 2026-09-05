import { useQuery, useMutation, type QueryKey } from "@tanstack/react-query";
import { queryClient } from "./queryClient";
import type { CaptureFilters, DeliverableFilters, GmailThreadFilter, WorkMailQuery } from "./types";

function stableKey<T extends object>(obj: T): string {
  return JSON.stringify(obj, Object.keys(obj as Record<string, unknown>).sort());
}

// ── Typed IPC hooks ───────────────────────────────────────────────────────────

export function useIpcQuery<TData>(
  key: QueryKey,
  fetcher: () => Promise<TData>,
  opts?: { enabled?: boolean; staleTime?: number },
) {
  return useQuery<TData, Error>({
    queryKey: key,
    queryFn: fetcher,
    ...opts,
  });
}

export function useIpcMutation<TVars, TData = unknown>(
  mutationFn: (vars: TVars) => Promise<TData>,
  opts?: {
    invalidates?: QueryKey[];
    onSuccess?: (data: TData, vars: TVars) => void;
    onError?: (error: Error, vars: TVars) => void;
  },
) {
  return useMutation<TData, Error, TVars>({
    mutationFn,
    onSuccess: (data, vars) => {
      if (opts?.invalidates) {
        opts.invalidates.forEach((key) => {
          void queryClient.invalidateQueries({ queryKey: key });
        });
      }
      opts?.onSuccess?.(data, vars);
    },
    onError: opts?.onError,
  });
}

// ── Query key factory ─────────────────────────────────────────────────────────
//
// Nested structure so invalidation uses prefix matching:
//   queryClient.invalidateQueries({ queryKey: qk.deliverables.all })
// drops every deliverable filter variant.

export const qk = {
  deliverables: {
    all: ["deliverables"] as const,
    list: (filters?: DeliverableFilters) =>
      ["deliverables", "list", stableKey(filters ?? {})] as const,
    byId: (id: string) => ["deliverables", "byId", id] as const,
    forStakeholder: (id: string, stateKey?: string) =>
      ["deliverables", "byStakeholder", id, stateKey ?? "all"] as const,
  },
  initiatives: {
    all: ["initiatives"] as const,
    list: ["initiatives", "list"] as const,
    byId: (id: string) => ["initiatives", "byId", id] as const,
  },
  stakeholders: {
    all: ["stakeholders"] as const,
    list: ["stakeholders", "list"] as const,
    details: ["stakeholders", "details"] as const,
    byId: (id: string) => ["stakeholders", "byId", id] as const,
  },
  gmail: {
    all: ["gmail"] as const,
    status: ["gmail", "status"] as const,
    threads: (filters?: GmailThreadFilter) =>
      ["gmail", "threads", stableKey(filters ?? {})] as const,
    workMailThreads: (query?: WorkMailQuery) =>
      ["gmail", "workMailThreads", stableKey(query ?? {})] as const,
    workMailBrief: ["gmail", "workMailBrief"] as const,
    workMailCounts: ["gmail", "workMailCounts"] as const,
    workMailActivity: (limit?: number) =>
      ["gmail", "workMailActivity", limit ?? 50] as const,
    categoryCounts: (scope?: string) =>
      ["gmail", "categoryCounts", scope ?? "all"] as const,
    stakeholderSuggestions: ["gmail", "stakeholderSuggestions"] as const,
    weeklyDigest: ["gmail", "weeklyDigest"] as const,
    stakeholderThreads: (id: string, limit?: number) =>
      ["gmail", "stakeholderThreads", id, limit ?? 100] as const,
    stakeholderHealth: (id: string) => ["gmail", "stakeholderHealth", id] as const,
  },
  gcal: {
    all: ["gcal"] as const,
    stakeholderEvents: (email: string) => ["gcal", "stakeholderEvents", email] as const,
  },
  ask: {
    all: ["ask"] as const,
    chats: (limit?: number) => ["ask", "chats", limit ?? 60] as const,
    chat: (id: string) => ["ask", "chat", id] as const,
  },
  captures: {
    all: ["captures"] as const,
    list: (filters?: CaptureFilters) =>
      ["captures", "list", stableKey(filters ?? {})] as const,
    suggestion: (captureId: string) => ["captures", "suggestion", captureId] as const,
  },
};
