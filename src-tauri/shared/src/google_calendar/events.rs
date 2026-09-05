//! Google Calendar event CRUD + sync + cache reads. From legacy.rs (13-std3).

use std::path::Path;

use chrono::{Duration, NaiveDate, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use ulid::Ulid;

use crate::db::sql_error;
use crate::models::GCalEvent;
use super::{get_valid_access_token, next_date_str, GCAL_API};

#[derive(Debug, Deserialize)]
struct GCalApiDate {
    date: Option<String>,
    #[serde(rename = "dateTime")]
    date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GCalApiAttendee {
    email: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "self", default)]
    is_self: bool,
    #[serde(rename = "responseStatus")]
    response_status: Option<String>,
    #[serde(default)]
    organizer: bool,
}

#[derive(Debug, Deserialize)]
struct GCalApiConferenceEntryPoint {
    #[serde(rename = "entryPointType")]
    entry_point_type: String,
    uri: String,
    label: Option<String>,
    pin: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GCalApiConferenceSolution {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GCalApiConferenceData {
    #[serde(rename = "conferenceId")]
    conference_id: Option<String>,
    #[serde(rename = "conferenceSolution")]
    conference_solution: Option<GCalApiConferenceSolution>,
    #[serde(rename = "entryPoints")]
    entry_points: Option<Vec<GCalApiConferenceEntryPoint>>,
}

#[derive(Debug, Deserialize)]
struct GCalApiOrganizer {
    email: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "self", default)]
    is_self: bool,
}

#[derive(Debug, Deserialize)]
struct GCalApiAttachment {
    #[serde(rename = "fileUrl")]
    file_url: String,
    title: Option<String>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(rename = "fileId")]
    file_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GCalApiReminderOverride {
    method: String,
    minutes: i64,
}

#[derive(Debug, Deserialize)]
struct GCalApiReminders {
    #[serde(rename = "useDefault", default)]
    use_default: bool,
    overrides: Option<Vec<GCalApiReminderOverride>>,
}

#[derive(Debug, Deserialize)]
struct GCalApiEvent {
    id: String,
    summary: Option<String>,
    description: Option<String>,
    location: Option<String>,
    start: GCalApiDate,
    end: GCalApiDate,
    #[serde(rename = "htmlLink")]
    html_link: Option<String>,
    status: Option<String>,
    attendees: Option<Vec<GCalApiAttendee>>,
    #[serde(rename = "conferenceData")]
    conference_data: Option<GCalApiConferenceData>,
    organizer: Option<GCalApiOrganizer>,
    #[serde(rename = "recurringEventId")]
    recurring_event_id: Option<String>,
    recurrence: Option<Vec<String>>,
    #[serde(rename = "colorId")]
    color_id: Option<String>,
    transparency: Option<String>,
    updated: Option<String>,
    attachments: Option<Vec<GCalApiAttachment>>,
    reminders: Option<GCalApiReminders>,
    visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GCalEventListResp {
    items: Option<Vec<GCalApiEvent>>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

/// Fetch events from Google Calendar between two RFC 3339 timestamps.
async fn list_events(
    token: &str,
    time_min: &str,
    time_max: &str,
) -> Result<Vec<GCalApiEvent>, String> {
    let client = reqwest::Client::new();
    let mut all: Vec<GCalApiEvent> = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let mut req = client
            .get(format!("{GCAL_API}/calendars/primary/events"))
            .bearer_auth(token)
            .query(&[
                ("timeMin", time_min),
                ("timeMax", time_max),
                ("singleEvents", "true"),
                ("maxResults", "250"),
            ]);

        if let Some(pt) = &page_token {
            req = req.query(&[("pageToken", pt.as_str())]);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("gcal list events failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!(
                "gcal list events error {}: {}",
                resp.status(),
                resp.text().await.unwrap_or_default()
            ));
        }

        let body: GCalEventListResp = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse gcal events: {e}"))?;

        let items = body.items.unwrap_or_default();
        let next = body.next_page_token;
        all.extend(
            items
                .into_iter()
                .filter(|e| e.status.as_deref() != Some("cancelled")),
        );

        match next {
            Some(pt) => page_token = Some(pt),
            None => break,
        }
    }

