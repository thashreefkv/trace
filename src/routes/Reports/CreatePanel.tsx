import { useState, type FormEvent } from "react";

import type { CreateReportRunInput, Initiative, ReportType } from "../../lib/types";

const REPORT_TYPES: Array<{ type: ReportType; label: string; description: string }> = [
  {
    type: "quarterly",
    label: "Quarterly / Q3 Report",
    description: "Themes, movement, risks, decisions, and recommendations.",
  },
  {
    type: "initiative",
    label: "Initiative Report",
    description: "Progress, dependencies, evidence, and implications.",
  },
  {
    type: "decision_memo",
    label: "Decision Memo",
    description: "Recommendation, alternatives, evidence, and actions.",
  },
];

export function CreatePanel({
  onCreate,
  onCancel,
  busy,
  initiatives,
}: {
  onCreate: (input: CreateReportRunInput) => Promise<void>;
  onCancel: () => void;
  busy: boolean;
  initiatives: Initiative[];
}) {
  const [reportType, setReportType] = useState<ReportType>("quarterly");
  const [title, setTitle] = useState(REPORT_TYPES[0].label);
  const [quarter, setQuarter] = useState("");
  const [initiativeId, setInitiativeId] = useState("");
  const [dateFrom, setDateFrom] = useState("");
  const [dateTo, setDateTo] = useState("");

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (reportType === "initiative" && !initiativeId) return;
    await onCreate({
      report_type: reportType,
      title,
      scope: {
        quarter: reportType === "quarterly" ? quarter || null : null,
        date_from: dateFrom || null,
        date_to: dateTo || null,
        initiative_ids: initiativeId ? [initiativeId] : [],
        stakeholder_ids: [],
        deliverable_ids: [],
        source_kinds: [],
      },
    });
  }

  function chooseType(type: ReportType, label: string) {
    setReportType(type);
    setTitle(label);
    if (type === "quarterly") setInitiativeId("");
    if (type !== "quarterly") setQuarter("");
  }

  return (
    <form onSubmit={(event) => void submit(event)}>
      <p className="text-xs leading-5 text-zinc-500">
        Reports always use Deep reasoning. Draft assertions stay review-gated until approved.
      </p>
      <div className="mt-4 grid gap-2 sm:grid-cols-3">
        {REPORT_TYPES.map((item) => (
          <label
            className={[
              "block cursor-pointer rounded-xl border px-3 py-3 transition-colors",
              reportType === item.type ? "border-zinc-300 bg-zinc-50" : "border-zinc-100 hover:border-zinc-200",
            ].join(" ")}
            key={item.type}
          >
            <input
              checked={reportType === item.type}
              className="mr-2"
              name="report-type"
              onChange={() => chooseType(item.type, item.label)}
              type="radio"
            />
            <span className="text-xs font-semibold text-zinc-900">{item.label}</span>
            <span className="mt-2 block text-[11px] leading-5 text-zinc-500">{item.description}</span>
          </label>
        ))}
      </div>
      <label className="mt-4 block text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
        Title
        <input
          className="mt-1 block w-full rounded-lg border border-zinc-200 px-3 py-2 text-sm font-normal normal-case tracking-normal text-zinc-900"
          onChange={(event) => setTitle(event.currentTarget.value)}
          required
          value={title}
        />
      </label>
      {reportType === "quarterly" ? (
        <label className="mt-3 block text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
          Quarter label
          <input
            className="mt-1 block w-full rounded-lg border border-zinc-200 px-3 py-2 text-sm font-normal normal-case tracking-normal text-zinc-900"
            onChange={(event) => setQuarter(event.currentTarget.value)}
            placeholder="Q3 2026"
            value={quarter}
          />
        </label>
      ) : (
        <label className="mt-3 block text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
          {reportType === "initiative" ? "Initiative" : "Related initiative (optional)"}
          <select
            className="mt-1 block w-full rounded-lg border border-zinc-200 bg-white px-3 py-2 text-sm font-normal normal-case tracking-normal text-zinc-900"
            onChange={(event) => setInitiativeId(event.currentTarget.value)}
            required={reportType === "initiative"}
            value={initiativeId}
          >
            <option value="">{reportType === "initiative" ? "Select an initiative" : "No initiative selected"}</option>
            {initiatives.map((initiative) => (
              <option key={initiative.id} value={initiative.id}>
                {initiative.title} ({initiative.status})
              </option>
            ))}
          </select>
          {reportType === "initiative" && initiatives.length === 0 ? (
            <span className="mt-1 block text-[11px] font-normal normal-case tracking-normal text-amber-700">
              Create an initiative before generating an Initiative Report.
            </span>
          ) : null}
        </label>
      )}
      <div className="mt-3">
        <div className="mb-1 text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
          Date range <span className="font-normal normal-case tracking-normal text-zinc-400">(optional)</span>
        </div>
        <div className="grid grid-cols-2 gap-2">
        <label className="text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
          From
          <input className="mt-1 block w-full rounded-lg border border-zinc-200 px-2 py-2 text-xs font-normal" onChange={(event) => setDateFrom(event.currentTarget.value)} type="date" value={dateFrom} />
        </label>
        <label className="text-[11px] font-semibold uppercase tracking-wide text-zinc-500">
          To
          <input className="mt-1 block w-full rounded-lg border border-zinc-200 px-2 py-2 text-xs font-normal" onChange={(event) => setDateTo(event.currentTarget.value)} type="date" value={dateTo} />
        </label>
        </div>
      </div>
      <div className="mt-5 flex items-center justify-end gap-2">
        <button className="btn" disabled={busy} onClick={onCancel} type="button">Cancel</button>
        <button
          className="btn btn-primary"
          disabled={busy || (reportType === "initiative" && !initiativeId)}
          type="submit"
        >
          {busy ? "Creating..." : "Create report run"}
        </button>
      </div>
    </form>
  );
}
