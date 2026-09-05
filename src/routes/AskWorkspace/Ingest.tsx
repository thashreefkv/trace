// Conversation ingest panel: paste-text → extracted candidates → commit.
// Extracted from AskWorkspace.tsx (E6).

import { useEffect, useState } from "react";
import { CheckCircle2, Plus, Sparkles, X } from "lucide-react";

import {
  commitConversationIngest,
  createStakeholder,
  extractConversation,
  listInitiatives,
  listStakeholders,
} from "../../lib/ipc";
import {
  deliverableTypeLabels,
  deliverableTypeOptions,
  type CommitConversationIngestInput,
  type DeliverableType,
  type ExtractedConversation,
  type ExtractedDeliverableCandidate,
  type Initiative,
  type Stakeholder,
} from "../../lib/types";
import { PanelRow, PanelSurface } from "./panels";

interface IngestCandidateDraft extends ExtractedDeliverableCandidate {
  accepted: boolean;
  stakeholder_id: string;
  initiative_ids: string[];
  new_stakeholder_name: string;
}

const emptyIngestConversation: ExtractedConversation = { title: "", summary: "", occurred_at: null };

function hydrateIngestCandidate(
  candidate: ExtractedDeliverableCandidate,
  initiatives: Initiative[],
  stakeholders: Stakeholder[],
): IngestCandidateDraft {
  const initiative_ids = candidate.initiative_titles
    .map((t) => initiatives.find((i) => i.title === t)?.id)
    .filter((id): id is string => Boolean(id));
  const stakeholder_id =
    stakeholders.find((s) => s.name === candidate.stakeholder_name)?.id ?? "";
  return {
    ...candidate,
    accepted: candidate.validation_errors.length === 0 && initiative_ids.length > 0,
    stakeholder_id,
    initiative_ids,
    new_stakeholder_name: candidate.stakeholder_name && !stakeholder_id ? candidate.stakeholder_name : "",
  };
}

