import { memo } from "react";
import { useDraggable } from "@dnd-kit/core";
import { CSS } from "@dnd-kit/utilities";
import { Link } from "react-router-dom";
import { motion } from "framer-motion";
import { Star, AlertTriangle, CalendarClock, Layers3, ArrowUp, ArrowDown, Clock } from "lucide-react";
import type { Deliverable, DeliverableState } from "../lib/types";
import { deliverableStateColors, priorityColors, priorityLabels, labelColors } from "../lib/types";
import { DeliverableTypeBadge } from "./DeliverableTypeBadge";

interface DeliverableCardProps {
  deliverable: Deliverable;
  onMoveState?: (id: string, state: DeliverableState) => void;
  onMoveUp?: () => void;
  onMoveDown?: () => void;
}

function daysInColumn(stateChangedAt: string | null | undefined): number | null {
  if (!stateChangedAt) return null;
  const diff = Date.now() - new Date(stateChangedAt).getTime();
  return Math.floor(diff / (1000 * 60 * 60 * 24));
}

const keyStateMap: Record<string, DeliverableState> = {
  "1": "drafting",
  "2": "in_review",
  "3": "shipped",
  "4": "killed",
};

function isOverdue(deadline: string | null): boolean {
  if (!deadline) return false;
  return new Date(deadline) < new Date();
}

export const DeliverableCard = memo(function DeliverableCard({ deliverable, onMoveState, onMoveUp, onMoveDown }: DeliverableCardProps) {
  const { attributes, listeners, setNodeRef, transform, isDragging } = useDraggable({
    id: deliverable.id,
    data: { state: deliverable.state },
  });

  const style = { transform: CSS.Translate.toString(transform) };
  const overdue = isOverdue(deliverable.deadline);
  const color = deliverableStateColors[deliverable.state];
  const primaryInitiative = deliverable.initiatives[0] ?? null;
  const days = daysInColumn(deliverable.state_changed_at);

  return (
    <article
      className="group relative outline-none"
      onKeyDown={(event) => {
        const nextState = keyStateMap[event.key];
        if (nextState && onMoveState) {
          event.preventDefault();
          onMoveState(deliverable.id, nextState);
        }
      }}
      ref={setNodeRef}
      style={style}
      {...attributes}
      {...listeners}
    >
      <Link to={`/deliverables/${deliverable.id}`} className="block">
        <motion.div
          layout
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          whileHover={{ y: -2, scale: 1.012 }}
          whileTap={{ scale: 0.98 }}
          transition={{ type: "spring", stiffness: 420, damping: 28 }}
          className={[
            "motion-panel flex flex-col gap-0 overflow-hidden",
            isDragging ? "shadow-2xl opacity-90 z-50 scale-[1.02]" : "",
            deliverable.is_focused
              ? "ring-1 ring-amber-400/70"
              : deliverable.blocker_reason
                ? "ring-1 ring-red-300/70"
                : "",
          ].join(" ")}
        >
          {/* Top accent bar — colored by state */}
          <div
            className={[
              "h-[3px] w-full rounded-t-xl",
              color === "blue"
                ? "bg-blue-400"
                : color === "amber"
                  ? "bg-amber-400"
                  : color === "green"
                    ? "bg-emerald-400"
                    : color === "violet"
                      ? "bg-violet-400"
                      : "bg-zinc-300",
            ].join(" ")}
          />

          <div className="flex flex-col gap-1 p-3.5">
            {/* Blocked banner */}
            {deliverable.blocker_reason && (
              <div className="mb-1 flex items-center gap-1.5 rounded-md bg-red-50 px-2 py-1 text-[11px] font-medium text-red-600 dark:bg-red-950/40 dark:text-red-400">
                <AlertTriangle size={10} className="shrink-0" />
                <span className="line-clamp-1">{deliverable.blocker_reason}</span>
              </div>
            )}

            {/* Title row */}
            <div className="flex items-start justify-between gap-2 px-0.5">
              <span className="block text-[13.5px] font-semibold leading-snug text-zinc-900 group-hover:text-sky-600 transition-colors dark:text-zinc-100 pr-5">
                {deliverable.title}
              </span>
              <div className="flex shrink-0 items-center gap-1 mt-0.5">
                {deliverable.priority && (
                  <span className={`rounded px-1 py-0.5 text-[10px] font-bold ${priorityColors[deliverable.priority]}`}>
                    {priorityLabels[deliverable.priority]}
                  </span>
                )}
                {deliverable.is_focused && (
                  <Star
                    aria-label="Focused"
                    className="fill-amber-400 text-amber-400"
                    size={12}
                  />
                )}
              </div>
            </div>

            {deliverable.claim ? (
              <p className="px-0.5 text-[12px] leading-relaxed text-zinc-500 line-clamp-2 dark:text-zinc-400">
                {deliverable.claim}
              </p>
            ) : null}

            {/* Labels row */}
            {deliverable.labels && deliverable.labels.length > 0 && (
              <div className="flex flex-wrap gap-1 px-0.5 mt-0.5">
                {deliverable.labels.map((lbl) => {
                  const lc = labelColors[lbl.color] ?? labelColors["zinc"];
                  return (
                    <span
                      key={lbl.id}
                      className={`rounded-full px-1.5 py-0.5 text-[10px] font-medium ${lc.bg} ${lc.text}`}
                    >
                      {lbl.name}
                    </span>
                  );
                })}
              </div>
            )}

            <div className="mt-2 flex flex-wrap items-center gap-1.5 px-0.5">
              <DeliverableTypeBadge type={deliverable.type} />

              {primaryInitiative && (
                <span className="inline-flex items-center gap-1 rounded-md bg-violet-50 px-1.5 py-0.5 text-[10px] font-medium text-violet-600 dark:bg-violet-950/40 dark:text-violet-400">
                  <Layers3 size={9} />
                  <span className="max-w-[80px] truncate">{primaryInitiative.title}</span>
                </span>
              )}

              {deliverable.stakeholder_name ? (
                <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-400">
                  {deliverable.stakeholder_name}
                </span>
              ) : null}

              {deliverable.deadline && (
                <span
                  className={[
                    "inline-flex items-center gap-1 text-[10px] font-medium",
                    overdue ? "text-red-500" : "text-zinc-400",
                  ].join(" ")}
                >
                  <CalendarClock size={10} />
                  {deliverable.deadline}
                </span>
              )}

              {days !== null && days > 0 && (
                <span className="inline-flex items-center gap-1 text-[10px] font-medium text-zinc-400">
                  <Clock size={10} />
                  {days}d
                </span>
              )}
            </div>
          </div>
        </motion.div>
      </Link>

      {/* Within-column reorder buttons — shown on hover, outside Link to prevent navigation */}
      {(onMoveUp || onMoveDown) && (
        <div className="absolute right-1.5 top-1.5 z-10 flex flex-col gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
          <button
            aria-label="Move up"
            className="rounded p-0.5 bg-white/80 hover:bg-white shadow-sm text-zinc-400 hover:text-zinc-700 disabled:opacity-20 disabled:cursor-not-allowed"
            disabled={!onMoveUp}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); onMoveUp?.(); }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <ArrowUp size={11} />
          </button>
          <button
            aria-label="Move down"
            className="rounded p-0.5 bg-white/80 hover:bg-white shadow-sm text-zinc-400 hover:text-zinc-700 disabled:opacity-20 disabled:cursor-not-allowed"
            disabled={!onMoveDown}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); onMoveDown?.(); }}
            onMouseDown={(e) => e.stopPropagation()}
          >
            <ArrowDown size={11} />
          </button>
        </div>
      )}
    </article>
  );
});
