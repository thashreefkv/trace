import { useCallback, useEffect, useMemo, useState } from "react";
import {
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  Beaker,
  Download,
  Play,
  Plus,
  Star,
  Trash2,
  XCircle,
} from "lucide-react";
import {
  createEvalFixture,
  deleteEvalFixture,
  getEvalSummary,
  importEvalFixtures,
  listEvalFixtures,
  listEvalRuns,
  runAllEvals,
  runEvalFixture,
  setEvalBaseline,
} from "../lib/ipc";
import { toast } from "../lib/toast";
import type {
  CreateEvalFixtureInput,
  EvalFixture,
  EvalFixtureKind,
  EvalRun,
  EvalSummary,
} from "../lib/types";

interface FixtureWithRun {
  fixture: EvalFixture;
  latestRun?: EvalRun;
  history: EvalRun[];
}

const KIND_OPTIONS: { value: EvalFixtureKind; label: string; hint: string }[] =
  [
    { value: "retrieval", label: "Retrieval", hint: "Working" },
    { value: "ask", label: "Ask", hint: "Working" },
    { value: "classification", label: "Classification", hint: "Working" },
    { value: "promotion", label: "Promotion", hint: "Awaiting Section 4" },
  ];

function formatScore(score: number, metric: string): string {
  if (metric.startsWith("precision_at_") || metric === "accuracy") {
    return `${Math.round(score * 100)}%`;
  }
  return score.toFixed(2);
}

function formatDelta(delta: number | null): {
  text: string;
  className: string;
} {
  if (delta == null) return { text: "no baseline", className: "text-zinc-400" };
  if (Math.abs(delta) < 0.005)
    return { text: "·", className: "text-zinc-400" };
  const sign = delta > 0 ? "+" : "";
  const cls = delta > 0 ? "text-emerald-600" : "text-red-600";
  return { text: `${sign}${(delta * 100).toFixed(1)}pp`, className: cls };
}

