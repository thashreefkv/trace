import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  CalendarDays,
  CalendarRange,
  ChevronLeft,
  ChevronRight,
  Clock,
  ExternalLink,
  LayoutGrid,
  ListChecks,
  Loader2,
  MapPin,
  Plus,
  RefreshCw,
  Sparkles,
  Star,
  Trash2,
  Users,
  Video,
  X,
} from "lucide-react";
import type {
  Deliverable,
  GCalEvent,
  GCalStatus,
  GeminiKeyStatus,
  Stakeholder,
  WeekDay,
  WeekTask,
  WeekView as WeekViewData,
} from "../lib/types";
import { parseGCalConferenceData, parseGCalEvent } from "../lib/gcal";
import {
  advanceMeetingDate,
  createGcalMeeting,
  deleteGcalMeeting,
  gcalConnect,
  gcalDisconnect,
  gcalSync,
  generateWeekPlan,
  getGeminiKeyStatus,
  getWeekView,
  listDeliverables,
  listStakeholderDetails,
  openExternalUrl,
  setDayDeliverable,
  setMeetingDate,
} from "../lib/ipc";

const DAY_NAMES = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"];

function getWeekStart(offset = 0): string {
  const now = new Date();
  const dow = now.getDay(); // 0=Sun, 1=Mon...
  const diff = dow === 0 ? -6 : 1 - dow; // how many days to Monday
  const monday = new Date(now);
  monday.setDate(now.getDate() + diff + offset * 7);
  return monday.toISOString().slice(0, 10);
}

