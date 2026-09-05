import type { WorkGraphNode } from "../types";
import { labelForKind } from "./kinds";

export interface InspectorField {
  label: string;
  value: string;
}

function fmtProperty(value: unknown): string | null {
  if (value == null) return null;
  if (typeof value === "string") return value.length > 200 ? value.slice(0, 200) + "…" : value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  if (Array.isArray(value)) {
    if (value.length === 0) return null;
    return value.map((item) => (typeof item === "string" ? item : JSON.stringify(item))).join(", ");
  }
  try {
    const json = JSON.stringify(value);
    return json.length > 200 ? json.slice(0, 200) + "…" : json;
  } catch {
    return String(value);
  }
}

const KIND_FIELDS: Record<string, (node: WorkGraphNode) => Array<InspectorField | null>> = {
  initiative: (node) => [
    field("Status", node.status),
    field("Framing", node.properties?.framing),
    field("Updated", node.updated_at),
  ],
  deliverable: (node) => [
    field("State", node.status),
    field("Type", node.properties?.type),
    field("Owner", node.properties?.stakeholder_id ?? node.properties?.owner),
    field("Initiative", node.properties?.initiative_id ?? node.properties?.initiative),
    field("Due", node.properties?.due_date),
    field("Updated", node.updated_at),
  ],
  stakeholder: (node) => [
    field("Org", node.properties?.org ?? node.properties?.organisation),
    field("Role", node.properties?.role),
    field("Email", node.properties?.email),
    field("Last meeting", node.properties?.last_meeting_at),
  ],
  memory: (node) => [
    field("Kind", node.properties?.memory_kind ?? node.properties?.kind),
    field("Scope", node.properties?.scope),
    field("Confidence", node.properties?.confidence),
    field("Importance", node.properties?.importance),
    field("Pinned", node.properties?.pinned),
    field("Updated", node.updated_at),
  ],
  meeting: (node) => [
    field("Date", node.properties?.date ?? node.updated_at),
    field("Duration", node.properties?.duration),
    field("Attendees", node.properties?.attendees ?? node.properties?.attendee_count),
  ],
  email_thread: (node) => [
    field("From", node.properties?.from ?? node.properties?.sender),
    field("Participants", node.properties?.participants ?? node.properties?.participant_count),
    field("Last message", node.properties?.last_message_at ?? node.updated_at),
    field("Sentiment", node.properties?.sentiment),
    field("Urgency", node.properties?.urgency),
  ],
  capture: (node) => [
    field("Source", node.properties?.kind),
    field("Status", node.status),
    field("Created", node.properties?.created_at ?? node.updated_at),
  ],
  task: (node) => [
    field("State", node.status),
    field("Deliverable", node.properties?.deliverable_id),
    field("Due", node.properties?.due_date),
  ],
  blocker: (node) => [
    field("Source", node.properties?.source),
    field("Severity", node.properties?.severity),
    field("Updated", node.updated_at),
  ],
  file: (node) => [
    field("Mime", node.properties?.mime_type),
    field("Source", node.properties?.kind),
    field("Updated", node.updated_at),
  ],
};

function field(label: string, value: unknown): InspectorField | null {
  const v = fmtProperty(value);
  if (!v) return null;
  return { label, value: v };
}

export function fieldsForNode(node: WorkGraphNode): InspectorField[] {
  const handler = KIND_FIELDS[node.kind];
  const items: Array<InspectorField | null> = handler ? handler(node) : fallbackFields(node);
  return items.filter((item): item is InspectorField => item != null);
}

function fallbackFields(node: WorkGraphNode): Array<InspectorField | null> {
  const out: Array<InspectorField | null> = [];
  if (node.status) out.push(field("Status", node.status));
  if (node.updated_at) out.push(field("Updated", node.updated_at));
  const props = node.properties ?? {};
  let count = 0;
  for (const [key, value] of Object.entries(props)) {
    if (count >= 6) break;
    const formatted = fmtProperty(value);
    if (!formatted) continue;
    out.push({ label: key.replace(/_/g, " "), value: formatted });
    count += 1;
  }
  return out;
}

export function deepLinkForNode(node: WorkGraphNode): string | null {
  if (node.url) return node.url;
  switch (node.kind) {
    case "initiative":
      return `/initiatives/${node.entity_id}`;
    case "deliverable":
      return `/deliverables/${node.entity_id}`;
    case "stakeholder":
      return `/stakeholders/${node.entity_id}`;
    case "meeting":
      return `/meetings/${node.entity_id}`;
    case "email_thread":
      return `/email?thread=${encodeURIComponent(node.entity_id)}`;
    case "capture":
      return "/captures";
    case "memory":
      return "/settings/brain";
    default:
      return null;
  }
}

export function kindHeadingLabel(node: WorkGraphNode): string {
  return labelForKind(node.kind);
}
