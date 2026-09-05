import { memo } from "react";
import type { DeliverableState, InitiativeStatus } from "../lib/types";
import { deliverableStateLabels, initiativeStatusLabels } from "../lib/types";

// Unified state/status pill — replaces DeliverableStatePill + StatusPill.
// Uses background + text colors (spec pattern) for all variants.

type Variant =
  | { kind: "deliverable"; state: DeliverableState }
  | { kind: "initiative"; status: InitiativeStatus };

const deliverableClasses: Record<DeliverableState, string> = {
  shipped:   "bg-emerald-50 text-emerald-700",
  in_review: "bg-sky-50 text-sky-700",
  drafting:  "bg-amber-50 text-amber-700",
  todo:      "bg-violet-50 text-violet-700",
  backlog:   "bg-zinc-100 text-zinc-600",
  killed:    "bg-zinc-100 text-zinc-400",
};

const initiativeClasses: Record<InitiativeStatus, string> = {
  live:    "bg-emerald-50 text-emerald-700",
  shipped: "bg-sky-50 text-sky-700",
  paused:  "bg-amber-50 text-amber-700",
  parked:  "bg-zinc-100 text-zinc-400",
};

export const StatePill = memo(function StatePill(props: Variant) {
  let cls: string;
  let label: string;

  if (props.kind === "deliverable") {
    cls = deliverableClasses[props.state];
    label = deliverableStateLabels[props.state];
  } else {
    cls = initiativeClasses[props.status];
    label = initiativeStatusLabels[props.status];
  }

  return (
    <span className={`inline-flex items-center rounded-md px-2 py-0.5 text-[10px] font-semibold uppercase tracking-wider ${cls}`}>
      {label}
    </span>
  );
});
