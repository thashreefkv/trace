import { useMemo, type ReactNode } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Filter,
  GitBranch,
  Layers,
  Network,
} from "lucide-react";
import { colorForKind, labelForKind } from "../../lib/brain/kinds";

interface BrainLeftRailProps {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  kindCounts: Map<string, number>;
  relationCounts: Map<string, number>;
  hiddenKinds: Set<string>;
  hiddenRelations: Set<string>;
  onToggleKind: (kind: string) => void;
  onToggleRelation: (relation: string) => void;
  onResetFilters: () => void;
  confidenceThreshold: number;
  onConfidenceChange: (value: number) => void;
  includeDismissedCaptures: boolean;
  includeKilledDeliverables: boolean;
  onToggleIncludeDismissed: () => void;
  onToggleIncludeKilled: () => void;
  communitiesVisible: boolean;
  onToggleCommunities: () => void;
  savedViewsSlot?: ReactNode;
}

export function BrainLeftRail({
  collapsed,
  onToggleCollapsed,
  kindCounts,
  relationCounts,
  hiddenKinds,
  hiddenRelations,
  onToggleKind,
  onToggleRelation,
  onResetFilters,
  confidenceThreshold,
  onConfidenceChange,
  includeDismissedCaptures,
  includeKilledDeliverables,
  onToggleIncludeDismissed,
  onToggleIncludeKilled,
  communitiesVisible,
  onToggleCommunities,
  savedViewsSlot,
}: BrainLeftRailProps) {
  const sortedKinds = useMemo(
    () =>
      Array.from(kindCounts.entries()).sort((a, b) => {
        if (b[1] !== a[1]) return b[1] - a[1];
        return labelForKind(a[0]).localeCompare(labelForKind(b[0]));
      }),
    [kindCounts],
  );
  const sortedRelations = useMemo(
    () => Array.from(relationCounts.entries()).sort((a, b) => b[1] - a[1]).slice(0, 14),
    [relationCounts],
  );

  if (collapsed) {
    return (
      <aside className="flex h-full w-12 flex-col items-center gap-3 rounded-2xl border border-zinc-100 bg-white py-3 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
        <button
          aria-label="Expand filters"
          className="grid h-8 w-8 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
          onClick={onToggleCollapsed}
          type="button"
        >
          <ChevronRight size={16} />
        </button>
        <div className="mt-1 grid h-8 w-8 place-items-center rounded-lg text-zinc-400">
          <Filter size={14} />
        </div>
        <div className="grid h-8 w-8 place-items-center rounded-lg text-zinc-400">
          <Layers size={14} />
        </div>
        <div className="grid h-8 w-8 place-items-center rounded-lg text-zinc-400">
          <GitBranch size={14} />
        </div>
      </aside>
    );
  }

  return (
    <aside className="flex h-full flex-col gap-3 overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
      <header className="flex items-center justify-between border-b border-zinc-100 px-4 py-3">
        <div>
          <p className="page-kicker">Filters</p>
          <h3 className="text-sm font-semibold tracking-tight text-zinc-950">Refine the graph</h3>
        </div>
        <button
          aria-label="Collapse filters"
          className="grid h-7 w-7 place-items-center rounded-lg text-zinc-500 hover:bg-zinc-50 hover:text-zinc-900"
          onClick={onToggleCollapsed}
          type="button"
        >
          <ChevronLeft size={15} />
        </button>
      </header>

      <div className="flex-1 overflow-y-auto px-4 pb-4">
        <Section icon={<Layers size={13} />} title="Entity kinds">
          <ul className="space-y-1.5">
            {sortedKinds.map(([kind, count]) => {
              const palette = colorForKind(kind);
              const hidden = hiddenKinds.has(kind);
              return (
                <li key={kind}>
                  <button
                    className={`flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-[12px] transition-colors ${
                      hidden ? "text-zinc-400 hover:bg-zinc-50" : "text-zinc-700 hover:bg-zinc-50"
                    }`}
                    onClick={() => onToggleKind(kind)}
                    type="button"
                  >
                    <span
                      aria-hidden
                      className="h-2.5 w-2.5 shrink-0 rounded-full border"
                      style={{
                        background: hidden ? "transparent" : palette.fill,
                        borderColor: palette.stroke,
                      }}
                    />
                    <span className="flex-1 truncate font-medium">{labelForKind(kind)}</span>
                    <span className="text-[11px] tabular-nums text-zinc-400">{count}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </Section>

        <Section icon={<GitBranch size={13} />} title="Relation types">
          {sortedRelations.length === 0 ? (
            <p className="text-[12px] text-zinc-400">No relations in this view.</p>
          ) : (
            <ul className="space-y-1">
              {sortedRelations.map(([rel, count]) => {
                const hidden = hiddenRelations.has(rel);
                return (
                  <li key={rel}>
                    <button
                      className={`flex w-full items-center gap-2 rounded-lg px-2 py-1 text-left text-[11px] transition-colors ${
                        hidden ? "text-zinc-400" : "text-zinc-600"
                      } hover:bg-zinc-50`}
                      onClick={() => onToggleRelation(rel)}
                      type="button"
                    >
                      <span className="flex-1 truncate">{rel.replace(/_/g, " ")}</span>
                      <span className="tabular-nums text-zinc-400">{count}</span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </Section>

        <Section icon={<Filter size={13} />} title="Inferred-edge confidence">
          <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-3">
            <input
              aria-label="Confidence threshold"
              className="w-full accent-sky-500"
              max={1}
              min={0}
              onChange={(event) => onConfidenceChange(Number(event.currentTarget.value))}
              step={0.05}
              type="range"
              value={confidenceThreshold}
            />
            <div className="mt-1 flex items-center justify-between text-[11px] text-zinc-500">
              <span>Show all</span>
              <span className="tabular-nums">≥ {Math.round(confidenceThreshold * 100)}%</span>
            </div>
          </div>
        </Section>

        <Section icon={<Filter size={13} />} title="Include">
          <label className="flex items-center gap-2 rounded-lg px-1 py-1 text-[12px] text-zinc-700">
            <input
              checked={includeDismissedCaptures}
              className="rounded border-zinc-300 text-sky-500 focus:ring-sky-200"
              onChange={onToggleIncludeDismissed}
              type="checkbox"
            />
            Dismissed captures
          </label>
          <label className="flex items-center gap-2 rounded-lg px-1 py-1 text-[12px] text-zinc-700">
            <input
              checked={includeKilledDeliverables}
              className="rounded border-zinc-300 text-sky-500 focus:ring-sky-200"
              onChange={onToggleIncludeKilled}
              type="checkbox"
            />
            Killed deliverables
          </label>
        </Section>

        <Section icon={<Network size={13} />} title="Overlays">
          <label className="flex items-center justify-between rounded-lg px-1 py-1 text-[12px] text-zinc-700">
            <span className="flex items-center gap-1.5">
              Community hulls
            </span>
            <button
              aria-pressed={communitiesVisible}
              className={`relative h-4 w-7 rounded-full transition-colors ${
                communitiesVisible ? "bg-violet-500" : "bg-zinc-200"
              }`}
              onClick={onToggleCommunities}
              type="button"
            >
              <span
                className="absolute top-0.5 h-3 w-3 rounded-full bg-white shadow-sm transition-all"
                style={{ left: communitiesVisible ? "14px" : "2px" }}
              />
            </button>
          </label>
        </Section>

        {savedViewsSlot}

        <button
          className="mt-3 w-full rounded-xl border border-zinc-200 bg-white px-3 py-2 text-[12px] font-medium text-zinc-600 transition-colors hover:bg-zinc-50 hover:text-zinc-900"
          onClick={onResetFilters}
          type="button"
        >
          Reset filters
        </button>
      </div>
    </aside>
  );
}

function Section({
  icon,
  title,
  children,
}: {
  icon: React.ReactNode;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <section className="mt-4 first:mt-2">
      <h4 className="mb-2 flex items-center gap-1.5 text-[11px] font-semibold uppercase tracking-wider text-zinc-400">
        <span className="text-zinc-300">{icon}</span>
        {title}
      </h4>
      {children}
    </section>
  );
}
