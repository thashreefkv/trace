import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, Reorder } from "framer-motion";
import { GripVertical, Loader2, Plus, RotateCcw, Trash2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { rerunReportStep, updateReportSections } from "../../lib/ipc";
import type {
  ReportCritique,
  ReportRun,
  ReportSectionDraft,
  ReportSectionPlan,
} from "../../lib/types";

interface Props {
  report: ReportRun;
  onUpdate: (report: ReportRun) => void;
  streamingText: Record<string, string>;
}

export function SectionEditor({ report, onUpdate, streamingText }: Props) {
  const [sections, setSections] = useState<ReportSectionPlan[]>(() =>
    parseSections(report.sections_json),
  );
  const [drafts, setDrafts] = useState<Record<string, ReportSectionDraft>>(() =>
    parseDrafts(report.section_drafts_json),
  );
  const [critique, setCritique] = useState<ReportCritique>(() =>
    parseCritique(report.critique_json),
  );

  useEffect(() => {
    setSections(parseSections(report.sections_json));
    setDrafts(parseDrafts(report.section_drafts_json));
    setCritique(parseCritique(report.critique_json));
  }, [report.sections_json, report.section_drafts_json, report.critique_json]);

  async function save(next: ReportSectionPlan[]) {
    const updated = await updateReportSections(report.id, next);
    onUpdate(updated);
  }

  function updateSection(id: string, patch: Partial<ReportSectionPlan>) {
    setSections((prev) => {
      const next = prev.map((s) => (s.id === id ? { ...s, ...patch } : s));
      void save(next);
      return next;
    });
  }

  function removeSection(id: string) {
    setSections((prev) => {
      const next = prev.filter((s) => s.id !== id);
      void save(next);
      return next;
    });
  }

  function addSection() {
    const newSection: ReportSectionPlan = {
      id: `sec_${crypto.randomUUID().replace(/-/g, "")}`,
      heading: "New section",
      instructions: "",
      status: "queued",
      position: sections.length,
    };
    const next = [...sections, newSection];
    setSections(next);
    void save(next);
  }

  async function handleReorder(reordered: ReportSectionPlan[]) {
    const withPositions = reordered.map((s, i) => ({ ...s, position: i }));
    setSections(withPositions);
    void save(withPositions);
  }

  if (sections.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-zinc-200 p-8 text-center">
        <p className="text-xs text-zinc-400">
          No sections yet — run the pipeline to plan them, or add one manually.
        </p>
        <button className="btn btn-primary mt-3 h-8 text-xs" onClick={addSection} type="button">
          <Plus size={13} /> Add section
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {critique.issues.length > 0 ? <CritiqueSummary critique={critique} /> : null}

      <Reorder.Group axis="y" onReorder={handleReorder} values={sections}>
        {sections.map((section) => (
          <Reorder.Item key={section.id} value={section}>
            <SectionCard
              critique={critique}
              draft={drafts[section.id] ?? null}
              onRedraft={() => rerunReportStep(report.id, "draft_section", section.id)}
              onRemove={() => removeSection(section.id)}
              onUpdate={(patch) => updateSection(section.id, patch)}
              section={section}
              streamText={streamingText[section.id]}
            />
          </Reorder.Item>
        ))}
      </Reorder.Group>

      <button
        className="flex w-full items-center justify-center gap-1.5 rounded-xl border border-dashed border-zinc-200 py-2.5 text-xs text-zinc-400 transition-colors hover:border-zinc-300 hover:text-zinc-600"
        onClick={addSection}
        type="button"
      >
        <Plus size={13} /> Add section
      </button>
    </div>
  );
}

function SectionCard({
  section,
  draft,
  critique,
  streamText,
  onUpdate,
  onRemove,
  onRedraft,
}: {
  section: ReportSectionPlan;
  draft: ReportSectionDraft | null;
  critique: ReportCritique;
  streamText: string | undefined;
  onUpdate: (patch: Partial<ReportSectionPlan>) => void;
  onRemove: () => void;
  onRedraft: () => void;
}) {
  const [editingHeading, setEditingHeading] = useState(false);
  const [editingInstructions, setEditingInstructions] = useState(false);
  const headingRef = useRef<HTMLInputElement>(null);
  const sectionIssues = critique.issues.filter((i) => i.section_id === section.id);

  useEffect(() => {
    if (editingHeading) headingRef.current?.focus();
  }, [editingHeading]);

  return (
    <motion.div
      animate={{ opacity: 1 }}
      className="mb-3 rounded-2xl border border-zinc-100 bg-white shadow-[0_2px_12px_rgba(0,0,0,0.04)]"
      initial={{ opacity: 0 }}
      layout
    >
      {/* Card header */}
      <div className="flex items-center gap-2 border-b border-zinc-100 px-4 py-3">
        <GripVertical
          className="shrink-0 cursor-grab text-zinc-300 active:cursor-grabbing"
          size={16}
        />
        <div className="min-w-0 flex-1">
          {editingHeading ? (
            <input
              className="w-full bg-transparent text-sm font-semibold text-zinc-900 outline-none"
              defaultValue={section.heading}
              onBlur={(e) => {
                onUpdate({ heading: e.currentTarget.value });
                setEditingHeading(false);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.currentTarget.blur();
              }}
              ref={headingRef}
            />
          ) : (
            <button
              className="text-left text-sm font-semibold text-zinc-900 hover:text-violet-700"
              onClick={() => setEditingHeading(true)}
              type="button"
            >
              {section.heading}
            </button>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          {draft ? (
            <button
              className="inline-flex h-7 items-center gap-1 rounded-lg border border-zinc-200 px-2.5 text-[11px] text-zinc-600 transition-colors hover:bg-zinc-50"
              onClick={onRedraft}
              type="button"
            >
              <RotateCcw size={11} /> Re-draft
            </button>
          ) : null}
          <button
            aria-label="Remove section"
            className="inline-flex h-7 w-7 items-center justify-center rounded-lg text-zinc-300 transition-colors hover:bg-red-50 hover:text-red-500"
            onClick={onRemove}
            type="button"
          >
            <Trash2 size={13} />
          </button>
        </div>
      </div>

      {/* Card body */}
      <div className="space-y-3 px-4 py-4">
        {sectionIssues.length > 0 ? (
          <div className="rounded-xl border border-amber-200 bg-amber-50 px-3 py-2.5">
            {sectionIssues.map((issue) => (
              <p className="text-xs text-amber-800" key={issue.id}>
                <span className="font-semibold">{issue.kind.replace("_", " ")}</span>:{" "}
                {issue.message}
                {issue.suggestion ? (
                  <span className="text-amber-700"> — {issue.suggestion}</span>
                ) : null}
              </p>
            ))}
          </div>
        ) : null}

        {/* Writer instructions */}
        <div>
          <label className="mb-1 block text-[10px] font-semibold uppercase tracking-wider text-zinc-400">
            Writer instructions
          </label>
          {editingInstructions ? (
            <AutoGrowTextarea
              defaultValue={section.instructions}
              onBlur={(value) => {
                onUpdate({ instructions: value });
                setEditingInstructions(false);
              }}
              placeholder="E.g. focus on Q4 milestones, skip pricing details…"
            />
          ) : (
            <button
              className={[
                "w-full rounded-lg border border-transparent px-2 py-1.5 text-left text-xs transition-colors hover:border-zinc-100 hover:bg-zinc-50",
                section.instructions ? "text-zinc-700" : "text-zinc-400",
              ].join(" ")}
              onClick={() => setEditingInstructions(true)}
              type="button"
            >
              {section.instructions || "Click to add writer instructions…"}
            </button>
          )}
        </div>

        {/* Draft content or placeholder */}
        {draft !== null || streamText !== undefined ? (
          <div className="border-t border-zinc-100 pt-3">
            <DraftBody draft={draft} streamText={streamText} />
          </div>
        ) : (
          <div className="flex items-center gap-2 text-xs text-zinc-400">
            <Loader2 className="animate-pulse" size={12} />
            Draft not yet generated
          </div>
        )}
      </div>
    </motion.div>
  );
}

function DraftBody({
  draft,
  streamText,
}: {
  draft: ReportSectionDraft | null;
  streamText?: string;
}) {
  const [expanded, setExpanded] = useState(true);
  const content = streamText ?? draft?.markdown ?? "";
  const citationCount = draft?.citation_ids.length ?? 0;
  const isStreaming = streamText !== undefined;

  return (
    <div>
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[10px] font-bold uppercase tracking-widest text-zinc-400">
          Draft · {citationCount} citation{citationCount !== 1 ? "s" : ""}
          {draft?.cache_hit ? " · cached" : ""}
          {isStreaming ? " · streaming…" : ""}
        </span>
        <button
          className="text-[10px] text-zinc-400 transition-colors hover:text-zinc-600"
          onClick={() => setExpanded((v) => !v)}
          type="button"
        >
          {expanded ? "Collapse" : "Expand"}
        </button>
      </div>

      <AnimatePresence>
        {expanded ? (
          <motion.div
            animate={{ height: "auto", opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            initial={{ height: 0, opacity: 0 }}
            transition={{ duration: 0.15 }}
          >
            <div className="rounded-xl border border-zinc-100 bg-zinc-50 px-5 py-4">
              <div className="prose prose-sm max-w-none prose-headings:font-semibold prose-headings:text-zinc-900 prose-p:text-zinc-700 prose-a:text-violet-600 prose-strong:text-zinc-900 prose-code:rounded prose-code:bg-zinc-100 prose-code:px-1 prose-code:py-0.5 prose-code:text-violet-700 prose-code:font-normal prose-code:before:content-none prose-code:after:content-none">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  code({ children, className }) {
                    const text = String(children).trim();
                    const citeMatch = /^\[S(\d+)\]$/.exec(text);
                    // Render [S1], [S2] etc. as violet citation badges
                    if (citeMatch && !className) {
                      return (
                        <span className="inline-flex items-center rounded bg-violet-50 px-1.5 py-0.5 text-[10px] font-semibold text-violet-700 ring-1 ring-inset ring-violet-700/10">
                          S{citeMatch[1]}
                        </span>
                      );
                    }
                    return <code className={className}>{children}</code>;
                  },
                }}
              >
                {preprocessMarkdown(content)}
              </ReactMarkdown>
              </div>
            </div>
          </motion.div>
        ) : null}
      </AnimatePresence>
    </div>
  );
}

/** Convert bare [S1], [S2] markers to inline code spans so the code override can badge them */
function preprocessMarkdown(md: string): string {
  return md.replace(/\[S(\d+)\]/g, "`[S$1]`");
}

function CritiqueSummary({ critique }: { critique: ReportCritique }) {
  const counts = critique.issues.reduce<Record<string, number>>((acc, issue) => {
    acc[issue.kind] = (acc[issue.kind] ?? 0) + 1;
    return acc;
  }, {});

  return (
    <div className="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3">
      <div className="flex items-center gap-2">
        <span className="text-xs font-semibold text-amber-900">
          Issues ({critique.issues.length})
        </span>
        <span className="text-xs text-amber-700">
          {Object.entries(counts)
            .map(([k, n]) => `${n} ${k.replace("_", " ")}`)
            .join(" · ")}
        </span>
      </div>
    </div>
  );
}

function AutoGrowTextarea({
  defaultValue,
  placeholder,
  onBlur,
}: {
  defaultValue: string;
  placeholder?: string;
  onBlur: (value: string) => void;
}) {
  const [value, setValue] = useState(defaultValue);

  return (
    <textarea
      autoFocus
      className="w-full resize-none rounded-lg border border-violet-300 bg-white px-2 py-1.5 text-xs text-zinc-800 outline-none focus:border-violet-400"
      onBlur={(e) => onBlur(e.currentTarget.value)}
      onChange={(e) => setValue(e.currentTarget.value)}
      placeholder={placeholder}
      rows={Math.max(2, value.split("\n").length)}
      value={value}
    />
  );
}

function parseSections(raw: string): ReportSectionPlan[] {
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as ReportSectionPlan[]) : [];
  } catch {
    return [];
  }
}

function parseDrafts(raw: string): Record<string, ReportSectionDraft> {
  try {
    const parsed = JSON.parse(raw);
    return typeof parsed === "object" && parsed !== null
      ? (parsed as Record<string, ReportSectionDraft>)
      : {};
  } catch {
    return {};
  }
}

function parseCritique(raw: string): ReportCritique {
  try {
    const parsed = JSON.parse(raw);
    if (
      parsed &&
      typeof parsed === "object" &&
      Array.isArray((parsed as ReportCritique).issues)
    ) {
      return parsed as ReportCritique;
    }
    return { issues: [], generated_at: null };
  } catch {
    return { issues: [], generated_at: null };
  }
}