function parseLocalDate(dateStr: string): Date {
  const [y, m, d] = dateStr.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function formatDateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function daysUntil(dateStr: string): number {
  const target = parseLocalDate(dateStr);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  return Math.round((target.getTime() - today.getTime()) / 86_400_000);
}

function weekLabel(weekStart: string): string {
  const start = parseLocalDate(weekStart);
  const end = new Date(start);
  end.setDate(start.getDate() + 4);
  const opts: Intl.DateTimeFormatOptions = { month: "short", day: "numeric" };
  return `${start.toLocaleDateString(undefined, opts)} – ${end.toLocaleDateString(undefined, opts)}`;
}

function dateForDay(weekStart: string, dayIndex: number): string {
  const date = parseLocalDate(weekStart);
  date.setDate(date.getDate() + dayIndex);
  return formatDateKey(date);
}

function deliverableScore(d: Deliverable): number {
  return (d.impact ?? 0) - (d.effort ?? 5);
}

// Returns "YYYY-MM-DD" Monday dates for all weeks that overlap a given month.
function weekStartsForMonth(monthStart: Date): string[] {
  const dow = monthStart.getDay();
  const firstMonday = new Date(monthStart);
  firstMonday.setDate(monthStart.getDate() + (dow === 0 ? -6 : 1 - dow));
  const lastOfMonth = new Date(monthStart.getFullYear(), monthStart.getMonth() + 1, 0);
  const starts: string[] = [];
  const cur = new Date(firstMonday);
  while (cur <= lastOfMonth) {
    starts.push(formatDateKey(cur));
    cur.setDate(cur.getDate() + 7);
  }
  return starts;
}

export function WeekView() {
  const [weekOffset, setWeekOffset] = useState(0);
  const weekStart = useMemo(() => getWeekStart(weekOffset), [weekOffset]);

  const [view, setView] = useState<WeekViewData | null>(null);
  const [allDeliverables, setAllDeliverables] = useState<Deliverable[]>([]);
  const [loading, setLoading] = useState(true);
  const [planLoading, setPlanLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [geminiStatus, setGeminiStatus] = useState<GeminiKeyStatus | null>(null);
  const [gcalStatus, setGcalStatus] = useState<GCalStatus | null>(null);
  const [gcalSyncing, setGcalSyncing] = useState(false);
  const [gcalConnecting, setGcalConnecting] = useState(false);
  const [stakeholdersByEmail, setStakeholdersByEmail] = useState<Map<string, Stakeholder>>(new Map());
  const [allStakeholders, setAllStakeholders] = useState<Stakeholder[]>([]);

  // Schedule meeting dialog
  const [schedulingDate, setSchedulingDate] = useState<string | null>(null);

  // Picker state: which column has its picker open
  const [pickerDay, setPickerDay] = useState<number | null>(null);
  const [pickerSearch, setPickerSearch] = useState("");

  // Meeting date editor
  const [editingMeeting, setEditingMeeting] = useState(false);
  const [meetingInput, setMeetingInput] = useState("");
  const [meetingLoading, setMeetingLoading] = useState(false);

  // Advance meeting confirmation
  const [advanceLoading, setAdvanceLoading] = useState(false);

  // View mode: week or month
  const [viewMode, setViewMode] = useState<"week" | "month">("week");
  const [monthOffset, setMonthOffset] = useState(0);
  const [monthData, setMonthData] = useState<Map<string, WeekDay>>(new Map());
  const [monthLoading, setMonthLoading] = useState(false);

  const currentMonthStart = useMemo(() => {
    const now = new Date();
    return new Date(now.getFullYear(), now.getMonth() + monthOffset, 1);
  }, [monthOffset]);

  async function load() {
    setLoading(true);
    setError(null);
    try {
      const [v, dels, keyStatus, stakeholderDetails] = await Promise.all([
        getWeekView(weekStart),
        listDeliverables(),
        getGeminiKeyStatus(),
        listStakeholderDetails(),
      ]);
      setView(v);
      setAllDeliverables(dels);
      setGeminiStatus(keyStatus);
      setGcalStatus({ connected: v.gcal_connected });
      const emailMap = new Map<string, Stakeholder>();
      for (const d of stakeholderDetails) {
        if (d.stakeholder.email) emailMap.set(d.stakeholder.email.toLowerCase(), d.stakeholder);
      }
      setStakeholdersByEmail(emailMap);
      setAllStakeholders(stakeholderDetails.map((d) => d.stakeholder));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [weekStart]);

  async function loadMonthData(monthStart: Date) {
    setMonthLoading(true);
    try {
      const starts = weekStartsForMonth(monthStart);
      const views = await Promise.all(starts.map((ws) => getWeekView(ws)));
      const map = new Map<string, WeekDay>();
      for (const v of views) {
        const ws = parseLocalDate(v.week_start);
        for (const day of v.days) {
          const d = new Date(ws);
          d.setDate(ws.getDate() + day.day_index);
          map.set(formatDateKey(d), day);
        }
      }
      setMonthData(map);
    } catch {
      // silently fail
    } finally {
      setMonthLoading(false);
    }
  }

  useEffect(() => {
    if (viewMode === "month") void loadMonthData(currentMonthStart);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [viewMode, currentMonthStart]);

  async function handleSetDay(dayIndex: number, deliverableId: string | null) {
    if (!view) return;
    try {
      const updated = await setDayDeliverable({ week_start: weekStart, day_index: dayIndex, deliverable_id: deliverableId });
      setView(updated);
    } catch (e) {
      setError(String(e));
    }
    setPickerDay(null);
    setPickerSearch("");
  }

  async function handlePlanWithAI() {
    setPlanLoading(true);
    setError(null);
    try {
      const plan = await generateWeekPlan(weekStart);
      let v = view;
      for (const a of plan.assignments) {
        if (v) v = await setDayDeliverable({ week_start: weekStart, day_index: a.day_index, deliverable_id: a.deliverable_id });
      }
      if (v) setView(v);
    } catch (e) {
      setError(String(e));
    } finally {
      setPlanLoading(false);
    }
  }

  async function handleSaveMeeting() {
    setMeetingLoading(true);
    try {
      const updated = await setMeetingDate(meetingInput.trim() || null);
      setView((v) => v ? { ...v, meeting_date: updated.next_meeting_date } : v);
      setEditingMeeting(false);
    } catch (e) {
      setError(String(e));
    } finally {
      setMeetingLoading(false);
    }
  }

  async function handleAdvanceMeeting() {
    setAdvanceLoading(true);
    try {
      const updated = await advanceMeetingDate();
      setView((v) => v ? { ...v, meeting_date: updated.next_meeting_date } : v);
    } catch (e) {
      setError(String(e));
    } finally {
      setAdvanceLoading(false);
    }
  }

  async function handleGcalConnect() {
    setGcalConnecting(true);
    setError(null);
    try {
      const status = await gcalConnect();
      setGcalStatus(status);
      // Reload to pick up freshly synced events
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setGcalConnecting(false);
    }
  }

  async function handleGcalDisconnect() {
    try {
      const status = await gcalDisconnect();
      setGcalStatus(status);
      setView((v) => v ? { ...v, gcal_connected: false, days: v.days.map((d) => ({ ...d, gcal_events: [] })) } : v);
    } catch (e) {
      setError(String(e));
    }
  }

  async function handleGcalSync() {
    setGcalSyncing(true);
    setError(null);
    try {
      await gcalSync();
      await load();
    } catch (e) {
      setError(String(e));
    } finally {
      setGcalSyncing(false);
    }
  }

  const meetingCountdown = view?.meeting_date ? daysUntil(view.meeting_date) : null;

  // Deliverables already assigned this week (for highlighting in picker)
  const assignedIds = new Set(view?.days.map((d) => d.deliverable?.id).filter(Boolean));

  // Active deliverables for the picker (non-shipped, non-killed)
  const activeDeliverables = useMemo(() => {
    return allDeliverables
      .filter((d) => d.state === "drafting" || d.state === "in_review")
      .sort((a, b) => deliverableScore(b) - deliverableScore(a));
  }, [allDeliverables]);

  const filteredPicker = useMemo(() => {
    const q = pickerSearch.toLowerCase();
    return activeDeliverables.filter(
      (d) => !q || d.title.toLowerCase().includes(q) || d.claim.toLowerCase().includes(q),
    );
  }, [activeDeliverables, pickerSearch]);

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <Loader2 className="animate-spin text-zinc-400" size={20} />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {/* Top bar */}
      <div className="flex flex-wrap items-center gap-3 border-b border-zinc-200/60 bg-[var(--surface-header)] px-5 py-3 backdrop-blur-md">
        {/* Week nav */}
        <div className="flex items-center gap-1.5">
          <button
            onClick={() => setWeekOffset((o) => o - 1)}
            className="flex h-7 w-7 items-center justify-center rounded hover:bg-zinc-100"
          >
            <ChevronLeft size={14} />
          </button>
          <span className="text-[13px] font-medium text-zinc-800">
            {weekLabel(weekStart)}
          </span>
          <button
            onClick={() => setWeekOffset((o) => o + 1)}
            className="flex h-7 w-7 items-center justify-center rounded hover:bg-zinc-100"
          >
            <ChevronRight size={14} />
          </button>
          {weekOffset !== 0 && (
            <button
              onClick={() => setWeekOffset(0)}
              className="ml-1 text-[11px] text-zinc-400 hover:text-zinc-700"
            >
              This week
            </button>
          )}
        </div>

        <div className="h-4 w-px bg-zinc-200" />

        {/* Meeting countdown */}
        <MeetingBadge
          meetingDate={view?.meeting_date ?? null}
          countdown={meetingCountdown}
          editing={editingMeeting}
          meetingInput={meetingInput}
          loading={meetingLoading}
          advanceLoading={advanceLoading}
          onStartEdit={() => {
            setMeetingInput(view?.meeting_date ?? "");
            setEditingMeeting(true);
          }}
          onInputChange={setMeetingInput}
          onSave={handleSaveMeeting}
          onCancel={() => setEditingMeeting(false)}
          onAdvance={handleAdvanceMeeting}
        />

        <div className="ml-auto flex items-center gap-2">
          {geminiStatus && !geminiStatus.configured && (
            <Link
              to="/settings/connections?focus=gemini-key"
              className="flex items-center gap-1 text-[11px] text-amber-600 hover:text-amber-800"
              title="Gemini API key not configured"
            >
              <AlertTriangle size={12} />
              No AI key
            </Link>
          )}

          {/* Google Calendar */}
          {gcalStatus?.connected ? (
            <div className="flex items-center gap-1">
              <button
                onClick={() => setSchedulingDate(formatDateKey(new Date()))}
                title="Schedule a meeting"
                className="flex h-7 items-center gap-1.5 rounded px-2 text-[11px] text-zinc-500 hover:bg-zinc-100"
              >
                <Plus size={12} />
                Schedule
              </button>
              <button
                onClick={handleGcalSync}
                disabled={gcalSyncing}
                title="Sync with Google Calendar"
                className="flex h-7 items-center gap-1.5 rounded px-2 text-[11px] text-zinc-500 hover:bg-zinc-100 disabled:opacity-40"
              >
                <RefreshCw size={12} className={gcalSyncing ? "animate-spin" : ""} />
                {gcalSyncing ? "Syncing…" : "Sync GCal"}
              </button>
              <button
                onClick={handleGcalDisconnect}
                title="Disconnect Google Calendar"
                className="text-[10px] text-zinc-300 hover:text-zinc-500"
              >
                <X size={11} />
              </button>
            </div>
          ) : (
            <button
              onClick={handleGcalConnect}
              disabled={gcalConnecting}
              title="Connect Google Calendar for bidirectional sync"
              className="flex h-7 items-center gap-1.5 rounded px-2 text-[11px] text-zinc-500 hover:bg-zinc-100 disabled:opacity-40"
            >
              <CalendarDays size={12} />
              {gcalConnecting ? "Connecting…" : "Connect GCal"}
            </button>
          )}

          <div className="h-4 w-px bg-zinc-200" />

          <button
            onClick={handlePlanWithAI}
            disabled={planLoading || geminiStatus?.configured === false}
            title={geminiStatus?.configured === false ? "Configure a Gemini API key in Settings first" : "Plan my week with AI"}
            className="btn flex items-center gap-1.5 text-[12px] disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {planLoading ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Sparkles size={13} />
            )}
            Plan with AI
          </button>

          <div className="h-4 w-px bg-zinc-200" />

          {/* Week / Month toggle */}
          <div className="flex items-center rounded-lg border border-zinc-200 p-0.5">
            <button
              onClick={() => setViewMode("week")}
              className={[
                "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[12px] font-medium transition-colors",
                viewMode === "week"
                  ? "bg-zinc-900 text-white"
                  : "text-zinc-500 hover:text-zinc-700",
              ].join(" ")}
            >
              <CalendarRange size={12} />
              Week
            </button>
            <button
              onClick={() => setViewMode("month")}
              className={[
                "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[12px] font-medium transition-colors",
                viewMode === "month"
                  ? "bg-zinc-900 text-white"
                  : "text-zinc-500 hover:text-zinc-700",
              ].join(" ")}
            >
              <LayoutGrid size={12} />
              Month
            </button>
          </div>
        </div>
      </div>

      {error && (
        <div className="mx-5 mt-3 flex items-center gap-2 rounded-md bg-red-50 px-3 py-2 text-[12px] text-red-600">
          <span className="flex-1">{error}</span>
          {error.toLowerCase().includes("key") || error.toLowerCase().includes("gemini") ? (
            <Link to="/settings/connections?focus=gemini-key" className="shrink-0 font-medium underline hover:text-red-800">
              Open Settings →
            </Link>
          ) : null}
          <button onClick={() => setError(null)} className="shrink-0 text-red-400 hover:text-red-600">
            <X size={13} />
          </button>
        </div>
      )}

      {/* Schedule meeting dialog */}
      {schedulingDate !== null && (
        <ScheduleMeetingDialog
          defaultDate={schedulingDate}
          stakeholders={allStakeholders}
          onClose={() => setSchedulingDate(null)}
          onCreated={() => {
            setSchedulingDate(null);
            void load();
          }}
        />
      )}

      {/* Week or Month grid */}
      {viewMode === "week" ? (
        <div className="flex min-h-0 flex-1 overflow-auto px-4 py-4 gap-3">
          {DAY_NAMES.map((dayName, idx) => {
            const day = view?.days.find((d) => d.day_index === idx);
            const deliverable = day?.deliverable ?? null;
            const isMeetingDay =
              view?.meeting_date &&
              parseLocalDate(view.meeting_date).toLocaleDateString(undefined, { weekday: "long" }) === dayName;

            return (
              <DayColumn
                key={idx}
                dayIndex={idx}
                dayName={dayName}
                date={dateForDay(weekStart, idx)}
                deliverable={deliverable}
                deadlineDeliverables={day?.deadline_deliverables ?? []}
                tasks={day?.tasks ?? []}
                gcalEvents={day?.gcal_events ?? []}
                stakeholdersByEmail={stakeholdersByEmail}
                isMeetingDay={!!isMeetingDay}
                pickerOpen={pickerDay === idx}
                pickerSearch={pickerSearch}
                filteredPicker={filteredPicker}
                assignedIds={assignedIds}
                onOpenPicker={() => {
                  setPickerDay(idx);
                  setPickerSearch("");
                }}
                onClosePicker={() => setPickerDay(null)}
                onPickerSearch={setPickerSearch}
                onAssign={(deliverableId) => handleSetDay(idx, deliverableId)}
                onClear={() => handleSetDay(idx, null)}
                onSchedule={(date) => setSchedulingDate(date)}
                onEventDeleted={() => void load()}
              />
            );
          })}
        </div>
      ) : (
        <MonthGrid
          monthStart={currentMonthStart}
          data={monthData}
          loading={monthLoading}
          stakeholdersByEmail={stakeholdersByEmail}
          onNavigate={(dir) => setMonthOffset((o) => o + dir)}
          onSchedule={(date) => setSchedulingDate(date)}
          onEventDeleted={() => void loadMonthData(currentMonthStart)}
        />
      )}
    </div>
  );
}

// ── Month view ────────────────────────────────────────────────────────────────

const MONTH_DAY_LABELS = ["Mon", "Tue", "Wed", "Thu", "Fri"];

function MonthGrid({
  monthStart,
  data,
  loading,
  stakeholdersByEmail,
  onNavigate,
  onSchedule,
  onEventDeleted,
}: {
  monthStart: Date;
  data: Map<string, WeekDay>;
  loading: boolean;
  stakeholdersByEmail: Map<string, Stakeholder>;
  onNavigate: (dir: -1 | 1) => void;
  onSchedule: (date: string) => void;
  onEventDeleted: () => void;
}) {
  const [selectedEvent, setSelectedEvent] = useState<GCalEvent | null>(null);
  const weeks = weekStartsForMonth(monthStart);
  const today = formatDateKey(new Date());
  const monthLabel = monthStart.toLocaleDateString("en-US", { month: "long", year: "numeric" });

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-auto px-6 py-5">
      {/* Month navigation */}
      <div className="mb-5 flex items-center gap-3">
        <button
          onClick={() => onNavigate(-1)}
          className="flex h-7 w-7 items-center justify-center rounded-lg hover:bg-zinc-100 text-zinc-500"
        >
          <ChevronLeft size={14} />
        </button>
        <span className="text-[15px] font-semibold text-zinc-900 min-w-[140px]">{monthLabel}</span>
        <button
          onClick={() => onNavigate(1)}
          className="flex h-7 w-7 items-center justify-center rounded-lg hover:bg-zinc-100 text-zinc-500"
        >
          <ChevronRight size={14} />
        </button>
        {loading && <Loader2 size={13} className="animate-spin text-zinc-300 ml-1" />}
      </div>

      {/* Calendar grid */}
      <div className="overflow-hidden rounded-xl border border-zinc-200">
        {/* Day-of-week header */}
        <div className="grid grid-cols-5 border-b border-zinc-200 bg-zinc-50">
          {MONTH_DAY_LABELS.map((d, i) => (
            <div
              key={d}
              className={[
                "py-2 text-center text-[10px] font-semibold uppercase tracking-widest text-zinc-400",
                i < 4 ? "border-r border-zinc-200" : "",
              ].join(" ")}
            >
              {d}
            </div>
          ))}
        </div>

        {/* Week rows — shared-border grid */}
        {weeks.map((ws, wi) => (
          <div
            key={ws}
            className={["grid grid-cols-5", wi < weeks.length - 1 ? "border-b border-zinc-200" : ""].join(" ")}
          >
            {[0, 1, 2, 3, 4].map((di) => {
              const d = parseLocalDate(ws);
              d.setDate(d.getDate() + di);
              const dateKey = formatDateKey(d);
              const day = data.get(dateKey);
              const isToday = dateKey === today;
              const inMonth = d.getMonth() === monthStart.getMonth();

              return (
                <MonthDayCell
                  key={di}
                  dateKey={dateKey}
                  dayNum={d.getDate()}
                  day={day}
                  isToday={isToday}
                  inMonth={inMonth}
                  hasBorderRight={di < 4}
                  onSelectEvent={setSelectedEvent}
                  onSchedule={onSchedule}
                />
              );
            })}
          </div>
        ))}
      </div>

      {selectedEvent && (
        <GCalEventDialog
          event={selectedEvent}
          stakeholdersByEmail={stakeholdersByEmail}
          onClose={() => setSelectedEvent(null)}
          onDeleted={() => {
            setSelectedEvent(null);
            onEventDeleted();
          }}
        />
      )}
    </div>
  );
}

