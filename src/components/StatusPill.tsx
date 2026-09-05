import type { InitiativeStatus } from "../lib/types";
import { initiativeStatusLabels } from "../lib/types";

const statusClasses: Record<InitiativeStatus, string> = {
  live: "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200",
  paused: "border-amber-200 bg-amber-50 text-amber-800 dark:border-amber-900 dark:bg-amber-950 dark:text-amber-200",
  shipped: "border-sky-200 bg-sky-50 text-sky-800 dark:border-sky-900 dark:bg-sky-950 dark:text-sky-200",
  parked: "border-zinc-200 bg-zinc-100 text-zinc-600 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-300",
};

interface StatusPillProps {
  status: InitiativeStatus;
}

export function StatusPill({ status }: StatusPillProps) {
  return (
    <span
      className={`inline-flex h-6 items-center rounded-md border px-2 text-xs font-medium ${statusClasses[status]}`}
    >
      {initiativeStatusLabels[status]}
    </span>
  );
}
