import { useEffect, useMemo, useState } from "react";
import { Search, X } from "lucide-react";
import {
  listDeliverables,
  listDeliverableTasks,
  listInitiatives,
  listStakeholders,
} from "../../lib/ipc";
import type { Deliverable, DeliverableTask, Initiative, Stakeholder } from "../../lib/types";
import {
  FILE_ENTITY_KIND_LABELS,
  linkFileToEntity,
  type FileEntityKind,
  type FileRow,
} from "../../lib/files";

interface FileLinkPickerProps {
  file: FileRow;
  onClose: () => void;
  onLinked: () => void;
}

type Tab = Exclude<FileEntityKind, "capture" | "meeting" | "conversation">;

const TABS: Tab[] = ["initiative", "deliverable", "deliverable_task", "stakeholder"];

export function FileLinkPicker({ file, onClose, onLinked }: FileLinkPickerProps) {
  const [tab, setTab] = useState<Tab>("initiative");
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [deliverables, setDeliverables] = useState<Deliverable[]>([]);
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);
  const [tasks, setTasks] = useState<DeliverableTask[]>([]);
  const [loading, setLoading] = useState(true);

  const linkedSet = useMemo(
    () => new Set(file.links.map((l) => `${l.entityKind}:${l.entityId}`)),
    [file.links],
  );

  useEffect(() => {
    void (async () => {
      try {
        const [is, ds, ss] = await Promise.all([
          listInitiatives(),
          listDeliverables(),
          listStakeholders(),
        ]);
        setInitiatives(is);
        setDeliverables(ds);
        setStakeholders(ss);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  // Tasks load when their tab is opened
  useEffect(() => {
    if (tab !== "deliverable_task") return;
    if (tasks.length > 0) return;
    void (async () => {
      try {
        const all: DeliverableTask[] = [];
        for (const d of deliverables) {
          const ts = await listDeliverableTasks(d.id);
          all.push(...ts);
        }
        setTasks(all);
      } catch (e) {
        setError(String(e));
      }
    })();
  }, [tab, deliverables, tasks.length]);

  async function handleLink(entityKind: FileEntityKind, entityId: string) {
    setBusy(true);
    setError(null);
    try {
      await linkFileToEntity(file.id, entityKind, entityId);
      onLinked();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase();
    const match = (s: string) => !q || s.toLowerCase().includes(q);
    switch (tab) {
      case "initiative":
        return initiatives
          .filter((i) => match(i.title))
          .map((i) => ({ id: i.id, title: i.title, subtitle: i.framing ?? "" }));
      case "deliverable":
        return deliverables
          .filter((d) => match(d.title))
          .map((d) => ({ id: d.id, title: d.title, subtitle: d.type }));
      case "deliverable_task":
        return tasks
          .filter((t) => match(t.title))
          .map((t) => ({ id: t.id, title: t.title, subtitle: t.status }));
      case "stakeholder":
        return stakeholders
          .filter((s) => match(s.name))
          .map((s) => ({ id: s.id, title: s.name, subtitle: s.role ?? "" }));
    }
  }, [tab, query, initiatives, deliverables, tasks, stakeholders]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30 p-4"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-start justify-between border-b border-zinc-100 px-4 py-3">
          <div className="min-w-0">
            <p className="page-kicker">Link file</p>
            <h3 className="truncate text-[15px] font-semibold text-zinc-950">{file.name}</h3>
          </div>
          <button
            aria-label="Close"
            className="ml-2 inline-flex h-7 w-7 items-center justify-center rounded-md text-zinc-400 hover:bg-zinc-100"
            onClick={onClose}
            type="button"
          >
            <X size={15} />
          </button>
        </div>

        <div className="flex items-center gap-1 border-b border-zinc-100 px-2 pt-2">
          {TABS.map((t) => (
            <button
              className={[
                "rounded-md px-3 py-1.5 text-[12px] font-medium transition-colors",
                tab === t ? "bg-zinc-900 text-white" : "text-zinc-500 hover:bg-zinc-100",
              ].join(" ")}
              key={t}
              onClick={() => setTab(t)}
              type="button"
            >
              {FILE_ENTITY_KIND_LABELS[t]}
            </button>
          ))}
        </div>

        <div className="border-b border-zinc-100 p-3">
          <label className="flex items-center gap-2 rounded-md border border-zinc-200 px-2 py-1.5">
            <Search className="text-zinc-400" size={14} />
            <input
              className="w-full bg-transparent text-[13px] outline-none placeholder:text-zinc-400"
              onChange={(e) => setQuery(e.currentTarget.value)}
              placeholder={`Search ${FILE_ENTITY_KIND_LABELS[tab].toLowerCase()}s…`}
              value={query}
            />
          </label>
        </div>

        {error ? <div className="notice notice-error mx-3 mt-3">{error}</div> : null}

        <div className="max-h-72 overflow-y-auto p-2">
          {loading ? (
            <div className="space-y-1.5 p-1">
              {[...Array(4)].map((_, i) => (
                <div key={i} className="skeleton h-10" />
              ))}
            </div>
          ) : rows.length === 0 ? (
            <div className="rounded-md border border-dashed border-zinc-300 p-4 text-center text-[12px] text-zinc-500">
              No matches.
            </div>
          ) : (
            <ul className="space-y-1">
              {rows.map((r) => {
                const key = `${tab}:${r.id}`;
                const linked = linkedSet.has(key);
                return (
                  <li key={key}>
                    <button
                      className={[
                        "flex w-full items-start justify-between gap-3 rounded-md px-3 py-2 text-left text-[13px] transition-colors",
                        linked
                          ? "bg-emerald-50 text-emerald-900"
                          : "hover:bg-zinc-50 text-zinc-800",
                      ].join(" ")}
                      disabled={linked || busy}
                      onClick={() => void handleLink(tab, r.id)}
                      type="button"
                    >
                      <span className="min-w-0">
                        <span className="block truncate font-medium">{r.title}</span>
                        {r.subtitle ? (
                          <span className="block truncate text-[11px] text-zinc-500">
                            {r.subtitle}
                          </span>
                        ) : null}
                      </span>
                      <span className="shrink-0 text-[11px] font-semibold text-zinc-400">
                        {linked ? "Linked" : "Link"}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </div>
    </div>
  );
}