export function IngestPanel({ onClose }: { onClose: () => void }) {
  const [pastedText, setPastedText] = useState("");
  const [conversation, setConversation] = useState<ExtractedConversation>(emptyIngestConversation);
  const [candidates, setCandidates] = useState<IngestCandidateDraft[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);
  const [isExtracting, setIsExtracting] = useState(false);
  const [isCommitting, setIsCommitting] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      try {
        const [nextInitiatives, nextStakeholders] = await Promise.all([listInitiatives(), listStakeholders()]);
        setInitiatives(nextInitiatives);
        setStakeholders(nextStakeholders);
      } catch (caught) {
        setError(String(caught));
      } finally {
        setIsLoading(false);
      }
    })();
  }, []);

  const acceptedCount = candidates.filter((c) => c.accepted).length;
  const canCommit =
    conversation.title.trim() &&
    conversation.summary.trim() &&
    candidates.some(
      (c) => c.accepted && c.title.trim() && c.claim.trim() && c.initiative_ids.length > 0,
    );

  async function handleExtract() {
    try {
      setError(null);
      setMessage(null);
      setIsExtracting(true);
      const result = await extractConversation({
        chat_url: null,
        pasted_text: pastedText.trim() || null,
      });
      setConversation(result.conversation);
      setCandidates(result.candidates.map((c) => hydrateIngestCandidate(c, initiatives, stakeholders)));
      setMessage(`Extracted ${result.candidates.length} candidate deliverable(s).`);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsExtracting(false);
    }
  }

  async function handleCommit() {
    try {
      setError(null);
      setMessage(null);
      setIsCommitting(true);
      const input: CommitConversationIngestInput = {
        chat_url: null,
        conversation: {
          title: conversation.title,
          summary: conversation.summary,
          occurred_at: conversation.occurred_at?.trim() || null,
        },
        deliverables: candidates.map((c) => ({
          accepted: c.accepted,
          title: c.title,
          type: c.type,
          claim: c.claim,
          artifact_url: c.artifact_url?.trim() || null,
          stakeholder_id: c.stakeholder_id || null,
          stakeholder_ids: c.stakeholder_id ? [c.stakeholder_id] : [],
          initiative_ids: c.initiative_ids,
        })),
      };
      await commitConversationIngest(input);
      setCandidates([]);
      setConversation(emptyIngestConversation);
      setPastedText("");
      onClose();
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsCommitting(false);
    }
  }

  async function handleCreateStakeholder(index: number) {
    const candidate = candidates[index];
    const name = candidate?.new_stakeholder_name.trim();
    if (!name) return;
    try {
      setError(null);
      const created = await createStakeholder({ name });
      setStakeholders((current) => [...current, created]);
      updateCandidate(index, { stakeholder_id: created.id, stakeholder_name: created.name, new_stakeholder_name: "" });
    } catch (caught) {
      setError(String(caught));
    }
  }

  function updateCandidate(index: number, patch: Partial<IngestCandidateDraft>) {
    setCandidates((current) =>
      current.map((c, i) => (i === index ? { ...c, ...patch } : c)),
    );
  }

  function toggleInitiative(index: number, initiativeId: string) {
    setCandidates((current) =>
      current.map((c, i) => {
        if (i !== index) return c;
        const initiative_ids = c.initiative_ids.includes(initiativeId)
          ? c.initiative_ids.filter((id) => id !== initiativeId)
          : [...c.initiative_ids, initiativeId];
        const validation_errors =
          initiative_ids.length > 0
            ? c.validation_errors.filter(
                (e) => !e.startsWith("Initiative") && !e.startsWith("Select at least"),
              )
            : c.validation_errors;
        return { ...c, initiative_ids, validation_errors };
      }),
    );
  }

  return (
    <div className="space-y-4">
      {error ? <div className="notice notice-error">{error}</div> : null}
      {message ? <div className="notice notice-success">{message}</div> : null}

      <PanelSurface>
        <PanelRow>
          <label className="block space-y-1.5">
            <span className="field-label">Claude chat/export text</span>
            <textarea
              className="field-control min-h-40"
              onChange={(e) => setPastedText(e.currentTarget.value)}
              value={pastedText}
            />
          </label>
          <div className="mt-3 flex items-center gap-2 border-t border-stone-200 pt-3">
            <button
              className="btn btn-primary"
              disabled={isExtracting || isLoading}
              onClick={() => void handleExtract()}
              type="button"
            >
              <Sparkles aria-hidden="true" size={14} />
              {isExtracting ? "Extracting…" : "Extract"}
            </button>
            <span className="text-[11px] text-stone-500">Review before commit</span>
          </div>
        </PanelRow>
      </PanelSurface>

      {candidates.length > 0 ? (
        <>
          <PanelSurface>
            <PanelRow>
              <h3 className="mb-3 text-xs font-semibold text-stone-900">Conversation</h3>
              <div className="space-y-2">
                <label className="block space-y-1">
                  <span className="field-label">Title</span>
                  <input
                    className="field-control"
                    onChange={(e) => setConversation((c) => ({ ...c, title: e.currentTarget.value }))}
                    value={conversation.title}
                  />
                </label>
                <label className="block space-y-1">
                  <span className="field-label">Summary</span>
                  <textarea
                    className="field-control min-h-20"
                    onChange={(e) => setConversation((c) => ({ ...c, summary: e.currentTarget.value }))}
                    value={conversation.summary}
                  />
                </label>
                <label className="block space-y-1">
                  <span className="field-label">Occurred at</span>
                  <input
                    className="field-control"
                    onChange={(e) => setConversation((c) => ({ ...c, occurred_at: e.currentTarget.value || null }))}
                    value={conversation.occurred_at ?? ""}
                  />
                </label>
              </div>
            </PanelRow>
          </PanelSurface>

          <div className="flex items-center justify-between">
            <p className="text-xs font-semibold text-stone-700">{candidates.length} candidate(s)</p>
            <button
              className="btn btn-primary"
              disabled={!canCommit || isCommitting}
              onClick={() => void handleCommit()}
              type="button"
            >
              <CheckCircle2 aria-hidden="true" size={14} />
              {isCommitting ? "Committing…" : `Commit ${acceptedCount}`}
            </button>
          </div>

          <div className="space-y-3">
            {candidates.map((candidate, index) => (
              <IngestCandidateCard
                candidate={candidate}
                index={index}
                initiatives={initiatives}
                key={index}
                onCreateStakeholder={handleCreateStakeholder}
                onToggleInitiative={toggleInitiative}
                onUpdate={updateCandidate}
                stakeholders={stakeholders}
              />
            ))}
          </div>
        </>
      ) : null}
    </div>
  );
}

