import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { motion } from "framer-motion";
import { ChevronDown, ChevronRight, Plus, Trash2, Navigation } from "lucide-react";
import {
  getInitiativeGantt,
  createInitiativeSection,
  deleteInitiativeSection,
  updateDeliverableGanttDates,
} from "../lib/ipc";
import type { GanttDeliverable, InitiativeGantt, InitiativeSection } from "../lib/types";
import { deliverableStateLabels, deliverableStateColors } from "../lib/types";

// ── Layout constants ──────────────────────────────────────────────────────────
const WEEK_W = 120;
const DAY_W = WEEK_W / 7;
const ROW_H = 56;
const SECTION_H = 36;
const MONTH_H = 28;
const WEEK_ROW_H = 30;
const HEADER_H = MONTH_H + WEEK_ROW_H;
const SIDEBAR_W = 272;

// ── State color scheme ────────────────────────────────────────────────────────
const STATE_STYLES: Record<string, {
  bar: string;
  text: string;
  dot: string;
  label: string;
}> = {
  blue:  { bar: "bg-sky-500",                     text: "text-white",               dot: "bg-sky-500",    label: "text-sky-600 dark:text-sky-400"     },
  amber: { bar: "bg-amber-400",                   text: "text-amber-950",           dot: "bg-amber-400",  label: "text-amber-600 dark:text-amber-400"  },
  green: { bar: "bg-emerald-500",                 text: "text-white",               dot: "bg-emerald-500",label: "text-emerald-600 dark:text-emerald-400"},
  zinc:  { bar: "bg-zinc-300 dark:bg-zinc-600",   text: "text-zinc-700 dark:text-zinc-200", dot: "bg-zinc-400", label: "text-zinc-400" },
};

// ── Date helpers ──────────────────────────────────────────────────────────────
function parseDate(s: string | null | undefined): Date | null {
  if (!s) return null;
  const d = new Date(s + "T00:00:00");
  return isNaN(d.getTime()) ? null : d;
}

function addDays(d: Date, n: number): Date {
  const r = new Date(d);
  r.setDate(r.getDate() + n);
  return r;
}

function diffDays(a: Date, b: Date): number {
  return Math.round((b.getTime() - a.getTime()) / 86_400_000);
}

function startOfWeek(d: Date): Date {
  const r = new Date(d);
  r.setDate(r.getDate() - r.getDay());
  r.setHours(0, 0, 0, 0);
  return r;
}

function buildRange(start: Date, end: Date) {
  const weekStarts: Date[] = [];
  let cur = startOfWeek(new Date(start));
  const rangeEnd = addDays(end, 0);
  while (cur <= rangeEnd) {
    weekStarts.push(new Date(cur));
    cur = addDays(cur, 7);
  }
  return { rangeStart: start, weekStarts };
}

interface MonthSpan {
  label: string;
  startWeekIdx: number;
  weekCount: number;
}

function computeMonths(weekStarts: Date[]): MonthSpan[] {
  const months: MonthSpan[] = [];
  let current = "";
  let startIdx = 0;
  weekStarts.forEach((ws, i) => {
    const label = ws.toLocaleDateString("en-US", { month: "long", year: "numeric" });
    if (label !== current) {
      if (current) months.push({ label: current, startWeekIdx: startIdx, weekCount: i - startIdx });
      current = label;
      startIdx = i;
    }
  });
  if (current) months.push({ label: current, startWeekIdx: startIdx, weekCount: weekStarts.length - startIdx });
  return months;
}

// ── Main component ────────────────────────────────────────────────────────────

interface GanttViewProps {
  initiativeId: string;
}

