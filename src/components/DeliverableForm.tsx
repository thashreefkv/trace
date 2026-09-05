import { FormEvent, useEffect, useState } from "react";
import { Check, X } from "lucide-react";
import type {
  CreateDeliverableInput,
  DeliverableState,
  DeliverableType,
  Initiative,
  Stakeholder,
} from "../lib/types";
import {
  deliverableStateLabels,
  deliverableStateOptions,
  deliverableTypeLabels,
  deliverableTypeOptions,
} from "../lib/types";
import { TokenPicker } from "./TokenPicker";

interface DeliverableFormProps {
  initialValue?: CreateDeliverableInput;
  initiatives: Initiative[];
  stakeholders: Stakeholder[];
  isSubmitting: boolean;
  submitLabel: string;
  onSubmit: (input: CreateDeliverableInput) => Promise<void>;
  onCancel?: () => void;
  onCreateStakeholder: (name: string) => Promise<Stakeholder>;
  onCreateInitiative?: (title: string) => Promise<Initiative>;
}

const defaultValue: CreateDeliverableInput = {
  title: "",
  type: "analysis",
  state: "drafting",
  claim: "",
  artifact_url: null,
  conversation_id: null,
  stakeholder_id: null,
  stakeholder_ids: [],
  initiative_ids: [],
};

export function DeliverableForm({
  initialValue = defaultValue,
  initiatives,
  stakeholders,
  isSubmitting,
  submitLabel,
  onSubmit,
  onCancel,
  onCreateStakeholder,
  onCreateInitiative,
}: DeliverableFormProps) {
  const [title, setTitle] = useState(initialValue.title);
  const [type, setType] = useState<DeliverableType>(initialValue.type);
  const [state, setState] = useState<DeliverableState>(initialValue.state);
  const [claim, setClaim] = useState(initialValue.claim);
  const [artifactUrl, setArtifactUrl] = useState(initialValue.artifact_url ?? "");
  const [stakeholderIds, setStakeholderIds] = useState<string[]>(
    initialValue.stakeholder_ids?.length
      ? initialValue.stakeholder_ids
      : initialValue.stakeholder_id
        ? [initialValue.stakeholder_id]
        : [],
  );
  const [initiativeIds, setInitiativeIds] = useState<string[]>(initialValue.initiative_ids);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setTitle(initialValue.title);
    setType(initialValue.type);
    setState(initialValue.state);
    setClaim(initialValue.claim);
    setArtifactUrl(initialValue.artifact_url ?? "");
    setStakeholderIds(
      initialValue.stakeholder_ids?.length
        ? initialValue.stakeholder_ids
        : initialValue.stakeholder_id
          ? [initialValue.stakeholder_id]
          : [],
    );
    setInitiativeIds(initialValue.initiative_ids);
    setError(null);
  }, [initialValue]);

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    await onSubmit({
      title,
      type,
      state,
      claim,
      artifact_url: artifactUrl.trim() ? artifactUrl : null,
      conversation_id: initialValue.conversation_id ?? null,
      stakeholder_id: stakeholderIds[0] ?? null,
      stakeholder_ids: stakeholderIds,
      initiative_ids: initiativeIds,
    });
  }

  function toggleStakeholder(id: string) {
    setStakeholderIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }

  function toggleInitiative(id: string) {
    setInitiativeIds((prev) =>
      prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id],
    );
  }

  async function handleCreateStakeholder(name: string) {
    const created = await onCreateStakeholder(name);
    setStakeholderIds((prev) => [...prev, created.id]);
  }

  async function handleCreateInitiative(title: string) {
    if (!onCreateInitiative) return;
    const created = await onCreateInitiative(title);
    setInitiativeIds((prev) => [...prev, created.id]);
  }

  return (
    <form className="space-y-4" onSubmit={handleSubmit}>
      {error ? (
        <div className="rounded-xl border border-red-100 bg-red-50 px-3 py-2 text-xs text-red-600">
          {error}
        </div>
      ) : null}

      <label className="block space-y-1.5">
        <span className="field-label">Title</span>
        <input
          className="field-control"
          maxLength={160}
          onChange={(e) => setTitle(e.currentTarget.value)}
          placeholder="Knowledge Graph schema v2"
          required
          value={title}
        />
      </label>

      <div className="grid gap-3 sm:grid-cols-2">
        <label className="block space-y-1.5">
          <span className="field-label">Type</span>
          <select
            className="field-control"
            onChange={(e) => setType(e.currentTarget.value as DeliverableType)}
            value={type}
          >
            {deliverableTypeOptions.map((option) => (
              <option key={option} value={option}>
                {deliverableTypeLabels[option]}
              </option>
            ))}
          </select>
        </label>

        <label className="block space-y-1.5">
          <span className="field-label">State</span>
          <select
            className="field-control"
            onChange={(e) => setState(e.currentTarget.value as DeliverableState)}
            value={state}
          >
            {deliverableStateOptions.map((option) => (
              <option key={option} value={option}>
                {deliverableStateLabels[option]}
              </option>
            ))}
          </select>
        </label>
      </div>

      <label className="block space-y-1.5">
        <span className="field-label">Claim</span>
        <textarea
          className="field-control min-h-20"
          onChange={(e) => setClaim(e.currentTarget.value)}
          placeholder="What does this argue?"
          value={claim}
        />
      </label>

      <label className="block space-y-1.5">
        <span className="field-label">Artifact URL</span>
        <input
          className="field-control"
          onChange={(e) => setArtifactUrl(e.currentTarget.value)}
          placeholder="https://…"
          value={artifactUrl}
        />
      </label>

      <div className="space-y-1.5">
        <span className="field-label">Stakeholders</span>
        <TokenPicker
          getId={(s) => s.id}
          getLabel={(s) => s.name}
          getSubLabel={(s) => s.role}
          items={stakeholders}
          onCreate={handleCreateStakeholder}
          onToggle={toggleStakeholder}
          placeholder="Search stakeholders…"
          selectedIds={stakeholderIds}
        />
      </div>

      <div className="space-y-1.5">
        <span className="field-label">Initiatives</span>
        <TokenPicker
          getId={(i) => i.id}
          getLabel={(i) => i.title}
          items={initiatives}
          onCreate={onCreateInitiative ? handleCreateInitiative : undefined}
          onToggle={toggleInitiative}
          placeholder="Search initiatives…"
          selectedIds={initiativeIds}
        />
      </div>

      <div className="flex flex-wrap items-center gap-2 pt-1">
        <button className="btn btn-primary" disabled={isSubmitting} type="submit">
          <Check aria-hidden="true" size={16} />
          {submitLabel}
        </button>
        {onCancel ? (
          <button className="btn" disabled={isSubmitting} onClick={onCancel} type="button">
            <X aria-hidden="true" size={16} />
            Cancel
          </button>
        ) : null}
      </div>
    </form>
  );
}
