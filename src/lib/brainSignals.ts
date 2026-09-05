import { recordBrainLearningEvent } from "./ipc";
import type { BrainTemplateKind } from "./types";

type BrainSignalEvent =
  | "shown"
  | "clicked"
  | "opened"
  | "useful"
  | "wrong"
  | "ignored"
  | "completed_after_seen"
  | "accepted_inference"
  | "rejected_inference"
  | "manual_link_created"
  | "dismissed"
  | "snoozed";

export interface BrainSignalInput {
  template?: BrainTemplateKind | string | null;
  itemId: string;
  itemKind?: string | null;
  eventType: BrainSignalEvent;
  reward?: number | null;
  context?: Record<string, unknown> | null;
}

const impressionKeys = new Set<string>();

export async function recordBrainSignal(input: BrainSignalInput) {
  const itemId = input.itemId.trim();
  if (!itemId) return;

  await recordBrainLearningEvent({
    template: input.template ?? null,
    item_id: itemId,
    item_kind: input.itemKind ?? null,
    event_type: input.eventType,
    reward: input.reward ?? null,
    context: input.context ?? null,
  }).catch(() => {});
}

export function recordBrainImpression(input: BrainSignalInput) {
  const key = `${input.template ?? "global"}:${input.itemId}:${input.eventType}`;
  if (impressionKeys.has(key)) return;
  impressionKeys.add(key);
  void recordBrainSignal({ ...input, eventType: "shown", reward: 0 });
}