export function EvalHarnessPanel() {
  const [fixtures, setFixtures] = useState<FixtureWithRun[]>([]);
  const [summary, setSummary] = useState<EvalSummary | null>(null);
  const [loading, setLoading] = useState(false);
  const [running, setRunning] = useState(false);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const [fixtureList, runList, summaryData] = await Promise.all([
        listEvalFixtures(),
        listEvalRuns(undefined, 200),
        getEvalSummary(),
      ]);
      const byFixture = new Map<string, EvalRun[]>();
      for (const run of runList) {
        const arr = byFixture.get(run.fixture_id) ?? [];
        arr.push(run);
        byFixture.set(run.fixture_id, arr);
      }
      setFixtures(
        fixtureList.map((fixture) => {
          const history = byFixture.get(fixture.id) ?? [];
          return {
            fixture,
            latestRun: history[0],
            history,
          };
        }),
      );
      setSummary(summaryData);
    } catch {
      // ipc wrapper toasts
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const handleRunAll = async () => {
    setRunning(true);
    try {
      const runs = await runAllEvals();
      toast.success(`Ran ${runs.length} eval${runs.length === 1 ? "" : "s"}`);
      await load();
    } catch {
      // ipc wrapper toasts
    } finally {
      setRunning(false);
    }
  };

  const handleRunOne = async (fixtureId: string) => {
    try {
      await runEvalFixture(fixtureId);
      await load();
    } catch {
      // ipc wrapper toasts
    }
  };

  const handleSetBaseline = async (fixtureId: string, runId: string) => {
    try {
      await setEvalBaseline(fixtureId, runId);
      toast.success("Baseline set");
      await load();
    } catch {
      // ipc wrapper toasts
    }
  };

  const handleDelete = async (fixtureId: string) => {
    if (!confirm("Delete this fixture and its run history?")) return;
    try {
      await deleteEvalFixture(fixtureId);
      await load();
    } catch {
      // ipc wrapper toasts
    }
  };

  const toggle = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const summaryLabel = useMemo(() => {
    if (!summary || summary.total === 0) return "No runs yet";
    return `${summary.passed}/${summary.total} passing · avg ${Math.round(summary.avg_score * 100)}%`;
  }, [summary]);

  return (
    <section className="overflow-hidden rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.06)]">
      <div className="flex items-center justify-between border-b border-zinc-100 px-5 py-4">
        <div className="flex items-center gap-3">
          <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-zinc-100 text-zinc-600">
            <Beaker size={15} />
          </div>
          <div>
            <h2 className="text-[13px] font-semibold text-zinc-900">
              Eval harness
            </h2>
            <p className="text-[11px] text-zinc-400">
              Labelled fixtures + baselines for retrieval and agent quality.
            </p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <span className="rounded-full bg-zinc-100 px-2.5 py-1 text-[11px] font-medium text-zinc-600">
            {summaryLabel}
          </span>
          <button
            className="btn"
            disabled={running || fixtures.length === 0}
            onClick={() => void handleRunAll()}
            type="button"
          >
            <Play size={14} />
            {running ? "Running…" : "Run all"}
          </button>
          <button
            className="btn"
            onClick={() => setImporting((v) => !v)}
            type="button"
          >
            <Download size={14} />
            Import
          </button>
          <button
            className="btn"
            onClick={() => setCreating((v) => !v)}
            type="button"
          >
            <Plus size={14} />
            New
          </button>
        </div>
      </div>

      {creating && (
        <NewFixtureForm
          onCancel={() => setCreating(false)}
          onCreated={async () => {
            setCreating(false);
            await load();
          }}
        />
      )}

      {importing && (
        <ImportFixturesForm
          onCancel={() => setImporting(false)}
          onImported={async () => {
            setImporting(false);
            await load();
          }}
        />
      )}

      {loading && fixtures.length === 0 ? (
        <div className="space-y-2 p-5">
          {Array.from({ length: 3 }).map((_, i) => (
            <div key={i} className="h-12 animate-pulse rounded-xl bg-zinc-100" />
          ))}
        </div>
      ) : fixtures.length === 0 ? (
        <div className="px-5 py-10 text-center">
          <Beaker className="mx-auto mb-2 text-zinc-200" size={24} />
          <p className="text-sm text-zinc-400">No fixtures yet.</p>
          <p className="mt-1 text-xs text-zinc-300">
            Add a labelled query to start measuring regressions.
          </p>
        </div>
      ) : (
        <ul className="divide-y divide-zinc-50">
          {fixtures.map(({ fixture, latestRun, history }) => (
            <FixtureRow
              key={fixture.id}
              fixture={fixture}
              latestRun={latestRun}
              history={history}
              expanded={expanded.has(fixture.id)}
              onToggle={() => toggle(fixture.id)}
              onRun={() => void handleRunOne(fixture.id)}
              onDelete={() => void handleDelete(fixture.id)}
              onSetBaseline={(runId) =>
                void handleSetBaseline(fixture.id, runId)
              }
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function FixtureRow({
  fixture,
  latestRun,
  history,
  expanded,
  onToggle,
  onRun,
  onDelete,
  onSetBaseline,
}: {
  fixture: EvalFixture;
  latestRun?: EvalRun;
  history: EvalRun[];
  expanded: boolean;
  onToggle: () => void;
  onRun: () => void;
  onDelete: () => void;
  onSetBaseline: (runId: string) => void;
}) {
  const delta = formatDelta(latestRun?.delta ?? null);
  return (
    <li>
      <div className="flex items-center gap-3 px-5 py-3">
        <button
          aria-expanded={expanded}
          className="text-zinc-300 hover:text-zinc-700"
          onClick={onToggle}
          type="button"
        >
          {expanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        </button>
        {latestRun?.passed ? (
          <CheckCircle2 className="text-emerald-500" size={14} />
        ) : latestRun ? (
          <XCircle className="text-red-500" size={14} />
        ) : (
          <div className="h-3.5 w-3.5 rounded-full border border-zinc-200" />
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-[12px] font-medium text-zinc-900">
              {fixture.name}
            </span>
            <span className="rounded-md bg-zinc-100 px-1.5 py-0.5 text-[10px] font-medium text-zinc-500">
              {fixture.kind}
            </span>
          </div>
          {latestRun ? (
            <p className="text-[11px] text-zinc-500">
              {formatScore(latestRun.score, latestRun.metric)} ·{" "}
              {latestRun.metric}
              <span className={`ml-2 ${delta.className}`}>{delta.text}</span>
            </p>
          ) : (
            <p className="text-[11px] text-zinc-400">Not run yet</p>
          )}
        </div>
        <button
          aria-label="Run fixture"
          className="btn h-7 w-7 px-0"
          onClick={onRun}
          type="button"
        >
          <Play size={12} />
        </button>
        <button
          aria-label="Delete fixture"
          className="btn h-7 w-7 px-0 text-zinc-400 hover:text-red-600"
          onClick={onDelete}
          type="button"
        >
          <Trash2 size={12} />
        </button>
      </div>
      {expanded && (
        <div className="space-y-3 border-t border-zinc-50 bg-zinc-50/50 px-5 py-4 text-[11px]">
          {fixture.notes && (
            <p className="italic text-zinc-500">{fixture.notes}</p>
          )}
          <div className="grid grid-cols-2 gap-3">
            <Pre label="Input" value={fixture.input_json} />
            <Pre label="Expectation" value={fixture.expectation_json} />
          </div>
          {history.length > 0 && (
            <div>
              <p className="page-kicker mb-1">History (latest 10)</p>
              <ul className="space-y-1">
                {history.slice(0, 10).map((run) => (
                  <li
                    key={run.id}
                    className="flex items-center gap-2 rounded-md border border-zinc-100 bg-white px-2 py-1"
                  >
                    {run.passed ? (
                      <CheckCircle2 className="text-emerald-500" size={12} />
                    ) : (
                      <XCircle className="text-red-500" size={12} />
                    )}
                    <span className="font-mono text-[10px] tabular-nums text-zinc-700">
                      {formatScore(run.score, run.metric)}
                    </span>
                    <span className="flex-1 text-[10px] text-zinc-400">
                      {new Date(run.ts).toLocaleString()}
                    </span>
                    <button
                      aria-label="Set as baseline"
                      className="btn h-6 w-6 px-0 text-zinc-400 hover:text-amber-600"
                      onClick={() => onSetBaseline(run.id)}
                      type="button"
                      title="Pin as baseline"
                    >
                      <Star size={10} />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </li>
  );
}

function Pre({ label, value }: { label: string; value: string }) {
  let pretty: string;
  try {
    pretty = JSON.stringify(JSON.parse(value), null, 2);
  } catch {
    pretty = value;
  }
  return (
    <div>
      <p className="mb-1 text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
        {label}
      </p>
      <pre className="max-h-40 overflow-auto rounded-lg border border-zinc-100 bg-white px-3 py-2 font-mono text-[10px] text-zinc-700">
        {pretty}
      </pre>
    </div>
  );
}

function NewFixtureForm({
  onCancel,
  onCreated,
}: {
  onCancel: () => void;
  onCreated: () => void | Promise<void>;
}) {
  const [kind, setKind] = useState<EvalFixtureKind>("retrieval");
  const [name, setName] = useState("");
  const [inputJson, setInputJson] = useState(
    JSON.stringify(
      {
        query: "blocked deliverables",
      },
      null,
      2,
    ),
  );
  const [expectationJson, setExpectationJson] = useState(
    JSON.stringify(
      {
        expected_entity_ids: ["<entity_id_here>"],
        top_k: 3,
      },
      null,
      2,
    ),
  );
  const [notes, setNotes] = useState("");
  const [saving, setSaving] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) {
      toast.warning("Name required");
      return;
    }
    setSaving(true);
    try {
      const input: CreateEvalFixtureInput = {
        kind,
        name: name.trim(),
        input_json: inputJson,
        expectation_json: expectationJson,
        notes: notes.trim() || null,
      };
      await createEvalFixture(input);
      await onCreated();
    } catch {
      // ipc wrapper toasts
    } finally {
      setSaving(false);
    }
  };

  return (
    <form
      className="space-y-3 border-b border-zinc-100 bg-zinc-50/50 px-5 py-4 text-[12px]"
      onSubmit={(e) => void handleSubmit(e)}
    >
      <div className="flex flex-wrap items-center gap-2">
        <select
          aria-label="Fixture kind"
          className="rounded-lg border border-zinc-200 bg-white px-2 py-1.5 text-[12px]"
          onChange={(e) => setKind(e.target.value as EvalFixtureKind)}
          value={kind}
        >
          {KIND_OPTIONS.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label} ({k.hint})
            </option>
          ))}
        </select>
        <input
          aria-label="Fixture name"
          className="flex-1 rounded-lg border border-zinc-200 bg-white px-2.5 py-1.5"
          onChange={(e) => setName(e.target.value)}
          placeholder="Descriptive name…"
          value={name}
        />
      </div>
      <div className="grid grid-cols-2 gap-2">
        <label className="space-y-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Input JSON
          </span>
          <textarea
            aria-label="Input JSON"
            className="h-32 w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5 font-mono text-[11px]"
            onChange={(e) => setInputJson(e.target.value)}
            value={inputJson}
          />
        </label>
        <label className="space-y-1">
          <span className="text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Expectation JSON
          </span>
          <textarea
            aria-label="Expectation JSON"
            className="h-32 w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5 font-mono text-[11px]"
            onChange={(e) => setExpectationJson(e.target.value)}
            value={expectationJson}
          />
        </label>
      </div>
      <input
        aria-label="Notes"
        className="field-control"
        onChange={(e) => setNotes(e.target.value)}
        placeholder="Notes (optional)"
        value={notes}
      />
      <div className="flex justify-end gap-2">
        <button
          className="btn"
          onClick={onCancel}
          type="button"
        >
          Cancel
        </button>
        <button className="btn btn-primary" disabled={saving} type="submit">
          {saving ? "Saving…" : "Create fixture"}
        </button>
      </div>
    </form>
  );
}

function ImportFixturesForm({
  onCancel,
  onImported,
}: {
  onCancel: () => void;
  onImported: () => void | Promise<void>;
}) {
  const [json, setJson] = useState("");
  const [skipExisting, setSkipExisting] = useState(true);
  const [importing, setImporting] = useState(false);

  const handleImport = async () => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(json);
    } catch (error) {
      toast.error("Invalid JSON", { description: String(error) });
      return;
    }
    const fixtures = Array.isArray(parsed)
      ? parsed
      : Array.isArray((parsed as { fixtures?: unknown[] })?.fixtures)
        ? (parsed as { fixtures: unknown[] }).fixtures
        : null;
    if (!fixtures) {
      toast.error("Expected a JSON array of fixtures (or { fixtures: [...] })");
      return;
    }
    setImporting(true);
    try {
      const result = await importEvalFixtures({
        fixtures,
        skip_existing: skipExisting,
      });
      const errorCount = result.errors.length;
      const message =
        `Imported ${result.imported} · skipped ${result.skipped}` +
        (errorCount > 0 ? ` · ${errorCount} error${errorCount === 1 ? "" : "s"}` : "");
      if (errorCount > 0) {
        toast.warning(message, {
          description: result.errors.slice(0, 3).join("; "),
        });
      } else if (result.imported > 0) {
        toast.success(message);
      } else {
        toast.info(message);
      }
      await onImported();
    } catch {
      // ipc wrapper toasts on error
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="space-y-3 border-b border-zinc-100 bg-zinc-50/50 px-5 py-4 text-[12px]">
      <p className="text-[11px] text-zinc-500">
        Paste a JSON array of fixtures. See{" "}
        <code className="rounded bg-zinc-100 px-1 py-0.5 font-mono text-[10px]">
          src-tauri/eval_fixtures/README.md
        </code>{" "}
        for the schema, or paste the contents of{" "}
        <code className="rounded bg-zinc-100 px-1 py-0.5 font-mono text-[10px]">
          seed.json
        </code>{" "}
        to bootstrap with placeholder fixtures.
      </p>
      <textarea
        aria-label="Fixture JSON"
        className="h-48 w-full rounded-lg border border-zinc-200 bg-white px-2 py-1.5 font-mono text-[11px]"
        onChange={(e) => setJson(e.target.value)}
        placeholder='[{"kind": "retrieval", "name": "blocked-work", "input_json": {...}, "expectation_json": {...}}]'
        value={json}
      />
      <label className="flex items-center gap-2 text-[11px] text-zinc-700">
        <input
          checked={skipExisting}
          className="h-3.5 w-3.5 accent-emerald-500"
          onChange={(e) => setSkipExisting(e.target.checked)}
          type="checkbox"
        />
        Skip fixtures whose name already exists
      </label>
      <div className="flex justify-end gap-2">
        <button className="btn" onClick={onCancel} type="button">
          Cancel
        </button>
        <button
          className="btn btn-primary"
          disabled={importing || !json.trim()}
          onClick={() => void handleImport()}
          type="button"
        >
          {importing ? "Importing…" : "Import"}
        </button>
      </div>
    </div>
  );
}