export function GanttView({ initiativeId }: GanttViewProps) {
  const [gantt, setGantt] = useState<InitiativeGantt | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [newSectionTitle, setNewSectionTitle] = useState("");
  const [addingSection, setAddingSection] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);
  const didScroll = useRef(false);

  useEffect(() => { void load(); }, [initiativeId]); // eslint-disable-line react-hooks/exhaustive-deps

  async function load() {
    try {
      setError(null);
      setIsLoading(true);
      didScroll.current = false;
      const data = await getInitiativeGantt(initiativeId);
      setGantt(data);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleAddSection() {
    if (!newSectionTitle.trim() || !gantt) return;
    try {
      const section = await createInitiativeSection({ initiative_id: initiativeId, title: newSectionTitle.trim() });
      setGantt(g => g ? { ...g, sections: [...g.sections, section] } : g);
      setNewSectionTitle("");
      setAddingSection(false);
    } catch (caught) { setError(String(caught)); }
  }

  async function handleDeleteSection(id: string) {
    if (!window.confirm("Delete this section? Deliverables will become ungrouped.")) return;
    try {
      await deleteInitiativeSection(id);
      setGantt(g => g ? {
        ...g,
        sections: g.sections.filter(s => s.id !== id),
        deliverables: g.deliverables.map(d => d.section_id === id ? { ...d, section_id: null } : d),
      } : g);
    } catch (caught) { setError(String(caught)); }
  }

  async function handleDateChange(id: string, startDate: string | null, deadline: string | null) {
    try {
      await updateDeliverableGanttDates(id, { start_date: startDate, deadline });
      setGantt(g => g ? {
        ...g,
        deliverables: g.deliverables.map(d => d.id === id ? { ...d, start_date: startDate, deadline } : d),
      } : g);
    } catch (caught) { setError(String(caught)); }
  }

  const { rangeStart, weekStarts } = useMemo(() => {
    if (!gantt?.deliverables.length) {
      const today = new Date();
      return buildRange(startOfWeek(addDays(today, -14)), addDays(today, 70));
    }
    const dates: Date[] = [new Date()];
    for (const d of gantt.deliverables) {
      if (d.start_date) dates.push(new Date(d.start_date));
      if (d.deadline) dates.push(new Date(d.deadline));
    }
    const min = dates.reduce((a, b) => a < b ? a : b);
    const max = dates.reduce((a, b) => a > b ? a : b);
    return buildRange(startOfWeek(addDays(min, -14)), addDays(max, 28));
  }, [gantt]);

  const months = useMemo(() => computeMonths(weekStarts), [weekStarts]);
  const today = new Date();
  const todayOffsetPx = diffDays(rangeStart, today) * DAY_W;
  const gridWidth = weekStarts.length * WEEK_W;

  // Auto-scroll to center today on first load
  useEffect(() => {
    if (!scrollRef.current || isLoading || didScroll.current) return;
    didScroll.current = true;
    const containerW = scrollRef.current.clientWidth - SIDEBAR_W;
    scrollRef.current.scrollLeft = Math.max(0, todayOffsetPx - containerW / 2);
  }, [isLoading, todayOffsetPx]);

  function scrollToToday() {
    if (!scrollRef.current) return;
    const containerW = scrollRef.current.clientWidth - SIDEBAR_W;
    scrollRef.current.scrollTo({ left: Math.max(0, todayOffsetPx - containerW / 2), behavior: "smooth" });
  }

  const grouped = useMemo(() => {
    if (!gantt) return { sections: [], ungrouped: [] };
    return {
      sections: gantt.sections.map(s => ({
        section: s,
        deliverables: gantt.deliverables.filter(d => d.section_id === s.id),
      })),
      ungrouped: gantt.deliverables.filter(d => !d.section_id),
    };
  }, [gantt]);

  if (isLoading) return <p className="px-4 py-6 text-sm text-zinc-400">Loading timeline…</p>;
  if (error) return <div className="px-4 py-4 text-sm text-red-500">{error}</div>;
  if (!gantt) return null;

  return (
    <div className="flex flex-col gap-3">
      {/* Toolbar */}
      <div className="flex items-center gap-2">
        <p className="flex-1 text-[11px] font-semibold uppercase tracking-widest text-zinc-400">
          Timeline · {gantt.deliverables.length} deliverable{gantt.deliverables.length !== 1 ? "s" : ""}
        </p>
        <button className="btn text-xs" onClick={scrollToToday} type="button" title="Jump to today">
          <Navigation size={12} />
          Today
        </button>
        <button className="btn text-xs" onClick={() => setAddingSection(true)} type="button">
          <Plus size={13} />
          Add section
        </button>
      </div>

      {addingSection && (
        <div className="flex items-center gap-2">
          <input
            autoFocus
            className="field-control h-8 flex-1 text-sm"
            placeholder="Section name…"
            value={newSectionTitle}
            onChange={e => setNewSectionTitle(e.currentTarget.value)}
            onKeyDown={e => {
              if (e.key === "Enter") void handleAddSection();
              if (e.key === "Escape") setAddingSection(false);
            }}
          />
          <button className="btn btn-primary text-xs" onClick={() => void handleAddSection()} type="button">Add</button>
          <button className="btn text-xs" onClick={() => setAddingSection(false)} type="button">Cancel</button>
        </div>
      )}

      {/* Gantt grid */}
      <div
        ref={scrollRef}
        className="overflow-x-auto rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)] dark:border-zinc-800 dark:bg-zinc-950"
      >
        <div style={{ width: SIDEBAR_W + gridWidth, minWidth: SIDEBAR_W + 480 }}>
          <GanttTimeAxis weekStarts={weekStarts} months={months} todayOffsetPx={todayOffsetPx} />

          <div className="relative">
            <GanttGridBackground weekStarts={weekStarts} months={months} />

            {/* Today line + badge */}
            {todayOffsetPx >= 0 && todayOffsetPx <= gridWidth && (
              <div
                className="pointer-events-none absolute top-0 bottom-0 z-10"
                style={{ left: SIDEBAR_W + todayOffsetPx }}
              >
                <div className="relative h-full">
                  <div className="absolute inset-0 w-0.5 bg-orange-400" />
                  <div className="absolute top-2 left-1/2 -translate-x-1/2 whitespace-nowrap rounded-full bg-orange-400 px-2 py-0.5 text-[8px] font-bold uppercase tracking-wider text-white shadow-sm">
                    Today
                  </div>
                </div>
              </div>
            )}

            {/* Named sections */}
            {grouped.sections.map(({ section, deliverables }) => (
              <SectionGroup
                key={section.id}
                section={section}
                deliverables={deliverables}
                collapsed={collapsed.has(section.id)}
                onCollapse={() => setCollapsed(c => {
                  const next = new Set(c);
                  if (next.has(section.id)) next.delete(section.id); else next.add(section.id);
                  return next;
                })}
                onDelete={() => void handleDeleteSection(section.id)}
                onDateChange={handleDateChange}
                rangeStart={rangeStart}
              />
            ))}

            {/* Ungrouped */}
            {grouped.ungrouped.length > 0 && (
              <>
                <SectionHeaderRow
                  label={gantt.sections.length > 0 ? "Ungrouped" : "All deliverables"}
                  count={grouped.ungrouped.length}
                  collapsed={collapsed.has("__ungrouped")}
                  onCollapse={() => setCollapsed(c => {
                    const next = new Set(c);
                    if (next.has("__ungrouped")) next.delete("__ungrouped"); else next.add("__ungrouped");
                    return next;
                  })}
                />
                {!collapsed.has("__ungrouped") && grouped.ungrouped.map(d => (
                  <GanttRow key={d.id} deliverable={d} rangeStart={rangeStart} onDateChange={handleDateChange} />
                ))}
              </>
            )}

            {gantt.deliverables.length === 0 && (
              <div className="flex items-center justify-center py-16 text-sm text-zinc-400">
                No deliverables in this initiative yet.
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Time axis header ──────────────────────────────────────────────────────────

function GanttTimeAxis({ weekStarts, months, todayOffsetPx }: {
  weekStarts: Date[];
  months: MonthSpan[];
  todayOffsetPx: number;
}) {
  const today = new Date();
  return (
    <div
      className="sticky top-0 z-30 flex border-b border-zinc-200/60 bg-white/95 backdrop-blur-md dark:border-zinc-700 dark:bg-zinc-950/95"
      style={{ height: HEADER_H }}
    >
      {/* Sidebar label */}
      <div
        className="flex shrink-0 items-end border-r border-zinc-200/60 px-4 pb-2 dark:border-zinc-700"
        style={{ width: SIDEBAR_W }}
      >
        <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-400">Deliverable</span>
      </div>

      {/* Timeline columns */}
      <div className="relative flex-1 overflow-hidden">
        {/* Month row */}
        <div className="flex border-b border-zinc-100 dark:border-zinc-800" style={{ height: MONTH_H }}>
          {months.map((m, i) => (
            <div
              key={i}
              className={[
                "flex shrink-0 items-center border-r px-3",
                i % 2 === 0
                  ? "border-zinc-100 dark:border-zinc-800"
                  : "border-zinc-100 bg-zinc-50/60 dark:border-zinc-800 dark:bg-zinc-900/30",
              ].join(" ")}
              style={{ width: m.weekCount * WEEK_W, height: MONTH_H }}
            >
              <span className="text-[10px] font-bold uppercase tracking-wider text-zinc-500 dark:text-zinc-300">
                {m.label}
              </span>
            </div>
          ))}
        </div>

        {/* Week row */}
        <div className="flex" style={{ height: WEEK_ROW_H }}>
          {weekStarts.map((ws, i) => {
            const isCurrentWeek = ws <= today && addDays(ws, 6) >= today;
            return (
              <div
                key={i}
                className="flex shrink-0 items-center border-r border-zinc-100/80 px-2.5 dark:border-zinc-800"
                style={{ width: WEEK_W, height: WEEK_ROW_H }}
              >
                <span className={[
                  "text-[10px] font-medium",
                  isCurrentWeek ? "font-bold text-orange-500" : "text-zinc-400",
                ].join(" ")}>
                  {ws.toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                </span>
              </div>
            );
          })}
        </div>

        {/* Today marker in header */}
        {todayOffsetPx >= 0 && (
          <div
            className="pointer-events-none absolute top-0 bottom-0 w-0.5 bg-orange-400/50"
            style={{ left: todayOffsetPx }}
          />
        )}
      </div>
    </div>
  );
}

// ── Grid background ───────────────────────────────────────────────────────────

function GanttGridBackground({ weekStarts, months }: { weekStarts: Date[]; months: MonthSpan[] }) {
  return (
    <div className="pointer-events-none absolute inset-0" style={{ left: SIDEBAR_W }}>
      {/* Alternating month bands */}
      {months.map((m, i) =>
        i % 2 === 1 ? (
          <div
            key={i}
            className="absolute top-0 bottom-0 bg-zinc-50/70 dark:bg-zinc-900/30"
            style={{ left: m.startWeekIdx * WEEK_W, width: m.weekCount * WEEK_W }}
          />
        ) : null
      )}
      {/* Week dividers */}
      {weekStarts.map((_, i) => (
        <div
          key={i}
          className="absolute top-0 bottom-0 w-px bg-zinc-100/80 dark:bg-zinc-800/60"
          style={{ left: i * WEEK_W }}
        />
      ))}
    </div>
  );
}

// ── Section components ────────────────────────────────────────────────────────

function SectionGroup({ section, deliverables, collapsed, onCollapse, onDelete, onDateChange, rangeStart }: {
  section: InitiativeSection;
  deliverables: GanttDeliverable[];
  collapsed: boolean;
  onCollapse: () => void;
  onDelete: () => void;
  onDateChange: (id: string, start: string | null, end: string | null) => void;
  rangeStart: Date;
}) {
  return (
    <>
      <SectionHeaderRow
        label={section.title}
        count={deliverables.length}
        collapsed={collapsed}
        onCollapse={onCollapse}
        onDelete={onDelete}
      />
      {!collapsed && deliverables.map(d => (
        <GanttRow key={d.id} deliverable={d} rangeStart={rangeStart} onDateChange={onDateChange} />
      ))}
    </>
  );
}

function SectionHeaderRow({ label, count, collapsed, onCollapse, onDelete }: {
  label: string;
  count: number;
  collapsed: boolean;
  onCollapse: () => void;
  onDelete?: () => void;
}) {
  return (
    <div
      className="group flex items-center border-b border-zinc-100 bg-zinc-50/70 dark:border-zinc-800 dark:bg-zinc-900/50"
      style={{ height: SECTION_H }}
    >
      <div
        className="flex shrink-0 items-center gap-1.5 border-r border-zinc-100/80 px-3 dark:border-zinc-800"
        style={{ width: SIDEBAR_W }}
      >
        <button className="flex items-center gap-1.5 text-left outline-none" onClick={onCollapse} type="button">
          {collapsed
            ? <ChevronRight size={11} className="shrink-0 text-zinc-400" />
            : <ChevronDown size={11} className="shrink-0 text-zinc-400" />}
          <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-600 dark:text-zinc-300">
            {label}
          </span>
          <span className="text-[9px] text-zinc-400">({count})</span>
        </button>
        {onDelete && (
          <button
            className="ml-auto text-zinc-300 opacity-0 transition-all hover:text-red-500 group-hover:opacity-100"
            onClick={onDelete}
            type="button"
          >
            <Trash2 size={11} />
          </button>
        )}
      </div>
      <div className="flex-1" />
    </div>
  );
}

// ── Gantt row ─────────────────────────────────────────────────────────────────

function GanttRow({ deliverable, rangeStart, onDateChange }: {
  deliverable: GanttDeliverable;
  rangeStart: Date;
  onDateChange: (id: string, start: string | null, end: string | null) => void;
}) {
  const colorKey = deliverableStateColors[deliverable.state as keyof typeof deliverableStateColors] ?? "blue";
  const styles = STATE_STYLES[colorKey];

  const startDate = parseDate(deliverable.start_date) ?? parseDate(deliverable.deadline);
  const endDate = parseDate(deliverable.deadline) ?? (startDate ? addDays(startDate, 7) : null);
  const hasNoDates = !deliverable.start_date && !deliverable.deadline;

  const leftPx = startDate ? Math.max(0, diffDays(rangeStart, startDate) * DAY_W) : 0;
  const widthPx = startDate && endDate
    ? Math.max(WEEK_W * 0.6, diffDays(startDate, endDate) * DAY_W)
    : 0;

  const progress = deliverable.task_count > 0
    ? deliverable.done_task_count / deliverable.task_count
    : deliverable.state === "shipped" ? 1 : 0;

  return (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      className="group flex items-center border-b border-zinc-100/60 transition-colors hover:bg-zinc-50/60 dark:border-zinc-800/60 dark:hover:bg-zinc-900/20"
      style={{ height: ROW_H }}
    >
      {/* Sidebar */}
      <div
        className="flex h-full shrink-0 items-center gap-2.5 border-r border-zinc-100/60 px-4 dark:border-zinc-800/60"
        style={{ width: SIDEBAR_W }}
      >
        <div className={["h-2 w-2 shrink-0 rounded-full", styles.dot].join(" ")} />
        <div className="min-w-0 flex-1">
          <Link
            className="block truncate text-[12.5px] font-medium text-zinc-800 transition-colors hover:text-sky-600 dark:text-zinc-200"
            to={`/deliverables/${deliverable.id}`}
          >
            {deliverable.title}
          </Link>
          <span className={["text-[10px] font-medium", styles.label].join(" ")}>
            {deliverableStateLabels[deliverable.state as keyof typeof deliverableStateLabels] ?? deliverable.state}
            {deliverable.task_count > 0 && (
              <span className="ml-1.5 text-zinc-400">
                · {deliverable.done_task_count}/{deliverable.task_count}
              </span>
            )}
          </span>
        </div>
      </div>

      {/* Timeline bar area */}
      <div className="relative h-full flex-1">
        {hasNoDates ? (
          <div className="flex h-full items-center px-4">
            <DateSetPrompt deliverableId={deliverable.id} onDateChange={onDateChange} />
          </div>
        ) : (
          <div
            className="absolute top-1/2 -translate-y-1/2"
            style={{ left: leftPx, width: widthPx }}
          >
            <GanttBar
              deliverable={deliverable}
              styles={styles}
              progress={progress}
              onDateChange={onDateChange}
            />
          </div>
        )}
      </div>
    </motion.div>
  );
}

// ── Gantt bar ─────────────────────────────────────────────────────────────────

function GanttBar({ deliverable, styles, progress, onDateChange }: {
  deliverable: GanttDeliverable;
  styles: (typeof STATE_STYLES)[string];
  progress: number;
  onDateChange: (id: string, start: string | null, end: string | null) => void;
}) {
  const [showEdit, setShowEdit] = useState(false);

  const fmt = (s: string | null) =>
    s ? new Date(s + "T00:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" }) : "—";

  const tooltip = [
    deliverable.title,
    `${fmt(deliverable.start_date)} → ${fmt(deliverable.deadline)}`,
    deliverable.task_count > 0 ? `${Math.round(progress * 100)}% complete` : null,
  ].filter(Boolean).join("  ·  ");

  return (
    <div className="relative">
      <button
        type="button"
        className={[
          "relative flex w-full items-center overflow-hidden rounded-full px-3 shadow-sm transition-all hover:brightness-95 active:brightness-90",
          styles.bar,
        ].join(" ")}
        style={{ height: 34 }}
        onClick={() => setShowEdit(v => !v)}
        title={tooltip}
      >
        {/* Progress fill */}
        {progress > 0 && (
          <div
            className="absolute inset-y-0 left-0 rounded-full bg-black/20"
            style={{ width: `${progress * 100}%` }}
          />
        )}
        {/* Label */}
        <span className={["relative z-10 block truncate text-[11px] font-semibold select-none leading-none", styles.text].join(" ")}>
          {deliverable.title}
          {deliverable.task_count > 0 && (
            <span className="ml-1.5 text-[10px] font-normal opacity-70">
              {Math.round(progress * 100)}%
            </span>
          )}
        </span>
      </button>

      {showEdit && (
        <DateEditPopup
          startDate={deliverable.start_date ?? ""}
          endDate={deliverable.deadline ?? ""}
          onSave={(s, e) => {
            onDateChange(deliverable.id, s || null, e || null);
            setShowEdit(false);
          }}
          onClose={() => setShowEdit(false)}
        />
      )}
    </div>
  );
}

// ── Date helpers ──────────────────────────────────────────────────────────────

function DateSetPrompt({ deliverableId, onDateChange }: {
  deliverableId: string;
  onDateChange: (id: string, start: string | null, end: string | null) => void;
}) {
  const [show, setShow] = useState(false);
  if (!show) {
    return (
      <button
        className="text-[10px] text-zinc-300 transition-colors hover:text-zinc-500"
        onClick={() => setShow(true)}
        type="button"
      >
        + set dates
      </button>
    );
  }
  return (
    <DateEditPopup
      startDate=""
      endDate=""
      onSave={(s, e) => { onDateChange(deliverableId, s || null, e || null); setShow(false); }}
      onClose={() => setShow(false)}
    />
  );
}

function DateEditPopup({ startDate, endDate, onSave, onClose }: {
  startDate: string;
  endDate: string;
  onSave: (start: string, end: string) => void;
  onClose: () => void;
}) {
  const [s, setS] = useState(startDate);
  const [e, setE] = useState(endDate);

  return (
    <div className="absolute left-0 top-full z-50 mt-2 flex items-end gap-3 rounded-2xl border border-zinc-200 bg-white p-4 shadow-2xl dark:border-zinc-700 dark:bg-zinc-900">
      <div className="space-y-1.5">
        <label className="block text-[9px] font-bold uppercase tracking-widest text-zinc-400">Start</label>
        <input autoFocus type="date" className="field-control h-7 w-32 text-xs" value={s} onChange={ev => setS(ev.currentTarget.value)} />
      </div>
      <div className="space-y-1.5">
        <label className="block text-[9px] font-bold uppercase tracking-widest text-zinc-400">End</label>
        <input type="date" className="field-control h-7 w-32 text-xs" value={e} onChange={ev => setE(ev.currentTarget.value)} />
      </div>
      <div className="flex gap-1.5">
        <button className="btn btn-primary h-7 px-3 text-xs" onClick={() => onSave(s, e)} type="button">Save</button>
        <button className="btn h-7 px-3 text-xs" onClick={onClose} type="button">✕</button>
      </div>
    </div>
  );
}