function MonthDayCell({
  dateKey,
  dayNum,
  day,
  isToday,
  inMonth,
  hasBorderRight,
  onSelectEvent,
  onSchedule,
}: {
  dateKey: string;
  dayNum: number;
  day: WeekDay | undefined;
  isToday: boolean;
  inMonth: boolean;
  hasBorderRight: boolean;
  onSelectEvent: (ev: GCalEvent) => void;
  onSchedule: (date: string) => void;
}) {
  // Filter out all-day events (working location, OOO markers, etc.)
  const events = (day?.gcal_events ?? []).filter((ev) => !ev.is_all_day);
  const deadlines = day?.deadline_deliverables ?? [];
  const MAX_VISIBLE = 3;
  const overflow = events.length + deadlines.length - MAX_VISIBLE;

  return (
    <div
      className={[
        "group relative flex min-h-[100px] flex-col gap-1 p-2.5 transition-colors",
        hasBorderRight ? "border-r border-zinc-200" : "",
        isToday ? "bg-sky-50/40" : inMonth ? "bg-white hover:bg-zinc-50/60" : "bg-zinc-50/50",
        !inMonth ? "opacity-40" : "",
      ].join(" ")}
    >
      {/* Date number */}
      <div className="mb-0.5 flex items-center justify-between">
        <span
          className={[
            "flex h-6 w-6 items-center justify-center rounded-full text-[12px] font-semibold",
            isToday
              ? "bg-sky-500 text-white"
              : inMonth
              ? "text-zinc-700"
              : "text-zinc-300",
          ].join(" ")}
        >
          {dayNum}
        </span>
        <button
          onClick={() => onSchedule(dateKey)}
          title="Schedule meeting"
          className="hidden h-5 w-5 items-center justify-center rounded text-zinc-300 hover:bg-zinc-200 hover:text-sky-500 group-hover:flex"
        >
          <Plus size={11} />
        </button>
      </div>

      {/* Events (capped at MAX_VISIBLE combined with deadlines) */}
      {events.slice(0, MAX_VISIBLE).map((ev, i) => {
        if (i + deadlines.slice(0, MAX_VISIBLE).length > MAX_VISIBLE - 1 && deadlines.length > 0 && i >= MAX_VISIBLE - deadlines.slice(0, MAX_VISIBLE).length) return null;
        const color = ev.color_id ? GCAL_COLORS[ev.color_id] : "#6366f1";
        return (
          <button
            key={ev.id}
            onClick={() => onSelectEvent(ev)}
            className="flex w-full items-center gap-1.5 rounded-md px-1.5 py-[3px] text-left transition-opacity hover:opacity-80"
            style={{ backgroundColor: color + "18", }}
            title={ev.title}
          >
            <span className="h-1.5 w-1.5 shrink-0 rounded-full" style={{ backgroundColor: color }} />
            <span className="truncate text-[10px] font-medium" style={{ color }}>
              {gcalTimeLabel(ev) && <span className="opacity-70">{gcalTimeLabel(ev)} · </span>}
              {ev.title}
            </span>
          </button>
        );
      })}

      {/* Deadline deliverables */}
      {deadlines.slice(0, Math.max(0, MAX_VISIBLE - events.slice(0, MAX_VISIBLE).length)).map((d) => (
        <div
          key={d.id}
          className="flex w-full items-center gap-1.5 rounded-md bg-rose-50 px-1.5 py-[3px]"
          title={d.title}
        >
          <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-rose-400" />
          <span className="truncate text-[10px] font-medium text-rose-500">
            <span className="opacity-70">due · </span>{d.title}
          </span>
        </div>
      ))}

      {/* Overflow count */}
      {overflow > 0 && (
        <span className="pl-1 text-[9px] font-medium text-zinc-400">+{overflow} more</span>
      )}
    </div>
  );
}