interface IngestCandidateCardProps {
  candidate: IngestCandidateDraft;
  index: number;
  initiatives: Initiative[];
  stakeholders: Stakeholder[];
  onUpdate: (index: number, patch: Partial<IngestCandidateDraft>) => void;
  onToggleInitiative: (index: number, initiativeId: string) => void;
  onCreateStakeholder: (index: number) => Promise<void>;
}

function IngestCandidateCard({
  candidate,
  index,
  initiatives,
  stakeholders,
  onUpdate,
  onToggleInitiative,
  onCreateStakeholder,
}: IngestCandidateCardProps) {
  return (
    <div className={["panel p-3 space-y-3", candidate.accepted ? "" : "opacity-60"].join(" ")}>
      <div className="flex items-center justify-between gap-2">
        <label className="inline-flex items-center gap-1.5 text-xs font-medium text-stone-900">
          <input
            checked={candidate.accepted}
            onChange={(e) => onUpdate(index, { accepted: e.currentTarget.checked })}
            type="checkbox"
          />
          Accept
        </label>
        <button className="btn" onClick={() => onUpdate(index, { accepted: false })} type="button">
          <X aria-hidden="true" size={13} />
          Reject
        </button>
      </div>
      {candidate.validation_errors.length > 0 ? (
        <div className="notice notice-warning">
          {candidate.validation_errors.map((e) => <p key={e}>{e}</p>)}
        </div>
      ) : null}
      <label className="block space-y-1">
        <span className="field-label">Title</span>
        <input
          className="field-control"
          onChange={(e) => onUpdate(index, { title: e.currentTarget.value })}
          value={candidate.title}
        />
      </label>
      <label className="block space-y-1">
        <span className="field-label">Type</span>
        <select
          className="field-control"
          onChange={(e) => onUpdate(index, { type: e.currentTarget.value as DeliverableType })}
          value={candidate.type}
        >
          {deliverableTypeOptions.map((opt) => (
            <option key={opt} value={opt}>{deliverableTypeLabels[opt]}</option>
          ))}
        </select>
      </label>
      <label className="block space-y-1">
        <span className="field-label">Stakeholder</span>
        <select
          className="field-control"
          onChange={(e) => {
            const stakeholder_id = e.currentTarget.value;
            onUpdate(index, {
              stakeholder_id,
              validation_errors: stakeholder_id
                ? candidate.validation_errors.filter((err) => !err.startsWith("Stakeholder"))
                : candidate.validation_errors,
            });
          }}
          value={candidate.stakeholder_id}
        >
          <option value="">None</option>
          {stakeholders.map((s) => <option key={s.id} value={s.id}>{s.name}</option>)}
        </select>
      </label>
      <label className="block space-y-1">
        <span className="field-label">Claim</span>
        <textarea
          className="field-control min-h-16"
          onChange={(e) => onUpdate(index, { claim: e.currentTarget.value })}
          value={candidate.claim}
        />
      </label>
      <label className="block space-y-1">
        <span className="field-label">Artifact URL</span>
        <input
          className="field-control"
          onChange={(e) => onUpdate(index, { artifact_url: e.currentTarget.value })}
          value={candidate.artifact_url ?? ""}
        />
      </label>
      <div className="flex gap-2">
        <input
          className="field-control"
          onChange={(e) => onUpdate(index, { new_stakeholder_name: e.currentTarget.value })}
          placeholder="Add stakeholder"
          value={candidate.new_stakeholder_name}
        />
        <button
          className="btn shrink-0"
          disabled={!candidate.new_stakeholder_name.trim()}
          onClick={() => void onCreateStakeholder(index)}
          type="button"
        >
          <Plus aria-hidden="true" size={14} />
          Add
        </button>
      </div>
      <fieldset className="space-y-1.5">
        <legend className="field-label">Initiatives</legend>
        <div className="grid gap-1.5 grid-cols-1">
          {initiatives.map((initiative) => (
            <label className="choice-row" key={initiative.id}>
              <input
                checked={candidate.initiative_ids.includes(initiative.id)}
                onChange={() => onToggleInitiative(index, initiative.id)}
                type="checkbox"
              />
              <span className="text-xs">{initiative.title}</span>
            </label>
          ))}
        </div>
      </fieldset>
    </div>
  );
}
