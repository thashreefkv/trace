import { useCallback, useEffect, useState } from "react";
import { ArrowLeft, FileText, Plus, RefreshCw, Trash2 } from "lucide-react";

import {
  createReportRun,
  deleteReportRun,
  listClaimReviewItems,
  listCommunityReports,
  listInitiatives,
  listReportRuns,
  listReportSteps,
  listReasoningReviewItems,
  refreshReasoningIndex,
  reviewClaim,
  reviewCommunityReport,
  reviewReasoningItem,
} from "../../lib/ipc";
import { toast } from "../../lib/toast";
import type {
  AgentReviewItem,
  ClaimReviewItem,
  CommunityReport,
  CreateReportRunInput,
  Initiative,
  ReportRun,
  ReportStep,
} from "../../lib/types";
import { Dialog, DialogConfirm } from "../../components/ui/Dialog";
import { CreatePanel } from "./CreatePanel";
import { ReportDetail } from "./ReportDetail";
import { ReviewQueue } from "./ReviewQueue";
import { StepTimeline } from "./StepTimeline";

export function ReportsWorkspace() {
  const [reports, setReports] = useState<ReportRun[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [steps, setSteps] = useState<ReportStep[]>([]);
  const [claims, setClaims] = useState<ClaimReviewItem[]>([]);
  const [communities, setCommunities] = useState<CommunityReport[]>([]);
  const [proposals, setProposals] = useState<AgentReviewItem[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [creating, setCreating] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<ReportRun | null>(null);
  const [busy, setBusy] = useState(false);
  const selected = reports.find((r) => r.id === selectedId) ?? null;

  // true = show step timeline in left rail instead of report list
  const showTimeline = selectedId !== null;

  const load = useCallback(async () => {
    const [reportRows, claimRows, communityRows, proposalRows, initiativeRows] =
      await Promise.all([
        listReportRuns(),
        listClaimReviewItems("pending"),
        listCommunityReports("pending"),
        listReasoningReviewItems("pending"),
        listInitiatives(),
      ]);
    setReports(reportRows);
    setClaims(claimRows);
    setCommunities(communityRows);
    setProposals(proposalRows);
    setInitiatives(initiativeRows);
    setSelectedId((current) => current ?? reportRows[0]?.id ?? null);
  }, []);

  useEffect(() => { void load(); }, [load]);

  // Load steps whenever selectedId changes
  useEffect(() => {
    if (!selectedId) { setSteps([]); return; }
    void listReportSteps(selectedId).then(setSteps);
  }, [selectedId]);

  async function create(input: CreateReportRunInput) {
    setBusy(true);
    try {
      const report = await createReportRun(input);
      setReports((prev) => [report, ...prev]);
      setSelectedId(report.id);
      setCreating(false);
    } finally {
      setBusy(false);
    }
  }

  async function removeReport() {
    if (!deleteTarget) return;
    setBusy(true);
    try {
      await deleteReportRun(deleteTarget.id);
      setReports((prev) => {
        const next = prev.filter((r) => r.id !== deleteTarget.id);
        setSelectedId((cur) => (cur === deleteTarget.id ? next[0]?.id ?? null : cur));
        return next;
      });
      setDeleteTarget(null);
      toast.success("Report run deleted.");
    } finally {
      setBusy(false);
    }
  }

  const updateReport = useCallback((report: ReportRun) => {
    setReports((prev) => prev.map((r) => (r.id === report.id ? report : r)));
  }, []); // setReports is a stable React state setter

  async function refreshIndex() {
    setBusy(true);
    try {
      const result = await refreshReasoningIndex();
      toast.success(
        `Evidence refreshed: ${result.source_units_created} new, ${result.source_units_updated} updated, ${result.candidate_claims_staged} candidate claims staged.`,
      );
      await load();
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto max-w-[1500px] px-5 py-6">
      <header className="mb-6 flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="page-kicker inline-flex items-center gap-2">
            <FileText size={13} /> Reports
          </div>
          <h1 className="mt-2 text-2xl font-semibold tracking-tight text-zinc-950">
            Strategic reasoning workspace
          </h1>
          <p className="mt-1 text-sm text-zinc-500">
            Step-by-step evidence-grounded reports with real-time pipeline visibility.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <button
            className="btn h-8"
            disabled={busy}
            onClick={() => void refreshIndex()}
            type="button"
          >
            <RefreshCw size={13} /> Refresh evidence
          </button>
          <button
            className="btn btn-primary h-8"
            onClick={() => setCreating(true)}
            type="button"
          >
            <Plus size={14} /> New report
          </button>
        </div>
      </header>

      <div className="grid gap-5 lg:grid-cols-[260px_minmax(480px,1fr)_280px]">
        {/* Left rail: report list or step timeline */}
        <section className="h-fit rounded-2xl border border-zinc-100 bg-white p-3 shadow-[0_2px_12px_rgba(0,0,0,0.04)]">
          {showTimeline && selected ? (
            <>
              <div className="mb-2 flex items-center gap-2 px-1">
                <button
                  className="text-zinc-400 hover:text-zinc-700"
                  onClick={() => setSelectedId(null)}
                  type="button"
                >
                  <ArrowLeft size={14} />
                </button>
                <span className="truncate text-xs font-semibold text-zinc-700">
                  {selected.title}
                </span>
              </div>
              <StepTimeline reportRunId={selected.id} steps={steps} />
            </>
          ) : (
            <>
              <h2 className="px-2 pb-3 text-xs font-semibold uppercase tracking-wider text-zinc-400">
                Report runs
              </h2>
              {reports.length === 0 ? (
                <p className="rounded-xl bg-zinc-50 px-3 py-4 text-xs leading-5 text-zinc-500">
                  No reports yet. Start a report to gather cited workgraph evidence.
                </p>
              ) : null}
              {reports.map((report) => (
                <div
                  className={[
                    "group mb-1 flex items-start gap-1 rounded-xl border px-1 py-1 transition-colors",
                    selectedId === report.id
                      ? "border-zinc-200 bg-zinc-50"
                      : "border-transparent hover:bg-zinc-50",
                  ].join(" ")}
                  key={report.id}
                >
                  <button
                    className="min-w-0 flex-1 px-2 py-2 text-left"
                    onClick={() => setSelectedId(report.id)}
                    type="button"
                  >
                    <div className="truncate text-xs font-semibold text-zinc-900">
                      {report.title}
                    </div>
                    <div className="mt-1 text-[10px] uppercase tracking-wide text-zinc-400">
                      {report.report_type.replace("_", " ")} ·{" "}
                      {report.status.replace("_", " ")}
                    </div>
                  </button>
                  <button
                    aria-label={`Delete ${report.title}`}
                    className="mt-1 inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md text-zinc-400 opacity-70 hover:bg-white hover:text-red-500"
                    onClick={() => setDeleteTarget(report)}
                    type="button"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              ))}
            </>
          )}
        </section>

        {/* Center: report detail */}
        {selected ? (
          <ReportDetail
            initiatives={initiatives}
            onStepsChange={setSteps}
            onUpdate={updateReport}
            report={selected}
            steps={steps}
          />
        ) : (
          <div className="rounded-2xl border border-dashed border-zinc-200 p-12 text-center text-sm text-zinc-400">
            <p>Create a report run to begin evidence gathering.</p>
            <button
              className="btn btn-primary mt-4"
              onClick={() => setCreating(true)}
              type="button"
            >
              <Plus size={14} /> New report
            </button>
          </div>
        )}

        {/* Right rail: review queue */}
        <ReviewQueue
          claims={claims}
          communities={communities}
          onReviewClaim={(id, d) => reviewClaim(id, d).then(() => void load())}
          onReviewCommunity={(id, d) => reviewCommunityReport(id, d).then(() => void load())}
          onReviewProposal={(id, d) => reviewReasoningItem(id, d).then(() => void load())}
          proposals={proposals}
        />
      </div>

      <Dialog
        description="Choose a report type and the evidence scope that Gemini Pro should reason over."
        kicker="Create"
        onOpenChange={setCreating}
        open={creating}
        size="lg"
        title="New strategic report"
      >
        <Dialog.Body>
          <CreatePanel
            busy={busy}
            initiatives={initiatives}
            onCancel={() => setCreating(false)}
            onCreate={create}
          />
        </Dialog.Body>
      </Dialog>

      <DialogConfirm
        confirmLabel="Delete report run"
        description="This removes the draft run, its exports, and any pending review artifacts. Approved knowledge, linked deliverables, and Google Docs remain unchanged."
        destructive
        loading={busy}
        onConfirm={() => void removeReport()}
        onOpenChange={(open) => { if (!open) setDeleteTarget(null); }}
        open={deleteTarget !== null}
        title={`Delete ${deleteTarget?.title ?? "report"}?`}
      />
    </div>
  );
}