// ── Sub-components ─────────────────────────────────────────────────────────────

interface MeetingBadgeProps {
  meetingDate: string | null;
  countdown: number | null;
  editing: boolean;
  meetingInput: string;
  loading: boolean;
  advanceLoading: boolean;
  onStartEdit: () => void;
  onInputChange: (v: string) => void;
  onSave: () => void;
  onCancel: () => void;
  onAdvance: () => void;
}

function MeetingBadge({
  meetingDate,
  countdown,
  editing,
  meetingInput,
  loading,
  advanceLoading,
  onStartEdit,
  onInputChange,
  onSave,
  onCancel,
  onAdvance,
}: MeetingBadgeProps) {
  if (editing) {
    return (
      <div className="flex items-center gap-1.5">
        <CalendarDays size={13} className="text-zinc-400" />
        <input
          type="date"
          value={meetingInput}
          onChange={(e) => onInputChange(e.target.value)}
          className="field-control h-6 py-0 text-[12px]"
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Enter") onSave();
            if (e.key === "Escape") onCancel();
          }}
        />
        <button
          onClick={onSave}
          disabled={loading}
          className="btn text-[11px] py-0.5"
        >
          {loading ? <Loader2 size={10} className="animate-spin" /> : "Save"}
        </button>
        <button onClick={onCancel} className="text-zinc-400 hover:text-zinc-600">
          <X size={13} />
        </button>
      </div>
    );
  }

  if (meetingDate) {
    const label =
      countdown === 0
        ? "Meeting today"
        : countdown === 1
          ? "Meeting tomorrow"
          : countdown !== null && countdown < 0
            ? `Meeting was ${Math.abs(countdown)}d ago`
            : countdown !== null
              ? `Meeting in ${countdown}d`
              : `Meeting ${meetingDate}`;

    const urgent = countdown !== null && countdown <= 1 && countdown >= 0;

    return (
      <div className="flex items-center gap-2">
        <button
          onClick={onStartEdit}
          className={[
            "flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-[11px] font-medium transition-colors",
            urgent
              ? "bg-amber-50 text-amber-700 hover:bg-amber-100"
              : "bg-zinc-100 text-zinc-600 hover:bg-zinc-200",
          ].join(" ")}
        >
          <Clock size={11} />
          {label}
        </button>
        {countdown !== null && countdown < 0 && (
          <button
            onClick={onAdvance}
            disabled={advanceLoading}
            className="text-[11px] text-zinc-400 hover:text-zinc-700"
          >
            {advanceLoading ? <Loader2 size={11} className="animate-spin" /> : "Mark met →"}
          </button>
        )}
      </div>
    );
  }

  return (
    <button
      onClick={onStartEdit}
      className="flex items-center gap-1.5 text-[11px] text-zinc-400 hover:text-zinc-600"
    >
      <CalendarDays size={12} />
      Set meeting date
    </button>
  );
}

interface DayColumnProps {
  dayIndex: number;
  dayName: string;
  date: string;
  deliverable: Deliverable | null;
  deadlineDeliverables: Deliverable[];
  tasks: WeekTask[];
  gcalEvents: GCalEvent[];
  stakeholdersByEmail: Map<string, Stakeholder>;
  isMeetingDay: boolean;
  pickerOpen: boolean;
  pickerSearch: string;
  filteredPicker: Deliverable[];
  assignedIds: Set<string | undefined>;
  onOpenPicker: () => void;
  onClosePicker: () => void;
  onPickerSearch: (v: string) => void;
  onAssign: (id: string) => void;
  onClear: () => void;
  onSchedule: (date: string) => void;
  onEventDeleted: () => void;
}

function DayColumn({
  dayIndex,
  dayName,
  date,
  deliverable,
  deadlineDeliverables,
  tasks,
  gcalEvents,
  stakeholdersByEmail,
  isMeetingDay,
  pickerOpen,
  pickerSearch,
  filteredPicker,
  assignedIds,
  onOpenPicker,
  onClosePicker,
  onPickerSearch,
  onAssign,
  onClear,
  onSchedule,
  onEventDeleted,
}: DayColumnProps) {
  return (
    <div className="flex min-w-[180px] flex-1 flex-col gap-2">
      {/* Day header */}
      <div className="flex items-center justify-between">
        <span
          className={[
            "text-[11px] font-semibold uppercase tracking-wide",
            isMeetingDay ? "text-amber-600" : "text-zinc-400",
          ].join(" ")}
        >
          {dayName}
          <span className="ml-1 font-normal text-zinc-300">
            {date.slice(5)}
          </span>
          {isMeetingDay && " · mtg"}
        </span>
        {deliverable && (
          <button
            onClick={onClear}
            className="text-zinc-300 hover:text-zinc-500"
            title="Clear day"
          >
            <X size={12} />
          </button>
        )}
      </div>

      {/* Manually pinned deliverable or add button */}
      {deliverable ? (
        <DeliverableWeekCard deliverable={deliverable} onReplace={onOpenPicker} label="focus" />
      ) : (
        <button
          onClick={onOpenPicker}
          className="flex h-16 items-center justify-center rounded-lg border border-dashed border-zinc-200 text-[11px] text-zinc-300 transition-colors hover:border-zinc-300 hover:text-zinc-400"
        >
          + assign
        </button>
      )}

      {/* Deadline deliverables — auto-populated */}
      {deadlineDeliverables.length > 0 && (
        <div className="space-y-1.5">
          <div className="flex items-center gap-1.5 px-1 text-[10px] font-semibold uppercase tracking-wide text-rose-400">
            <CalendarDays size={11} />
            Due today
          </div>
          {deadlineDeliverables.map((d) => (
            <DeliverableWeekCard key={d.id} deliverable={d} label="due" />
          ))}
        </div>
      )}

      <DayTaskStack dayIndex={dayIndex} tasks={tasks} />

      {/* Google Calendar events */}
      <GCalEventStack
        events={gcalEvents}
        stakeholdersByEmail={stakeholdersByEmail}
        onSchedule={() => onSchedule(date)}
        onEventDeleted={onEventDeleted}
      />

      {/* Picker dropdown */}
      {pickerOpen && (
        <DeliverablePicker
          search={pickerSearch}
          deliverables={filteredPicker}
          assignedIds={assignedIds}
          onSearch={onPickerSearch}
          onSelect={onAssign}
          onClose={onClosePicker}
        />
      )}
    </div>
  );
}

