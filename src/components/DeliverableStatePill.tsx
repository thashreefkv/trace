import type { DeliverableState } from "../lib/types";
import { deliverableStateLabels } from "../lib/types";

const stateClasses: Record<DeliverableState, string> = {
  backlog: "text-zinc-500 dark:text-zinc-400",
  todo: "text-violet-600 dark:text-violet-400",
  drafting: "text-zinc-400 dark:text-zinc-500",
  in_review: "text-amber-600 dark:text-amber-500",
  shipped: "text-emerald-600 dark:text-emerald-500",
  killed: "text-red-500 dark:text-red-600",
};

interface DeliverableStatePillProps {
  state: DeliverableState;
}

export function DeliverableStatePill({ state }: DeliverableStatePillProps) {
  return (
    <span className={`inline-flex items-center text-[10px] font-bold tracking-widest uppercase ${stateClasses[state]}`}>
      {deliverableStateLabels[state]}
    </span>
  );
}
