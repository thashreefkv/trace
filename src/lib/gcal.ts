import type {
  GCalAttachment,
  GCalAttendee,
  GCalConferenceData,
  GCalEvent,
  GCalOrganizer,
  GCalReminders,
} from "./types";

// `GCalEvent` carries several columns that are TEXT in SQLite and serialized as
// JSON strings on the wire. `parseGCalEvent` decodes them into typed values so
// callers don't have to scatter `JSON.parse` over the codebase.

export interface ParsedGCalEvent {
  attendees: GCalAttendee[];
  conference_data: GCalConferenceData | null;
  organizer: GCalOrganizer | null;
  recurrence: string[];
  attachments: GCalAttachment[];
  reminders: GCalReminders | null;
}

function parseJson<T>(raw: string | null | undefined): T | null {
  if (!raw) return null;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return null;
  }
}

export function parseGCalEvent(event: GCalEvent): ParsedGCalEvent {
  return {
    attendees: parseJson<GCalAttendee[]>(event.attendees) ?? [],
    conference_data: parseJson<GCalConferenceData>(event.conference_data),
    organizer: parseJson<GCalOrganizer>(event.organizer),
    recurrence: parseJson<string[]>(event.recurrence) ?? [],
    attachments: parseJson<GCalAttachment[]>(event.attachments) ?? [],
    reminders: parseJson<GCalReminders>(event.reminders),
  };
}

export function parseGCalConferenceData(event: GCalEvent): GCalConferenceData | null {
  return parseJson<GCalConferenceData>(event.conference_data);
}