function DeliverableWeekCard({
  deliverable,
  onReplace,
}: {
  deliverable: Deliverable;
  onReplace?: () => void;
  label?: "focus" | "due";
}) {
  const isBlocked = Boolean(deliverable.blocker_reason);
  const isFocused = deliverable.is_focused;
  const hasDeadline = Boolean(deliverable.deadline);
  const isOverdue = hasDeadline && new Date(deliverable.deadline!) < new Date();

  return (
    <div
      className={[
        "group relative flex flex-col gap-1.5 rounded-lg border p-3 transition-shadow hover:shadow-sm",
        isFocused ? "border-amber-300 bg-amber-50/40" : "border-zinc-200 bg-white",
        isBlocked ? "opacity-80" : "",
      ].join(" ")}
    >
      {/* Status indicators */}
      <div className="flex items-start justify-between gap-1">
        <Link
          to={`/deliverables/${deliverable.id}`}
          className="text-[12px] font-medium leading-snug text-zinc-800 hover:text-zinc-950 hover:underline"
        >
          {deliverable.title}
        </Link>
        <div className="flex shrink-0 items-center gap-1">
          {isFocused && <Star size={11} className="fill-amber-400 text-amber-400" />}
          {isBlocked && <AlertTriangle size={11} className="text-red-400" />}
        </div>
      </div>

      {/* Claim */}
      <p className="line-clamp-2 text-[11px] leading-snug text-zinc-400">{deliverable.claim}</p>

      {/* Meta row */}
      <div className="flex flex-wrap items-center gap-1.5">
        <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-[10px] text-zinc-500">
          {deliverable.type}
        </span>
        {deliverable.deadline && (
          <span
            className={[
              "text-[10px]",
              isOverdue ? "text-red-500" : "text-zinc-400",
            ].join(" ")}
          >
            due {deliverable.deadline}
          </span>
        )}
        {deliverable.effort !== null && deliverable.impact !== null && (
          <span className="text-[10px] text-zinc-400">
            E{deliverable.effort}/I{deliverable.impact}
          </span>
        )}
      </div>

      {/* State badge */}
      <div className="flex items-center justify-between">
        <span
          className={[
            "rounded-full px-2 py-0.5 text-[10px] font-medium",
            deliverable.state === "in_review"
              ? "bg-blue-50 text-blue-600"
              : "bg-zinc-100 text-zinc-500",
          ].join(" ")}
        >
          {deliverable.state === "in_review" ? "In Review" : "Drafting"}
        </span>
        {onReplace && (
          <button
            onClick={onReplace}
            className="invisible text-[10px] text-zinc-400 hover:text-zinc-600 group-hover:visible"
          >
            swap
          </button>
        )}
      </div>
    </div>
  );
}

function DayTaskStack({ dayIndex, tasks }: { dayIndex: number; tasks: WeekTask[] }) {
  if (tasks.length === 0) {
    return (
      <div className="rounded-lg border border-dashed border-zinc-100 px-2 py-3 text-center text-[10px] text-zinc-300">
        no dated tasks
      </div>
    );
  }

  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-1.5 px-1 text-[10px] font-semibold uppercase tracking-wide text-zinc-400">
        <ListChecks size={11} />
        Tasks
      </div>
      {tasks.map((task) => (
        <WeekTaskItem dayIndex={dayIndex} key={task.id} task={task} />
      ))}
    </div>
  );
}

function WeekTaskItem({ dayIndex, task }: { dayIndex: number; task: WeekTask }) {
  const palette = taskPalette(task.deliverable_id, dayIndex);
  const done = task.status === "done";

  return (
    <Link
      className={[
        "block rounded-md border bg-white px-2.5 py-2 text-left transition hover:shadow-sm",
        done ? "opacity-55" : "",
      ].join(" ")}
      style={{ borderLeft: `3px solid ${palette}` }}
      to={`/deliverables/${task.deliverable_id}`}
      title={task.deliverable_title}
    >
      <div className="mb-1 flex items-start justify-between gap-2">
        <span className="line-clamp-2 text-[11px] font-medium leading-snug text-zinc-800">
          {task.title}
        </span>
        <span
          className={[
            "shrink-0 rounded-full px-1.5 py-0.5 text-[9px] font-semibold",
            done
              ? "bg-emerald-50 text-emerald-600"
              : task.status === "doing"
                ? "bg-sky-50 text-sky-600"
                : "bg-zinc-100 text-zinc-500",
          ].join(" ")}
        >
          {task.status}
        </span>
      </div>
      <p className="line-clamp-1 text-[10px] text-zinc-400">
        {task.deliverable_title}
      </p>
      {task.stakeholder_name ? (
        <p className="mt-1 line-clamp-1 text-[9px] font-semibold uppercase tracking-wide text-zinc-300">
          {task.stakeholder_name}
        </p>
      ) : null}
    </Link>
  );
}

function GCalEventStack({
  events,
  stakeholdersByEmail,
  onSchedule,
  onEventDeleted,
}: {
  events: GCalEvent[];
  stakeholdersByEmail: Map<string, Stakeholder>;
  onSchedule: () => void;
  onEventDeleted: () => void;
}) {
  const [selected, setSelected] = useState<GCalEvent | null>(null);

  return (
    <>
      <div className="space-y-1.5">
        <div className="flex items-center justify-between px-1">
          <div className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wide text-sky-400">
            <CalendarDays size={11} />
            Calendar
          </div>
          <button
            onClick={onSchedule}
            title="Schedule a meeting on this day"
            className="flex h-4 w-4 items-center justify-center rounded text-sky-400 hover:bg-sky-50 hover:text-sky-600"
          >
            <Plus size={11} />
          </button>
        </div>
        {events.map((ev) => (
          <GCalEventItem key={ev.id} event={ev} onOpen={() => setSelected(ev)} />
        ))}
      </div>

      {selected && (
        <GCalEventDialog
          event={selected}
          stakeholdersByEmail={stakeholdersByEmail}
          onClose={() => setSelected(null)}
          onDeleted={() => {
            setSelected(null);
            onEventDeleted();
          }}
        />
      )}
    </>
  );
}

const GCAL_COLORS: Record<string, string> = {
  "1": "#d50000", "2": "#e67c73", "3": "#f4511e", "4": "#f6bf26",
  "5": "#33b679", "6": "#0b8043", "7": "#039be5", "8": "#3f51b5",
  "9": "#7986cb", "10": "#8e24aa", "11": "#616161",
};

function GCalEventItem({ event, onOpen }: { event: GCalEvent; onOpen: () => void }) {
  const timeLabel = gcalTimeLabel(event);
  const accentColor = event.color_id ? GCAL_COLORS[event.color_id] : undefined;
  const isFree = event.transparency === "transparent";
  const isRecurring = !!event.recurring_event_id;
  const hasConference = !!parseGCalConferenceData(event)?.video_uri;

  return (
    <button
      onClick={onOpen}
      className="w-full text-left flex flex-col gap-0.5 rounded-md border border-sky-100 bg-sky-50/60 px-2.5 py-1.5 hover:border-sky-200 hover:bg-sky-100/60 transition-colors overflow-hidden"
      style={accentColor ? { borderLeftColor: accentColor, borderLeftWidth: 3 } : undefined}
    >
      <div className="flex items-center gap-1 min-w-0">
        <span className="line-clamp-2 text-[11px] font-medium leading-snug text-sky-800 flex-1 min-w-0">
          {event.title}
        </span>
        {hasConference && <span className="shrink-0 text-[8px] font-semibold text-emerald-600 bg-emerald-50 rounded px-1 py-0.5">Meet</span>}
        {isRecurring && <span className="shrink-0 text-[8px] text-sky-400">↻</span>}
      </div>
      <div className="flex items-center gap-1.5">
        <span className="text-[9px] text-sky-500">{timeLabel}</span>
        {isFree && <span className="text-[8px] text-zinc-400">free</span>}
      </div>
    </button>
  );
}

