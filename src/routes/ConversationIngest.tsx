import { useEffect, useMemo, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Check, Plus, RefreshCw, Sparkles, X } from "lucide-react";
import {
  commitConversationIngest,
  createStakeholder,
  extractConversation,
  listInitiatives,
  listStakeholders,
  promoteClaudeCaptureToIngest,
} from "../lib/ipc";
import type {
  CommitConversationIngestInput,
  DeliverableType,
  ExtractedDeliverableCandidate,
  ExtractedConversation,
  Initiative,
  Stakeholder,
} from "../lib/types";
import { deliverableTypeLabels, deliverableTypeOptions } from "../lib/types";

interface CandidateDraft extends ExtractedDeliverableCandidate {
  accepted: boolean;
  stakeholder_id: string;
  initiative_ids: string[];
  new_stakeholder_name: string;
}

const emptyConversation: ExtractedConversation = {
  title: "",
  summary: "",
  occurred_at: null,
};

export function ConversationIngest() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const captureId = searchParams.get("captureId");
  const [pastedText, setPastedText] = useState("");
  const [conversation, setConversation] = useState<ExtractedConversation>(emptyConversation);
  const [candidates, setCandidates] = useState<CandidateDraft[]>([]);
  const [initiatives, setInitiatives] = useState<Initiative[]>([]);
  const [stakeholders, setStakeholders] = useState<Stakeholder[]>([]);
  const [isExtracting, setIsExtracting] = useState(false);
  const [isCommitting, setIsCommitting] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadLookups();
  }, []);

  const acceptedCount = candidates.filter((candidate) => candidate.accepted).length;
  const canCommit = useMemo(() => {
    if (!conversation.title.trim() || !conversation.summary.trim()) {
      return false;
    }

    return candidates.some(
      (candidate) =>
        candidate.accepted &&
        candidate.title.trim() &&
        candidate.claim.trim() &&
        candidate.initiative_ids.length > 0,
    );
  }, [candidates, conversation]);

  async function loadLookups() {
    try {
      setError(null);
      setIsLoading(true);
      const [nextInitiatives, nextStakeholders] = await Promise.all([
        listInitiatives(),
        listStakeholders(),
      ]);
      setInitiatives(nextInitiatives);
      setStakeholders(nextStakeholders);
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsLoading(false);
    }
  }

  async function handleExtract() {
    try {
      setError(null);
      setMessage(null);
      setIsExtracting(true);
      const result = await extractConversation({
        chat_url: null,
        pasted_text: pastedText.trim() ? pastedText : null,
      });
      setConversation(result.conversation);
      setCandidates(
        result.candidates.map((candidate) => hydrateCandidate(candidate, initiatives, stakeholders)),
      );
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
        deliverables: candidates.map((candidate) => ({
          accepted: candidate.accepted,
          title: candidate.title,
          type: candidate.type,
          claim: candidate.claim,
          artifact_url: candidate.artifact_url?.trim() || null,
          stakeholder_id: candidate.stakeholder_id || null,
          stakeholder_ids: candidate.stakeholder_id ? [candidate.stakeholder_id] : [],
          initiative_ids: candidate.initiative_ids,
        })),
      };

      const result = captureId
        ? await promoteClaudeCaptureToIngest(captureId, input)
        : await commitConversationIngest(input);
      const firstDeliverable = result.deliverables[0];
      navigate(firstDeliverable ? `/deliverables/${firstDeliverable.id}` : "/deliverables");
    } catch (caught) {
      setError(String(caught));
    } finally {
      setIsCommitting(false);
    }
  }

  async function handleCreateStakeholder(index: number) {
    const candidate = candidates[index];
    const name = candidate?.new_stakeholder_name.trim();
    if (!name) {
      return;
    }

    try {
      setError(null);
      const created = await createStakeholder({ name });
      setStakeholders((current) => [...current, created]);
      updateCandidate(index, {
        stakeholder_id: created.id,
        stakeholder_name: created.name,
        new_stakeholder_name: "",
      });
    } catch (caught) {
      setError(String(caught));
    }
  }

  function updateCandidate(index: number, patch: Partial<CandidateDraft>) {
    setCandidates((current) =>
      current.map((candidate, candidateIndex) =>
        candidateIndex === index ? { ...candidate, ...patch } : candidate,
      ),
    );
  }

  function toggleInitiative(index: number, initiativeId: string) {
    setCandidates((current) =>
      current.map((candidate, candidateIndex) => {
        if (candidateIndex !== index) {
          return candidate;
        }

        const initiative_ids = candidate.initiative_ids.includes(initiativeId)
          ? candidate.initiative_ids.filter((id) => id !== initiativeId)
          : [...candidate.initiative_ids, initiativeId];
        const validation_errors =
          initiative_ids.length > 0
            ? candidate.validation_errors.filter(
                (validationError) =>
                  !validationError.startsWith("Initiative") &&
                  !validationError.startsWith("Select at least"),
              )
            : candidate.validation_errors;
        return { ...candidate, initiative_ids, validation_errors };
      }),
    );
  }

  return (
    <div className="mx-auto grid min-h-full max-w-7xl gap-6 px-5 py-6 xl:grid-cols-[minmax(0,460px)_minmax(0,1fr)]">
      <section className="space-y-5">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="page-kicker">Conversation ingest</p>
            <h1 className="page-title">Backfill Claude work</h1>
          </div>
          <button className="btn" onClick={() => void loadLookups()} type="button">
            <RefreshCw aria-hidden="true" size={16} />
            Refresh
          </button>
        </div>

        {error ? <div className="notice notice-error">{error}</div> : null}
        {message ? <div className="notice notice-success">{message}</div> : null}

        <div className="panel p-4">
          <label className="block space-y-1.5">
            <span className="field-label">Claude chat/export text</span>
            <textarea
              className="field-control min-h-72"
              onChange={(event) => setPastedText(event.currentTarget.value)}
              value={pastedText}
            />
          </label>

          <div className="mt-4 flex flex-wrap items-center gap-2 border-t border-stone-200 pt-4">
            <button
              className="btn btn-primary"
              disabled={isExtracting || isLoading}
              onClick={() => void handleExtract()}
              type="button"
            >
              <Sparkles aria-hidden="true" size={16} />
              {isExtracting ? "Extracting" : "Extract"}
            </button>
            <span className="text-xs text-stone-500">
              {captureId ? "From capture inbox" : "Review before commit"}
            </span>
          </div>
        </div>

        <div className="panel p-4">
          <h2 className="mb-3 text-sm font-semibold text-stone-950">Conversation</h2>
          <div className="space-y-3">
            <label className="block space-y-1.5">
              <span className="field-label">Title</span>
              <input
                className="field-control"
                onChange={(event) =>
                  setConversation((current) => ({ ...current, title: event.currentTarget.value }))
                }
                value={conversation.title}
              />
            </label>
            <label className="block space-y-1.5">
              <span className="field-label">Summary</span>
              <textarea
                className="field-control min-h-24"
                onChange={(event) =>
                  setConversation((current) => ({
                    ...current,
                    summary: event.currentTarget.value,
                  }))
                }
                value={conversation.summary}
              />
            </label>
            <label className="block space-y-1.5">
              <span className="field-label">Occurred at</span>
              <input
                className="field-control"
                onChange={(event) =>
                  setConversation((current) => ({
                    ...current,
                    occurred_at: event.currentTarget.value || null,
                  }))
                }
                value={conversation.occurred_at ?? ""}
              />
            </label>
          </div>
        </div>
      </section>

      <section className="min-w-0 space-y-4">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <p className="page-kicker">Review queue</p>
            <h2 className="page-title text-xl">Candidates</h2>
          </div>
          <button
            className="btn btn-primary"
            disabled={!canCommit || isCommitting}
            onClick={() => void handleCommit()}
            type="button"
          >
            <Check aria-hidden="true" size={16} />
            Commit {acceptedCount}
          </button>
        </div>

        {candidates.length === 0 ? (
          <div className="empty-state">
            <p>No candidates extracted.</p>
          </div>
        ) : (
          <div className="grid gap-3">
            {candidates.map((candidate, index) => (
              <CandidateEditor
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
        )}
      </section>
    </div>
  );
}

interface CandidateEditorProps {
  candidate: CandidateDraft;
  index: number;
  initiatives: Initiative[];
  stakeholders: Stakeholder[];
  onUpdate: (index: number, patch: Partial<CandidateDraft>) => void;
  onToggleInitiative: (index: number, initiativeId: string) => void;
  onCreateStakeholder: (index: number) => Promise<void>;
}

function CandidateEditor({
  candidate,
  index,
  initiatives,
  stakeholders,
  onUpdate,
  onToggleInitiative,
  onCreateStakeholder,
}: CandidateEditorProps) {
  return (
    <article className={["panel p-4", candidate.accepted ? "" : "opacity-70"].join(" ")}>
      <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
        <label className="inline-flex items-center gap-2 text-sm font-medium text-stone-900">
          <input
            checked={candidate.accepted}
            onChange={(event) => onUpdate(index, { accepted: event.currentTarget.checked })}
            type="checkbox"
          />
          Accept
        </label>
        <button
          className="btn"
          onClick={() => onUpdate(index, { accepted: false })}
          type="button"
        >
          <X aria-hidden="true" size={16} />
          Reject
        </button>
      </div>

      {candidate.validation_errors.length > 0 ? (
        <div className="notice notice-warning mb-4">
          {candidate.validation_errors.map((validationError) => (
            <p key={validationError}>{validationError}</p>
          ))}
        </div>
      ) : null}

      <div className="grid gap-3 md:grid-cols-2">
        <label className="block space-y-1.5 md:col-span-2">
          <span className="field-label">Title</span>
          <input
            className="field-control"
            onChange={(event) => onUpdate(index, { title: event.currentTarget.value })}
            value={candidate.title}
          />
        </label>

        <label className="block space-y-1.5">
          <span className="field-label">Type</span>
          <select
            className="field-control"
            onChange={(event) =>
              onUpdate(index, { type: event.currentTarget.value as DeliverableType })
            }
            value={candidate.type}
          >
            {deliverableTypeOptions.map((option) => (
              <option key={option} value={option}>
                {deliverableTypeLabels[option]}
              </option>
            ))}
          </select>
        </label>

        <label className="block space-y-1.5">
          <span className="field-label">Stakeholder</span>
          <select
            className="field-control"
            onChange={(event) => {
              const stakeholder_id = event.currentTarget.value;
              onUpdate(index, {
                stakeholder_id,
                validation_errors: stakeholder_id
                  ? candidate.validation_errors.filter(
                      (validationError) => !validationError.startsWith("Stakeholder"),
                    )
                  : candidate.validation_errors,
              });
            }}
            value={candidate.stakeholder_id}
          >
            <option value="">None</option>
            {stakeholders.map((stakeholder) => (
              <option key={stakeholder.id} value={stakeholder.id}>
                {stakeholder.name}
              </option>
            ))}
          </select>
        </label>

        <label className="block space-y-1.5 md:col-span-2">
          <span className="field-label">Claim</span>
          <textarea
            className="field-control min-h-24"
            onChange={(event) => onUpdate(index, { claim: event.currentTarget.value })}
            value={candidate.claim}
          />
        </label>

        <label className="block space-y-1.5 md:col-span-2">
          <span className="field-label">Artifact URL</span>
          <input
            className="field-control"
            onChange={(event) => onUpdate(index, { artifact_url: event.currentTarget.value })}
            value={candidate.artifact_url ?? ""}
          />
        </label>
      </div>

      <div className="mt-3 flex gap-2">
        <input
          className="field-control"
          onChange={(event) => onUpdate(index, { new_stakeholder_name: event.currentTarget.value })}
          placeholder="Add stakeholder"
          value={candidate.new_stakeholder_name}
        />
        <button
          className="btn"
          disabled={!candidate.new_stakeholder_name.trim()}
          onClick={() => void onCreateStakeholder(index)}
          type="button"
        >
          <Plus aria-hidden="true" size={16} />
          Add
        </button>
      </div>

      <fieldset className="mt-4 space-y-2">
        <legend className="field-label">Initiatives</legend>
        <div className="grid gap-2 sm:grid-cols-2">
          {initiatives.map((initiative) => (
            <label className="choice-row" key={initiative.id}>
              <input
                checked={candidate.initiative_ids.includes(initiative.id)}
                onChange={() => onToggleInitiative(index, initiative.id)}
                type="checkbox"
              />
              <span>{initiative.title}</span>
            </label>
          ))}
        </div>
      </fieldset>
    </article>
  );
}

function hydrateCandidate(
  candidate: ExtractedDeliverableCandidate,
  initiatives: Initiative[],
  stakeholders: Stakeholder[],
): CandidateDraft {
  const initiative_ids = candidate.initiative_titles
    .map((title) => initiatives.find((initiative) => initiative.title === title)?.id)
    .filter((id): id is string => Boolean(id));
  const stakeholder_id =
    stakeholders.find((stakeholder) => stakeholder.name === candidate.stakeholder_name)?.id ?? "";

  return {
    ...candidate,
    accepted: candidate.validation_errors.length === 0 && initiative_ids.length > 0,
    stakeholder_id,
    initiative_ids,
    new_stakeholder_name: candidate.stakeholder_name && !stakeholder_id ? candidate.stakeholder_name : "",
  };
}
