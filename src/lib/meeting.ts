// Helpers for Meeting wire types. `Meeting.key_decisions` is stored and
// transmitted as a JSON-encoded `string[]` (the column type is TEXT). Parse it
// through `parseKeyDecisions` at the consumer edge.

export function parseKeyDecisions(raw: string | null | undefined): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((s): s is string => typeof s === "string") : [];
  } catch {
    return [];
  }
}