function GCalEventDialog({ event, stakeholdersByEmail, onClose, onDeleted }: { event: GCalEvent; stakeholdersByEmail: Map<string, Stakeholder>; onClose: () => void; onDeleted: () => void }) {
  const timeRange = gcalTimeRange(event);
  const dateLabel = gcalDateLabel(event);
  const description = stripHtml(event.description ?? "");
  const { attendees, conference_data: conference, organizer, attachments } = parseGCalEvent(event);
  const isRecurring = !!event.recurring_event_id;
  const isFree = event.transparency === "transparent";
  const accentColor = event.color_id ? GCAL_COLORS[event.color_id] : undefined;
  const [deleting, setDeleting] = useState(false);

  // Extract a Zoom URL from the description if present
  const zoomUrlMatch = description.match(/https?:\/\/[^\s]*zoom\.us\/j\/[^\s]*/);
  const zoomJoinUrl = zoomUrlMatch ? zoomUrlMatch[0] : null;

  async function handleDelete() {
    if (!event.gcal_event_id) return;
    setDeleting(true);
    try {
      await deleteGcalMeeting(event.gcal_event_id);
      onDeleted();
    } catch {
      setDeleting(false);
    }
  }

  function responseColor(status: string | null): string {
    if (status === "declined") return "line-through text-zinc-400";
    if (status === "tentative") return "text-amber-600";
    return "";
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-[2px]"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-md rounded-xl border border-zinc-200 bg-white shadow-2xl overflow-hidden"
        onClick={(e) => e.stopPropagation()}
        style={accentColor ? { borderTopColor: accentColor, borderTopWidth: 3 } : undefined}
      >
        {/* Header */}
        <div className="flex items-start justify-between gap-3 border-b border-zinc-100 px-5 py-4">
          <div className="flex min-w-0 flex-col gap-1">
            <div className="flex items-center gap-2">
              <span className="text-[11px] font-semibold uppercase tracking-wide text-sky-500">
                Google Calendar
              </span>
              {isRecurring && (
                <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-[9px] font-medium text-zinc-500">
                  Recurring
                </span>
              )}
              {isFree && (
                <span className="rounded bg-zinc-100 px-1.5 py-0.5 text-[9px] font-medium text-zinc-400">
                  Free
                </span>
              )}
            </div>
            <h2 className="text-[15px] font-semibold leading-snug text-zinc-900">
              {event.title}
            </h2>
          </div>
          <button
            onClick={onClose}
            className="mt-0.5 shrink-0 rounded p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600"
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div className="flex max-h-[55vh] flex-col gap-4 overflow-y-auto px-5 py-4">
          {/* Date + time */}
          <div className="flex items-start gap-2.5">
            <CalendarDays size={15} className="mt-0.5 shrink-0 text-zinc-400" />
            <div className="flex flex-col gap-0.5">
              <span className="text-[13px] font-medium text-zinc-800">{dateLabel}</span>
              {timeRange && (
                <span className="text-[12px] text-zinc-500">{timeRange}</span>
              )}
            </div>
          </div>

          {/* Location */}
          {event.location && (
            <div className="flex items-start gap-2.5">
              <MapPin size={15} className="mt-0.5 shrink-0 text-zinc-400" />
              <span className="text-[12px] text-zinc-700">{event.location}</span>
            </div>
          )}

          {/* Conference / video call */}
          {conference?.video_uri && (
            <div className="flex items-start gap-2.5">
              <Video size={15} className="mt-0.5 shrink-0 text-emerald-500" />
              <div className="flex flex-col gap-1">
                <button
                  onClick={() => void openExternalUrl(conference.video_uri!)}
                  className="text-[12px] font-medium text-emerald-600 hover:text-emerald-700 underline text-left"
                >
                  Join {conference.solution_name ?? "meeting"}
                </button>
                {conference.phone_uri && (
                  <span className="text-[11px] text-zinc-400">
                    Dial-in: {conference.phone_uri.replace("tel:", "")}
                    {conference.phone_pin ? ` · PIN: ${conference.phone_pin}` : ""}
                  </span>
                )}
              </div>
            </div>
          )}
          {/* Zoom join link extracted from description */}
          {!conference?.video_uri && zoomJoinUrl && (
            <div className="flex items-start gap-2.5">
              <Video size={15} className="mt-0.5 shrink-0 text-blue-500" />
              <button
                onClick={() => void openExternalUrl(zoomJoinUrl)}
                className="text-[12px] font-medium text-blue-600 hover:text-blue-700 underline text-left"
              >
                Join Zoom meeting
              </button>
            </div>
          )}

          {/* Organizer (only if not self) */}
          {organizer && !organizer.self && (
            <div className="flex items-start gap-2.5">
              <Users size={15} className="mt-0.5 shrink-0 text-zinc-300" />
              <div className="flex items-center gap-1.5">
                <span className="text-[12px] text-zinc-500">Organised by</span>
                <span className="text-[12px] font-medium text-zinc-700">
                  {organizer.name ?? organizer.email}
                </span>
              </div>
            </div>
          )}

          {/* Attendees */}
          {attendees.length > 0 && (
            <div className="flex items-start gap-2.5">
              <Users size={15} className="mt-0.5 shrink-0 text-zinc-400" />
              <div className="flex flex-col gap-1.5">
                {attendees.map((a, i) => {
                  const stakeholder = stakeholdersByEmail.get(a.email.toLowerCase());
                  return (
                    <div key={i} className="flex items-center gap-1.5">
                      <div className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-zinc-100 text-[9px] font-bold text-zinc-500">
                        {(a.name ?? a.email).charAt(0).toUpperCase()}
                      </div>
                      <span className={`text-[12px] ${a.self ? "font-semibold text-sky-600" : "text-zinc-700"} ${responseColor(a.responseStatus)}`}>
                        {a.name ?? a.email}
                      </span>
                      {a.self && (
                        <span className="rounded bg-sky-50 px-1 py-0.5 text-[9px] font-medium text-sky-500">you</span>
                      )}
                      {a.organizer && !a.self && (
                        <span className="rounded bg-zinc-100 px-1 py-0.5 text-[9px] text-zinc-400">organizer</span>
                      )}
                      {a.responseStatus === "declined" && (
                        <span className="text-[9px] text-red-400">declined</span>
                      )}
                      {a.responseStatus === "tentative" && (
                        <span className="text-[9px] text-amber-500">maybe</span>
                      )}
                      {stakeholder && (
                        <Link
                          to={`/stakeholders/${stakeholder.id}`}
                          onClick={onClose}
                          className="rounded bg-violet-50 px-1.5 py-0.5 text-[9px] font-medium text-violet-600 hover:bg-violet-100"
                        >
                          {stakeholder.role || "stakeholder"} →
                        </Link>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Description */}
          {description && (
            <div className="flex items-start gap-2.5">
              <ListChecks size={15} className="mt-0.5 shrink-0 text-zinc-400" />
              <div className="text-[12px] leading-relaxed text-zinc-600">
                <LinkedText text={description} />
              </div>
            </div>
          )}

          {/* Attachments */}
          {attachments.length > 0 && (
            <div className="flex items-start gap-2.5">
              <ExternalLink size={15} className="mt-0.5 shrink-0 text-zinc-400" />
              <div className="flex flex-col gap-1">
                {attachments.map((a, i) => (
                  <button
                    key={i}
                    onClick={() => void openExternalUrl(a.fileUrl)}
                    className="text-left text-[12px] text-sky-600 hover:text-sky-700 underline truncate max-w-xs"
                  >
                    {a.title ?? a.fileUrl}
                  </button>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between gap-2 border-t border-zinc-100 px-5 py-3">
          <button
            onClick={() => void handleDelete()}
            disabled={deleting}
            className="flex items-center gap-1.5 rounded-lg border border-red-100 bg-red-50 px-3 py-1.5 text-[12px] font-medium text-red-500 hover:bg-red-100 hover:text-red-600 disabled:opacity-50 transition-colors"
          >
            {deleting ? <Loader2 size={12} className="animate-spin" /> : <Trash2 size={12} />}
            {deleting ? "Deleting…" : "Delete event"}
          </button>
          {event.html_link && (
            <button
              onClick={() => void openExternalUrl(event.html_link!)}
              className="flex items-center gap-1.5 rounded-lg bg-sky-500 px-3 py-1.5 text-[12px] font-medium text-white hover:bg-sky-600 transition-colors"
            >
              <ExternalLink size={12} />
              Open in Google Calendar
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

// Renders plain text but turns http/https URLs into clickable buttons
function LinkedText({ text }: { text: string }) {
  const URL_RE = /(https?:\/\/[^\s]+)/g;
  const parts = text.split(URL_RE);

  return (
    <p className="whitespace-pre-wrap">
      {parts.map((part, i) =>
        URL_RE.test(part) ? (
          <button
            key={i}
            onClick={() => void openExternalUrl(part)}
            className="break-all text-sky-600 underline hover:text-sky-800"
          >
            {part}
          </button>
        ) : (
          part
        )
      )}
    </p>
  );
}

function gcalTimeLabel(event: GCalEvent): string {
  if (event.is_all_day) return "All day";
  if (!event.start_datetime) return "";
  return new Date(event.start_datetime).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
}

function gcalTimeRange(event: GCalEvent): string {
  if (event.is_all_day) return "All day";
  if (!event.start_datetime) return "";
  const start = new Date(event.start_datetime).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
  if (!event.end_datetime) return start;
  const end = new Date(event.end_datetime).toLocaleTimeString(undefined, {
    hour: "numeric",
    minute: "2-digit",
  });
  return `${start} – ${end}`;
}

function gcalDateLabel(event: GCalEvent): string {
  const d = parseLocalDate(event.start_date);
  return d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric", year: "numeric" });
}

function stripHtml(html: string): string {
  return html
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(/<\/p>/gi, "\n")
    .replace(/<[^>]+>/g, "")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

// ── Schedule Meeting Dialog ────────────────────────────────────────────────────

function defaultStartTime(): string {
  const now = new Date();
  const h = String(now.getHours() + 1).padStart(2, "0");
  return `${h}:00`;
}

function defaultEndTime(start: string): string {
  const [h, m] = start.split(":").map(Number);
  const end = new Date(0, 0, 0, h + 1, m);
  return `${String(end.getHours()).padStart(2, "0")}:${String(end.getMinutes()).padStart(2, "0")}`;
}

interface ScheduleMeetingDialogProps {
  defaultDate: string;
  stakeholders: Stakeholder[];
  onClose: () => void;
  onCreated: () => void;
}

type ConferenceType = "none" | "meet" | "zoom";

function ScheduleMeetingDialog({ defaultDate, stakeholders, onClose, onCreated }: ScheduleMeetingDialogProps) {
  const initStart = defaultStartTime();
  const [title, setTitle] = useState("");
  const [date, setDate] = useState(defaultDate);
  const [startTime, setStartTime] = useState(initStart);
  const [endTime, setEndTime] = useState(defaultEndTime(initStart));
  const [conferenceType, setConferenceType] = useState<ConferenceType>("meet");
  const [zoomUrl, setZoomUrl] = useState("");
  const [attendeeEmails, setAttendeeEmails] = useState<string[]>([]);
  const [location, setLocation] = useState("");
  const [description, setDescription] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!title.trim()) return;
    if (conferenceType === "zoom" && !zoomUrl.trim()) return;
    setSubmitting(true);
    setError(null);
    try {
      await createGcalMeeting({
        title: title.trim(),
        date,
        start_time: startTime,
        end_time: endTime,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        attendees: attendeeEmails,
        add_meet: conferenceType === "meet",
        zoom_url: conferenceType === "zoom" ? zoomUrl.trim() : undefined,
      });
      onCreated();
    } catch (e) {
      setError(String(e));
      setSubmitting(false);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 backdrop-blur-[2px]"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-lg rounded-2xl border border-zinc-200 bg-white shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex items-center justify-between border-b border-zinc-100 px-6 py-4">
          <h2 className="text-[15px] font-semibold text-zinc-900">Schedule meeting</h2>
          <button
            onClick={onClose}
            className="rounded p-1 text-zinc-400 hover:bg-zinc-100 hover:text-zinc-600"
          >
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleSubmit}>
          <div className="flex flex-col gap-4 px-6 py-5">
            {error && (
              <div className="rounded-lg border border-red-100 bg-red-50 px-3 py-2 text-[12px] text-red-600">
                {error}
              </div>
            )}

            {/* Title */}
            <input
              autoFocus
              className="w-full border-0 border-b border-zinc-200 pb-2 text-[20px] font-semibold text-zinc-900 placeholder:text-zinc-300 focus:border-zinc-400 focus:outline-none"
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Add title"
              required
              value={title}
            />

            {/* Date + times */}
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-1.5 rounded-lg border border-zinc-200 px-3 py-1.5">
                <CalendarDays size={13} className="shrink-0 text-zinc-400" />
                <input
                  className="w-[120px] text-[13px] text-zinc-700 focus:outline-none"
                  onChange={(e) => setDate(e.target.value)}
                  type="date"
                  value={date}
                />
              </div>
              <div className="flex items-center gap-1.5 rounded-lg border border-zinc-200 px-3 py-1.5">
                <Clock size={13} className="shrink-0 text-zinc-400" />
                <input
                  className="w-[64px] text-[13px] text-zinc-700 focus:outline-none"
                  onChange={(e) => {
                    setStartTime(e.target.value);
                    setEndTime(defaultEndTime(e.target.value));
                  }}
                  type="time"
                  value={startTime}
                />
                <span className="text-zinc-300">–</span>
                <input
                  className="w-[64px] text-[13px] text-zinc-700 focus:outline-none"
                  onChange={(e) => setEndTime(e.target.value)}
                  type="time"
                  value={endTime}
                />
              </div>
            </div>

            {/* Video conference selector */}
            <div className="flex flex-col gap-2">
              <span className="text-[11px] font-semibold uppercase tracking-wide text-zinc-400">
                Video conferencing
              </span>
              <div className="flex gap-1.5">
                {([
                  { key: "none", label: "None" },
                  { key: "meet", label: "Google Meet" },
                  { key: "zoom", label: "Zoom" },
                ] as { key: ConferenceType; label: string }[]).map(({ key, label }) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => setConferenceType(key)}
                    className={[
                      "flex items-center gap-1.5 rounded-lg border px-3 py-1.5 text-[12px] font-medium transition-colors",
                      conferenceType === key
                        ? key === "meet"
                          ? "border-emerald-400 bg-emerald-50 text-emerald-700"
                          : key === "zoom"
                          ? "border-blue-400 bg-blue-50 text-blue-700"
                          : "border-zinc-400 bg-zinc-100 text-zinc-700"
                        : "border-zinc-200 bg-white text-zinc-500 hover:border-zinc-300 hover:text-zinc-700",
                    ].join(" ")}
                  >
                    {key !== "none" && <Video size={11} />}
                    {label}
                  </button>
                ))}
              </div>
              {conferenceType === "zoom" && (
                <input
                  className="w-full rounded-lg border border-zinc-200 px-3 py-2 text-[13px] text-zinc-700 placeholder:text-zinc-300 focus:border-blue-400 focus:outline-none"
                  onChange={(e) => setZoomUrl(e.target.value)}
                  placeholder="Paste Zoom meeting link…"
                  required
                  type="url"
                  value={zoomUrl}
                />
              )}
            </div>

            {/* Attendees */}
            <div className="space-y-1.5">
              <span className="text-[11px] font-semibold uppercase tracking-wide text-zinc-400">
                Guests
              </span>
              <AttendeePicker
                emails={attendeeEmails}
                stakeholders={stakeholders}
                onChange={setAttendeeEmails}
              />
            </div>

            {/* Location */}
            <div className="flex items-center gap-2.5 rounded-lg border border-zinc-200 px-3 py-2">
              <MapPin size={13} className="shrink-0 text-zinc-400" />
              <input
                className="flex-1 text-[13px] text-zinc-700 placeholder:text-zinc-300 focus:outline-none"
                onChange={(e) => setLocation(e.target.value)}
                placeholder="Add location"
                value={location}
              />
            </div>

            {/* Description */}
            <textarea
              className="min-h-[72px] w-full resize-none rounded-lg border border-zinc-200 px-3 py-2 text-[13px] text-zinc-700 placeholder:text-zinc-300 focus:border-zinc-400 focus:outline-none"
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Add description"
              value={description}
            />
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-2 border-t border-zinc-100 px-6 py-4">
            <button
              className="btn text-[13px]"
              disabled={submitting}
              onClick={onClose}
              type="button"
            >
              Cancel
            </button>
            <button
              className="btn btn-primary text-[13px]"
              disabled={submitting || !title.trim()}
              type="submit"
            >
              {submitting ? <Loader2 size={13} className="animate-spin" /> : <Plus size={13} />}
              {submitting ? "Scheduling…" : "Schedule"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

// ── Attendee email chip picker ─────────────────────────────────────────────────

function AttendeePicker({
  emails,
  stakeholders,
  onChange,
}: {
  emails: string[];
  stakeholders: Stakeholder[];
  onChange: (emails: string[]) => void;
}) {
  const [query, setQuery] = useState("");
  const [isOpen, setIsOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const suggestions = useMemo(() => {
    const q = query.toLowerCase();
    if (!q) return stakeholders.filter((s) => s.email && !emails.includes(s.email)).slice(0, 6);
    return stakeholders
      .filter(
        (s) =>
          s.email &&
          !emails.includes(s.email) &&
          (s.name.toLowerCase().includes(q) || s.email.toLowerCase().includes(q)),
      )
      .slice(0, 6);
  }, [query, stakeholders, emails]);

  function addEmail(email: string) {
    const trimmed = email.trim().toLowerCase();
    if (trimmed && !emails.includes(trimmed)) {
      onChange([...emails, trimmed]);
    }
    setQuery("");
    inputRef.current?.focus();
  }

  function removeEmail(email: string) {
    onChange(emails.filter((e) => e !== email));
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
    if ((e.key === "Enter" || e.key === "," || e.key === "Tab") && query.trim()) {
      e.preventDefault();
      addEmail(query);
    }
    if (e.key === "Backspace" && !query && emails.length > 0) {
      onChange(emails.slice(0, -1));
    }
    if (e.key === "Escape") {
      setIsOpen(false);
    }
  }

  return (
    <div className="relative rounded-lg border border-zinc-200 px-2.5 py-2">
      <div className="flex flex-wrap gap-1.5">
        {emails.map((email) => (
          <span
            key={email}
            className="flex items-center gap-1 rounded-md bg-sky-50 px-2 py-0.5 text-[11px] font-medium text-sky-700"
          >
            {email}
            <button
              className="ml-0.5 rounded text-sky-400 hover:text-sky-700"
              onClick={() => removeEmail(email)}
              type="button"
            >
              <X size={9} />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          className="min-w-[160px] flex-1 text-[13px] text-zinc-700 placeholder:text-zinc-300 focus:outline-none"
          onBlur={() => setTimeout(() => setIsOpen(false), 150)}
          onChange={(e) => {
            setQuery(e.target.value);
            setIsOpen(true);
          }}
          onFocus={() => setIsOpen(true)}
          onKeyDown={handleKeyDown}
          placeholder={emails.length === 0 ? "Add guests by email…" : ""}
          value={query}
        />
      </div>

      {isOpen && suggestions.length > 0 && (
        <div className="absolute left-0 top-full z-20 mt-1 w-full overflow-hidden rounded-xl border border-zinc-200 bg-white shadow-[0_4px_20px_rgba(0,0,0,0.09)]">
          {suggestions.map((s) => (
            <button
              className="flex w-full items-center gap-2.5 px-3 py-2 text-left text-sm hover:bg-zinc-50"
              key={s.id}
              onMouseDown={(e) => {
                e.preventDefault();
                addEmail(s.email);
              }}
              type="button"
            >
              <div className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-zinc-100 text-[9px] font-bold text-zinc-500">
                {s.name.charAt(0).toUpperCase()}
              </div>
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[12px] font-medium text-zinc-800">{s.name}</span>
                <span className="block truncate text-[11px] text-zinc-400">{s.email}</span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function taskPalette(deliverableId: string, dayIndex: number) {
  const colors = ["#0ea5e9", "#10b981", "#f59e0b", "#8b5cf6", "#ef4444", "#14b8a6"];
  const total = [...deliverableId].reduce(
    (sum, char) => sum + char.charCodeAt(0),
    dayIndex,
  );
  return colors[total % colors.length];
}

interface DeliverablePickerProps {
  search: string;
  deliverables: Deliverable[];
  assignedIds: Set<string | undefined>;
  onSearch: (v: string) => void;
  onSelect: (id: string) => void;
  onClose: () => void;
}

function DeliverablePicker({
  search,
  deliverables,
  assignedIds,
  onSearch,
  onSelect,
  onClose,
}: DeliverablePickerProps) {
  return (
    <div className="relative z-20 rounded-xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center gap-2 border-b border-zinc-100 px-2 py-1.5">
        <input
          type="text"
          placeholder="Search deliverables…"
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          className="min-w-0 flex-1 text-[12px] outline-none placeholder:text-zinc-300"
          autoFocus
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "Enter" && deliverables.length > 0) onSelect(deliverables[0].id);
          }}
        />
        <button onClick={onClose} className="text-zinc-300 hover:text-zinc-500">
          <X size={13} />
        </button>
      </div>
      <div className="max-h-56 overflow-y-auto">
        {deliverables.length === 0 && (
          <p className="px-3 py-4 text-center text-[11px] text-zinc-400">No deliverables found</p>
        )}
        {deliverables.map((d) => {
          const alreadyAssigned = assignedIds.has(d.id);
          return (
            <button
              key={d.id}
              onClick={() => onSelect(d.id)}
              className={[
                "flex w-full flex-col gap-0.5 px-3 py-2 text-left hover:bg-zinc-50",
                alreadyAssigned ? "opacity-50" : "",
              ].join(" ")}
            >
              <span className="flex items-center gap-1.5 text-[12px] font-medium text-zinc-700">
                {d.is_focused && <Star size={10} className="fill-amber-400 text-amber-400" />}
                {d.title}
                {alreadyAssigned && (
                  <span className="text-[10px] text-zinc-400">(assigned)</span>
                )}
              </span>
              <span className="line-clamp-1 text-[10px] text-zinc-400">{d.claim}</span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
