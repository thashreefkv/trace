import { type Dispatch, type SetStateAction, useEffect, useMemo, useRef, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { listen } from "@tauri-apps/api/event";
import { ExternalLink, FileDown, Link2, Play, Sparkles } from "lucide-react";

import {
  approveReport,
  createReportDeliverable,
  exportReport,
  getReportRun,
  listReportExports,
  listReportSteps,
  runReportPipeline,
} from "../../lib/ipc";
import { toast } from "../../lib/toast";
import type {
  Initiative,
  ReasoningScope,
  ReportChannelEvent,
  ReportExport,
  ReportRun,
  ReportStep,
} from "../../lib/types";
import { ScopePreview, EmptyScope } from "./ScopePreview";
import { SectionEditor } from "./SectionEditor";
import { SteerComposer } from "./SteerComposer";

interface Props {
  report: ReportRun;
  onUpdate: (report: ReportRun) => void;
  initiatives: Initiative[];
  steps: ReportStep[];
  onStepsChange: Dispatch<SetStateAction<ReportStep[]>>;
}

export function ReportDetail({ report, onUpdate, initiatives, steps, onStepsChange }: Props) {
  const [exports, setExports] = useState<ReportExport[]>([]);
  const [launching, setLaunching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Streaming text keyed by section_id, updated live from SectionDelta events
  const [streamingText, setStreamingText] = useState<Record<string, string>>({});

  // Stable refs so the event listener closure never becomes stale
  const onUpdateRef = useRef(onUpdate);
  useEffect(() => { onUpdateRef.current = onUpdate; }, [onUpdate]);
  const onStepsChangeRef = useRef(onStepsChange);
  useEffect(() => { onStepsChangeRef.current = onStepsChange; }, [onStepsChange]);

  const scope = useMemo(() => parseScope(report.scope_json), [report.scope_json]);
  const scopedInitiatives = scope?.initiative_ids
    .map((id) => initiatives.find((i) => i.id === id)?.title ?? id)
    .join(", ");

  const sections = useMemo(() => parseSections(report.sections_json), [report.sections_json]);
  const isRunning = steps.some((s) => s.status === "running");
  const hasSections = sections.length > 0;
  const isDrafted = ["drafted", "approved"].includes(report.status);

  useEffect(() => {
    setError(null);
  }, [report.id]);

  useEffect(() => {
    void listReportExports(report.id).then(setExports);
  }, [report.id]);

  // Subscribe to report:event — in-place step updates, no DB round-trip per event.
  // Only depends on the stable report.id; callbacks are read from refs.
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    async function setup() {
      unlisten = await listen<ReportChannelEvent>("report:event", (event) => {
        if (cancelled) return;
        const p = event.payload;
        // Filter: ignore events for other reports
        if ("report_run_id" in p && p.report_run_id !== report.id) return;

        switch (p.kind) {
          case "StepCreated":
            onStepsChangeRef.current((prev) => {
              // Avoid duplicates (re-mount may load via DB before event arrives)
              if (prev.some((s) => s.id === p.step_id)) return prev;
              return [...prev, {
                id: p.step_id, report_run_id: p.report_run_id,
                step_name: p.step_name, section_index: p.section_index ?? null,
                status: "queued" as const, model: null, cache_hit: false,
                started_at: null, finished_at: null, latency_ms: null,
                input_json: "{}", output_json: "{}", clarification_json: null,
                ticker_label: null, error_text: null,
                created_at: new Date().toISOString(), updated_at: new Date().toISOString(),
              }];
            });
            break;
          case "StepStarted":
            onStepsChangeRef.current((prev) => prev.map((s) =>
              s.id === p.step_id ? { ...s, status: "running" as const, ticker_label: p.ticker_label ?? null } : s
            ));
            break;
          case "StepProgress":
            onStepsChangeRef.current((prev) => prev.map((s) =>
              s.id === p.step_id ? { ...s, ticker_label: p.ticker_label } : s
            ));
            break;
          case "StepCompleted":
            onStepsChangeRef.current((prev) => prev.map((s) =>
              s.id === p.step_id
                ? { ...s, status: "done" as const, latency_ms: p.latency_ms ?? null, model: p.model ?? null, cache_hit: p.cache_hit }
                : s
            ));
            // Fetch fresh report to pick up new section_drafts_json / sections_json
            void getReportRun(report.id).then((r) => { if (!cancelled) onUpdateRef.current(r); });
            break;
          case "SectionDelta":
            // Live streaming: accumulate text per section_id
            setStreamingText((prev) => ({
              ...prev,
              [p.section_id]: (prev[p.section_id] ?? "") + p.text,
            }));
            break;
          case "StepFailed":
            onStepsChangeRef.current((prev) => prev.map((s) =>
              s.id === p.step_id ? { ...s, status: "error" as const, error_text: p.error } : s
            ));
            break;
          case "StepNeedsClarification":
            onStepsChangeRef.current((prev) => prev.map((s) =>
              s.id === p.step_id
                ? { ...s, status: "awaiting_clarification" as const, clarification_json: JSON.stringify(p.prompt) }
                : s
            ));
            break;
          case "RunFinished":
            // Re-fetch authoritative state at run end
            void getReportRun(report.id).then((r) => { if (!cancelled) onUpdateRef.current(r); });
            void listReportSteps(report.id).then((ss) => { if (!cancelled) onStepsChangeRef.current(ss); });
            setStreamingText({});
            break;
        }
      });
    }

    void setup();
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [report.id]); // Stable: only depends on the report id

  async function launch() {
    setLaunching(true);
    setError(null);
    try {
      await runReportPipeline(report.id);
      toast.success("Pipeline started — watch the step timeline for progress.");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setLaunching(false);
    }
  }

  async function doExport(format: "markdown" | "docx" | "pdf" | "google_docs") {
    let directory: string | undefined;
    if (format !== "google_docs") {
      const selected = await openDialog({ directory: true, multiple: false, title: "Export approved report" });
      if (typeof selected !== "string") return;
      directory = selected;
    } else if (!window.confirm("Create or update this approved report in Google Docs?")) {
      return;
    }
    const exported = await exportReport(report.id, format, directory, format === "google_docs");
    setExports((prev) => [exported, ...prev]);
    toast.success("Report exported.");
  }

  return (
    <section className="flex min-h-[70vh] flex-col rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
      {/* Header */}
      <div className="flex flex-wrap items-start justify-between gap-4 border-b border-zinc-100 px-6 py-5">
        <div>
          <div className="text-[11px] font-semibold uppercase tracking-widest text-violet-600">
            {report.report_type.replace("_", " ")} · {report.status.replace("_", " ")}
          </div>
          <h1 className="mt-1 text-xl font-semibold text-zinc-950">{report.title}</h1>
          {scopedInitiatives ? (
            <p className="mt-1 text-xs text-zinc-500">Initiative: {scopedInitiatives}</p>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {report.deliverable_id ? (
            <span className="inline-flex items-center gap-1 rounded-lg bg-green-50 px-3 py-2 text-xs text-green-700">
              <Link2 size={13} /> Deliverable linked
            </span>
          ) : report.status === "approved" ? (
            <button
              className="btn h-8 text-xs"
              onClick={() =>
                createReportDeliverable(report.id).then(async () => {
                  onUpdate(await getReportRun(report.id));
                  toast.success("Deliverable created.");
                })
              }
              type="button"
            >
              Create deliverable
            </button>
          ) : null}

          {report.status !== "approved" ? (
            <button
              className="btn btn-primary h-8 gap-1.5 text-xs"
              disabled={launching || isRunning}
              onClick={() => void launch()}
              type="button"
            >
              {isRunning ? (
                <><span className="h-3 w-3 animate-spin rounded-full border-2 border-white border-t-transparent" /> Running…</>
              ) : (
                <><Play size={12} /> {hasSections ? "Re-run pipeline" : "Run pipeline"}</>
              )}
            </button>
          ) : null}
        </div>
      </div>

      {error ? <div className="notice notice-error mx-6 mt-4 text-sm">{error}</div> : null}

      {/* Body */}
      <div className="flex-1 overflow-y-auto px-6 py-5 space-y-5">
        {/* Scope Preview — always show once report exists */}
        {scope && scope.initiative_ids.length > 0 ? (
          <ScopePreview onUpdate={onUpdate} report={report} />
        ) : sections.length === 0 ? (
          <EmptyScope />
        ) : null}

        {/* Section editor — visible once sections are planned */}
        {hasSections ? (
          <div>
            <div className="mb-3 flex items-center justify-between">
              <h2 className="text-sm font-semibold text-zinc-900">Sections</h2>
              {isDrafted ? (
                <span className="text-[10px] text-zinc-400">
                  {sections.length} section{sections.length !== 1 ? "s" : ""} · click to edit
                </span>
              ) : null}
            </div>
            <SectionEditor onUpdate={onUpdate} report={report} streamingText={streamingText} />
          </div>
        ) : null}

        {/* Approve button */}
        {report.status === "drafted" ? (
          <div className="rounded-xl border border-zinc-100 bg-zinc-50 p-4">
            <p className="text-xs text-zinc-600">Review the sections above, then approve to lock the report.</p>
            <button
              className="btn btn-primary mt-3 h-8 text-xs"
              onClick={() =>
                approveReport(report.id).then(onUpdate).catch((err) => toast.error(String(err)))
              }
              type="button"
            >
              <Sparkles size={13} /> Approve report
            </button>
          </div>
        ) : null}

        {/* Export section (approved only) */}
        {report.status === "approved" ? (
          <aside className="border-t border-zinc-100 pt-5">
            <h2 className="mb-3 text-sm font-semibold text-zinc-900">Export</h2>
            <div className="flex flex-wrap gap-2">
              {(["markdown", "docx", "pdf", "google_docs"] as const).map((format) => (
                <button
                  className="inline-flex items-center gap-1 rounded-lg border border-zinc-200 px-3 py-2 text-xs text-zinc-700 hover:bg-zinc-50"
                  key={format}
                  onClick={() => void doExport(format)}
                  type="button"
                >
                  <FileDown size={13} /> {format.replace("_", " ")}
                </button>
              ))}
            </div>
            {exports.map((item) => (
              <div className="mt-2 flex items-center gap-2 text-xs text-zinc-500" key={item.id}>
                <ExternalLink size={12} />
                {item.format.replace("_", " ")} · {item.export_path ?? item.artifact_url}
              </div>
            ))}
          </aside>
        ) : null}
      </div>

      {/* Bottom steering composer — always visible when pipeline has run at least once */}
      {hasSections || steps.length > 0 ? (
        <SteerComposer disabled={isRunning} reportRunId={report.id} />
      ) : null}
    </section>
  );
}

function parseScope(raw: string): ReasoningScope | null {
  try { return JSON.parse(raw) as ReasoningScope; }
  catch { return null; }
}

function parseSections(raw: string): unknown[] {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch { return []; }
}