    Ok(all)
}

#[derive(Debug, Serialize)]
struct GCalEventBody<'a> {
    summary: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    start: GCalDateValue<'a>,
    end: GCalDateValue<'a>,
    #[serde(rename = "extendedProperties", skip_serializing_if = "Option::is_none")]
    extended_properties: Option<ExtendedProps<'a>>,
}

#[derive(Debug, Serialize)]
struct GCalDateValue<'a> {
    date: &'a str,
}

#[derive(Debug, Serialize)]
struct ExtendedProps<'a> {
    private: TraceProps<'a>,
}

#[derive(Debug, Serialize)]
struct TraceProps<'a> {
    trace_entity_type: &'a str,
    trace_entity_id: &'a str,
}

/// Create a new event in GCal; returns the new event ID.
pub async fn create_event(
    token: &str,
    summary: &str,
    description: Option<&str>,
    date: &str,
    entity_type: &str,
    entity_id: &str,
) -> Result<String, String> {
    // GCal all-day events need end = date + 1 day
    let end_date = next_date_str(date)?;

    let body = GCalEventBody {
        summary,
        description,
        start: GCalDateValue { date },
        end: GCalDateValue { date: &end_date },
        extended_properties: Some(ExtendedProps {
            private: TraceProps {
                trace_entity_type: entity_type,
                trace_entity_id: entity_id,
            },
        }),
    };

    let resp = reqwest::Client::new()
        .post(format!("{GCAL_API}/calendars/primary/events"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("gcal create event failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "gcal create event error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }

    #[derive(Deserialize)]
    struct Created {
        id: String,
    }
    let created: Created = resp
        .json()
        .await
        .map_err(|e| format!("failed to parse create response: {e}"))?;
    Ok(created.id)
}

/// Update an existing GCal event's summary, description, and date.
pub async fn update_event(
    token: &str,
    gcal_event_id: &str,
    summary: &str,
    description: Option<&str>,
    date: &str,
) -> Result<(), String> {
    let end_date = next_date_str(date)?;

    let body = GCalEventBody {
        summary,
        description,
        start: GCalDateValue { date },
        end: GCalDateValue { date: &end_date },
        extended_properties: None,
    };

    let resp = reqwest::Client::new()
        .put(format!(
            "{GCAL_API}/calendars/primary/events/{gcal_event_id}"
        ))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("gcal update event failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "gcal update event error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}

/// Delete a GCal event.
pub async fn delete_event(token: &str, gcal_event_id: &str) -> Result<(), String> {
    let resp = reqwest::Client::new()
        .delete(format!(
            "{GCAL_API}/calendars/primary/events/{gcal_event_id}"
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("gcal delete event failed: {e}"))?;

    // 204 = success, 410 = already deleted — both are fine
    if !resp.status().is_success() && resp.status().as_u16() != 410 {
        return Err(format!(
            "gcal delete event error {}: {}",
            resp.status(),
            resp.text().await.unwrap_or_default()
        ));
    }
    Ok(())
}

// ── Full bidirectional sync ───────────────────────────────────────────────────

pub async fn sync_calendar(pool: &SqlitePool, dir: &Path) -> Result<(), String> {
    let token = get_valid_access_token(dir).await?;

    // Pull a 6-week window: 1 week back, 5 weeks forward
    let today = Utc::now().date_naive();
    let time_min_date = today - Duration::days(7);
    let time_max_date = today + Duration::days(35);
    let time_min = format!("{time_min_date}T00:00:00Z");
    let time_max = format!("{time_max_date}T23:59:59Z");

    // 1. Pull GCal → cache into gcal_events
    let events = list_events(&token, &time_min, &time_max).await?;
    let now_iso = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);

    // Clear stale cache for this window then re-insert
    sqlx::query("DELETE FROM gcal_events WHERE start_date >= ? AND start_date <= ?")
        .bind(time_min_date.format("%Y-%m-%d").to_string())
        .bind(time_max_date.format("%Y-%m-%d").to_string())
        .execute(pool)
        .await
        .map_err(sql_error)?;

    for ev in &events {
        let is_all_day = ev.start.date.is_some();
        let start_date = ev
            .start
            .date
            .clone()
            .or_else(|| ev.start.date_time.as_ref().map(|dt| dt[..10].to_string()))
            .unwrap_or_default();
        let end_date_val = ev.end.date.clone();
        let start_dt = ev.start.date_time.clone();
        let end_dt = ev.end.date_time.clone();

        let attendees_json = ev.attendees.as_ref().map(|list| {
            serde_json::to_string(
                &list
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "email": a.email,
                            "name": a.display_name,
                            "self": a.is_self,
                            "responseStatus": a.response_status,
                            "organizer": a.organizer,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default()
        });

        // Extract the most useful bits of conferenceData into a flat JSON object
        let conference_json = ev.conference_data.as_ref().map(|cd| {
            let video_uri = cd.entry_points.as_ref().and_then(|eps| {
                eps.iter()
                    .find(|ep| ep.entry_point_type == "video")
                    .map(|ep| ep.uri.as_str())
            });
            let phone_uri = cd.entry_points.as_ref().and_then(|eps| {
                eps.iter()
                    .find(|ep| ep.entry_point_type == "phone")
                    .map(|ep| ep.uri.as_str())
            });
            let phone_pin = cd.entry_points.as_ref().and_then(|eps| {
                eps.iter()
                    .find(|ep| ep.entry_point_type == "phone")
                    .and_then(|ep| ep.pin.as_deref())
            });
            serde_json::to_string(&serde_json::json!({
                "conference_id": cd.conference_id,
                "solution_name": cd.conference_solution.as_ref().and_then(|s| s.name.as_deref()),
                "video_uri": video_uri,
                "phone_uri": phone_uri,
                "phone_pin": phone_pin,
            }))
            .unwrap_or_default()
        });

        let organizer_json = ev.organizer.as_ref().map(|o| {
            serde_json::to_string(&serde_json::json!({
                "email": o.email,
                "name": o.display_name,
                "self": o.is_self,
            }))
            .unwrap_or_default()
        });

        let recurrence_json = ev
            .recurrence
            .as_ref()
            .map(|r| serde_json::to_string(r).unwrap_or_default());

        let attachments_json = ev.attachments.as_ref().map(|list| {
            serde_json::to_string(
                &list
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "fileUrl": a.file_url,
                            "title": a.title,
                            "mimeType": a.mime_type,
                            "fileId": a.file_id,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default()
        });

        let reminders_json = ev.reminders.as_ref().map(|r| {
            serde_json::to_string(&serde_json::json!({
                "useDefault": r.use_default,
                "overrides": r.overrides.as_ref().map(|ovs| {
                    ovs.iter().map(|o| serde_json::json!({
                        "method": o.method,
                        "minutes": o.minutes,
                    })).collect::<Vec<_>>()
                }),
            }))
            .unwrap_or_default()
        });

        let id = Ulid::new().to_string();
        sqlx::query(
            "INSERT OR REPLACE INTO gcal_events
             (id, gcal_event_id, title, description, location, attendees,
              conference_data, organizer, recurring_event_id, recurrence,
              color_id, transparency, event_updated_at, attachments, reminders, visibility,
              start_date, end_date, start_datetime, end_datetime, is_all_day, html_link, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&ev.id)
        .bind(ev.summary.as_deref().unwrap_or("(No title)"))
        .bind(&ev.description)
        .bind(&ev.location)
        .bind(&attendees_json)
        .bind(&conference_json)
        .bind(&organizer_json)
        .bind(&ev.recurring_event_id)
        .bind(&recurrence_json)
        .bind(&ev.color_id)
        .bind(&ev.transparency)
        .bind(&ev.updated)
        .bind(&attachments_json)
        .bind(&reminders_json)
        .bind(&ev.visibility)
        .bind(&start_date)
        .bind(&end_date_val)
        .bind(&start_dt)
        .bind(&end_dt)
        .bind(is_all_day)
        .bind(&ev.html_link)
        .bind(&now_iso)
        .execute(pool)
        .await
        .map_err(sql_error)?;
    }

    // 2. Push deliverables with deadlines → GCal
    #[derive(sqlx::FromRow)]
    struct DeliverableRow {
        id: String,
        title: String,
        claim: String,
        deadline: Option<String>,
        state: String,
    }

    let deliverables: Vec<DeliverableRow> = sqlx::query_as(
        "SELECT id, title, claim, deadline, state FROM deliverables WHERE deadline IS NOT NULL AND deadline != ''"
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for d in &deliverables {
        let deadline = match &d.deadline {
            Some(dl) => dl,
            None => continue,
        };

        let is_terminal = d.state == "shipped" || d.state == "killed";

        // Check if we already have a sync entry
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT gcal_event_id FROM gcal_sync_map WHERE entity_type = 'deliverable' AND entity_id = ?"
        )
        .bind(&d.id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

        match existing {
            Some((gcal_id,)) if is_terminal => {
                // Remove from GCal if shipped/killed
                let _ = delete_event(&token, &gcal_id).await;
                sqlx::query(
                    "DELETE FROM gcal_sync_map WHERE entity_type = 'deliverable' AND entity_id = ?",
                )
                .bind(&d.id)
                .execute(pool)
                .await
                .map_err(sql_error)?;
            }
            Some((gcal_id,)) => {
                // Update existing event
                let desc = Some(d.claim.as_str());
                let _ = update_event(&token, &gcal_id, &d.title, desc, deadline).await;
                sqlx::query(
                    "UPDATE gcal_sync_map SET last_synced_at = ? WHERE entity_type = 'deliverable' AND entity_id = ?"
                )
                .bind(&now_iso).bind(&d.id).execute(pool).await.map_err(sql_error)?;
            }
            None if !is_terminal => {
                // Create new event
                let desc = Some(d.claim.as_str());
                match create_event(&token, &d.title, desc, deadline, "deliverable", &d.id).await {
                    Ok(gcal_id) => {
                        sqlx::query(
                            "INSERT OR REPLACE INTO gcal_sync_map (entity_type, entity_id, gcal_event_id, last_synced_at) VALUES ('deliverable', ?, ?, ?)"
                        )
                        .bind(&d.id).bind(&gcal_id).bind(&now_iso)
                        .execute(pool).await.map_err(sql_error)?;
                    }
                    Err(_) => {} // skip individual failures
                }
            }
            _ => {}
        }
    }

    // 3. Push tasks with due_dates → GCal
    #[derive(sqlx::FromRow)]
    struct TaskRow {
        id: String,
        title: String,
        deliverable_title: String,
        due_date: Option<String>,
        status: String,
    }

    let tasks: Vec<TaskRow> = sqlx::query_as(
        r#"SELECT t.id, t.title, d.title AS deliverable_title, t.due_date, t.status
           FROM deliverable_tasks t
           INNER JOIN deliverables d ON d.id = t.deliverable_id
           WHERE t.due_date IS NOT NULL AND t.due_date != ''
             AND d.state NOT IN ('shipped', 'killed')"#,
    )
    .fetch_all(pool)
    .await
    .map_err(sql_error)?;

    for t in &tasks {
        let due_date = match &t.due_date {
            Some(d) => d,
            None => continue,
        };
        let is_done = t.status == "done";

        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT gcal_event_id FROM gcal_sync_map WHERE entity_type = 'task' AND entity_id = ?",
        )
        .bind(&t.id)
        .fetch_optional(pool)
        .await
        .map_err(sql_error)?;

        let summary = format!("{} [{}]", t.title, t.deliverable_title);

        match existing {
            Some((gcal_id,)) if is_done => {
                let _ = delete_event(&token, &gcal_id).await;
                sqlx::query(
                    "DELETE FROM gcal_sync_map WHERE entity_type = 'task' AND entity_id = ?",
                )
                .bind(&t.id)
                .execute(pool)
                .await
                .map_err(sql_error)?;
            }
            Some((gcal_id,)) => {
                let _ = update_event(&token, &gcal_id, &summary, None, due_date).await;
                sqlx::query(
                    "UPDATE gcal_sync_map SET last_synced_at = ? WHERE entity_type = 'task' AND entity_id = ?"
                )
                .bind(&now_iso).bind(&t.id).execute(pool).await.map_err(sql_error)?;
            }
            None if !is_done => {
                match create_event(&token, &summary, None, due_date, "task", &t.id).await {
                    Ok(gcal_id) => {
                        sqlx::query(
                            "INSERT OR REPLACE INTO gcal_sync_map (entity_type, entity_id, gcal_event_id, last_synced_at) VALUES ('task', ?, ?, ?)"
                        )
                        .bind(&t.id).bind(&gcal_id).bind(&now_iso)
                        .execute(pool).await.map_err(sql_error)?;
                    }
                    Err(_) => {}
                }
            }
            _ => {}
        }
    }

    Ok(())
}

// ── Query cache ───────────────────────────────────────────────────────────────

pub async fn get_cached_events(
    pool: &SqlitePool,
    week_start: &str,
) -> Result<Vec<GCalEvent>, String> {
    let start = NaiveDate::parse_from_str(week_start, "%Y-%m-%d")
        .map_err(|_| "week_start must use YYYY-MM-DD".to_string())?;
    let end = start + Duration::days(4);
    let week_end = end.format("%Y-%m-%d").to_string();

    sqlx::query_as::<_, GCalEvent>(
        "SELECT id, gcal_event_id, title, description, location, attendees,
                conference_data, organizer, recurring_event_id, recurrence,
                color_id, transparency, event_updated_at, attachments, reminders, visibility,
                start_date, end_date, start_datetime, end_datetime, is_all_day, html_link
         FROM gcal_events
         WHERE start_date >= ? AND start_date <= ?
           AND gcal_event_id NOT IN (SELECT gcal_event_id FROM gcal_sync_map)
         ORDER BY start_date ASC, is_all_day DESC, title ASC",
    )
    .bind(week_start)
    .bind(&week_end)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

/// Get calendar events that include a specific email as an attendee.
/// Returns events in a ±30-day window from today, sorted by date.
pub async fn get_stakeholder_calendar_events(
    pool: &SqlitePool,
    email: &str,
) -> Result<Vec<GCalEvent>, String> {
    let today = chrono::Utc::now().date_naive();
    let from = (today - chrono::Duration::days(30))
        .format("%Y-%m-%d")
        .to_string();
    let to = (today + chrono::Duration::days(60))
        .format("%Y-%m-%d")
        .to_string();
    let pattern = format!("%\"email\":\"{email}\"%");

    sqlx::query_as::<_, GCalEvent>(
        "SELECT id, gcal_event_id, title, description, location, attendees,
                conference_data, organizer, recurring_event_id, recurrence,
                color_id, transparency, event_updated_at, attachments, reminders, visibility,
                start_date, end_date, start_datetime, end_datetime, is_all_day, html_link
         FROM gcal_events
         WHERE attendees LIKE ?
           AND start_date >= ? AND start_date <= ?
           AND gcal_event_id NOT IN (SELECT gcal_event_id FROM gcal_sync_map)
         ORDER BY start_date ASC",
    )
    .bind(&pattern)
    .bind(&from)
    .bind(&to)
    .fetch_all(pool)
    .await
    .map_err(sql_error)
}

